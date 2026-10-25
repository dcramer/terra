//! The feedback key: the screen as it is and a note of the moment beside it, numbered, in
//! `~/.cache/terra-review/feedback/<n>-<time>.{png,json}` (`TERRA_FEEDBACK_DIR` elsewhere, as
//! a check keeps its own). The note holds what an agent needs to stand there again: where the
//! player and the camera are, in the start's terms `--at` and `--vantage` take, the clock's
//! day and hour (the weather follows from them), and the animals within 50 m.

use super::{Walk, moment};
use bevy::prelude::*;
use bevy::render::view::screenshot::save_to_disk;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use terra_creatures::creatures::Creature;
use terra_creatures::species::Bestiary;
use terra_walker::Player;
use terra_world::climate::SnowCover;
use terra_world::daylight::{Clock, WorldClimate};
use terra_world::planet::MainCamera;
use terra_world::shell::Picture;
use terra_world::start::Start;
use terra_world::weather::Weather;

/// Animals this near the player are named in the note.
pub const NEAR_M: f32 = 50.0;

/// Where the notes go.
pub fn folder() -> PathBuf {
    if let Some(dir) = std::env::var_os("TERRA_FEEDBACK_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    PathBuf::from(home.unwrap_or_default()).join(".cache/terra-review/feedback")
}

/// The world's state the note is made of.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct Moment<'w, 's> {
    climate: Res<'w, WorldClimate>,
    clock: Res<'w, Clock>,
    weather: Res<'w, Weather>,
    snow: Res<'w, SnowCover>,
    start: Option<Res<'w, Start>>,
    bestiary: Option<Res<'w, Bestiary>>,
    player: Query<'w, 's, &'static GlobalTransform, With<Player>>,
    camera: Query<'w, 's, &'static GlobalTransform, With<MainCamera>>,
    creatures: Query<'w, 's, (&'static Creature, &'static GlobalTransform)>,
}

pub(super) fn take(
    mut commands: Commands,
    moment_now: Moment,
    picture: Option<Res<Picture>>,
    mut walk: ResMut<Walk>,
) {
    let folder = folder();
    if let Err(problem) = std::fs::create_dir_all(&folder) {
        walk.notice(format!(
            "feedback not saved: {}: {problem}",
            folder.display()
        ));
        return;
    }
    let names: Vec<String> = std::fs::read_dir(&folder)
        .map(|dir| {
            dir.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    let number = super::rules::next_number(names.iter().map(String::as_str));
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let base = folder.join(format!("{number}-{}", terra_world::crash::stamp(seconds)));
    let (png, json) = (base.with_extension("png"), base.with_extension("json"));
    let mut note = note(&moment_now, &walk);
    note["number"] = json!(number);
    note["picture"] = json!(png.display().to_string());
    let text = serde_json::to_string_pretty(&note).unwrap_or_default();
    if let Err(problem) = std::fs::write(&json, text + "\n") {
        walk.notice(format!("feedback not saved: {}: {problem}", json.display()));
        return;
    }
    let shot = terra_world::shell::screenshot(picture.as_deref());
    commands.spawn(shot).observe(save_to_disk(png.clone()));
    info!(
        "forest-walk: feedback {number} saved {} {}",
        png.display(),
        json.display()
    );
    walk.notice(format!("feedback {number} saved"));
}

/// The note of the moment, but its number and picture.
fn note(now: &Moment, walk: &Walk) -> Value {
    let start = now
        .start
        .as_deref()
        .map_or(Start(Transform::IDENTITY), |s| *s);
    let local = |at: Vec3| {
        let l = start.local(at);
        [round(l.x), round(l.y), round(l.z)]
    };
    let player = now
        .player
        .iter()
        .next()
        .map(|t| (t.translation(), t.forward()));
    let camera = now
        .camera
        .iter()
        .next()
        .map(|t| (t.translation(), t.forward()));
    let (clock, year) = (*now.clock, &now.climate.0.year);
    let mut animals: Vec<(f32, Value)> = match (player, now.bestiary.as_deref()) {
        (Some((at, _)), Some(bestiary)) => now
            .creatures
            .iter()
            .map(|(c, t)| (t.translation().distance(at), c, t.translation()))
            .filter(|(d, _, _)| *d <= NEAR_M)
            .map(|(d, c, there)| {
                let l = local(there);
                let kind = bestiary.name(c.species);
                (
                    d,
                    json!({ "kind": kind, "distance_m": round(d), "at": [l[0], l[2]] }),
                )
            })
            .collect(),
        _ => Vec::new(),
    };
    animals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let hour = (clock.hour * 100.0).round() / 100.0;
    let mut again = format!(
        "cargo xtask sandbox --experiment forest-walk --date {} --time {hour}",
        clock.day
    );
    let mut fields = json!({
        "moment": moment(&now.climate, &clock, &walk.weather),
        "season": super::rules::season(year, clock.day),
        "date": terra_climate::Date::calendar(clock.day, year),
        "day": clock.day,
        "hour": hour,
        "weather": walk.weather,
        "weather_at_eye": {
            "cloud": round(now.weather.cloud),
            "falling_mm_h": round(now.weather.falling_mm_h),
            "snow_share": round(now.weather.snow_share),
            "wind_m_s": round(now.weather.wind_m_s),
            "temperature_c": round(now.weather.temperature_c),
        },
        "snow_lying": round(now.snow.0),
        "animals_within_50_m": animals.into_iter().map(|(_, a)| a).collect::<Vec<_>>(),
    });
    if let Some((at, forward)) = player {
        let (here, ahead) = (local(at), local(at + forward * 10.0));
        again += &format!(
            " --at {},{} --face {},{}",
            here[0], here[2], ahead[0], ahead[2]
        );
        fields["player"] = json!({ "at": here, "faces": [ahead[0], ahead[2]] });
    }
    fields["again"] = json!(again);
    if let Some((eye, forward)) = camera {
        let (eye, looks) = (local(eye), local(eye + forward * 10.0));
        let vantage = format!(
            "{},{},{},{},{},{}",
            eye[0], eye[1], eye[2], looks[0], looks[1], looks[2]
        );
        fields["camera"] = json!({ "eye": eye, "looks_at": looks });
        fields["again_view"] = json!(format!(
            "cargo xtask sandbox --experiment forest-walk --date {} --time {hour} --vantage {vantage}",
            clock.day
        ));
    }
    fields
}

/// To the centimetre, as a note is read.
fn round(value: f32) -> f32 {
    (value * 100.0).round() / 100.0
}
