//! The third scene (`--scene forest`): the second biome, a temperate wood by day, seen from
//! the ruin's action-RPG camera. The glade's land, oaks, grass, pond and beasts (the
//! default scene's modules, without its party and camera), and in its clearing by the pond
//! the knight fighting the iso pipeline's tree creatures, with the ruin's spell and hit;
//! a moose walking the far bank, birds crossing, pollen in the sun (life.rs).

mod fight;
mod life;

use crate::ruin::effects::Stage;
use crate::terrain::shape;
use bevy::prelude::*;

/// Where the knight stands: in the clearing between the camp and the pond.
pub const KNIGHT: Vec2 = Vec2::new(4.0, 5.5);
/// The tree creature his blow lands on, and the one his bolts fly at.
pub const FOE: Vec2 = Vec2::new(6.0, 4.1);
pub const TARGET: Vec2 = Vec2::new(3.3, 1.7);
/// The exposure (the glade's by day; scene.rs sets it as the hour comes in).
const EV100: f32 = 9.0;

pub fn stage() -> Stage {
    Stage {
        knight: KNIGHT,
        foe: FOE,
        target: TARGET,
        floor: shape(KNIGHT),
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(stage())
        .add_plugins((crate::ruin::effects::plugin, crate::ruin::fight::plugin, life::plugin))
        .add_systems(Startup, (camera, fight::spawn));
}

/// The ruin's camera, on the middle of the fight.
fn camera(mut commands: Commands, args: Res<crate::Args>, mut images: ResMut<Assets<Image>>) {
    let mid = (KNIGHT + FOE + TARGET) / 3.0;
    let focus = mid.extend(shape(mid) + 1.2).xzy();
    let cam = crate::ruin::camera::arpg(&mut commands, &args, &mut images, focus, EV100);
    let d = crate::ruin::camera::DISTANCE;
    commands.entity(cam).insert((
        // The glade's camp fire lights on its casters' layer (camp::FIRE_CASTS).
        bevy::camera::visibility::RenderLayers::from_layers(&[0, crate::camp::FIRE_CASTS]),
        // The glade's haze, its colour the hour's (scene.rs), over the land beyond the fight.
        bevy::pbr::DistanceFog {
            color: Color::NONE,
            directional_light_color: Color::NONE,
            falloff: bevy::pbr::FogFalloff::Linear {
                start: d - 2.0,
                end: d + 70.0,
            },
            ..default()
        },
        bevy::core_pipeline::tonemapping::Tonemapping::AgX,
    ));
}
