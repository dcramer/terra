//! The third scene (`--scene forest`): the second biome, a temperate wood by day, seen from
//! the ruin's action-RPG camera. The glade's land, oaks, grass, pond and beasts (the
//! default scene's modules, without its party and camera), and in its clearing by the pond
//! the knight fighting the iso pipeline's tree creatures, with the ruin's spell and hit;
//! a moose walking the far bank, birds crossing, pollen in the sun (life.rs).

pub mod clearing;
mod dress;
mod fight;
pub mod lawn;
mod life;
mod pond;

use crate::ruin::effects::Stage;
use crate::terrain::shape;
use bevy::prelude::*;

/// Where the knight stands: in the clearing between the camp and the pond.
pub const KNIGHT: Vec2 = Vec2::new(4.0, 5.5);
/// The tree creature his blow lands on, and the one his bolts fly at: to his left as the
/// camera sees it, so the bolts cross the frame and their glow does not hide it, and clear
/// of the camp (behind him up the frame, j11's still showed it a leaf clump in the glow).
pub const FOE: Vec2 = Vec2::new(6.0, 4.1);
pub const TARGET: Vec2 = Vec2::new(1.3, 6.3);
/// Where that second creature stands: 1.1 m past the bolts' mark from the knight, so the
/// impact's flash (about 0.7 m across) bursts on its reaching arms, not over its body.
pub const SECOND: Vec2 = Vec2::new(0.25, 6.6);
/// What the camera looks at: the fight a little left of the middle, so the pond's north
/// shore fills the lower right and the moose's bank the upper right.
const FOCUS: Vec2 = Vec2::new(4.6, 4.8);
/// The forest's day over the glade's (scene.rs's `light_the_hour`). The glade's (sun 1,200
/// lux, sky 450 nits, EV 9, AgX) left the still murky: luma p50 0.046, p95 0.12,
/// saturation 0.17, bright share 0.007, against the day refs' (le-river-beam,
/// poe2-shock-nova, le-desert-fight) 0.06 to 0.10, 0.30 to 0.40, 0.27 to 0.47 and 0.03 to
/// 0.11. A sun of 2,600 alone moved p95 only to 0.13 (the floor is dark grass and soil), so
/// the exposure opens a stop, the sky's fill drops so the shade stays dark, Tony McMapface
/// replaces AgX (which greys greens and a flame) and the grade adds saturation. At r4 (EV 8,
/// sun 3,200, sky 330) the frame still graded as dusk: p50 0.044, p95 0.216, saturation
/// 0.23 (judge forest-r4), the fight's ground in an oak's shade; half a stop more, a
/// brighter sun over a darker sky fill, the fight in a sun break (clearing.rs).
const EV100: f32 = 7.5;
const SUN_LUX: f32 = 4_200.0;
const SKY_NITS: f32 = 300.0;
/// 1.45 at r1 made the sunlit grass acid lime (judge forest-r1, both judges); 1.3 keeps
/// the shade's greens. The midtones get their own (the greens' band) over it.
const SATURATION: f32 = 1.3;
/// 1.15 left saturation 0.254 (the day refs' 0.27 to 0.47).
const MID_SATURATION: f32 = 1.25;

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
        .add_plugins((
            crate::ruin::effects::plugin,
            crate::ruin::fight::plugin,
            life::plugin,
            dress::plugin,
            pond::plugin,
            clearing::plugin,
            lawn::plugin,
        ))
        .add_systems(Startup, (camera, fight::spawn))
        .add_systems(
            PostStartup,
            smoulder.before(crate::ruin::effects::kit::Found),
        )
        .add_systems(PostUpdate, day);
}

/// By day the camp fire is out but for its coals and smoke (judge forest-r1: a fire burning
/// at noon): its halo card is taken from the ruin's flipbook fire before the fires are found
/// (kit::Found) and hidden, and its light reaches only the coals.
fn smoulder(
    mut commands: Commands,
    args: Res<crate::Args>,
    halos: Query<Entity, With<crate::motes::Halo>>,
    mut lights: Query<(&mut PointLight, &bevy::camera::visibility::RenderLayers)>,
) {
    if args.hour != crate::scene::Hour::Day {
        return;
    }
    for e in &halos {
        commands
            .entity(e)
            .remove::<crate::motes::Halo>()
            .insert(Visibility::Hidden);
    }
    let fire = bevy::camera::visibility::RenderLayers::layer(crate::camp::FIRE_CASTS);
    for (mut light, layers) in &mut lights {
        if layers.intersects(&fire) {
            light.range = 1.4;
            light.shadow_maps_enabled = false;
        }
    }
}

/// The forest's day, set after the glade's hour has set its own (scene.rs, in Update).
fn day(
    hour: Res<crate::scene::Hour>,
    mut sun: Query<&mut DirectionalLight>,
    mut cams: Query<(
        &mut bevy::camera::Exposure,
        &mut bevy::core_pipeline::tonemapping::Tonemapping,
    )>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    if !hour.is_changed() || *hour != crate::scene::Hour::Day {
        return;
    }
    for mut s in &mut sun {
        s.illuminance = SUN_LUX;
    }
    for (mut e, mut t) in &mut cams {
        e.ev100 = EV100;
        *t = bevy::core_pipeline::tonemapping::Tonemapping::TonyMcMapface;
    }
    ambient.brightness = SKY_NITS;
}

/// The ruin's camera, on the fight.
fn camera(mut commands: Commands, args: Res<crate::Args>, mut images: ResMut<Assets<Image>>) {
    let focus = FOCUS.extend(shape(FOCUS) + 1.2).xzy();
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
        bevy::core_pipeline::tonemapping::Tonemapping::TonyMcMapface,
        bevy::render::view::ColorGrading {
            global: bevy::render::view::ColorGradingGlobal {
                post_saturation: SATURATION,
                ..default()
            },
            midtones: bevy::render::view::ColorGradingSection {
                saturation: MID_SATURATION,
                ..default()
            },
            ..default()
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fighters stand on dry land, clear of the camp fire, and the second creature
    /// stands beyond the bolts' mark by more than the impact's flash, on the far side.
    #[test]
    fn the_fight_is_staged_clear() {
        for p in [KNIGHT, FOE, TARGET, SECOND] {
            assert!(crate::terrain::pond(p) < -0.05, "{p} is in the pond");
            let fire = p.distance(crate::scene::FIRE);
            assert!(fire > 2.5, "{p} is {fire} m from the camp fire");
        }
        let clear = SECOND.distance(TARGET);
        assert!(
            clear > 0.7,
            "the flash at {TARGET} covers the creature at {SECOND}"
        );
        assert!(SECOND.distance(KNIGHT) > TARGET.distance(KNIGHT));
    }
}
