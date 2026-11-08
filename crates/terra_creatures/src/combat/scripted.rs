//! A blow dealt by a script (D69): `creatures.strike` lands one on the nearest creature of a
//! kind as this machine's player, through the same rule a swing's does (`strike`), so a check
//! of what follows a kill (a mark, a carcass, a gate) need not win the fight first.

use super::{Blow, Struck, strike};
use crate::creatures::Creature;
use crate::species::{Bestiary, Part};
use bevy::prelude::*;
use bevy::remote::BrpResult;
use serde::Deserialize;
use terra_survival::skills::{Did, Skill};
use terra_survival::told::Told;
use terra_walker::{IsLocalPlayer, Player};
use terra_world::remote::refused;

pub(super) fn plugin(app: &mut App) {
    terra_world::remote::method(app, "creatures.strike", struck_by_script);
}

#[derive(Deserialize)]
struct Strike {
    /// The kind's name, as `data/creatures.ron` has it.
    name: String,
    damage: f32,
}

/// `creatures.strike {"name": "Troll", "damage": 1000}`: the nearest living one of that kind
/// takes a blow of so much to the body from this machine's player. Says what it did.
#[allow(clippy::too_many_arguments)]
fn struck_by_script(
    In(params): In<Option<serde_json::Value>>,
    mut commands: Commands,
    bestiary: Res<Bestiary>,
    player: Query<(Entity, &Transform), (With<Player>, IsLocalPlayer)>,
    mut creatures: Query<(Entity, &Transform, &mut Creature)>,
    (mut did, mut struck, mut told): (
        MessageWriter<Did>,
        MessageWriter<Struck>,
        MessageWriter<Told>,
    ),
) -> BrpResult {
    let Strike { name, damage } = bevy::remote::builtin_methods::parse_some(params)?;
    let (by, from) = player
        .single()
        .map_err(|_| refused("There is no player to strike as yet."))?;
    let nearest = creatures
        .iter_mut()
        .filter(|(_, _, creature)| creature.health > 0.0 && bestiary.name(creature.species) == name)
        .min_by(|a, b| {
            let away = |at: &Transform| at.translation.distance_squared(from.translation);
            away(a.1).total_cmp(&away(b.1))
        });
    let Some((entity, at, mut creature)) = nearest else {
        return Err(refused(format!(
            "No living {name} is in this world: data/creatures.ron names the kinds, and one is put out only where its `lives` says."
        )));
    };
    let blow = Blow {
        damage,
        at: at.translation,
        part: Part::Body,
        skill: Skill::Fighting,
    };
    let away_m = at.translation.distance(from.translation);
    let dealt = strike(
        &mut commands,
        &bestiary,
        by,
        entity,
        &mut creature,
        blow,
        &mut did,
        &mut struck,
        &mut told,
    );
    Ok(serde_json::json!({ "dealt": dealt, "health": creature.health.max(0.0), "away_m": away_m }))
}
