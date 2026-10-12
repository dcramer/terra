//! What a script can ask of the play experiment and do in it, over the remote protocol
//! (D69), so a test reads what is true instead of guessing it from the screen or the log:
//!
//! - `play.state`: where the player is and faces, their body, what they hold, carry and
//!   wear, and the creatures and people near them.
//! - `play.warp {at, facing?}`: moves the player, as `--warp` does, on the next frame.
//! - `play.give {item, count?}`: adds to what the player carries, or sets their body, as
//!   `--give` does, on the next frame.
//! - `play.take {item, count?}`: takes raw items away, all of that kind if no count is
//!   given, so a test can start from a known pack, on the next frame.
//! - `play.aim {at, up?}`: turns the view until the middle of it, where a bow shoots and a
//!   tool reaches, is on a spot so far above the ground (a metre if not given), and says so.
//!
//! With `--trace FILE`, the same, of the whole world and with every number exact, is
//! written every frame, to find where two runs part (`tools/sandbox/diverge.ts`).
//!
//! Places are in metres east and south of the start, as the command-line options take them.

use super::{
    Player, View,
    creatures::Creature,
    inventory::{self, Backpack, Bar, Gear, Items, Raw},
    people::Person,
    vitals::Vitals,
};
use crate::build::Site;
use crate::planet::{MainCamera, Planet};
use crate::remote::{method, refused};
use bevy::{
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
};
use serde::Deserialize;
use serde_json::{Value, json};

/// How far round the player `play.state` looks for creatures and people.
const NEAR_M: f32 = 40.0;
/// How close the middle of the view must pass to a spot to be on it, and how many frames
/// of turning toward it are allowed.
const ON_M: f32 = 0.05;
const AIM_FRAMES: u32 = 90;

pub fn plugin(app: &mut App) {
    app.init_resource::<Asked>()
        .add_systems(Startup, open_trace)
        .add_systems(Update, (give, take, aim).before(super::tool::Acting))
        .add_systems(Last, trace);
    method(app, "play.state", state);
    method(app, "play.warp", warp);
    method(app, "play.give", ask_give);
    method(app, "play.take", ask_take);
    method(app, "play.aim", ask_aim);
}

