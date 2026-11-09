//! Fish in the water (docs/ambient-life.md item 3, D647): for the eye only. Each kind is
//! a row of `assets/data/fish.ron`: a model, in schools put by rule where the water suits
//! it (how deep, how many to a hectare), near whoever looks, each school swimming a path
//! of its own ([`school`]) and darting from a wader. A new fish is a model and a row
//! (D136). They are each game's own, as the birds are: nothing of them crosses the
//! network.
//!
//! How many (D127, D182): three kinds; a school is one entity and its fish its children,
//! instances of one mesh in one material ([`look`]); at most [`MOST_FISH`] swim at once,
//! and a kind is not drawn past its row's `seen_m`.

mod look;
pub mod school;

use bevy::camera::visibility::VisibilityRange;
use bevy::light::NotShadowCaster;
use bevy::mesh::MeshTag;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use school::{Member, Path};
use serde::Deserialize;
use terra_gen::hash::unit3;
use terra_world::instance::{InstanceApp, in_world};
use terra_world::planet::{MainCamera, Planet};
use terra_world::start::Start;

const FILE: &str = "fish.ron";
/// A cell holds at most one school, metres across.
const CELL_M: f32 = 12.0;
const CELL_HA: f32 = CELL_M * CELL_M / 10_000.0;
/// No more fish than this swim at once, whatever the water.
pub const MOST_FISH: usize = 400;
/// How many cells are looked at a frame: each asks the land how deep its water is.
const CELLS_A_FRAME: usize = 6;
/// A wader this near a school's water makes it dart, so much faster, for so long.
const STARTLE_M: f32 = 3.0;
const DARTS: f32 = 4.0;
const DART_S: f32 = 1.5;
/// A fish keeps this far off the bed.
const OFF_THE_BED_M: f32 = 0.12;

/// A kind of fish, as its row says (the file's header says each field).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FishDef {
    pub name: String,
    pub model: String,
    pub long_m: (f32, f32),
    pub school: (u32, u32),
    pub deep_m: (f32, f32),
    pub under_m: (f32, f32),
    pub a_ha: f32,
    pub seen_m: f32,
    pub speed: f32,
    pub keeps: f32,
    pub path_m: (f32, f32),
    pub stroke_hz: f32,
    pub bend: f32,
}

/// The kinds, and each one's look once its model is read.
#[derive(Resource)]
struct Kinds {
    defs: Vec<FishDef>,
    looks: Vec<Option<look::Look>>,
    /// How many cells round the eye are kept: as far as the furthest seen kind.
    reach: i32,
}

/// The schools standing, by cell, round the cell the eye is in.
#[derive(Resource, Default)]
struct Schools {
    middle: Option<IVec2>,
    cells: HashMap<IVec2, Option<(Entity, usize)>>,
    waiting: Vec<IVec2>,
    fish: usize,
}

/// A school: its kind, its path and where on it it is, and how long it darts yet.
#[derive(Component)]
struct School {
    kind: usize,
    path: Path,
    clock: f32,
    darts_s: f32,
    reach_m: f32,
}

/// One fish of a school.
#[derive(Component)]
struct Swims(Member);

pub fn plugin(app: &mut App) {
    let defs: Vec<FishDef> = terra_world::data::read(FILE);
    let furthest = defs.iter().map(|def| def.seen_m).fold(0.0, f32::max);
    app.add_plugins(MaterialPlugin::<look::FishMaterial>::default())
        .insert_resource(Kinds {
            looks: defs.iter().map(|_| None).collect(),
            reach: (furthest / CELL_M).ceil() as i32,
            defs,
        })
        .init_world_resource::<Schools>()
        .add_systems(Startup, load)
        .add_systems(
            Update,
            (stand, swim)
                .chain()
                .run_if(in_world)
                .run_if(resource_exists::<Planet>)
                .run_if(resource_exists::<Schools>),
        );
    terra_world::remote::method(app, "fish.state", state);
}

fn load(
    assets: Res<AssetServer>,
    mut kinds: ResMut<Kinds>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<look::FishMaterial>>,
) {
    let kinds = &mut *kinds;
    for (def, slot) in kinds.defs.iter().zip(&mut kinds.looks) {
        let swims = (def.stroke_hz, def.bend);
        match look::look(&def.model, swims, &assets, &mut meshes, &mut materials) {
            Ok(look) => *slot = Some(look),
            Err(problem) => error!(
                "{}'s model cannot be read, so none swim (data/fish.ron): {problem}",
                def.name
            ),
        }
    }
}

