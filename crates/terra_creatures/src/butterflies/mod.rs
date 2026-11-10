//! Butterflies as things in the world (docs/ambient-life.md, D655, D647): for the eye only
//! in v0. Each breed is a row of `assets/data/butterflies.ron`: a model, in small groups
//! put by rule from the world's seed where the place and its season suit it, in cells
//! round whoever looks, so two players find a group in the same place; out when the air is
//! warm enough in its hours, each flying a path of its own about its group ([`flight`]). A
//! new breed is a model and a row (D136). They are each game's own, as the fish are:
//! nothing of them crosses the network, and a group begins its flight when it is first
//! come on.
//!
//! How many (D127, D182): five breeds, told apart by colour, pattern and size; a group is
//! one entity and its butterflies its children, instances of one mesh in one material
//! ([`look`]); at most [`MOST`] stand at once, and a breed is not drawn past its row's
//! `seen_m`. On open ground that suits them all the rows come to some 29 groups and 95
//! butterflies to a hectare: dozens within sight, a group in some views and none in others.

pub mod flight;
mod look;

use bevy::camera::visibility::VisibilityRange;
use bevy::light::NotShadowCaster;
use bevy::mesh::MeshTag;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use flight::{Flight, Member};
use serde::Deserialize;
use std::f32::consts::TAU;
use terra_gen::hash::unit3;
use terra_world::climate::ClimateFields;
use terra_world::daylight::Clock;
use terra_world::drift::{Round, in_hours, round_at, warm_enough};
use terra_world::flora::{Obstacles, Trunks};
use terra_world::instance::{InstanceApp, WorldSpec, in_world};
use terra_world::planet::{MainCamera, Planet};
use terra_world::start::Start;
use terra_world::weather::Weather;

const FILE: &str = "butterflies.ron";
/// A cell holds at most one group, metres across.
const CELL_M: f32 = 10.0;
const CELL_HA: f32 = CELL_M * CELL_M / 10_000.0;
/// No more butterflies than this stand at once, whatever the place.
pub const MOST: usize = 120;
/// How many cells are looked at a frame: each that the seed gives a group asks the land
/// what its place is.
const CELLS_A_FRAME: usize = 6;
/// None fly in rain heavier than this, millimetres an hour.
const RAIN_MM_H: f32 = 0.2;
/// A group comes out or goes in over so long, seconds, and a butterfly turns at most so
/// fast, radians a second.
const OUT_S: f32 = 1.5;
const TURNS: f32 = 9.0;
/// A group flies over ground no steeper than this: the cosine of its slope.
const LEVEL: f32 = 0.8;

/// A breed of butterfly, as its row says (the file's header says each field).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ButterflyDef {
    pub name: String,
    pub model: String,
    pub span_m: (f32, f32),
    pub group: (u32, u32),
    pub spread_m: f32,
    pub roams_m: f32,
    pub a_ha: f32,
    pub woods: f32,
    pub open: f32,
    pub water: f32,
    pub season_c: (f32, f32),
    pub air_c: f32,
    pub hours: Vec<(f32, f32)>,
    pub beat_hz: f32,
    pub bout_s: (f32, f32),
    pub speed: f32,
    pub over_m: (f32, f32),
    pub seen_m: f32,
    pub realms: Vec<String>,
}

/// The air, the clock and the rain now, as a breed asks them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Now {
    pub air_c: f32,
    pub hour: f32,
    pub rain_mm_h: f32,
}

impl ButterflyDef {
    fn lives_in(&self, realm: &str) -> bool {
        self.realms.iter().any(|one| one == realm)
    }

    /// How much of this breed a place holds, 0 to 1: by what the place is, where its
    /// growing season is as warm as the breed's (the drift's meaning of each, D379).
    pub fn suits(&self, round: &Round) -> f32 {
        if !(self.season_c.0..=self.season_c.1).contains(&round.season_c) {
            return 0.0;
        }
        (self.woods * round.woods)
            .max(self.open * round.open)
            .max(self.water * round.water)
    }

