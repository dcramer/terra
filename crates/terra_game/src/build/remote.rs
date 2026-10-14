//! Building over the remote protocol (D69; tools/sandbox): a script lays plans, builds,
//! takes apart and undoes, and reads the base, as this
//! machine's player, through the same operations and checks the hammer uses
//! (`build::act::Building`). Each answers at once with what was done, or why it was
//! refused in the words the player is shown, and is logged as `Build: …`.
//!
//! Places on the base are counted in its own cubes: east, up and south from its claim
//! stone, half a metre each; a tile is a column of a storey, its `y` the storey.
//!
//! - `build.state`: the base: its parts and how far each is built and how many of their
//!   cubes are cut to a shape, its roofs, fitted props, planned cubes, rooms, the claim's
//!   offcuts, where it stands, and where the player's feet are in it.
//! - `build.lay {what, min, max, kind?, floor?, style?, pitch?, along_x?}`: lays a plan over
//!   tiles: a `room` (the shortcut: floor, four walls and a flat roof), a `wall` from `min`
//!   to `max`, a `floor`, a `flat_roof`, or a pitched `roof` (`Hip`, `Gable`, `LeanTo`);
//!   or from the cube `min`, as the hammer places them (D193): a `flight` of stairs (a ramp
//!   with `ramp: true`) up a storey toward `facing`, `wide` metres across, a `post` a
//!   storey high, or one `block` of a `shape`.
//! - `build.opening {tiles, door?}`: cuts a doorway (or, with `door: false`, a window) into a
//!   wall's plan along those tiles.
//! - `build.fill {part?, steps?}`: builds what goes in next, as holding the hammer does, so
//!   many times (one if not given), stopping at the first refusal.
//! - `build.take_apart {at, steps?}`: takes out what came last of the part a cube is in.
//! - `build.undo`: undoes the player's last change to the base.
//! - `build.op {ops}`: any operations, as `terra_build::ops::Op` serialises them, as one act.
//! - `build.point {at}`: where a place in the base, in metres, is in the world and from the
//!   start, to look at it or walk to it.
//! - `build.material {kind}`: the material in the player's hammer, as Ctrl and the wheel
//!   pick it (D232); `build.lay` lays in it when no `kind` is given.
//! - `build.replace {at, brush?, kind?}`: changes the cubes of a brush's box round `at` (a
//!   brush 1, 2, 4 or 8 cubes across, D226) to the material in hand, or `kind`; `build.fill`
//!   then takes the old out, swaps and builds the new.
//! - `build.carve {at, brush?}`: marks the cubes of a brush's box to be carved out.
//! - `build.take_out {at, brush?}`: takes a step out of the highest layer of what is in a
//!   brush's box, as the hammer takes out the cube aimed at.
//! - `build.cube {at}`: a cube: its material, shape, fill, how far in, and what is pending.
//! - `build.ground {columns}`: the ground's height under the middle of each column, in metres
//!   in the base's own space, as the planet has it now: to see what building changed of it.

use super::act::{BuildRules, Building, Offcuts};
use super::{Site, plans};
use crate::play::Builder;
use crate::play::{LocalPlayer, Player};
use crate::remote::{method, refused};
use crate::start::Start;
use bevy::{
    ecs::system::SystemState,
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
};
use serde::Deserialize;
use serde_json::{Value, json};
use terra_build::ops::{Done, Op, Refusal};
use terra_build::{CELL_M, Facing, Fill, Panel, RoofSection, RoofStyle, STOREY_M, Shape};

pub fn plugin(app: &mut App) {
    method(app, "build.state", state);
    method(app, "build.lay", lay);
    method(app, "build.opening", opening);
    method(app, "build.fill", fill);
    method(app, "build.take_apart", take_apart);
    method(app, "build.undo", undo);
    method(app, "build.op", ops);
    method(app, "build.point", point);
    method(app, "build.ground", ground);
    method(app, "build.material", material);
    method(app, "build.replace", replace);
    method(app, "build.carve", carve);
    method(app, "build.take_out", take_out);
    method(app, "build.cube", cube);
}

#[derive(Deserialize)]
struct Columns {
    columns: Vec<[i32; 2]>,
}