/// Which kind's school a cell holds, if any, where its water is so deep: by the seed, among
/// the kinds that keep to water that deep, each by its share of the schools there.
fn kind_at(seed: u32, cell: IVec2, defs: &[FishDef], depth_m: f32) -> Option<usize> {
    let (c, r) = (cell.x as u32, cell.y as u32);
    let suits = |def: &FishDef| depth_m >= def.deep_m.0 && depth_m <= def.deep_m.1;
    let total: f32 = defs
        .iter()
        .filter(|def| suits(def))
        .map(|def| def.a_ha)
        .sum();
    if unit3(seed, c, r, 0) >= (total * CELL_HA).min(1.0) {
        return None;
    }
    let mut pick = unit3(seed, c, r, 3) * total;
    let mut last = None;
    for (kind, def) in defs.iter().enumerate().filter(|(_, def)| suits(def)) {
        last = Some(kind);
        pick -= def.a_ha;
        if pick < 0.0 {
            break;
        }
    }
    last
}

/// Between a least and a most, by a share.
fn between((least, most): (f32, f32), share: f32) -> f32 {
    least + (most - least) * share
}

/// Takes up the cells come within reach of the eye, a few a frame, and lets go the schools
/// gone past it.
fn stand(
    mut commands: Commands,
    mut schools: ResMut<Schools>,
    kinds: Res<Kinds>,
    (planet, start): (Res<Planet>, Res<Start>),
    eye: Query<&GlobalTransform, With<MainCamera>>,
) {
    let Some(eye) = eye.iter().next() else {
        return;
    };
    let here = start.local(eye.translation());
    let middle = (here.xz() / CELL_M).floor().as_ivec2();
    let schools = &mut *schools;
    if schools.middle.replace(middle) != Some(middle) {
        let within = |cell: IVec2, cells: i32| (cell - middle).length_squared() <= cells * cells;
        let mut freed = 0;
        schools.cells.retain(|cell, school| {
            let keep = within(*cell, kinds.reach + 1);
            if let (false, Some((entity, fish))) = (keep, school.as_ref()) {
                commands.entity(*entity).try_despawn();
                freed += fish;
            }
            keep
        });
        schools.fish -= freed;
        schools.waiting.clear();
        for row in -kinds.reach..=kinds.reach {
            for column in -kinds.reach..=kinds.reach {
                let cell = middle + IVec2::new(column, row);
                if within(cell, kinds.reach) && !schools.cells.contains_key(&cell) {
                    schools.waiting.push(cell);
                }
            }
        }
        // The nearest last, to be taken first.
        schools
            .waiting
            .sort_by_key(|cell| std::cmp::Reverse((*cell - middle).length_squared()));
    }
    let seed = planet.seed().derive("fish").0 as u32;
    for _ in 0..CELLS_A_FRAME {
        let Some(cell) = schools.waiting.pop() else {
            return;
        };
        let room = MOST_FISH - schools.fish.min(MOST_FISH);
        let school = put(&mut commands, &kinds, &planet, &start, seed, cell, room);
        schools.fish += school.map_or(0, |(_, fish)| fish);
        schools.cells.insert(cell, school);
    }
}

/// Puts a cell's school in its water, if the seed gives it one and the water suits: the
/// school and how many fish it is.
fn put(
    commands: &mut Commands,
    kinds: &Kinds,
    planet: &Planet,
    start: &Start,
    seed: u32,
    cell: IVec2,
    room: usize,
) -> Option<(Entity, usize)> {
    let (c, r) = (cell.x as u32, cell.y as u32);
    let luck = |salt: u32| unit3(seed, c, r, salt);
    let spot = (cell.as_vec2() + Vec2::new(luck(1), luck(2))) * CELL_M;
    let depth_at = |at: Vec2| {
        let ground = planet.ground_at(start.at(at.x, at.y));
        planet.water_radius_m(ground) - ground.length()
    };
    let depth = depth_at(spot);
    let kind = kind_at(seed, cell, &kinds.defs, depth)?;
    let (def, look) = (&kinds.defs[kind], kinds.looks[kind].as_ref()?);
    let count =
        (between((def.school.0 as f32, def.school.1 as f32 + 0.99), luck(4)) as usize).min(room);
    if count == 0 {
        return None;
    }
    let long = between(def.long_m, luck(5));
    let keeps_m = def.keeps * long;
    let under = between(def.under_m, luck(6)).min(depth - OFF_THE_BED_M);
    // Its path as wide as its water allows: deep enough under all of it, or half or a
    // quarter as wide (a steep shore's shallows are a narrow band), or it is not there.
    let wide = between(def.path_m, luck(7));
    let path = [wide, wide * 0.5, wide * 0.25]
        .into_iter()
        .find_map(|radius_m| {
            let path = Path {
                radius_m,
                speed: def.speed,
                phase: luck(8) * std::f32::consts::TAU,
            };
            let reach = school::reach_m(&path, count, keeps_m);
            let deep_enough = [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y]
                .iter()
                .all(|way| depth_at(spot + *way * reach) >= under + OFF_THE_BED_M);
            deep_enough.then_some(path)
        })?;
    let ground = planet.ground_at(start.at(spot.x, spot.y));
    let up = ground.normalize();
    // Level with the water, in the start's frame turned onto this place.
    let rotation = Quat::from_rotation_arc(start.rotation * Vec3::Y, up) * start.rotation;
    let translation = up * (planet.water_radius_m(ground) - under);
    let members = school::members(seed ^ (c << 16) ^ r, count, keeps_m, def.speed);
    let school = School {
        kind,
        path,
        clock: 0.0,
        darts_s: 0.0,
        reach_m: school::reach_m(&path, count, keeps_m),
    };
    let scale = Vec3::splat(long / look.long_m);
    let seen = VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: def.seen_m * 0.8..def.seen_m,
        use_aabb: false,
    };
    let entity = commands
        .spawn((
            Transform::from_translation(translation).with_rotation(rotation),
            Visibility::default(),
            Name::new(format!("School of {}", def.name)),
            school,
        ))
        .with_children(|fish| {
            for member in members {
                fish.spawn((
                    Mesh3d(look.mesh.clone()),
                    MeshMaterial3d(look.material.clone()),
                    Transform::from_scale(scale),
                    MeshTag::new(member.stroke),
                    Swims(member),
                    seen.clone(),
                    NotShadowCaster,
                ));
            }
        })
        .id();
    Some((entity, count))
}