    /// How much of this breed is out now, 0 to 1: by the air's warmth and the hour, and
    /// none in rain.
    pub fn out(&self, now: &Now) -> f32 {
        if now.rain_mm_h > RAIN_MM_H {
            return 0.0;
        }
        warm_enough(self.air_c, now.air_c) * in_hours(&self.hours, now.hour)
    }
}

/// The breeds, and each one's look once its model is read.
#[derive(Resource)]
struct Kinds {
    defs: Vec<ButterflyDef>,
    looks: Vec<Option<look::Look>>,
    /// How many cells round the eye are kept: as far as the furthest seen breed.
    reach: i32,
}

/// The groups standing, by cell, round the cell the eye is in, and how many butterflies
/// they are.
#[derive(Resource, Default)]
struct Groups {
    middle: Option<IVec2>,
    cells: HashMap<IVec2, Option<(Entity, usize)>>,
    waiting: Vec<IVec2>,
    stand: usize,
}

impl Groups {
    /// How many of a group that wants so many may stand, under the most at once, counted.
    fn take(&mut self, wanted: usize) -> usize {
        let taken = wanted.min(MOST - self.stand.min(MOST));
        self.stand += taken;
        taken
    }
}

/// A group: its breed, how it flies and how long it has, the luck that says when in its
/// breed's hours it is out, how far out it is, 0 to 1 (not yet looked at below 0), and
/// how big its butterflies are drawn.
#[derive(Component)]
struct Group {
    kind: usize,
    flight: Flight,
    clock: f32,
    luck: f32,
    out: f32,
    scale: f32,
}

/// One butterfly of a group, and which way it heads, radians.
#[derive(Component)]
struct Flies(Member, f32);

pub fn plugin(app: &mut App) {
    let defs: Vec<ButterflyDef> = terra_world::data::read(FILE);
    let furthest = defs.iter().map(|def| def.seen_m).fold(0.0, f32::max);
    app.add_plugins(MaterialPlugin::<look::ButterflyMaterial>::default())
        .insert_resource(Kinds {
            looks: defs.iter().map(|_| None).collect(),
            reach: (furthest / CELL_M).ceil() as i32,
            defs,
        })
        .init_world_resource::<Groups>()
        .add_systems(Startup, load)
        .add_systems(
            Update,
            (stand, fly)
                .chain()
                .run_if(in_world)
                .run_if(resource_exists::<Planet>)
                .run_if(resource_exists::<Groups>),
        );
    terra_world::remote::method(app, "butterflies.state", state);
}

fn load(
    assets: Res<AssetServer>,
    mut kinds: ResMut<Kinds>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<look::ButterflyMaterial>>,
) {
    let kinds = &mut *kinds;
    for (def, slot) in kinds.defs.iter().zip(&mut kinds.looks) {
        let (beat, bout) = (def.beat_hz, def.bout_s);
        match look::look(&def.model, beat, bout, &assets, &mut meshes, &mut materials) {
            Ok(look) => *slot = Some(look),
            Err(problem) => warn!(
                "{} has no model yet, so none fly (data/butterflies.ron): {problem}",
                def.name
            ),
        }
    }
}

/// What the seed gives a cell: a group of a breed, so many, put so far across and along
/// the cell, with the luck its place and its hours are held against.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Plan {
    kind: usize,
    count: usize,
    at: Vec2,
    place_luck: f32,
    hours_luck: f32,
}

/// A cell's group, if the seed gives it one: among the breeds of this realm, each by its
/// share of the groups. The same for the same seed and cell, wherever it is asked.
fn plan(seed: u32, cell: IVec2, defs: &[ButterflyDef], realm: &str) -> Option<Plan> {
    let (c, r) = (cell.x as u32, cell.y as u32);
    let luck = |salt: u32| unit3(seed, c, r, salt);
    let total: f32 = defs
        .iter()
        .filter(|def| def.lives_in(realm))
        .map(|def| def.a_ha)
        .sum();
    if luck(0) >= (total * CELL_HA).min(1.0) {
        return None;
    }
    let mut pick = luck(1) * total;
    let mut last = None;
    for (kind, def) in (defs.iter().enumerate()).filter(|(_, def)| def.lives_in(realm)) {
        last = Some(kind);
        pick -= def.a_ha;
        if pick < 0.0 {
            break;
        }
    }
    let kind = last?;
    let (least, most) = defs[kind].group;
    Some(Plan {
        kind,
        count: (least as f32 + (most - least) as f32 * luck(2) + 0.5) as usize,
        at: Vec2::new(luck(3), luck(4)),
        place_luck: luck(5),
        hours_luck: luck(6),
    })
}

