//! Setting down an item from the build menu over the remote protocol (D69), as the hammer
//! sets it down, so a script can furnish a base:
//!
//! - `storage.place {item?, at, turn?}`: at a place in metres in the base's own space, a
//!   chest if none is named, paid for from the pack and the claim's storage as the hammer
//!   pays (D141). Its name is storage's, as scripts know it; acting on the base is building
//!   play's.

use crate::build::act::Building;
use crate::play::claim::{self, Claim};
use crate::play::props::{self, Models, PlacedBy};
use crate::play::{LocalPlayer, Player, catalogue};
use crate::remote::{method, refused};
use bevy::{
    ecs::system::SystemState,
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) fn plugin(app: &mut App) {
    method(app, "storage.place", place);
}

#[derive(Deserialize)]
struct Place {
    item: Option<String>,
    at: [f32; 3],
    turn: Option<f32>,
}

/// Sets down an item from the build menu where it is asked, paid for from the pack and the
/// claim's storage, as the hammer pays; refused outside the claim or short of its cost. A
/// claim stone goes down outside one, as the hammer sets it, if the player has none and it
/// keeps clear of other claims and the village.
fn place(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Place { item, at, turn } = parse_some(params)?;
    let name = item.unwrap_or_else(|| "Chest".to_string());
    let item = world
        .resource::<catalogue::Builds>()
        .item_named(&name)
        .ok_or_else(|| refused(format!("there is nothing called {name} to build")))?;
    let mut state = SystemState::<(
        Building,
        Commands,
        Res<Models>,
        Query<(Entity, &Claim), (With<Player>, With<LocalPlayer>)>,
        Query<&Claim>,
        Option<Res<claim::Village>>,
        Res<catalogue::Builds>,
    )>::new(world);
    let answer = {
        let (mut building, mut commands, models, players, claims, village, builds) = state
            .get_mut(world)
            .map_err(|problem| refused(format!("cannot build now: {problem}")))?;
        let (player, claim) = players
            .single()
            .map_err(|_| refused("there is no player to build as"))?;
        let at = Vec3::from_array(at);
        let on_planet = building.site.frame.transform_point(at);
        let too_near = || {
            let others = claims.iter().filter_map(Claim::keeps);
            let villages = village.iter().flat_map(|v| v.1.iter().map(|&p| (p, 0.0)));
            claim::too_near(on_planet, builds.claim.across_m[0], others, villages)
        };
        let claiming = item.claims && !building.free();
        if claiming && claim.stone.is_some() {
            json!({ "done": false, "refused": "You already have a claim" })
        } else if let Some(why) = claiming.then(too_near).flatten() {
            json!({ "done": false, "refused": why })
        } else if !claiming && !building.free() && !claim.covers(on_planet) {
            json!({ "done": false, "refused": "That is outside your claim" })
        } else if !building.pay(player, &item.cost, 1) {
            json!({ "done": false, "refused": format!("Not enough to build a {name}") })
        } else {
            match props::place(
                &mut commands,
                &models,
                &building.site,
                &building.planet,
                &item,
                at,
                0.0,
                turn.unwrap_or(0.0),
                false,
            ) {
                Some(placed) => {
                    commands.entity(placed).insert(PlacedBy(player));
                    info!("Placed a {name} at {at} by script.");
                    json!({ "done": true, "entity": placed.to_bits() })
                }
                None => json!({ "done": false, "refused": format!("{name} has no model") }),
            }
        }
    };
    state.apply(world);
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use bevy::remote::RemoteMethods;

    #[test]
    fn the_hammer_answers_storage_place_over_the_remote() {
        let mut app = App::new();
        app.init_resource::<RemoteMethods>();
        super::super::plugin(&mut app);
        let methods = app.world().resource::<RemoteMethods>();
        assert!(methods.get("storage.place").is_some());
    }
}