/// Swims every school along its path, its fish with it, and darts those a wader comes on.
fn swim(
    time: Res<Time>,
    mut schools: Query<(&mut School, &GlobalTransform, &Children)>,
    mut fish: Query<(&Swims, &mut Transform)>,
    waders: Query<&GlobalTransform, With<terra_walker::walker::Player>>,
) {
    let dt = time.delta_secs();
    for (mut school, at, children) in &mut schools {
        let near = STARTLE_M + school.reach_m;
        if (waders.iter()).any(|wader| wader.translation().distance(at.translation()) < near) {
            school.darts_s = DART_S;
        }
        let hurry = if school.darts_s > 0.0 { DARTS } else { 1.0 };
        school.darts_s = (school.darts_s - dt).max(0.0);
        school.clock += dt * hurry;
        for child in children.iter() {
            let Ok((swims, mut transform)) = fish.get_mut(child) else {
                continue;
            };
            let (place, heading) = school::place(&school.path, &swims.0, school.clock);
            transform.translation = place;
            // The model faces +Z (D303).
            transform.rotation = Quat::from_rotation_y(libm::atan2f(heading.x, heading.y));
        }
    }
}

/// `fish.state`: how many schools and fish swim, of which kinds, and the nearest school to
/// the eye (its kind, how far, where from the start, how long it has swum and whether it
/// darts), for a scripted run to go and look.
fn state(
    In(_): In<Option<serde_json::Value>>,
    kinds: Res<Kinds>,
    (start, planet, kept): (
        Option<Res<Start>>,
        Option<Res<Planet>>,
        Option<Res<Schools>>,
    ),
    schools: Query<(&School, &GlobalTransform, &Children)>,
    eye: Query<&GlobalTransform, With<MainCamera>>,
) -> bevy::remote::BrpResult {
    let eye = eye.iter().next().map(GlobalTransform::translation);
    let mut by_kind = serde_json::Map::new();
    let mut fish = 0;
    let mut nearest: Option<(f32, &School, Vec3)> = None;
    for (school, at, children) in &schools {
        fish += children.len();
        let name = kinds.defs[school.kind].name.clone();
        let count = by_kind.get(&name).and_then(serde_json::Value::as_u64);
        by_kind.insert(name, (count.unwrap_or(0) + children.len() as u64).into());
        let away = eye.map_or(0.0, |eye| eye.distance(at.translation()));
        if nearest.as_ref().is_none_or(|(least, ..)| away < *least) {
            nearest = Some((away, school, at.translation()));
        }
    }
    let nearest = nearest.map(|(away, school, at)| {
        let from_start = start.as_ref().map(|start| start.local(at).to_array());
        serde_json::json!({
            "kind": kinds.defs[school.kind].name,
            "away_m": away,
            "from_start": from_start,
            "darting": school.darts_s > 0.0,
            "swum_s": school.clock,
        })
    });
    // How deep the water under the eye is, and how much of the water round it was looked at:
    // what says why no fish swim where some were expected.
    let depth_here = (eye.zip(planet.as_ref())).map(|(eye, planet)| {
        let ground = planet.ground_at(eye);
        planet.water_radius_m(ground) - ground.length()
    });
    Ok(serde_json::json!({
        "models": kinds.looks.iter().flatten().count(),
        "cells": kept.as_ref().map(|kept| kept.cells.len()),
        "waiting": kept.as_ref().map(|kept| kept.waiting.len()),
        "depth_here_m": depth_here,
        "schools": schools.iter().count(),
        "fish": fish,
        "kinds": by_kind,
        "nearest": nearest,
    }))
}

#[cfg(test)]
mod tests;
