//! A realm's built parts, stood by rule (D642, docs/realm-two.md): what its row of
//! `data/realms.ron` says stands on its land (the industrial realm's pylons, vent stacks,
//! tanks, generators and signs), each a catalogue item, scattered by the world's seed round
//! the start: a cell of the ground holds one or none by the seed, which one by their shares,
//! where the ground is dry and level enough, clear of the start and of what is kept clear (the
//! gate). They are the realm's, not a player's: never saved, the same on every entering.
//!
//! Round the start only, for now: a realm's parts beyond it are to stream as its woods do.

use super::catalogue::Builds;
use super::props::{self, Models, Theirs};
use crate::build::Site;
use bevy::prelude::*;
use terra_gen::hash::{mix3, unit3};
use terra_world::flora::{ClearAround, KeptClear};
use terra_world::instance::{InstanceApp, WorldSpec};
use terra_world::planet::Planet;
use terra_world::realm::Realms;
use terra_world::start::Start;

/// How far round the start the parts are stood, and the side of a cell that holds one at
/// most, in metres.
const REACH_M: f32 = 500.0;
const CELL_M: f32 = 30.0;
/// No part within this of the start, where a player arrives.
const CLEAR_OF_START_M: f32 = 25.0;
/// The ground may rise or fall this much across a part's footing, in metres.
const LEVEL_M: f32 = 0.6;

/// Made afresh in each world: its parts have been stood.
#[derive(Resource, Default)]
struct Stood(bool);

pub fn plugin(app: &mut App) {
    app.init_world_resource::<Stood>().add_systems(
        Update,
        stand
            .run_if(terra_world::instance::in_world)
            .run_if(resource_exists::<Site>),
    );
}

/// The cells of the ground round the start that hold a part, each with which part (an index
/// into `shares`) and where in the cell and how turned, by the seed: `shares` are how many of
/// each part a square kilometre holds.
pub fn cells(seed: u32, shares: &[f32]) -> Vec<(usize, Vec2, f32)> {
    let total: f32 = shares.iter().sum();
    // The chance a cell holds one: the parts a square kilometre holds over its cells.
    let chance = (total * CELL_M * CELL_M / 1.0e6).min(1.0);
    let reach = (REACH_M / CELL_M).ceil() as i32;
    let mut held = Vec::new();
    for row in -reach..=reach {
        for column in -reach..=reach {
            let (c, r) = (column as u32, row as u32);
            if unit3(seed, c, r, 0) >= chance {
                continue;
            }
            let at = Vec2::new(
                (column as f32 + unit3(seed, c, r, 1)) * CELL_M,
                (row as f32 + unit3(seed, c, r, 2)) * CELL_M,
            );
            if !(CLEAR_OF_START_M..REACH_M).contains(&at.length()) {
                continue;
            }
            // Which part: by its share of them all.
            let mut pick = unit3(seed, c, r, 3) * total;
            let part = shares
                .iter()
                .position(|share| {
                    pick -= share;
                    pick < 0.0
                })
                .unwrap_or(shares.len() - 1);
            let turn = (mix3(seed, c, r, 4) % 360) as f32;
            held.push((part, at, turn));
        }
    }
    held
}

/// Stands the realm's parts, once in each world entered that has any.
#[allow(clippy::too_many_arguments)]
fn stand(
    mut commands: Commands,
    mut stood: ResMut<Stood>,
    (realms, spec): (Res<Realms>, Res<WorldSpec>),
    (planet, start, site): (Res<Planet>, Res<Start>, Res<Site>),
    (builds, models): (Res<Builds>, Res<Models>),
    kept: Option<Res<KeptClear>>,
    mut clear: MessageWriter<ClearAround>,
) {
    if std::mem::replace(&mut stood.0, true) {
        return;
    }
    let Some(realm) = realms
        .of(&spec.kind)
        .filter(|realm| !realm.parts.is_empty())
    else {
        return;
    };
    let items: Vec<_> = realm
        .parts
        .iter()
        .filter_map(|part| {
            let item = builds.item_from(&part.file);
            if item.is_none() {
                error!(
                    "{} has {} among its parts, which is not in the catalogue (data/builds.ron).",
                    realm.name, part.file
                );
            }
            Some((item?, part.a_km2))
        })
        .collect();
    let shares: Vec<f32> = items.iter().map(|(_, share)| *share).collect();
    let seed = planet.seed().derive("realm parts").0 as u32;
    let to_site = site.frame.compute_affine().inverse();
    let on_ground = |at: Vec2| planet.ground_at(start.at(at.x, at.y));
    let mut count = 0;
    for (part, at, turn) in cells(seed, &shares) {
        let (item, _) = &items[part];
        let ground = on_ground(at);
        // Dry, level under its footing, and clear of what is kept clear.
        let footing = item.dims.x.max(item.dims.z) / 2.0;
        let rise = [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y]
            .iter()
            .map(|way| (on_ground(at + *way * footing).length() - ground.length()).abs())
            .fold(0.0, f32::max);
        let wet = ground.length() < planet.water_near_m(ground) + 0.05;
        if wet || rise > LEVEL_M || kept.as_ref().is_some_and(|kept| kept.holds(ground)) {
            continue;
        }
        let Some(entity) = props::place(
            &mut commands,
            &models,
            &site,
            &planet,
            item,
            to_site.transform_point3(ground).with_y(0.0),
            0.0,
            turn,
            false,
        ) else {
            continue;
        };
        commands.entity(entity).insert(Theirs);
        clear.write(ClearAround(ground, footing + 0.8));
        count += 1;
    }
    info!(
        "{}: {count} of its parts stand round the start.",
        realm.name
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seed_scatters_the_parts_by_their_shares_and_clear_of_the_start() {
        let shares = [20.0, 25.0, 25.0, 30.0, 20.0];
        let held = cells(7, &shares);
        // About as many as the land round the start holds at those shares: 120 a square
        // kilometre over a disc of 500 m, 0.785 km2: some 94.
        assert!((60..130).contains(&held.len()), "{} parts", held.len());
        // Each kind stands, none within the start's clearing or past the reach.
        for part in 0..shares.len() {
            assert!(
                held.iter().any(|(which, ..)| *which == part),
                "no part {part}"
            );
        }
        assert!(
            held.iter()
                .all(|(_, at, _)| (CLEAR_OF_START_M..REACH_M).contains(&at.length()))
        );
        // The same seed, the same land; another seed, another.
        assert_eq!(cells(7, &shares), held);
        assert_ne!(cells(8, &shares), held);
    }
}
