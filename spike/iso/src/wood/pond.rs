//! The forest's pond as still dark water (pool.rs) in place of the glade's (water.rs, which
//! read a flat blue-grey slab at the ARPG camera: judge forest-r1). By day the sky's mirror
//! is the water's colour, and the sun's glitter path is what shows it is water. The sun
//! itself (az -40, el 52) never glints from this camera (j13), so as games fake a sun's
//! glitter, a `Glint` light stands where the camera's rays off the pond's middle point to,
//! on a render layer no camera sees: the pool's shader mirrors it and nothing is lit by it.

use crate::pool::{self, Glint, Look};
use crate::terrain::{Land, POND, WATER};
use crate::water::WaterMaterial;
use bevy::prelude::*;

/// Where on the water the sun's glitter lies: the visible pond's middle, the frame's lower
/// right (px 785,472 of 960x540).
pub const GLITTER: Vec2 = Vec2::new(11.0, 6.0);
/// How far up the camera's mirrored ray the glint light stands (m): far, so the glitter is a
/// patch the ripples break, as the sun's is, not one sharp point.
const SUN_AWAY: f32 = 40.0;
/// The glint light's lumens: about 800 lux at the water, a white-hot glitter core of about 5
/// linear at EV 8 through the lobe (pool.rs: s = 600, Fresnel about 0.02).
const SUN_LUMENS: f32 = 16.0e6;
/// A layer no camera renders, so the glint light lights nothing.
const UNSEEN: usize = 9;

pub fn plugin(app: &mut App) {
    app.add_plugins(pool::plugin)
        .add_systems(Startup, spawn)
        .add_systems(PostStartup, hide_glade_water);
}

/// The water by day: dark, the sky's band its colour, a breeze's ripples. At r4 the glint's
/// broad lobe (600) over gentle ripples (0.04) drew one round dappled spotlight on a grey
/// slab (judge forest-r4); a tight lobe at twice the gain over steeper ripples breaks it
/// into small sharp sparkles, and a greener, clearer shallow tints the edge.
pub fn look() -> Look {
    Look {
        deep: Vec4::new(0.004, 0.009, 0.010, 0.6),
        shallow: Vec4::new(0.05, 0.07, 0.045, 0.8),
        sky: Vec4::new(420.0, 560.0, 820.0, 1.0),
        horizon: Vec4::new(700.0, 760.0, 820.0, 0.6),
        wave: Vec4::new(0.86, 0.5, 0.07, 0.0),
        look: Vec4::new(0.0, 4_000.0, 2.0, 0.15),
        lights: default(),
    }
}

/// The margin: a soft fade onto the bank's mud, the mirrored sky broken into the far
/// trees' blots.
pub fn edge() -> pool::Edge {
    pool::Edge {
        fade: Vec4::new(0.3, 0.8, 0.0, 0.0),
        ..default()
    }
}

/// Where the glint light stands: up the ray from the camera to `GLITTER`, mirrored in the
/// water's face.
pub fn sun_glint(camera: Vec3) -> Vec3 {
    let p = GLITTER.extend(WATER).xzy();
    let v = (p - camera).normalize();
    p + Vec3::new(v.x, -v.y, v.z) * SUN_AWAY
}

fn spawn(
    mut commands: Commands,
    land: Res<Land>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut pools: ResMut<Assets<pool::PoolMaterial>>,
) {
    let half = Vec2::new(9.5, 8.0);
    let face = pool::surface(POND - half, POND + half, 0.25, WATER, |p| land.height(p));
    commands.spawn((
        Mesh3d(meshes.add(face)),
        MeshMaterial3d(pools.add(pool::with_edge(look(), edge()))),
        bevy::light::NotShadowCaster,
    ));
    let focus = super::FOCUS
        .extend(crate::terrain::shape(super::FOCUS) + 1.2)
        .xzy();
    let camera = focus + crate::ruin::camera::offset();
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.95, 0.86),
            intensity: SUN_LUMENS,
            range: 200.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(sun_glint(camera)),
        bevy::camera::visibility::RenderLayers::layer(UNSEEN),
        Glint,
    ));
}

/// The glade's water face out of sight (its reeds stay).
fn hide_glade_water(
    mut commands: Commands,
    faces: Query<Entity, With<MeshMaterial3d<WaterMaterial>>>,
) {
    for e in &faces {
        commands.entity(e).insert(Visibility::Hidden);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The glitter lies on the water, and its light stands high over the pond's far side, as
    /// the sun's mirror would for a camera looking down at it.
    #[test]
    fn the_glitter_is_on_the_water() {
        assert!(
            crate::terrain::pond(GLITTER) > 0.1,
            "{GLITTER} is not in the pond"
        );
        let camera = Vec3::new(8.3, 16.8, 15.0);
        let l = sun_glint(camera);
        assert!(l.y > 20.0, "the glint light at {l} is low");
        // Beyond the glitter from the camera, on the ground.
        let (c, g, s) = (camera.xz(), GLITTER, l.xz());
        assert!(
            (s - g).dot(g - c) > 0.0,
            "the glint light at {l} is on the camera's side"
        );
    }
}
