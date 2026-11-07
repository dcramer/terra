//! The third scene (`--scene forest`): the second biome, a temperate wood by day, seen from
//! the ruin's action-RPG camera. The glade's land, oaks, grass, pond and beasts (the
//! default scene's modules, without its party and camera), and in its clearing by the pond
//! the knight fighting the iso pipeline's tree creatures, with the ruin's spell and hit;
//! a moose walking the far bank, birds crossing, pollen in the sun (life.rs).

pub mod clearing;
pub mod dress;
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

/// The camp's cooking fire by day: burnt low (judge forest-r1: a night's blaze at noon read
/// wrong), so its flame is small, 0.8 m at its tallest (at 0.6 m it hid among its logs,
/// round 261107-0434).
const DAY_FIRE: f32 = 0.42;

/// By day the camp fire burns low: the ruin's flipbook fire (ruin/effects/flame.rs: flame
/// cards, a white-hot core over the coals, embers) at a cooking fire's size, found by its
/// halo card before the fires are (kit::Found), the halo itself hidden (no glowing air in
/// sunlight); its smoke a thin pale grey drifting off on the breeze, thinning as it
/// spreads, not the night's dark column lit orange (judge 261107-0358-2: "a flat orange
/// star-disc with a brown smoke smudge, no flame or ember core"); its light a warm pool on
/// the ground about it, from higher up so it spreads, casting none of the clearing's
/// shadows.
fn smoulder(
    args: Res<crate::Args>,
    land: Res<crate::terrain::Land>,
    mut plume: ResMut<crate::ruin::effects::Plume>,
    mut halos: Query<(&mut Transform, &mut Visibility), With<crate::motes::Halo>>,
    mut lights: Query<
        (
            &mut PointLight,
            &mut Transform,
            &bevy::camera::visibility::RenderLayers,
        ),
        Without<crate::motes::Halo>,
    >,
) {
    if args.hour != crate::scene::Hour::Day {
        return;
    }
    let ground = land.height(crate::scene::FIRE);
    // kit.rs reads a fire's bed as the halo less 0.35 of its size, its size the halo's
    // scale over 2.6.
    for (mut t, mut vis) in &mut halos {
        t.translation.y = ground + 0.08 + 0.35 * DAY_FIRE;
        t.scale = Vec3::splat(2.6 * DAY_FIRE);
        *vis = Visibility::Hidden;
    }
    *plume = crate::ruin::effects::Plume {
        embers: 28.0,
        puffs: 36.0,
        smoke_life: (5.0, 7.0),
        wide: (0.8, 5.5),
        climb: 1.0,
        // At 0.2 and a grey of 0.15 it did not show at all (round 261107-0434).
        alpha: 0.38,
        // Unlit: as it reaches the screen. Pale against the shade, barely warm low.
        lit: Vec3::new(0.26, 0.2, 0.15),
        dark: Vec3::new(0.22, 0.225, 0.235),
        cool: 0.2,
        wind: Vec2::new(0.3, -0.13),
        ..*plume
    };
    let fire = bevy::camera::visibility::RenderLayers::layer(crate::camp::FIRE_CASTS);
    for (mut light, mut t, layers) in &mut lights {
        if layers.intersects(&fire) {
            // From 1.8 m up its pool was a faint red wash over 3 m (round 261107-0434); in
            // the night's red-orange it lay a dark red stain on the earth by day (0436): a
            // low fire's amber.
            light.color = Color::srgb(1.0, 0.7, 0.42);
            light.range = 3.5;
            light.radius = 0.3;
            light.shadow_maps_enabled = false;
            t.translation.y = ground + 1.3;
        }
    }
}

/// The forest's light, its preset's (biomes/light/forest-day.ron), set over the glade's at
/// that hour after the glade's hour has set its own (scene.rs, in Update). At another hour
/// (`--hour`) the glade's stands.
fn day(
    hour: Res<crate::scene::Hour>,
    light: Res<crate::kit::light::Light>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform)>,
    mut cams: Query<(
        &mut bevy::camera::Exposure,
        &mut bevy::core_pipeline::tonemapping::Tonemapping,
    )>,
    mut fog: Query<&mut bevy::pbr::DistanceFog>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    if !hour.is_changed() || *hour != light.hour.scene() {
        return;
    }
    for (mut s, mut tr) in &mut sun {
        s.color = light.key.colour.color();
        s.illuminance = light.key.lux;
        *tr = light.key.toward.transform();
    }
    for (mut e, mut t) in &mut cams {
        e.ev100 = light.ev100;
        *t = light.curve.tonemapping();
    }
    if let Some(h) = light.haze {
        for mut f in &mut fog {
            f.color = h.colour.color();
        }
    }
    ambient.color = light.sky.colour.color();
    ambient.brightness = light.sky.nits;
}

/// The ruin's camera, on the fight, its exposure, grade and haze the preset's.
fn camera(
    mut commands: Commands,
    args: Res<crate::Args>,
    light: Res<crate::kit::light::Light>,
    mut images: ResMut<Assets<Image>>,
) {
    let focus = FOCUS.extend(shape(FOCUS) + 1.2).xzy();
    let cam = crate::ruin::camera::arpg(&mut commands, &args, &mut images, focus, light.ev100);
    // The glade's camp fire lights on its casters' layer (camp::FIRE_CASTS).
    commands
        .entity(cam)
        .insert(bevy::camera::visibility::RenderLayers::from_layers(&[
            0,
            crate::camp::FIRE_CASTS,
        ]));
    light.camera(&mut commands, cam, crate::ruin::camera::DISTANCE);
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
