//! The one wind (D255) from the climate (docs/demos.md §2, step 4): which way it blows and how
//! hard where the camera is, as `terra_climate`'s fields say, taken again every few seconds of
//! the clock, and handed to everything that sways: the grass, the grown plants and trees, the
//! flora and the lakes' ripples. Its strength is the climate's speed over its mean, so the
//! sway the art was tuned at (1) is the climate's mean wind, a calm spell stills it and a
//! gale bends it twice as far.

use crate::climate::ClimateFields;
use crate::daylight::Clock;
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

pub fn plugin(app: &mut App) {
    app.add_systems(Update, follow.after(crate::daylight::Ticking));
}

/// Takes the wind where the camera is, from the climate, every few seconds of the clock; the
/// resource changes only when the wind has, so materials are touched only then.
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
    if last.is_some_and(|when| (now - when).abs() < EVERY_DAYS) {
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
