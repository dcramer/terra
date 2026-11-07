//! The forest's pond as the kit's water (kit/water.rs, iso-kit j28), its look the biome
//! file's `water` (biomes/forest.ron), as the marsh's channels are: clear by day over its
//! bed, a planar mirror of the canopy and the sky, ripples on the wind, the bank fading in.
//! Before, pool.rs's own look read a flat sheet under confetti glints (judge round
//! 261106-2239) and the glade's (water.rs) a flat blue-grey slab (judge forest-r1). The sun
//! itself (az -40, el 52) never glints from this camera (j13), so as games fake a sun's
//! glitter, a `Glint` light stands where the camera's rays off the pond's middle point to,
//! on a render layer no camera sees: the water's shader draws its lane and nothing is lit
//! by it.

use crate::kit::biome::Biome;
use crate::kit::water::{Glint, KitWater};
use crate::terrain::{Land, POND, WATER};
use crate::water::WaterMaterial;
use bevy::prelude::*;

/// Where on the water the sun's glitter lies: the visible pond's middle, the frame's lower
/// right (px 785,472 of 960x540).
pub const GLITTER: Vec2 = Vec2::new(11.0, 6.0);
/// How far up the camera's mirrored ray the glint light stands (m): far, as the sun is, so
/// the lane spreads over the pond and only the ripples choose where it glints.
const SUN_AWAY: f32 = 400.0;
/// The glint light's lumens: about 800 lux at the water.
const SUN_LUMENS: f32 = 1.6e9;
/// A layer no camera renders, so the glint light lights nothing: not the kit water's own
/// (`kit::water::LAYER`, 9), which the main camera sees and whose lights light the scene.
const UNSEEN: usize = 10;

pub fn plugin(app: &mut App) {
    app.add_plugins(crate::kit::water::plugin)
        .add_systems(Startup, spawn)
        .add_systems(PostStartup, hide_glade_water);
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
    biome: Res<Biome>,
    light: Res<crate::kit::light::Light>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut waters: ResMut<Assets<KitWater>>,
) {
    let Some(water) = biome.water.as_ref() else {
        panic!(
            "the forest's biome file has no `water`: its pond needs one (biomes/forest.ron, \
             `water: (kind: Pond, level: -0.55, ...)`, kit.md \"The water\"; D634)"
        );
    };
    let wind = biome
        .weather
        .as_ref()
        .map_or(crate::foliage::WIND, |w| Vec2::new(w.wind.0, w.wind.1));
    let half = Vec2::new(9.5, 8.0);
    // Drawn over the marsh's made-up shelving depth (marsh/land.rs `seen_depth`), so the
    // face fades in over about half a metre of wet mud and the floating leaves mat that
    // margin: drawn at its true depth it read "a flat mirror cut out of the ground with a
    // hard dark rim" (judge, round 261107-0507).
    let level = water.level;
    let face = crate::kit::water::surface(POND - half, POND + half, 0.25, level, |p| {
        level - crate::marsh::land::seen_depth(level - land.height(p))
    });
    crate::kit::water::spawn_face(
        &mut commands,
        &mut meshes,
        &mut waters,
        water,
        wind,
        Some(&light),
        face,
    );
    let focus = super::FOCUS
        .extend(crate::terrain::shape(super::FOCUS) + 1.2)
        .xzy();
    let camera = focus + crate::ruin::camera::offset();
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.95, 0.86),
            intensity: SUN_LUMENS,
            range: 2_000.0,
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

    /// The biome file's water stands where the land's bowl is carved for it, so the face
    /// fills the pond to its bank and the bed shows through the shallows.
    #[test]
    fn the_water_fills_the_pond() {
        let b = Biome::load("forest").unwrap_or_else(|e| panic!("{e}"));
        let w = b.water.expect("forest.ron has no `water`");
        assert!(
            (w.level - WATER).abs() < 1e-4,
            "forest.ron's water level {} is not the pond's (terrain.rs WATER {WATER})",
            w.level
        );
        // Over the pond's middle, open water deep enough to hide most of the bed.
        let depth = w.level - crate::terrain::shape(POND);
        assert!(depth > 0.5, "the pond's middle is {depth} m deep");
    }
}
