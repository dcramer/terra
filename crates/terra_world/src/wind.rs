//! The one wind (D255) from the climate (docs/demos.md §2, step 4): which way it blows and how
//! hard where the camera is, as `terra_climate`'s fields say, taken again every few seconds of
//! the clock, and handed to everything that sways: the grass and the grown plants and trees
//! read it from the buffer they share (`crate::parting`), written in place, so it never
//! changes their materials; the flora and the lakes' ripples are told it in theirs. Its strength is the climate's speed over its mean, so the
//! sway the art was tuned at (1) is the climate's mean wind, a calm spell stills it and a
//! gale bends it twice as far.

use crate::climate::ClimateFields;
use crate::daylight::Clock;
use crate::instance::{Building, InWorld, InstanceApp};
use crate::planet::MainCamera;
use bevy::prelude::*;

/// The wind where the camera is.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct WorldWind {
    /// Which way it blows, level with the ground there.
    pub toward: Vec3,
    /// How hard, as a share of the climate's mean wind, and in metres a second.
    pub strength: f32,
    pub speed_m_s: f32,
}

/// The sway is never stiller than this share of the mean wind's, nor more than this, so a
/// still day's grass still breathes and a gale's does not fold flat.
const STILLEST: f32 = 0.25;
const WILDEST: f32 = 2.2;
/// Taken again this often, in days of the clock: a few seconds at half an hour a day.
const EVERY_DAYS: f64 = 0.002;

/// The wind of the world the run is in (D469), and the snow it lifts round the start.
pub fn plugin(app: &mut App) {
    entering(app);
    app.add_systems(Update, follow.after(crate::daylight::Ticking));
}

/// What of the wind is the world's (D469).
pub(crate) fn entering(app: &mut App) {
    app.world_resource::<WorldWind>()
        .add_systems(OnEnter(InWorld), blown_snow.in_set(Building::View));
}

/// Blown snow is looked for this far round the start, on a grid this fine, and at most so many
/// patches lift it.
const DRIFT_REACH_M: f32 = 160.0;
const DRIFT_STEP_M: f32 = 9.0;
const DRIFTS: usize = 64;

/// Where snow lies round the start, on the land's rules (`Round::snow_m`), the wind lifts it
/// (D325): an emitter of blown snow on each patch, as thick as the snow is deep, the smoke's
/// cards carrying it low downwind once the wind is hard enough (`smoke::Burning::Snow`).
fn blown_snow(
    mut commands: Commands,
    planet: Res<crate::planet::Planet>,
    start: Res<crate::start::Start>,
) {
    let Some(round) = planet.round() else {
        return;
    };
    let steps = (DRIFT_REACH_M / DRIFT_STEP_M) as i32;
    let mut patches: Vec<(f32, Vec3)> = Vec::new();
    for i in -steps..=steps {
        for j in -steps..=steps {
            // Off the grid a little, by the cell, so patches do not line up.
            let wobble = |k: i32| {
                (k.wrapping_mul(7919).rem_euclid(97) as f32 / 97.0 - 0.5) * DRIFT_STEP_M * 0.8
            };
            let (x, z) = (
                i as f32 * DRIFT_STEP_M + wobble(i * 31 + j),
                j as f32 * DRIFT_STEP_M + wobble(j * 17 - i),
            );
            if x * x + z * z > DRIFT_REACH_M * DRIFT_REACH_M {
                continue;
            }
            let point = planet.ground_at(start.at(x, z));
            let snow = round.snow_m(point, planet.normal(point));
            if snow > 0.04 {
                patches.push((snow, point));
            }
        }
    }
    patches.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (snow, point) in patches.into_iter().take(DRIFTS) {
        commands.spawn((
            Name::new("Blown snow"),
            crate::smoke::Smoke {
                burning: crate::smoke::Burning::Snow,
                thickness: (snow / 0.15).clamp(0.3, 1.0),
            },
            Transform::from_translation(point + point.normalize() * 0.1),
            Visibility::default(),
        ));
    }
}

/// Takes the wind where the camera is, from the climate, every few seconds of the clock; the
/// resource changes only when the wind has, so the materials that hold it are touched only then.
fn follow(
    mut commands: Commands,
    fields: Option<Res<ClimateFields>>,
    clock: Res<Clock>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    wind: Option<ResMut<WorldWind>>,
    mut last: Local<Option<f64>>,
) {
    let (Some(fields), Some(camera)) = (fields, camera) else {
        return;
    };
    let now = clock.time().0;
    // At once in a world just entered, which has no wind yet.
    if wind.is_some() && last.is_some_and(|when| (now - when).abs() < EVERY_DAYS) {
        return;
    }
    *last = Some(now);
    let at = camera.translation();
    let weather = fields.fields.at(fields.place(at), clock.time());
    let up = at.normalize_or(Vec3::Y);
    // It blows from its bearing, clockwise from north.
    let (north, east) = (fields.frame.north(at), fields.frame.east(at));
    let from = north * libm::cosf(weather.wind.from) + east * libm::sinf(weather.wind.from);
    let toward = (-from).reject_from(up).normalize_or(east);
    let mean = fields.fields.climate().wind.mean_m_s.max(0.1);
    let new = WorldWind {
        toward,
        strength: (weather.wind.speed_m_s / mean).clamp(STILLEST, WILDEST),
        speed_m_s: weather.wind.speed_m_s,
    };
    match wind {
        Some(mut wind) => {
            let turned = wind.toward.dot(new.toward) < 0.996;
            if turned || (wind.strength - new.strength).abs() > 0.04 {
                *wind = new;
            }
        }
        None => commands.insert_resource(new),
    }
}

/// How hard each material sways of its own, as it was made, so the wind's strength is laid over
/// it: a material may sway more or less than another.
pub struct OwnSway<A: Asset>(bevy::platform::collections::HashMap<AssetId<A>, f32>);

impl<A: Asset> Default for OwnSway<A> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<A: Asset> OwnSway<A> {
    /// A material's own sway, remembered the first time it is asked for.
    pub fn of(&mut self, id: AssetId<A>, now: f32) -> f32 {
        *self.0.entry(id).or_insert(now)
    }
}