/// The seed butterflies are put by.
fn seed_of(planet: &Planet) -> u32 {
    planet.seed().derive("butterflies").0 as u32
}

/// Takes up the cells come within reach of the eye, a few a frame, and lets go the groups
/// gone past it.
fn stand(
    mut commands: Commands,
    mut groups: ResMut<Groups>,
    kinds: Res<Kinds>,
    (planet, start, clock): (Res<Planet>, Res<Start>, Res<Clock>),
    (climate, trunks, obstacles, spec): (
        Option<Res<ClimateFields>>,
        Option<Res<Trunks>>,
        Option<Res<Obstacles>>,
        Option<Res<WorldSpec>>,
    ),
    eye: Query<&GlobalTransform, With<MainCamera>>,
) {
    let (Some(eye), Some(trunks), Some(obstacles)) = (eye.iter().next(), trunks, obstacles) else {
        return;
    };
    let here = start.local(eye.translation());
    let middle = (here.xz() / CELL_M).floor().as_ivec2();
    let groups = &mut *groups;
    if groups.middle.replace(middle) != Some(middle) {
        let within = |cell: IVec2, cells: i32| (cell - middle).length_squared() <= cells * cells;
        let mut freed = 0;
        groups.cells.retain(|cell, group| {
            let keep = within(*cell, kinds.reach + 1);
            if let (false, Some((entity, count))) = (keep, group.as_ref()) {
                commands.entity(*entity).try_despawn();
                freed += count;
            }
            keep
        });
        groups.stand -= freed;
        groups.waiting.clear();
        for row in -kinds.reach..=kinds.reach {
            for column in -kinds.reach..=kinds.reach {
                let cell = middle + IVec2::new(column, row);
                if within(cell, kinds.reach) && !groups.cells.contains_key(&cell) {
                    groups.waiting.push(cell);
                }
            }
        }
        // The nearest last, to be taken first.
        groups
            .waiting
            .sort_by_key(|cell| std::cmp::Reverse((*cell - middle).length_squared()));
    }
    let seed = seed_of(&planet);
    // A run with no world named is the first realm's.
    let realm = spec.as_ref().map_or("world", |spec| spec.kind.as_str());
    for _ in 0..CELLS_A_FRAME {
        let Some(cell) = groups.waiting.pop() else {
            return;
        };
        let group = plan(seed, cell, &kinds.defs, realm).and_then(|plan| {
            let (def, look) = (&kinds.defs[plan.kind], kinds.looks[plan.kind].as_ref()?);
            let spot = (cell.as_vec2() + plan.at) * CELL_M;
            let ground = planet.ground_at(start.at(spot.x, spot.y));
            if planet.water_radius_m(ground) > ground.length() {
                return None;
            }
            let round = round_at(
                &planet,
                climate.as_deref(),
                &clock,
                &trunks,
                &obstacles,
                ground,
            );
            if plan.place_luck >= def.suits(&round) {
                return None;
            }
            let flight = flight_of(def, seed, cell);
            let frame = frame(&planet, &start, spot, flight.reach_m())?;
            let count = groups.take(plan.count);
            (count > 0).then(|| {
                let plan = Plan { count, ..plan };
                let entity = put(&mut commands, def, look, &plan, flight, frame, seed, cell);
                (entity, count)
            })
        });
        groups.cells.insert(cell, group);
    }
}

/// How a cell's group flies: its breed's flight, begun where the seed says.
fn flight_of(def: &ButterflyDef, seed: u32, cell: IVec2) -> Flight {
    let luck = |salt: u32| unit3(seed, cell.x as u32, cell.y as u32, salt);
    Flight {
        spread_m: def.spread_m,
        roams_m: def.roams_m,
        speed: def.speed,
        over_m: def.over_m,
        bout_s: def.bout_s,
        phase: (luck(7) * TAU, luck(8) * TAU),
    }
}