/// The ground's height under columns of the base, in its own space.
fn ground(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Columns { columns } = parse_some(params)?;
    let frame = world.resource::<Site>().frame;
    let planet = world.resource::<crate::planet::Planet>();
    let heights: Vec<f32> = columns
        .iter()
        .map(|&[x, z]| {
            let middle = Vec3::new((x as f32 + 0.5) * CELL_M, 0.0, (z as f32 + 0.5) * CELL_M);
            -planet.distance(frame.transform_point(middle))
        })
        .collect();
    Ok(json!(heights))
}

#[derive(Deserialize)]
struct Point {
    at: [f32; 3],
}

/// Where a place in the base's own space, in metres, is in the world, and from the start.
fn point(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Point { at } = parse_some(params)?;
    let start = *world.resource::<Start>();
    let frame = world.resource::<Site>().frame;
    let placed = frame.transform_point(Vec3::from_array(at));
    Ok(json!({
        "world": placed.to_array(),
        "from_start": start.local(placed).to_array(),
    }))
}

type Acting<'w, 's> = (
    Building<'w, 's>,
    Query<'w, 's, Entity, (With<Player>, With<LocalPlayer>)>,
);

/// Runs something as this machine's player, with what acting on the base needs.
fn as_player(world: &mut World, run: impl FnOnce(&mut Building, Entity) -> BrpResult) -> BrpResult {
    let mut state = SystemState::<Acting>::new(world);
    let result = {
        let (mut building, local) = state
            .get_mut(world)
            .map_err(|problem| refused(format!("cannot build now: {problem}")))?;
        let player = local
            .single()
            .map_err(|_| refused("there is no player to build as"))?;
        run(&mut building, player)
    };
    state.apply(world);
    result
}

/// An act's answer: what it took and gave back, or why it was refused.
fn answer(what: &str, result: Result<Done, Refusal>) -> Value {
    match result {
        Ok(done) => json!({
            "done": true,
            "what": what,
            "took": done.took.to_string(),
            "gave": done.gave.to_string(),
        }),
        Err(why) => json!({
            "done": false,
            "what": what,
            "refused": why.message(),
        }),
    }
}

/// An answer with the cubes its operations named, so a script knows where it built.
fn with_cubes(mut answer: Value, ops: &[Op]) -> Value {
    let cubes: Vec<[i32; 3]> = ops
        .iter()
        .flat_map(|op| match op {
            Op::Fill { cubes } | Op::Empty { cubes } | Op::Open { cubes } => cubes.clone(),
            _ => Vec::new(),
        })
        .collect();
    answer["cubes"] = json!(cubes);
    answer
}

#[derive(Deserialize)]
struct Lay {
    what: String,
    min: [i32; 3],
    max: [i32; 3],
    kind: Option<String>,
    floor: Option<String>,
    style: Option<RoofStyle>,
    pitch: Option<f32>,
    along_x: Option<bool>,
    /// For a flight or a cut block: which way it rises (D193).
    facing: Option<Facing>,
    /// For a flight: how many metres wide, and whether it is a ramp.
    wide: Option<f32>,
    ramp: Option<bool>,
    /// For a block: its shape.
    shape: Option<Shape>,
    /// For a block: how many cubes across a box of them is (D226).
    brush: Option<i32>,
}

