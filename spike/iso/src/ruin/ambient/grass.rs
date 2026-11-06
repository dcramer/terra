//! Wind on the dead grass: the grass the ruin's grass.rs lays keeps its mesh and look, and
//! its material becomes the standard one extended (wind.wesl) so its blades bend in gusts
//! rolling across the court along the wind, as the glade's foliage.wesl bends its own. The
//! same material hangs a cloth from its top edge when its mesh carries a second uv.

use super::WIND;
use crate::ruin::effects::kit::Clock;
use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

pub type WindMaterial = ExtendedMaterial<StandardMaterial, WindExt>;

#[derive(Clone, Copy, Debug, Default, Reflect, ShaderType)]
pub struct Wind {
    /// xy: the wind's direction, z: how far a blade's tip bends (m), w: the time (s).
    pub wind: Vec4,
    /// x: how far a cloth's free end flutters (m), y: the blades' height for full bend (m).
    pub cloth: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct WindExt {
    #[uniform(100)]
    pub wind: Wind,
}

const SHADER: &str = "embedded://iso/ruin/ambient/wind.wesl";

impl MaterialExtension for WindExt {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// How far a blade's tip moves in a full gust, and the blades' height for that.
const BEND: f32 = 0.09;
const TALL: f32 = 0.45;

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "wind.wesl");
    app.add_plugins(MaterialPlugin::<WindMaterial>::default())
        .add_systems(PostStartup, take_grass)
        .add_systems(Update, blow);
}

/// A wind material over `base`; `flutter` how far a cloth's free end goes.
pub fn material(base: StandardMaterial, flutter: f32) -> WindMaterial {
    ExtendedMaterial {
        base,
        extension: WindExt {
            wind: Wind {
                wind: Vec4::new(WIND.x, WIND.y, BEND, 0.0),
                cloth: Vec4::new(flutter, TALL, 0.0, 0.0),
            },
        },
    }
}

/// Finds the dead grass (the scene's one lit, opaque, two-sided standard material: the
/// flames and the spell are unlit) and gives it the wind's material.
fn take_grass(
    mut commands: Commands,
    meshes: Query<(Entity, &MeshMaterial3d<StandardMaterial>)>,
    standard: Res<Assets<StandardMaterial>>,
    mut winds: ResMut<Assets<WindMaterial>>,
) {
    for (e, m) in &meshes {
        let Some(base) = standard.get(&m.0) else {
            continue;
        };
        if is_grass(base) {
            commands
                .entity(e)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(winds.add(material(base.clone(), 0.0))));
        }
    }
}

fn is_grass(m: &StandardMaterial) -> bool {
    m.cull_mode.is_none() && !m.unlit && m.alpha_mode == AlphaMode::Opaque
}

/// The wind's time to every wind material.
fn blow(clock: Res<Clock>, mut winds: ResMut<Assets<WindMaterial>>) {
    for (_, mut m) in winds.iter_mut() {
        m.extension.wind.wind.w = clock.t;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only the grass is taken: the flames' two-sided cards are unlit, the floor is culled.
    #[test]
    fn takes_only_grass() {
        let grass = StandardMaterial {
            cull_mode: None,
            double_sided: true,
            ..default()
        };
        assert!(is_grass(&grass));
        assert!(!is_grass(&StandardMaterial::default()));
        assert!(!is_grass(&StandardMaterial {
            unlit: true,
            ..grass.clone()
        }));
    }
}
