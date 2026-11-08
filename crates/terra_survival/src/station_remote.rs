//! Stations over the remote protocol (D69), so a scripted run crafts as a player does at
//! the station's screen, with no picture:
//!
//! - `station.state`: every station standing: what it is, where from the start, its queue
//!   and what waits made, by name, and how many seconds into the first it is.
//! - `station.queue {recipe}`: queues one of what a recipe makes, by name, at the station
//!   nearest the player within reach that makes it, paid for from their pack as the screen
//!   pays (`crafting::queue`).
//! - `station.take`: takes what waits made at the station nearest the player within reach,
//!   into their pack, as opening its screen does.

use crate::crafting::{self, Crafts, Staff, Works};
use crate::interact::REACH_M;
use crate::inventory::Belongings;
use crate::props::Prop;
use crate::told::Notice;
use bevy::{
    ecs::system::SystemState,
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
};
use serde::Deserialize;
use serde_json::{Value, json};
use terra_walker::{LocalPlayer, Player};
use terra_world::remote::{method, refused};

pub fn plugin(app: &mut App) {
    method(app, "station.state", state);
    method(app, "station.queue", queue);
    method(app, "station.take", take);
}

type Stood<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Prop,
        &'static Transform,
        &'static mut Works,
    ),
>;
type Feet<'w, 's> = Query<'w, 's, &'static Transform, (With<Player>, With<LocalPlayer>)>;

fn state(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let mut state = SystemState::<(
        Crafts,
        Belongings,
        Stood,
        Option<Res<terra_world::start::Start>>,
    )>::new(world);
    let (crafts, belongings, stations, start) = state
        .get_mut(world)
        .map_err(|problem| refused(format!("cannot read the stations now: {problem}")))?;
    let items = belongings.items();
    let name = |index: &usize| crafts.recipes[*index].makes.name(items).to_string();
    let cm = |value: f32| (f64::from(value) * 100.0).round() / 100.0;
    let mut found: Vec<Value> = stations
        .iter()
        .filter_map(|(_, prop, at, works)| {
            Some(json!({
                "station": crafts.stations.name(prop.item.station?),
                "from_start": start.as_ref().map(|start| start.local(at.translation).to_array().map(cm)),
                "queue": works.queue.iter().map(name).collect::<Vec<_>>(),
                "seconds": works.progress,
                "made": works.done.iter().map(name).collect::<Vec<_>>(),
            }))
        })
        .collect();
    found.sort_by_key(|station| station["from_start"].to_string());
    Ok(json!({ "stations": found }))
}

/// The station nearest the feet within reach that passes, or why there is none.
fn nearest<'a>(
    stations: &'a mut Stood,
    feet: Vec3,
    passes: impl Fn(&Prop) -> bool,
) -> Result<(Entity, Mut<'a, Works>), bevy::remote::BrpError> {
    let apart = |at: &Transform| at.translation.distance(feet);
    let away = (stations.iter())
        .filter(|(_, prop, ..)| passes(prop))
        .map(|(_, _, at, _)| apart(at))
        .min_by(f32::total_cmp);
    stations
        .iter_mut()
        .filter(|(_, prop, at, _)| passes(prop) && apart(at) < REACH_M + 1.0)
        .min_by(|a, b| apart(a.2).total_cmp(&apart(b.2)))
        .map(|(entity, _, _, works)| (entity, works))
        .ok_or_else(|| {
            refused(match away {
                Some(away) => format!(
                    "no such station is within reach ({} m): the nearest stands {away:.1} m from the player; stand by it first",
                    REACH_M + 1.0
                ),
                None => "no such station stands in this world: set one down first (storage.place)".to_string(),
            })
        })
}

#[derive(Deserialize)]
struct Queue {
    recipe: String,
}

fn queue(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Queue { recipe } = parse_some(params)?;
    let mut state =
        SystemState::<(Crafts, Staff, Belongings, Stood, Feet, ResMut<Notice>)>::new(world);
    let answer = {
        let (crafts, staff, mut belongings, mut stations, feet, mut notice) = state
            .get_mut(world)
            .map_err(|problem| refused(format!("cannot craft now: {problem}")))?;
        let feet = (feet.single())
            .map_err(|_| refused("there is no player yet"))?
            .translation;
        let mut mine = (belongings.mine()).ok_or_else(|| refused("there is no player yet"))?;
        let wanted = recipe.replace('_', " ");
        let makes = |index: usize| {
            let name = crafts.recipes[index].makes.name(mine.items());
            name.eq_ignore_ascii_case(&wanted)
        };
        let index = (0..crafts.recipes.iter().count())
            .find(|index| makes(*index))
            .ok_or_else(|| refused(format!("no recipe makes {recipe} (data/recipes.ron)")))?;
        let station = crafts.recipes[index].station;
        let (entity, mut works) = nearest(&mut stations, feet, |prop| {
            prop.item.station == Some(station)
        })?;
        let before = works.queue.len();
        let kinds = (&*crafts.recipes, &*crafts.stations);
        let staffed = staff.works(entity, station);
        crafting::queue(&mut works, index, staffed, kinds, &mut mine, &mut notice);
        if works.queue.len() == before {
            return Err(refused(format!(
                "the {} did not take it: {}",
                crafts.stations.name(station).to_lowercase(),
                notice.text
            )));
        }
        json!({ "queued": works.queue.len() })
    };
    state.apply(world);
    Ok(answer)
}

fn take(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let mut state = SystemState::<(Crafts, Belongings, Stood, Feet)>::new(world);
    let answer = {
        let (crafts, mut belongings, mut stations, feet) = state
            .get_mut(world)
            .map_err(|problem| refused(format!("cannot take now: {problem}")))?;
        let feet = (feet.single())
            .map_err(|_| refused("there is no player yet"))?
            .translation;
        let mut mine = (belongings.mine()).ok_or_else(|| refused("there is no player yet"))?;
        let (_, mut works) = nearest(&mut stations, feet, |prop| prop.item.station.is_some())?;
        let (taken, waits) = crafting::collect(&mut works, &crafts.recipes, &mut mine);
        json!({ "took": taken, "no_room_for": waits })
    };
    state.apply(world);
    Ok(answer)
}