fn lay(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let lay: Lay = parse_some(params)?;
    let rules = world.resource::<BuildRules>().clone();
    let held = material_of(world);
    let kind_of = |name: &Option<String>| match name {
        Some(name) => rules
            .kind(name)
            .ok_or_else(|| refused(format!("there is no kind of construction called {name}"))),
        None => Ok(held),
    };
    let kind = kind_of(&lay.kind)?;
    let floor = match &lay.floor {
        Some(_) => kind_of(&lay.floor)?,
        None => kind,
    };
    let (min, max) = (IVec3::from_array(lay.min), IVec3::from_array(lay.max));
    let (low, high) = (min.min(max), min.max(max));
    as_player(world, |building, player| {
        let base = &building.site.base;
        let ground = |column: [i32; 2]| building.ground_cm(column);
        let blocks = building.rules.blocks.clone();
        let footings = building.rules.footing.map(|kind| plans::Footings {
            kind,
            ground_cm: &ground,
            blocks: &blocks,
        });
        let (ops, room) = match lay.what.as_str() {
            "room" => (
                plans::room(base, low, high, kind, floor, footings),
                Some((low, high)),
            ),
            "wall" => {
                let tiles = plans::wall_tiles(min.xz(), max.xz(), min.y);
                (plans::wall(base, &tiles, kind, footings), None)
            }
            "floor" => (plans::floor(base, low, high, kind, footings), None),
            "flat_roof" => (plans::flat_roof(base, low, high, kind), None),
            "roof" => {
                let section = RoofSection {
                    min: low.to_array(),
                    max: high.to_array(),
                    style: lay.style.unwrap_or(RoofStyle::Gable),
                    along_x: lay.along_x.unwrap_or(high.x - low.x >= high.z - low.z),
                    pitch: lay.pitch.unwrap_or(0.6),
                };
                (plans::roof(base, section, kind), None)
            }
            // From the foot cube `min`, up a storey toward `facing`.
            "flight" => {
                let facing = lay.facing.unwrap_or_default();
                let top = if lay.ramp.unwrap_or(false) {
                    Shape::Slope(facing)
                } else {
                    Shape::Stair(facing)
                };
                let wide = terra_build::cells(lay.wide.unwrap_or(1.0));
                (
                    plans::flight(base, min, facing, wide, top, kind, footings),
                    None,
                )
            }
            "post" => (plans::post(base, min, kind, footings), None),
            "block" => (
                plans::blocks(
                    base,
                    &plans::brush(min, lay.brush.unwrap_or(1)),
                    lay.shape.unwrap_or_default(),
                    kind,
                ),
                None,
            ),
            other => return Err(refused(format!("cannot lay a {other}"))),
        };
        if ops.is_empty() {
            return Err(refused(
                "there is nothing to lay there that is not laid already",
            ));
        }
        let what = format!("lay a {}", lay.what.replace('_', " "));
        let result = building.lay(Some(player), &what, ops, room);
        Ok(answer(&what, result))
    })
}

#[derive(Deserialize)]
struct Opening {
    tiles: Vec<[i32; 3]>,
    #[serde(default = "yes")]
    door: bool,
}

fn yes() -> bool {
    true
}

fn opening(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let opening: Opening = parse_some(params)?;
    as_player(world, |building, player| {
        let tiles: Vec<IVec3> = opening
            .tiles
            .iter()
            .map(|&t| IVec3::from_array(t))
            .collect();
        let panel = if opening.door {
            Panel::Door
        } else {
            Panel::WINDOW
        };
        let cubes = plans::opening_cubes(&tiles, panel);
        let ops = plans::opening(&building.site.base, &cubes);
        if ops.is_empty() {
            return Err(refused("it is open already"));
        }
        let what = if opening.door {
            "cut a doorway"
        } else {
            "cut a window"
        };
        let result = building.act(Some(player), what, &ops);
        Ok(answer(what, result))
    })
}

#[derive(Deserialize)]
struct FillAsked {
    part: Option<u32>,
    steps: Option<u32>,
}

fn fill(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let fill: FillAsked = parse_some(params.or(Some(json!({}))))?;
    as_player(world, |building, player| {
        let near = player_near(building, player);
        let mut done = Vec::new();
        for _ in 0..fill.steps.unwrap_or(1) {
            let Some(work) =
                building.next_work(Some(player), fill.part.map(terra_build::PartId), near)
            else {
                break;
            };
            if let Err(why) = &work.check {
                done.push(answer(&work.what, Err(why.clone())));
                info!("Build: {}: refused: {}", work.what, why.message());
                break;
            }
            let result = building.act(Some(player), &work.what, &work.ops);
            let refused = result.is_err();
            done.push(with_cubes(answer(&work.what, result), &work.ops));
            if refused {
                break;
            }
        }
        Ok(Value::Array(done))
    })
}

/// Where the player stands, in the base's own space.
fn player_near(building: &Building, player: Entity) -> Vec3 {
    building.local(building.player_at(player).unwrap_or_default())
}

#[derive(Deserialize)]
struct TakeApart {
    at: [i32; 3],
    steps: Option<u32>,
}

