//! The forest walk (D467): the lush forest played on foot, its wildlife about, with keys that
//! turn the season, the time of day and the weather, and one that keeps the moment as a
//! feedback note an agent can stand in again. It is a demo of what exists, so each key moves
//! the world's own clock, as `--date`, `--time` and `--weather` do: winter is the climate's
//! January, with the snow its past left; a weather is the next day that has it at this hour.
//! What the season changes is what reads the climate (the ground's snow, the leaves, snow on
//! limbs and trunks); flowers that die back are placed by the season the run began in.
//!
//!   cargo xtask sandbox --experiment forest-walk

mod feedback;
mod panel;
mod rules;
#[cfg(test)]
mod tests;

use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;
use terra_world::climate::{ClimateFields, SnowCover};
use terra_world::daylight::{Clock, DAY_SECONDS, LATITUDE, Ticking, WorldClimate};
use terra_world::planet::MainCamera;
use terra_world::weather::{Named, WeatherFile};

/// What the walk stands for: the game with the lush forest on its rolling yard, the player at
/// the head of the forest film's path facing down it (`trailer/runs/forest.json`), at noon in
/// high summer. A run's own options come after, so they win.
pub const WITH: &[&str] = &[
    "--experiment",
    "play",
    "--experiment",
    "lush",
    "--level",
    "yard",
    "--no-lair",
    "--at",
    "-154,-104.4",
    "--face",
    "-128.6,-88.4",
    "--date",
    rules::SUMMER,
    "--time",
    "12",
];

/// The keys, free in play (`terra_walker::controls`, and F1 to F5 and F9 are the sandbox's).
pub const SEASON: KeyCode = KeyCode::F6;
pub const TIME_OF_DAY: KeyCode = KeyCode::F7;
pub const WEATHER: KeyCode = KeyCode::F8;
pub const EARLIER: KeyCode = KeyCode::KeyJ;
pub const LATER: KeyCode = KeyCode::KeyL;
pub const FEEDBACK: KeyCode = KeyCode::KeyP;
pub const PANEL: KeyCode = KeyCode::KeyO;

/// The walk's state: what the panel shows, and what was last said in the log.
#[derive(Resource)]
pub struct Walk {
    pub panel: bool,
    /// A line shown on the panel for a few seconds, and how long it has left.
    pub notice: Option<(String, f32)>,
    /// The climate's weather at the eye now, by name, as the panel shows it.
    pub weather: String,
    /// A key moved the clock: say the new moment.
    pub moved: bool,
    said_snow: Option<f32>,
}

impl Default for Walk {
    fn default() -> Self {
        Self {
            panel: true,
            notice: None,
            weather: String::new(),
            moved: true,
            said_snow: None,
        }
    }
}

impl Walk {
    fn notice(&mut self, text: String) {
        info!("forest-walk: {text}");
        self.notice = Some((text, NOTICE_S));
    }
}

const NOTICE_S: f32 = 4.0;

/// The named weathers, in their file's order.
#[derive(Resource)]
struct Weathers(Vec<Named>);