/// Where a group flies from: a frame lying on the ground at a spot so many metres east and
/// south of the start, tilted with the slope and lifted clear of the ground so far round
/// it. None where the ground is too steep to fly a level round over.
fn frame(planet: &Planet, start: &Start, spot: Vec2, reach_m: f32) -> Option<Transform> {
    let ground_at = |at: Vec2| planet.ground_at(start.at(at.x, at.y));
    let ground = ground_at(spot);
    let up = ground.normalize();
    let [east, west, south, north] =
        [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y].map(|way| ground_at(spot + way * reach_m));
    let mut slope = (south - north).cross(east - west).normalize_or(up);
    if slope.dot(up) < 0.0 {
        slope = -slope;
    }
    if slope.dot(up) < LEVEL {
        return None;
    }
    let clear = [east, west, south, north]
        .iter()
        .map(|at| (*at - ground).dot(slope))
        .fold(0.0, f32::max);
    let rotation = Quat::from_rotation_arc(start.rotation * Vec3::Y, slope) * start.rotation;
    Some(Transform::from_translation(ground + slope * clear).with_rotation(rotation))
}

/// Puts a group out: one entity, its butterflies its children.
fn put(
    commands: &mut Commands,
    def: &ButterflyDef,
    look: &look::Look,
    plan: &Plan,
    flight: Flight,
    frame: Transform,
    seed: u32,
    cell: IVec2,
) -> Entity {
    let (c, r) = (cell.x as u32, cell.y as u32);
    let span = def.span_m.0 + (def.span_m.1 - def.span_m.0) * unit3(seed, c, r, 9);
    let seen = VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: def.seen_m * 0.8..def.seen_m,
        use_aabb: false,
    };
    let group = Group {
        kind: plan.kind,
        flight,
        clock: 0.0,
        luck: plan.hours_luck,
        out: -1.0,
        scale: span / look.span_m,
    };
    commands
        .spawn((
            frame,
            Visibility::Hidden,
            Name::new(format!("Group of {}", def.name)),
            group,
        ))
        .with_children(|group| {
            for member in flight::members(seed ^ (c << 16) ^ r, plan.count) {
                group.spawn((
                    Mesh3d(look.mesh.clone()),
                    MeshMaterial3d(look.material.clone()),
                    Transform::from_scale(Vec3::ZERO),
                    MeshTag::new(member.tag),
                    Flies(member, 0.0),
                    seen.clone(),
                    NotShadowCaster,
                ));
            }
        })
        .id()
}

/// The air, the hour and the rain now: the weather at the eye, or a mild dry day where a
/// run has none.
fn now(clock: &Clock, weather: Option<&Weather>) -> Now {
    Now {
        air_c: weather.map_or(15.0, |weather| weather.temperature_c),
        hour: clock.hour,
        rain_mm_h: weather.map_or(0.0, Weather::rain_mm_h),
    }
}

/// Brings each group out or in as its breed's hours and the air say, and flies those out:
/// every butterfly about its group's middle, turned the way it goes.
fn fly(
    time: Res<Time>,
    kinds: Res<Kinds>,
    clock: Res<Clock>,
    weather: Option<Res<Weather>>,
    mut groups: Query<(&mut Group, &mut Visibility, &Children)>,
    mut butterflies: Query<(&mut Flies, &mut Transform)>,
) {
    let dt = time.delta_secs();
    // The shader's clock, which the lift of a bout is in step with.
    let beat_clock = time.elapsed_secs_wrapped();
    let now = now(&clock, weather.as_deref());
    let out: Vec<f32> = kinds.defs.iter().map(|def| def.out(&now)).collect();
    for (mut group, mut visibility, children) in &mut groups {
        let wanted = out[group.kind] > group.luck;
        let fresh = group.out < 0.0;
        group.out = if fresh {
            f32::from(u8::from(wanted))
        } else {
            (group.out + if wanted { dt } else { -dt } / OUT_S).clamp(0.0, 1.0)
        };
        let shown = if group.out > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != shown {
            *visibility = shown;
        }
        if group.out <= 0.0 {
            continue;
        }
        group.clock += dt;
        let flight = group.flight;
        let middle = flight.middle(group.clock);
        let scale = Vec3::splat(group.scale * group.out * group.out * (3.0 - 2.0 * group.out));
        let beats = flight::beats(flight.bout_s);
        for child in children.iter() {
            let Ok((mut flies, mut transform)) = butterflies.get_mut(child) else {
                continue;
            };
            let through = flight::bout(flight.bout_s, flies.0.tag, beat_clock);
            let lift = flight::lift(through, beats);
            let (place, heading) = flight::place(&flight, &flies.0, group.clock, lift);
            // The model's head is to +Z (D303).
            let toward = libm::atan2f(heading.x, heading.y);
            flies.1 = if fresh {
                toward
            } else {
                flight::turned(flies.1, toward, TURNS * dt)
            };
            transform.translation = place + Vec3::new(middle.x, 0.0, middle.y);
            transform.rotation = Quat::from_rotation_y(flies.1);
            transform.scale = scale;
        }
    }
}