fn take_apart(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let take: TakeApart = parse_some(params)?;
    as_player(world, |building, player| {
        let near = player_near(building, player);
        let mut done = Vec::new();
        for _ in 0..take.steps.unwrap_or(1) {
            let Some(work) = building.last_work(Some(player), IVec3::from_array(take.at), near)
            else {
                break;
            };
            let result = building.act(Some(player), &work.what, &work.ops);
            let refused = result.is_err();
            done.push(with_cubes(answer(&work.what, result), &work.ops));
            if refused {
                break;
            }
        }
        Ok(Value::Array(done))
    })
}

fn undo(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    as_player(world, |building, player| {
        Ok(match building.undo(player) {
            Ok(what) => json!({ "done": true, "what": what }),
            Err(why) => json!({ "done": false, "refused": why }),
        })
    })
}

#[derive(Deserialize)]
struct Ops {
    ops: Vec<Op>,
}

fn ops(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Ops { ops } = parse_some(params)?;
    as_player(world, |building, player| {
        let result = building.act(Some(player), "operations", &ops);
        Ok(answer("operations", result))
    })
}

/// The material in this machine's player's hammer (D232): the one picked, else the first
/// kit's.
fn material_of(world: &mut World) -> terra_build::Kind {
    let laying = world.resource::<super::BuildLook>().laying;
    world
        .query_filtered::<&Builder, (With<Player>, With<LocalPlayer>)>()
        .iter(world)
        .next()
        .and_then(|builder| builder.material)
        .unwrap_or(laying)
}

#[derive(Deserialize)]
struct Material {
    kind: String,
}

/// Puts a material in the player's hammer, as Ctrl and the wheel do.
fn material(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Material { kind } = parse_some(params)?;
    let rules = world.resource::<BuildRules>().clone();
    let picked = rules
        .kind(&kind)
        .filter(|kind| rules.names.contains_key(kind))
        .ok_or_else(|| refused(format!("there is no material called {kind}")))?;
    for mut builder in world
        .query_filtered::<&mut Builder, (With<Player>, With<LocalPlayer>)>()
        .iter_mut(world)
    {
        builder.material = Some(picked);
    }
    info!("Build: material: {}", rules.name(picked));
    Ok(json!({ "material": rules.name(picked) }))
}

#[derive(Deserialize)]
struct Brushed {
    at: [i32; 3],
    brush: Option<i32>,
    kind: Option<String>,
}

/// Changes the built cubes of a brush's box round a cube to the material in hand, as the
/// hammer's Change material does (D232); `build.fill` then builds it.
fn replace(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Brushed = parse_some(params)?;
    let rules = world.resource::<BuildRules>().clone();
    let kind = match &asked.kind {
        Some(name) => rules
            .kind(name)
            .ok_or_else(|| refused(format!("there is no material called {name}")))?,
        None => material_of(world),
    };
    as_player(world, |building, player| {
        let cubes = plans::brush(IVec3::from_array(asked.at), asked.brush.unwrap_or(1));
        let ops = plans::replace(&building.site.base, &cubes, kind);
        if ops.is_empty() {
            return Err(refused("there is nothing there to change"));
        }
        let what = format!("change to {}", rules.name(kind));
        let result = building.act(Some(player), &what, &ops);
        Ok(answer(&what, result))
    })
}

/// Marks the built cubes of a brush's box round a cube to be carved out (D226).
fn carve(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Brushed = parse_some(params)?;
    as_player(world, |building, player| {
        let cubes = plans::brush(IVec3::from_array(asked.at), asked.brush.unwrap_or(1));
        let ops = plans::carve(&building.site.base, &cubes);
        if ops.is_empty() {
            return Err(refused("there is nothing there to carve"));
        }
        let result = building.act(Some(player), "carve", &ops);
        Ok(answer("carve", result))
    })
}

/// Takes a step out of what is in a brush's box round a cube, its highest layer, as the
/// hammer's take-apart does where nothing rests on it (D226).
fn take_out(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Brushed = parse_some(params)?;
    as_player(world, |building, player| {
        let cubes = plans::brush(IVec3::from_array(asked.at), asked.brush.unwrap_or(1));
        let ops = plans::take_out(&building.site.base, &cubes);
        if ops.is_empty() {
            return Err(refused("nothing is in there"));
        }
        let result = building.act(Some(player), "take out", &ops);
        Ok(with_cubes(answer("take out", result), &ops))
    })
}

