//! The land where the eye is, as a scripted run reads it (D69; v0's "both walk three biomes
//! in changing weather"): `land.state` says the biome there by the land's own survey
//! (`terra_land::survey`), the weather there now, the clock, and the nearest place of each
//! biome that is well inside it within a reach the ask gives (`reach_m`), in the terms `warp` takes (metres east and south of the start), so a
//! check walks players through the biomes a world has and compares what each game says.

use crate::daylight::Clock;
use crate::planet::{MainCamera, Planet};
use crate::start::Start;
use crate::weather::Weather;
use bevy::prelude::*;
use terra_land::survey::{Biome, Grid};

/// How far round the eye the land is surveyed to say the biome there, in metres; a place
/// of each biome is looked for as far as the ask says (`reach_m`), and not at all without.
const HERE_M: f32 = 150.0;

#[derive(serde::Deserialize, Default)]
struct Ask {
    reach_m: Option<f32>,
}

pub fn plugin(app: &mut App) {
    crate::remote::method(app, "land.state", state);
}

fn state(
    In(params): In<Option<serde_json::Value>>,
    planet: Option<Res<Planet>>,
    start: Option<Res<Start>>,
    eye: Query<&GlobalTransform, With<MainCamera>>,
    weather: Option<Res<Weather>>,
    clock: Option<Res<Clock>>,
) -> bevy::remote::BrpResult {
    use crate::remote::refused;
    let (Some(planet), Some(start)) = (planet, start) else {
        return Err(refused("No world is made yet."));
    };
    let Some(round) = planet.round() else {
        return Err(refused(
            "This world's land is not surveyed (a test level, or a world with no land round its start worked out).",
        ));
    };
    let eye = eye
        .iter()
        .next()
        .ok_or_else(|| refused("There is no camera yet."))?
        .translation();
    let ask: Ask = params
        .map(serde_json::from_value)
        .transpose()
        .map_err(|problem| {
            refused(format!(
                "land.state takes {{\"reach_m\": metres}} or nothing: {problem}"
            ))
        })?
        .unwrap_or_default();
    let reach_m = ask
        .reach_m
        .map_or(HERE_M, |reach| reach.clamp(HERE_M, 5000.0));
    let grid = Grid::round(&round.land, eye.normalize(), reach_m);
    let n = grid.region.cells;
    let east_south = |column: usize, row: usize| {
        let at = start.local(grid.region.direction(column, row) * eye.length());
        [at.x, at.z]
    };
    // The biome under the eye, and the nearest cell of each biome to it.
    let here = grid
        .region
        .cell_of(eye.normalize())
        .unwrap_or((n / 2, n / 2));
    let mut nearest: Vec<(Biome, f32, [f32; 2])> = Vec::new();
    // Of each biome, a cell with its own kind all round it: a place well inside it, which a
    // survey laid from anywhere near calls the same (one on its edge another calls its
    // neighbour's).
    let inside = |column: usize, row: usize| {
        let biome = grid.biomes[row * n + column];
        (row.saturating_sub(1)..=(row + 1).min(n - 1)).all(|r| {
            (column.saturating_sub(1)..=(column + 1).min(n - 1))
                .all(|c| grid.biomes[r * n + c] == biome)
        })
    };
    for row in 0..n {
        for column in 0..n {
            if !inside(column, row) {
                continue;
            }
            let biome = grid.biomes[row * n + column];
            let away =
                (grid.region.offset(column, row) - grid.region.offset(here.0, here.1)).length();
            match nearest.iter_mut().find(|(each, ..)| *each == biome) {
                Some(found) if found.1 <= away => {}
                Some(found) => *found = (biome, away, east_south(column, row)),
                None => nearest.push((biome, away, east_south(column, row))),
            }
        }
    }
    let places: serde_json::Map<String, serde_json::Value> = nearest
        .into_iter()
        .map(|(biome, away, at)| {
            (
                biome.name().to_string(),
                serde_json::json!({ "at": at, "away_m": away }),
            )
        })
        .collect();
    Ok(serde_json::json!({
        "biome": grid.biomes[here.1 * n + here.0].name(),
        "weather": weather.map(|weather| serde_json::json!({
            "cloud": weather.cloud,
            "falling_mm_h": weather.falling_mm_h,
            "snow_share": weather.snow_share,
            "wind_m_s": weather.wind_m_s,
            "temperature_c": weather.temperature_c,
        })),
        "clock": clock.map(|clock| serde_json::json!({ "hour": clock.hour, "day": clock.day })),
        "nearest": ask.reach_m.map(|_| places),
    }))
}