/// What a script asked for, done on the next frame by the systems that do the same for
/// the command line.
#[derive(Resource, Default)]
pub(super) struct Asked {
    /// Where to stand, and what to face if not straight ahead.
    pub warp: Option<((f32, f32), Option<(f32, f32)>)>,
    gives: Vec<(String, u32)>,
    takes: Vec<(String, Option<u32>)>,
    /// A spot to aim at, east and south and how far above the ground, and the frames
    /// spent turning toward it so far.
    aim: Option<((f32, f32), f32, u32)>,
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
struct Take {
    item: String,
    count: Option<u32>,
}

fn ask_take(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Take { item, count } = parse_some(params)?;
    world.resource_mut::<Asked>().takes.push((item, count));
    Ok(json!("taking on the next frame"))
}

fn take(
    mut asked: ResMut<Asked>,
    items: Res<Items>,
    player: Single<(&mut Backpack, &mut Gear, &mut Bar), With<Player>>,
) {
    if asked.takes.is_empty() {
        return;
    }
    let (mut backpack, mut gear, mut bar) = player.into_inner();
    for (name, count) in std::mem::take(&mut asked.takes) {
        let wanted = name.replace('_', " ");
        let Some(raw) = Raw::ALL
            .into_iter()
            .find(|raw| raw.name().eq_ignore_ascii_case(&wanted))
        else {
            warn!("Took nothing: there is no raw item called {name}.");
            continue;
        };
        let kind = items.kind(raw);
        let loose = backpack.0.count(kind);
        let from_pack = count.unwrap_or(u32::MAX).min(loose);
        let paid = backpack.0.take(&[(kind, from_pack)]).is_ok();
        let from_gear = gear
            .0
            .draw(kind, count.map_or(u32::MAX, |count| count - from_pack));
        bar.0.tidy(&backpack.0);
        let took = if paid { from_pack } else { 0 } + from_gear;
        info!("Took {took} {} for testing.", raw.name());
    }
}

#[derive(Deserialize)]
struct Aim {
    at: [f32; 2],
    up: Option<f32>,
}

fn ask_aim(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Aim { at, up } = parse_some(params)?;
    world.resource_mut::<Asked>().aim = Some(((at[0], at[1]), up.unwrap_or(1.0), 0));
    Ok(json!("aiming from the next frame"))
}

/// Turns the view toward the spot asked for. The camera hangs behind the player's
/// shoulder and moves as the view turns, so it looks again from where the camera now is
/// until the middle of the view passes within a few centimetres of the spot.
fn aim(
    mut asked: ResMut<Asked>,
    site: Res<Site>,
    planet: Res<Planet>,
    mut view: ResMut<View>,
    camera: Single<&Transform, With<MainCamera>>,
) {
    let Some(((east, south), up_m, frames)) = asked.aim else {
        return;
    };
    let ground = planet.ground_at(site.frame.transform_point(Vec3::new(east, 0.0, south)));
    let target = ground + ground.normalize() * up_m;
    let eye = camera.translation;
    let off = (target - eye).reject_from(*camera.forward()).length();
    if frames > 0 && off < ON_M {
        info!("Aimed at {east:.1} m east and {south:.1} m south, {up_m:.1} m up.");
        asked.aim = None;
        return;
    }
    if frames == AIM_FRAMES {
        warn!("Could not aim at {east:.1} m east and {south:.1} m south: still {off:.2} m off.");
        asked.aim = None;
        return;
    }
    let up = eye.normalize();
    let toward = (target - eye).normalize();
    view.forward = toward.reject_from(up).normalize_or(view.forward);
    view.pitch = libm::asinf(toward.dot(up).clamp(-1.0, 1.0)).clamp(-1.2, 1.2);
    asked.aim = Some(((east, south), up_m, frames + 1));
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
fn east_south(site: &Transform, at: Vec3) -> [f32; 2] {
    let local = site.compute_affine().inverse().transform_point3(at);
    [local.x, local.z]
}

/// To the centimetre, which is as fine as a test needs and easier to read.
fn round(value: f32) -> f64 {
    (f64::from(value) * 100.0).round() / 100.0
}

fn state(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    digest(world, false)
}

/// What is true of the player and what is round them. `whole` takes in every creature and
/// person wherever they are, with every number exactly as it is, for comparing one run
/// with another (`--trace`); otherwise only what is near, to the centimetre, for reading.
pub(super) fn digest(world: &mut World, whole: bool) -> BrpResult {
    let number = |value: f32| {
        if whole {
            f64::from(value)
        } else {
            round(value)
        }
    };
    let place = |[east, south]: [f32; 2]| [number(east), number(south)];
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

    let near = |distance: f32| whole || distance < NEAR_M;
    let mut around = Vec::new();
    for (creature, at) in creatures.iter(world) {
        let distance = at.translation().distance(feet);
        if near(distance) {
            let home = east_south(&site, creature.home());
            around.push((
                distance,
                format!("{:?} {:.3} {:.3}", creature.species, home[0], home[1]),
                json!({
                    "what": format!("{:?}", creature.species),
                    "at": place(east_south(&site, at.translation())),
                    "distance": number(distance),
                    "health": number(creature.health),
                    "doing": format!("{:?}", creature.state),
                }),
            ));
        }
    }
    for (name, at) in people.iter(world) {
        let distance = at.translation().distance(feet);
        if near(distance) {
            around.push((
                distance,
                name.as_str().to_string(),
                json!({
                    "who": name.as_str(),
                    "at": place(east_south(&site, at.translation())),
                    "distance": number(distance),
                }),
            ));
        }
    }
    // Nearest first to read. To compare runs, by who they are: people by name, creatures by
    // where they were placed, since the world's own order can differ from run to run.
    if whole {
        around.sort_by(|a, b| a.1.cmp(&b.1));
    } else {
        around.sort_by(|a, b| a.0.total_cmp(&b.0));
    }

    Ok(json!({
        "at": place(here),
        "facing": place([ahead[0] - here[0], ahead[1] - here[1]]),
        "on_ground": player.on_ground,
        "indoors": player.indoors,
        "down": player.down.is_some(),
        "health": number(body.health),
        "stamina": number(body.stamina),
        "fed": number(body.fed),
        "holding": inventory::holding(items, &backpack.0, &bar.0).map(|tool| tool.name()),
        "carrying": carrying,
        "wearing": wearing,
        "near": around.into_iter().map(|(_, _, each)| each).collect::<Vec<_>>(),
    }))
}

/// For `--trace`: the whole state, every frame, one line of JSON each, to compare runs.
#[derive(Resource)]
struct Trace(std::io::BufWriter<std::fs::File>);

fn open_trace(mut commands: Commands, options: Res<crate::shell::Options>) {
    let Some(path) = &options.trace else { return };
    match std::fs::File::create(path) {
        Ok(file) => commands.insert_resource(Trace(std::io::BufWriter::new(file))),
        Err(error) => error!("Could not write a trace to {}: {error}", path.display()),
    }
}

fn trace(world: &mut World) {
    if !world.contains_resource::<Trace>() {
        return;
    }
    let Ok(state) = digest(world, true) else {
        return;
    };
    let line = json!({
        "frame": world.resource::<bevy::diagnostic::FrameCount>().0,
        "seconds": world.resource::<Time<Virtual>>().elapsed_secs_f64(),
        "state": state,
    });
    let mut trace = world.resource_mut::<Trace>();
    use std::io::Write;
    if writeln!(trace.0, "{line}")
        .and_then(|_| trace.0.flush())
        .is_err()
    {
        error!("Could not write the trace; no more will be written.");
        world.remove_resource::<Trace>();
    }
}
