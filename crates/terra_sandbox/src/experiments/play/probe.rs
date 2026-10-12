//! What a script can ask of the play experiment and do in it, over the remote protocol
//! (D69), so a test reads what is true instead of guessing it from the screen or the log:
//!
//! - `play.state`: where the player is and faces, their body, what they hold, carry and
//!   wear, and the creatures and people near them.
//! - `play.warp {at, facing?}`: moves the player, as `--warp` does, on the next frame.
//! - `play.give {item, count?}`: adds to what the player carries, or sets their body, as
//!   `--give` does, on the next frame.
//!
//! Places are in metres east and south of the start, as the command-line options take them.

use super::{
    Player, View,
    creatures::Creature,
    inventory::{self, Backpack, Bar, Gear, Items},
    people::Person,
    vitals::Vitals,
};
use crate::experiments::build::Site;
use crate::remote::{method, refused};
use bevy::{
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
};
use serde::Deserialize;
use serde_json::{Value, json};

/// How far round the player `play.state` looks for creatures and people.
const NEAR_M: f32 = 40.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<Asked>()
        .add_systems(Update, give.before(super::tool::Acting));
    method(app, "play.state", state);
    method(app, "play.warp", warp);
    method(app, "play.give", ask_give);
}

/// What a script asked for, done on the next frame by the systems that do the same for
/// the command line.
#[derive(Resource, Default)]
pub(super) struct Asked {
    /// Where to stand, and what to face if not straight ahead.
    pub warp: Option<((f32, f32), Option<(f32, f32)>)>,
    gives: Vec<(String, u32)>,
}

#[derive(Deserialize)]
struct Warp {
    at: [f32; 2],
    facing: Option<[f32; 2]>,
}

fn warp(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Warp { at, facing } = parse_some(params)?;
    world.resource_mut::<Asked>().warp = Some(((at[0], at[1]), facing.map(|f| (f[0], f[1]))));
    Ok(json!("warping on the next frame"))
}

#[derive(Deserialize)]
struct Give {
    item: String,
    count: Option<u32>,
}

fn ask_give(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Give { item, count } = parse_some(params)?;
    let count = count.unwrap_or(1);
    world.resource_mut::<Asked>().gives.push((item, count));
    Ok(json!("giving on the next frame"))
}

fn give(
    mut asked: ResMut<Asked>,
    mut items: ResMut<Items>,
    player: Single<(&mut Backpack, &mut Gear, &mut Bar, &mut Vitals), With<Player>>,
) {
    if asked.gives.is_empty() {
        return;
    }
    let gives = std::mem::take(&mut asked.gives);
    let (mut backpack, mut gear, mut bar, mut body) = player.into_inner();
    for (name, value) in &gives {
        let value = *value as f32;
        match name.as_str() {
            "health" => body.health = value,
            "fed" => body.fed = value,
            "stamina" => body.stamina = value,
            _ => {}
        }
    }
    inventory::give_for_testing(&mut items, &mut backpack.0, &mut gear.0, &mut bar.0, &gives);
    for (name, count) in gives {
        info!("Given {count} {name} for testing.");
    }
}

/// Metres east and south of the start, as the command line gives places.
fn east_south(site: &Transform, at: Vec3) -> [f64; 2] {
    let local = site.compute_affine().inverse().transform_point3(at);
    [round(local.x), round(local.z)]
}

/// To the centimetre, which is as fine as a test needs and easier to read.
fn round(value: f32) -> f64 {
    (f64::from(value) * 100.0).round() / 100.0
}

fn state(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let mut players =
        world.query_filtered::<(&Transform, &Player, &Vitals, &Backpack, &Gear, &Bar), ()>();
    let mut creatures = world.query::<(&Creature, &GlobalTransform)>();
    let mut people = world.query_filtered::<(&Name, &GlobalTransform), With<Person>>();
    let site = world
        .get_resource::<Site>()
        .ok_or_else(|| refused("no site yet"))?
        .frame;
    let forward = world.resource::<View>().forward;
    let (transform, player, body, backpack, gear, bar) = players
        .single(world)
        .map_err(|_| refused("no player yet"))?;
    let items = world.resource::<Items>();
    let feet = transform.translation;
    let here = east_south(&site, feet);
    let ahead = east_south(&site, feet + forward);
    let carrying: serde_json::Map<String, Value> = items
        .catalogue
        .iter()
        .filter_map(|(kind, def)| {
            let count = backpack.0.count(kind) + gear.0.stacked(kind);
            (count > 0).then(|| (def.name.clone(), json!(count)))
        })
        .collect();
    let wearing: serde_json::Map<String, Value> = gear
        .0
        .iter()
        .filter_map(|(slot, worn)| {
            let (_, name) = inventory::SLOTS.iter().find(|(each, _)| *each == slot)?;
            Some((name.to_string(), json!(items.name(worn?.kind))))
        })
        .collect();

    let mut near = Vec::new();
    for (creature, place) in creatures.iter(world) {
        let distance = place.translation().distance(feet);
        if distance < NEAR_M {
            near.push((
                distance,
                json!({
                    "what": format!("{:?}", creature.species),
                    "at": east_south(&site, place.translation()),
                    "distance": round(distance),
                    "health": round(creature.health),
                    "doing": format!("{:?}", creature.state),
                }),
            ));
        }
    }
    for (name, place) in people.iter(world) {
        let distance = place.translation().distance(feet);
        if distance < NEAR_M {
            near.push((
                distance,
                json!({
                    "who": name.as_str(),
                    "at": east_south(&site, place.translation()),
                    "distance": round(distance),
                }),
            ));
        }
    }
    near.sort_by(|a, b| a.0.total_cmp(&b.0));

    Ok(json!({
        "at": here,
        "facing": [round((ahead[0] - here[0]) as f32), round((ahead[1] - here[1]) as f32)],
        "on_ground": player.on_ground,
        "indoors": player.indoors,
        "down": player.down.is_some(),
        "health": round(body.health),
        "stamina": round(body.stamina),
        "fed": round(body.fed),
        "holding": inventory::holding(items, &backpack.0, &bar.0).map(|tool| tool.name()),
        "carrying": carrying,
        "wearing": wearing,
        "near": near.into_iter().map(|(_, each)| each).collect::<Vec<_>>(),
    }))
}