/// `butterflies.state`: how many groups and butterflies stand and how many of them are
/// out, of which breeds, how much of each breed the air and the hour bring out, and the
/// nearest group out to the eye (its breed, how many, how far, where it and each of its
/// butterflies are from the start), for a scripted run to go and look.
fn state(
    In(_): In<Option<serde_json::Value>>,
    kinds: Res<Kinds>,
    (start, kept, clock, weather): (
        Option<Res<Start>>,
        Option<Res<Groups>>,
        Option<Res<Clock>>,
        Option<Res<Weather>>,
    ),
    groups: Query<(&Group, &GlobalTransform, &Children)>,
    butterflies: Query<&GlobalTransform, With<Flies>>,
    eye: Query<&GlobalTransform, With<MainCamera>>,
) -> bevy::remote::BrpResult {
    let eye = eye.iter().next().map(GlobalTransform::translation);
    let mut by_kind = serde_json::Map::new();
    let (mut standing, mut flying, mut groups_out) = (0, 0, 0);
    let mut nearest: Option<(f32, &Group, Vec3, &Children)> = None;
    for (group, at, children) in &groups {
        standing += children.len();
        if group.out <= 0.5 {
            continue;
        }
        groups_out += 1;
        flying += children.len();
        let name = kinds.defs[group.kind].name.clone();
        let count = by_kind.get(&name).and_then(serde_json::Value::as_u64);
        by_kind.insert(name, (count.unwrap_or(0) + children.len() as u64).into());
        let middle = group.flight.middle(group.clock);
        let at = at.transform_point(Vec3::new(middle.x, 0.0, middle.y));
        let away = eye.map_or(0.0, |eye| eye.distance(at));
        if nearest.as_ref().is_none_or(|(least, ..)| away < *least) {
            nearest = Some((away, group, at, children));
        }
    }
    let nearest = nearest.map(|(away, group, at, children)| {
        let from_start = |at: Vec3| start.as_ref().map(|start| start.local(at).to_array());
        let each: Vec<_> = (children.iter())
            .filter_map(|child| from_start(butterflies.get(child).ok()?.translation()))
            .collect();
        serde_json::json!({
            "kind": kinds.defs[group.kind].name,
            "count": children.len(),
            "away_m": away,
            "from_start": from_start(at),
            "each_from_start": each,
            "spread_m": group.flight.spread_m,
            "flown_s": group.clock,
        })
    });
    let out: serde_json::Map<String, serde_json::Value> = clock
        .as_ref()
        .map(|clock| now(clock, weather.as_deref()))
        .map(|now| {
            (kinds.defs.iter())
                .map(|def| (def.name.clone(), def.out(&now).into()))
                .collect()
        })
        .unwrap_or_default();
    Ok(serde_json::json!({
        "models": kinds.looks.iter().flatten().count(),
        "cells": kept.as_ref().map(|kept| kept.cells.len()),
        "waiting": kept.as_ref().map(|kept| kept.waiting.len()),
        "groups": groups.iter().count(),
        "standing": standing,
        "groups_out": groups_out,
        "flying": flying,
        "kinds": by_kind,
        "out": out,
        "nearest": nearest,
    }))
}

#[cfg(test)]
mod tests;
