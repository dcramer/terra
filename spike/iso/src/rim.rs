//! A rim on the people and beasts: their glTF materials are swapped, as their scenes come
//! in, for the same material with a fresnel term added to the emission, so a figure's edge
//! catches a thin light of the hour's sky and it reads against the grass (as Pillars of
//! Eternity and Pathfinder light their figures). A light could not do it alone: Bevy
//! culls a light by the view, not the mesh, so a back light also whitens every grass blade.

use crate::party::Actor;
use crate::scene::Hour;
use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

pub type RimMaterial = ExtendedMaterial<StandardMaterial, RimExt>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct RimExt {
    /// The rim's colour as it reaches the screen (the glTF materials' emission is not
    /// scaled by the exposure); alpha is the fresnel's power.
    #[uniform(100)]
    pub rim: LinearRgba,
}

impl MaterialExtension for RimExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://iso/shaders/rim.wesl".into()
    }
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/rim.wesl");
    app.add_plugins(MaterialPlugin::<RimMaterial>::default())
        .init_resource::<Swapped>()
        .add_systems(Update, (swap, tint).chain());
}

/// A colour the model's own is multiplied by (a pale Tripo coat brought to the beast's).
#[derive(Component, Clone, Copy)]
pub struct Tint(pub Vec3);

#[derive(Resource, Default)]
struct Swapped(HashMap<(AssetId<StandardMaterial>, [u32; 3]), Handle<RimMaterial>>);

/// The rim for the hour: faint by day, the sky's blue at dusk and by night.
fn rim_for(hour: Hour) -> LinearRgba {
    match hour {
        Hour::Day => LinearRgba::new(0.10, 0.10, 0.09, 3.6),
        Hour::Dusk => LinearRgba::new(0.16, 0.19, 0.32, 3.4),
        Hour::Night => LinearRgba::new(0.07, 0.10, 0.22, 3.2),
    }
}

#[allow(clippy::too_many_arguments)]
fn swap(
    mut commands: Commands,
    added: Query<(Entity, &MeshMaterial3d<StandardMaterial>), Added<MeshMaterial3d<StandardMaterial>>>,
    parents: Query<&ChildOf>,
    actors: Query<Option<&Tint>, With<Actor>>,
    standard: Res<Assets<StandardMaterial>>,
    mut rims: ResMut<Assets<RimMaterial>>,
    mut swapped: ResMut<Swapped>,
    hour: Option<Res<Hour>>,
) {
    let rim = rim_for(hour.map_or(Hour::Dusk, |h| *h));
    for (e, mat) in &added {
        let Some(tint) = parents.iter_ancestors(e).find_map(|a| actors.get(a).ok()) else {
            continue;
        };
        let tint = tint.map_or(Vec3::ONE, |t| t.0);
        let key = (mat.id(), tint.to_array().map(f32::to_bits));
        let handle = match swapped.0.get(&key) {
            Some(h) => h.clone(),
            None => {
                let Some(base) = standard.get(&mat.0) else {
                    continue;
                };
                let mut base = base.clone();
                let c = base.base_color.to_linear();
                base.base_color = LinearRgba::new(c.red * tint.x, c.green * tint.y, c.blue * tint.z, c.alpha).into();
                let h = rims.add(ExtendedMaterial {
                    base,
                    extension: RimExt { rim },
                });
                swapped.0.insert(key, h.clone());
                h
            }
        };
        commands
            .entity(e)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(handle));
    }
}

fn tint(hour: Option<Res<Hour>>, swapped: Res<Swapped>, mut rims: ResMut<Assets<RimMaterial>>) {
    let Some(hour) = hour else { return };
    if !hour.is_changed() {
        return;
    }
    let rim = rim_for(*hour);
    for h in swapped.0.values() {
        if let Some(mut m) = rims.get_mut(h) {
            m.extension.rim = rim;
        }
    }
}