impl Weathers {
    fn names(&self) -> Vec<String> {
        self.0.iter().map(|n| n.name.clone()).collect()
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Walk>()
        .insert_resource(Weathers(WeatherFile::read().named))
        .add_systems(Startup, panel::spawn)
        .add_systems(
            Update,
            (
                season.run_if(input_just_pressed(SEASON)),
                time_of_day.run_if(input_just_pressed(TIME_OF_DAY)),
                earlier.run_if(input_just_pressed(EARLIER)),
                later.run_if(input_just_pressed(LATER)),
                weather.run_if(input_just_pressed(WEATHER)),
            )
                .before(Ticking),
        )
        .add_systems(
            Update,
            (
                toggle.run_if(input_just_pressed(PANEL)),
                say,
                feedback::take.run_if(input_just_pressed(FEEDBACK)),
                panel::show,
            )
                .chain()
                .after(Ticking),
        );
}

/// Summer to winter and back, by the date, the hour kept.
fn season(climate: Res<WorldClimate>, mut clock: ResMut<Clock>, mut walk: ResMut<Walk>) {
    let year = &climate.0.year;
    clock.day = rules::flipped(year, clock.day);
    walk.moved = true;
}

/// Dawn, noon, dusk and night in turn, on the same day.
fn time_of_day(climate: Res<WorldClimate>, mut clock: ResMut<Clock>, mut walk: ResMut<Walk>) {
    let times = rules::times_of_day(LATITUDE, climate.declination(clock.day));
    let (name, hour) = rules::next_time_of_day(&times, clock.hour);
    clock.hour = hour;
    walk.notice(format!("{name}, {}", rules::clock_text(hour)));
    walk.moved = true;
}

fn earlier(clock: ResMut<Clock>, walk: ResMut<Walk>) {
    step(clock, walk, -1.0);
}

fn later(clock: ResMut<Clock>, walk: ResMut<Walk>) {
    step(clock, walk, 1.0);
}

/// An hour on or back, into the next or last day past midnight.
fn step(mut clock: ResMut<Clock>, mut walk: ResMut<Walk>, hours: f32) {
    clock.advance(hours / 24.0 * DAY_SECONDS);
    walk.moved = true;
}

/// The next weather in the cycle that a day within a month has at this hour, the clock moved
/// to it.
fn weather(
    fields: Option<Res<ClimateFields>>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    weathers: Res<Weathers>,
    mut clock: ResMut<Clock>,
    mut walk: ResMut<Walk>,
) {
    let (Some(fields), Some(camera)) = (fields, camera) else {
        return;
    };
    let names = weathers.names();
    let days: Vec<(i64, String)> = rules::nearest_days()
        .map(|on| {
            let at = Clock {
                day: clock.day + on,
                hour: clock.hour,
            };
            (on, called_at(&fields, &weathers, camera.translation(), at))
        })
        .collect();
    let cycle: Vec<String> = std::iter::once(rules::CLEAR.to_string())
        .chain(names)
        .collect();
    match rules::next_weather(&cycle, &days[0].1, &days) {
        Some((name, on, passed)) => {
            clock.day += on;
            let on = match on {
                0 => "today".to_string(),
                on if on < 0 => format!("{} days back", -on),
                on => format!("{on} days on"),
            };
            let passed = if passed.is_empty() {
                String::new()
            } else {
                format!(" (none of {} within a month either way)", passed.join(", "))
            };
            walk.notice(format!("{name}, {on}{passed}"));
        }
        None => walk.notice(format!(
            "{}: no other weather within {} days",
            days[0].1,
            rules::LOOKS_DAYS
        )),
    }
    walk.moved = true;
}

/// What the climate's weather is called at a place and moment.
fn called_at(fields: &ClimateFields, weathers: &Weathers, at: Vec3, clock: Clock) -> String {
    let now = fields.fields.at(fields.place(at), clock.time());
    rules::called(
        &weathers.names(),
        |i| weathers.0[i].is(&now),
        now.cloud,
        now.precipitation_mm_per_hour,
    )
}

fn toggle(mut walk: ResMut<Walk>) {
    walk.panel = !walk.panel;
}

/// Says the moment in the log when a key moved it, and the snow lying round the eye when it
/// has changed by a tenth, so a scripted run can be checked by what it says.
#[allow(clippy::too_many_arguments)]
fn say(
    time: Res<Time>,
    climate: Res<WorldClimate>,
    clock: Res<Clock>,
    fields: Option<Res<ClimateFields>>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    weathers: Res<Weathers>,
    snow: Res<SnowCover>,
    mut walk: ResMut<Walk>,
    mut every: Local<f32>,
) {
    *every -= time.delta_secs();
    if let Some((_, left)) = &mut walk.notice {
        *left -= time.delta_secs();
        if *left <= 0.0 {
            walk.notice = None;
        }
    }
    let (Some(fields), Some(camera)) = (fields, camera) else {
        return;
    };
    if walk.moved || *every <= 0.0 {
        *every = 1.0;
        walk.weather = called_at(&fields, &weathers, camera.translation(), *clock);
    }
    if walk.moved {
        walk.moved = false;
        info!("forest-walk: {}", moment(&climate, &clock, &walk.weather));
    }
    if walk
        .said_snow
        .is_none_or(|said| (snow.0 - said).abs() >= 0.1)
    {
        walk.said_snow = Some(snow.0);
        info!("forest-walk: snow lying {:.0}%", snow.0 * 100.0);
    }
}

/// The moment as the panel and the log give it: `summer 07-20 (day 329) 12:00 clear`.
fn moment(climate: &WorldClimate, clock: &Clock, weather: &str) -> String {
    let year = &climate.0.year;
    format!(
        "{} {} (day {}) {} {weather}",
        rules::season(year, clock.day),
        terra_climate::Date::calendar(clock.day, year),
        clock.day,
        rules::clock_text(clock.hour)
    )
}