#[derive(Deserialize)]
struct At {
    at: [i32; 3],
}

/// A cube of the base: its material, shape, how far it is in and any change pending.
fn cube(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let At { at } = parse_some(params)?;
    let rules = world.resource::<BuildRules>().clone();
    let site = world.resource::<Site>();
    Ok(match site.base.cube(IVec3::from_array(at)) {
        None => json!({ "at": at, "kind": Value::Null }),
        Some(cube) => json!({
            "at": at,
            "kind": rules.name(cube.kind),
            "shape": format!("{:?}", cube.shape),
            "fill": format!("{:?}", cube.fill),
            "level": cube.level(),
            "built": cube.is_built(),
            "pending": format!("{:?}", cube.pending),
            "part": cube.part.map(|part| part.0),
        }),
    })
}

fn state(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let feet = world
        .query_filtered::<&Transform, (With<Player>, With<LocalPlayer>)>()
        .iter(world)
        .next()
        .map(|at| at.translation);
    let start = *world.resource::<Start>();
    let rules = world.resource::<BuildRules>().clone();
    let offcuts = world.resource::<Offcuts>().0.to_string();
    let site = world.resource::<Site>();
    let base = &site.base;
    let frame = site.frame;
    let from_start = |at: Vec3| start.local(frame.transform_point(at)).to_array();
    let parts: Vec<Value> = base
        .parts()
        .map(|(id, part)| {
            let progress = base.progress(id);
            let cubes: Vec<IVec3> = base.cubes_of(id).map(|(at, _)| at).collect();
            let low = cubes.iter().copied().reduce(IVec3::min).unwrap_or_default();
            let high = cubes.iter().copied().reduce(IVec3::max).unwrap_or_default();
            let shaped = base
                .cubes_of(id)
                .filter(|(_, cube)| cube.shape != terra_build::Shape::Whole)
                .count();
            json!({
                "id": id.0,
                "kind": rules.name(part.kind),
                "cubes": progress.cubes,
                "shaped": shaped,
                "built": progress.built,
                "started": progress.started,
                "openings": progress.openings,
                "done": progress.done(),
                "min": low.to_array(),
                "max": high.to_array(),
            })
        })
        .collect();
    let roofs: Vec<Value> = base
        .roofs()
        .map(|(id, roof)| {
            json!({
                "id": id.0,
                "min": roof.section.min,
                "max": roof.section.max,
                "style": format!("{:?}", roof.section.style),
                "lowest": roof.stages.iter().min(),
                "highest": roof.stages.iter().max(),
                "covered": roof.stages.iter().all(|&stage| stage == terra_build::ROOF_STAGES),
            })
        })
        .collect();
    let props: Vec<Value> = base
        .props()
        .map(|prop| {
            let (low, high) = (IVec3::from_array(prop.min), IVec3::from_array(prop.max));
            let middle = (low.as_vec3() + (high + IVec3::ONE).as_vec3()) * 0.5 * CELL_M;
            json!({
                "kind": super::props::name(prop.kind),
                "min": prop.min,
                "max": prop.max,
                "open": prop.open,
                "at": from_start(middle.with_y(low.y as f32 * CELL_M)),
            })
        })
        .collect();
    let openings = base
        .cubes()
        .filter(|(_, cube)| cube.fill == Fill::Open)
        .count();
    Ok(json!({
        "origin": from_start(Vec3::ZERO),
        "cell_m": CELL_M,
        "storey_m": STOREY_M,
        "cubes": base.cubes().count(),
        "planned": base.store().planned(),
        "openings": openings,
        "floating": base.floating().len(),
        "room_tiles": base.tile_count(),
        "parts": parts,
        "roofs": roofs,
        "props": props,
        "offcuts": offcuts,
        "since": site.since.len(),
        // Where this machine's player's feet are, in metres in the base's own space.
        "player": feet.map(|feet| frame.compute_affine().inverse().transform_point3(feet).to_array()),
    }))
}
