//! A staged figure's own changes of look over the remote, as a creature that wakes: its
//! morph targets eased to weights by name (`staged.morph`, a mouth that splits open), and
//! the glow of its painted emissive parts eased to a strength (`staged.glow`, eyes that
//! light). Nothing here names a model or a target: both are in the calls.

use bevy::{
    mesh::morph::MorphWeights,
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// What a figure's look is easing toward: each morph target named, and its glow.
#[derive(Component, Default)]
pub struct Wake {
    morphs: Vec<(String, Ramp)>,
    glow: Option<Ramp>,
    /// Its emissive parts have materials of their own, so one figure lights alone.
    own: bool,
}

/// From one value to another over `length` seconds, eased in and out.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ramp {
    from: f32,
    to: f32,
    elapsed: f32,
    length: f32,
}

impl Ramp {
    fn new(from: f32, to: f32, blend_ms: u64) -> Self {
        Self {
            from,
            to,
            elapsed: 0.0,
            length: blend_ms as f32 / 1000.0,
        }
    }

    fn now(&self) -> f32 {
        if self.length <= 0.0 {
            return self.to;
        }
        let t = (self.elapsed / self.length).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * t * t * (3.0 - 2.0 * t)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Morph {
    name: String,
    weights: BTreeMap<String, f32>,
    #[serde(default)]
    blend_ms: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Glow {
    name: String,
    strength: f32,
    #[serde(default)]
    blend_ms: u64,
}

/// Eases a figure's morph targets, by name, to the weights given.
pub fn morph(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Morph = parse_some(params)?;
    let entity = super::named(world, &asked.name)?;
    let mut wake = world.entity_mut(entity).take::<Wake>().unwrap_or_default();
    for (target, to) in asked.weights {
        let from = wake
            .morphs
            .iter()
            .find(|(name, _)| *name == target)
            .map_or(0.0, |(_, ramp)| ramp.now());
        wake.morphs.retain(|(name, _)| *name != target);
        wake.morphs
            .push((target, Ramp::new(from, to, asked.blend_ms)));
    }
    world.entity_mut(entity).insert(wake);
    Ok(json!({ "done": true }))
}

/// Eases the glow of a figure's parts painted to glow (those with an emissive map) to a
/// strength: 0 is dark, 1 the map's own colours.
pub fn glow(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Glow = parse_some(params)?;
    let entity = super::named(world, &asked.name)?;
    let mut wake = world.entity_mut(entity).take::<Wake>().unwrap_or_default();
    let from = wake.glow.map_or(0.0, |ramp| ramp.now());
    wake.glow = Some(Ramp::new(from, asked.strength, asked.blend_ms));
    world.entity_mut(entity).insert(wake);
    Ok(json!({ "done": true }))
}

/// Each waking figure's look a frame further on: its morph weights by name on its meshes,
/// and its emissive parts' strength.
pub fn ease(
    time: Res<Time>,
    mut figures: Query<(Entity, &mut Wake)>,
    children: Query<&Children>,
    mut weights: Query<&mut MorphWeights>,
    parts: Query<&MeshMaterial3d<StandardMaterial>>,
    meshes: Res<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (root, mut wake) in &mut figures {
        for (_, ramp) in &mut wake.morphs {
            ramp.elapsed += dt;
        }
        if let Some(ramp) = wake.glow.as_mut() {
            ramp.elapsed += dt;
        }
        for each in children.iter_descendants(root) {
            let Ok(mut node) = weights.get_mut(each) else {
                continue;
            };
            let names = node
                .first_mesh()
                .and_then(|mesh| meshes.get(mesh))
                .and_then(|mesh| mesh.morph_target_names().map(<[String]>::to_vec))
                .unwrap_or_default();
            for (slot, name) in node.weights_mut().iter_mut().zip(&names) {
                if let Some((_, ramp)) = wake.morphs.iter().find(|(target, _)| target == name) {
                    *slot = ramp.now();
                }
            }
        }
        let Some(strength) = wake.glow.map(|ramp| ramp.now()) else {
            continue;
        };
        let own = wake.own;
        for each in children.iter_descendants(root) {
            let Ok(part) = parts.get(each) else { continue };
            let Some(material) = materials.get(&part.0).cloned() else {
                continue;
            };
            if material.emissive_texture.is_none() {
                continue;
            }
            let lit = LinearRgba::WHITE * strength;
            if own {
                if let Some(mut material) = materials.get_mut(&part.0) {
                    material.emissive = lit;
                }
            } else {
                // The glTF's material is shared by every figure of the model.
                let mut mine = material;
                mine.emissive = lit;
                commands
                    .entity(each)
                    .insert(MeshMaterial3d(materials.add(mine)));
            }
        }
        wake.own = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ramp starts where it was, ends where it was told, and passes the middle halfway.
    #[test]
    fn a_ramp_eases_from_where_it_was_to_where_it_was_told() {
        let mut ramp = Ramp::new(0.2, 1.0, 2000);
        assert_eq!(ramp.now(), 0.2);
        ramp.elapsed = 1.0;
        assert!((ramp.now() - 0.6).abs() < 1e-6);
        ramp.elapsed = 5.0;
        assert_eq!(ramp.now(), 1.0);
        assert_eq!(Ramp::new(0.0, 0.7, 0).now(), 0.7);
    }
}
