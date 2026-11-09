//! `wildlife.put {kind, count?}`: so many of a kind put out a few metres ahead of this
//! machine's player, on the ground there, as its kind's groups are put about the land
//! (`groups::put_group`): for testing a creature without walking the land for one (the debug
//! menu's Play page, D81).

use super::Surroundings;
use super::groups::put_group;
use crate::creatures::Kinds;
use crate::species::Bestiary;
use bevy::prelude::*;
use bevy::remote::{BrpResult, builtin_methods::parse_some};
use serde::Deserialize;
use terra_world::remote::refused;

/// How far ahead of the player they are put, metres: in sight, out of a blow's reach.
const AHEAD_M: f32 = 8.0;
/// The most put at once.
const MOST: u32 = 12;

#[derive(Deserialize)]
struct Put {
    kind: String,
    count: Option<u32>,
}

pub(super) fn put(
    In(params): In<Option<serde_json::Value>>,
    mut commands: Commands,
    (kinds, bestiary): (Res<Kinds>, Res<Bestiary>),
    around: Surroundings,
    player: Query<&Transform, terra_walker::IsLocalPlayer>,
    time: Res<Time>,
) -> BrpResult {
    let Put { kind, count } = parse_some(params)?;
    let species = bestiary.named(&kind).ok_or_else(|| {
        let known: Vec<&str> = bestiary.all().map(|each| bestiary.name(each)).collect();
        refused(format!(
            "there is no kind of creature called {kind}; the kinds are {}",
            known.join(", ")
        ))
    })?;
    let body = (player.iter().next()).ok_or_else(|| refused("There is no player yet."))?;
    let up = body.translation.normalize_or(Vec3::Y);
    let ahead = (body.rotation * Vec3::NEG_Z)
        .reject_from(up)
        .normalize_or_zero();
    let spot = around.ground(body.translation + ahead * AHEAD_M);
    let count = count.unwrap_or(1).clamp(1, MOST);
    // Where each of a group stands is rolled; from the clock, so two asked for are not one.
    let mut dice = terra_gen::Seed(time.elapsed().as_millis() as u64).rng();
    let put = put_group(
        &mut commands,
        (&kinds, &bestiary),
        &around,
        species,
        (spot, ahead.cross(up) * 2.5),
        count,
        &mut dice,
    );
    Ok(serde_json::json!({ "kind": bestiary.name(species), "put": put.len() }))
}
