//! The pose a body shows, for the fight's record (`record`): every bone of its model and every
//! look it holds, read in `Last`, after every pose writer (the clips and the hold layer, bends,
//! the foot lock, the lift, a strap) and the transforms' propagation, so a check measures the
//! pose a person sees rather than a clip alone (`tools/sandbox/duel/pose.ts`, D530).
//!
//! A bone is `[x, y, z, qx, qy, qz, qw, s]` in its model's space (the model's root, `Body`: the
//! glTF scene's frame, the model facing +Z, as `tools/characters/glb.ts`'s `posed` gives it), so
//! a tool sets the base body's skin on it as the move check does; the model's root is the
//! same in the start's terms (`root`). A held look is its root's place in the model's space,
//! the file it shows (`look`), the bone or socket it rides (`on`) and whether it is strapped.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use serde_json::{Value, json};
use std::collections::HashMap;
use terra_walker::character::{Body, Held};
use terra_walker::strap::Strapped;

/// What a pose is read from.
#[derive(SystemParam)]
pub(super) struct Posing<'w, 's> {
    children: Query<'w, 's, &'static Children>,
    names: Query<'w, 's, &'static Name>,
    places: Query<'w, 's, &'static GlobalTransform>,
    bodies: Query<'w, 's, (), With<Body>>,
    held: Query<'w, 's, (&'static WorldAssetRoot, &'static ChildOf, Has<Strapped>), With<Held>>,
    /// Each body's model root and its bones by name, found once their bones exist.
    found: Local<'s, HashMap<Entity, (Entity, Vec<(String, Entity)>)>>,
}

/// Four places, a quaternion's to a hundredth of a degree or so.
fn r4(x: f32) -> f64 {
    (f64::from(x) * 10000.0).round() / 10000.0
}

/// A place as `[x, y, z, qx, qy, qz, qw, s]`, the scale its mean.
fn eight(place: &Transform) -> Value {
    let (t, q, s) = (place.translation, place.rotation, place.scale);
    let mm = |x: f32| (f64::from(x) * 1000.0).round() / 1000.0;
    json!([
        mm(t.x),
        mm(t.y),
        mm(t.z),
        r4(q.x),
        r4(q.y),
        r4(q.z),
        r4(q.w),
        r4((s.x + s.y + s.z) / 3.0)
    ])
}

impl Posing<'_, '_> {
    /// The named entities under a model's root, down to but not into a held look.
    fn bones_of(&self, model: Entity) -> Vec<(String, Entity)> {
        let mut bones = Vec::new();
        let mut open = vec![model];
        while let Some(at) = open.pop() {
            for child in self.children.get(at).into_iter().flatten() {
                if self.held.contains(*child) {
                    continue;
                }
                if let Ok(name) = self.names.get(*child) {
                    bones.push((name.to_string(), *child));
                }
                open.push(*child);
            }
        }
        bones
    }

    /// The pose `who` shows, its root in the start's terms (`terms`), or none before its model
    /// has loaded.
    pub(super) fn of(&mut self, who: Entity, terms: &bevy::math::Affine3A) -> Option<Value> {
        if !self.found.contains_key(&who) {
            let model = self
                .children
                .iter_descendants(who)
                .find(|each| self.bodies.contains(*each))?;
            let bones = self.bones_of(model);
            if bones.is_empty() {
                return None;
            }
            self.found.insert(who, (model, bones));
        }
        let (model, bones) = &self.found[&who];
        let root = self.places.get(*model).ok()?.compute_transform();
        let into = root.compute_affine().inverse();
        let local =
            |place: &GlobalTransform| Transform::from_matrix((into * place.affine()).into());
        let mut posed = serde_json::Map::new();
        for (name, bone) in bones {
            if let Ok(place) = self.places.get(*bone) {
                posed.insert(name.clone(), eight(&local(place)));
            }
        }
        let held: Vec<Value> = self
            .children
            .iter_descendants(who)
            .filter_map(|each| {
                let (look, parent, strapped) = self.held.get(each).ok()?;
                let place = self.places.get(each).ok()?;
                let on = self.names.get(parent.parent()).ok()?;
                Some(json!({
                    "look": look.0.path().map(|path| path.path().to_string_lossy().to_string()),
                    "on": on.as_str(),
                    "strapped": strapped,
                    "at": eight(&local(place)),
                }))
            })
            .collect();
        let world = Transform::from_matrix((*terms * root.compute_affine()).into());
        Some(json!({ "root": eight(&world), "bones": posed, "held": held }))
    }
}
