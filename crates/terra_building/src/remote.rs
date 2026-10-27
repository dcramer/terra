//! Building over the remote protocol (D69; tools/sandbox): a script lays plans, builds,
//! takes apart and undoes, and reads the base, as this
//! machine's player, through the same operations and checks the hammer uses
//! (`act::Building`). Each answers at once with what was done, or why it was
//! refused in the words the player is shown, and is logged as `Build: …`.
//!
//! Places on the base are counted in its own cubes: east, up and south from its claim
//! stone, half a metre each; a tile is a column of a storey, its `y` the storey.
//!
//! - `build.state`: the base: its parts and how far each is built and how many of their
//!   cubes are cut to a shape, its roof cubes and how far up each is (D241), fitted props,
//!   planned cubes, rooms, the claim's offcuts, where it stands, and where the player's
//!   feet are in it.
//! - `build.lay {what, min, max, kind?, floor?, style?, along_x?, over?}`: lays a plan over
//!   tiles: a `room` (the shortcut: floor, four walls and a flat roof), a `wall` from `min`
//!   to `max`, a `floor`, a `flat_roof`, or "add a roof" (`roof`, D241): roof cubes in a
//!   style (`Hip`, `Gable`, `LeanTo`) over the tiles, or over the space closing in the tile
//!   `over`, in a roof material, or the roof of the walls' `kind`;
//!   or from the cube `min`, as the hammer places them (D193): a `flight` of stairs (a ramp
//!   with `ramp: true`) up a storey toward `facing`, `wide` metres across, a `post` a
//!   storey high, or one `block` of a `shape`.
//! - `build.opening {tiles, door?}`: cuts a doorway (or, with `door: false`, a window) into a
//!   wall's plan along those tiles.
//! - `build.fill {part?, steps?}`: builds what goes in next, as holding the hammer does, so
//!   many times (one if not given), stopping at the first refusal.
//! - `build.take_apart {at, steps?}`: takes out what came last of the part a cube is in.
//! - `build.fit {at, kind?}`: fits a door or shutters, by the catalogue's name (the first
//!   door, or for a window the first shutters, if not given), into the hole a cube is
//!   part of, as the hammer fits them, if the hole fits it.
//! - `build.door {at, open}`: opens or shuts the door or shutters fitted at a cube.
//! - `build.undo`: undoes the player's last change to the base.
//! - `build.op {ops}`: any operations, as `terra_build::ops::Op` serialises them, as one act.
//! - `build.point {at}`: where a place in the base, in metres, is in the world and from the
//!   start, to look at it or walk to it.
//! - `build.look {from, at}`: holds the camera at a place in the base, in metres, looking at
//!   another, as `sandbox.look` does in the world's terms (a film's camera, `--calls`).
//! - `build.damage_drawn {drawn}`: draws damage, or not, everywhere, to compare what is not
//!   damaged both ways (`cargo xtask artifacts --frames`).
//! - `build.example {name}`: puts up an example building whole as the base, in place of what
//!   is there: `castle`, to edit and time a base of 140,000 cubes (`cargo xtask qa castle`),
//!   or `burg`, the castle built to burn (D377).
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
//! - `build.damage {at, to?, amount?}`: harms the cubes of the box from `at` to `to`
//!   (`at` alone if not given) by `amount`, enough to destroy each if not given, as an attack
//!   will (D90, D305): what then hangs from nothing comes down, breaking into fragments
//!   that fall and fade.
//! - `build.subside {columns, by_m}`: the ground under columns gives way by so many metres,
//!   bringing down what it alone held (D193).
//! - `build.repair {at, brush?}`: mends the damaged cubes of a brush's box round `at`, as
//!   the hammer does with nothing picked.

use crate::act::{BuildRules, Building, Offcuts};
use crate::tool::Builder;
use bevy::{
    ecs::system::SystemState,
    prelude::*,
    remote::{BrpResult, RemoteMethodSystemId, RemoteMethods, builtin_methods::parse_some},
};
use serde::Deserialize;
use serde_json::{Value, json};
use terra_bases::{Site, plans};
use terra_build::ops::{Done, Op, Refusal};
use terra_build::roofer::Style;
use terra_build::{CELL_M, Facing, Fill, Panel, STOREY_M, Shape};
use terra_build_layout::fitting::Hang;
use terra_walker::{LocalPlayer, Player};
use terra_world::remote::{method, refused};
use terra_world::start::Start;

pub fn plugin(app: &mut App) {
    method(app, "build.state", state);
    method(app, "build.lay", lay);
    method(app, "build.opening", opening);
    method(app, "build.fill", fill);
    method(app, "build.take_apart", take_apart);
    method(app, "build.fit", fit);
    method(app, "build.door", door);
    method(app, "build.undo", undo);
    method(app, "build.op", ops);
    method(app, "build.point", point);
    method(app, "build.look", look);
    method(app, "build.example", example);
    method(app, "build.damage_drawn", damage_drawn);
    method(app, "build.ground", ground);
    method(app, "build.material", material);
    method(app, "build.replace", replace);
    method(app, "build.carve", carve);
    method(app, "build.take_out", take_out);
    method(app, "build.cube", cube);
    method(app, "build.damage", damage);
    method(app, "build.subside", subside);
    method(app, "build.repair", repair);
}

#[derive(Deserialize)]
struct Harm {
    at: [i32; 3],
    to: Option<[i32; 3]>,
    amount: Option<u16>,
}

impl Harm {
    /// The cubes of its box with anything in them.
    fn cubes(&self, base: &terra_build::Base) -> Vec<[i32; 3]> {
        let (a, b) = (
            IVec3::from_array(self.at),
            IVec3::from_array(self.to.unwrap_or(self.at)),
        );
        let (low, high) = (a.min(b), a.max(b));
        (low.x..=high.x)
            .flat_map(|x| {
                (low.y..=high.y).flat_map(move |y| (low.z..=high.z).map(move |z| [x, y, z]))
            })
            .filter(|&at| base.is_in(IVec3::from_array(at)))
            .collect()
    }
}

/// Harms a box of cubes, as an attack will (D305): not the player's act, so not theirs to
/// undo.
fn damage(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let harm: Harm = parse_some(params)?;
    as_player(world, |building, _| {
        let cubes = harm.cubes(&building.site.base);
        if cubes.is_empty() {
            return Err(refused("nothing is built there"));
        }
        let amount = harm.amount.unwrap_or(u16::MAX);
        let before = building.site.base.cubes().count();
        let started = std::time::Instant::now();
        let mut said = act_answer(building, None, "damage", vec![Op::Damage { cubes, amount }]);
        if said.get("applying").is_none() {
            let took_ms = started.elapsed().as_secs_f32() * 1000.0;
            let fell = before - building.site.base.cubes().count();
            said["fell"] = json!(fell);
            said["took_ms"] = json!(took_ms);
        }
        Ok(said)
    })
}

#[derive(Deserialize)]
struct Subside {
    columns: Vec<[i32; 2]>,
    by_m: f32,
}

/// The ground under columns gives way by so many metres (D193).
fn subside(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Subside = parse_some(params)?;
    as_player(world, |building, _| {
        let ground = building.site.base.ground();
        let samples = asked
            .columns
            .iter()
            .map(|&column| terra_build::Sample {
                column,
                height_cm: ground
                    .height_cm(column)
                    .saturating_sub((asked.by_m * 100.0).round() as i16),
            })
            .collect();
        let before = building.site.base.cubes().count();
        let result = building.act(None, "the ground gives way", &[Op::Subside { samples }]);
        let fell = before - building.site.base.cubes().count();
        let mut said = answer("the ground gives way", result);
        said["fell"] = json!(fell);
        Ok(said)
    })
}

/// Mends the damaged cubes of a brush's box round a cube, as the hammer does.
fn repair(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Brushed = parse_some(params)?;
    as_player(world, |building, player| {
        let cubes: Vec<[i32; 3]> =
            plans::brush(IVec3::from_array(asked.at), asked.brush.unwrap_or(1))
                .into_iter()
                .filter(|&at| building.site.base.damage(at) > 0)
                .map(|at| at.to_array())
                .collect();
        if cubes.is_empty() {
            return Err(refused("nothing there is damaged"));
        }
        Ok(act_answer(
            building,
            Some(player),
            "repair",
            vec![Op::Repair { cubes }],
        ))
    })
}

#[derive(Deserialize)]
struct Columns {
    columns: Vec<[i32; 2]>,
}

/// The ground's height under columns of the base, in its own space.
fn ground(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Columns { columns } = parse_some(params)?;
    let frame = world.resource::<Site>().frame;
    let planet = world.resource::<terra_world::planet::Planet>();
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

#[derive(Deserialize)]
struct DamageDrawn {
    drawn: bool,
}

/// Draws damage, or not, everywhere: a check compares what is not damaged both ways, which
/// must look the same (`cargo xtask artifacts --frames`, undamaged).
fn damage_drawn(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let DamageDrawn { drawn } = parse_some(params)?;
    let mut arrays = world
        .get_resource_mut::<terra_bases::arrays::Arrays>()
        .ok_or_else(|| refused("nothing is drawn in the texture arrays"))?;
    arrays.hide_damage(!drawn);
    info!("Build: damage drawn: {drawn}");
    Ok(json!({ "drawn": drawn }))
}

#[derive(Deserialize)]
struct Example {
    name: String,
}

/// Puts up an example building whole as the player's base, in place of what is there: the
/// castle (`terra_build::castle`, some 140,000 cubes), so edits to a base of its size are made
/// and timed to the screen as the hammer makes them (D185).
fn example(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Example { name } = parse_some(params)?;
    let kits = world
        .resource::<terra_bases::BuildLook>()
        .kits
        .clone()
        .ok_or_else(|| refused("there are no kits"))?;
    let ops = match name.as_str() {
        "castle" => {
            let kinds = crate::village::village_kinds(&kits)
                .ok_or_else(|| refused("the castle needs the settlement's materials"))?;
            let first = kinds.first();
            terra_build::castle::castle(first.masonry, first.beams, first.masonry_roof)
        }
        // The castle built to burn (D377), in the materials its parts are of.
        "burg" => {
            let kinds = crate::village::village_kinds(&kits)
                .ok_or_else(|| refused("the burg needs the settlement's materials"))?
                .first();
            terra_build::burg::burg(terra_build::burg::Materials {
                stone: kinds.masonry,
                beams: kinds.beams,
                logs: kinds.timber,
                shakes: kinds.timber_roof,
            })
        }
        _ => {
            return Err(refused(format!(
                "there is no example called {name}: castle, burg"
            )));
        }
    };
    let mut state = SystemState::<(
        ResMut<Site>,
        Commands,
        Res<terra_world::planet::Planet>,
        ResMut<Assets<Mesh>>,
    )>::new(world);
    let result = {
        let (mut site, mut commands, planet, mut meshes) = state
            .get_mut(world)
            .map_err(|problem| refused(format!("cannot build now: {problem}")))?;
        site.put_up(&ops, &mut commands, &planet, &mut meshes)
    };
    state.apply(world);
    result.map_err(refused)?;
    let cubes = world.resource::<Site>().base.cubes().count();
    info!("Build: put up the {name} as the base, {cubes} cubes.");
    Ok(json!({ "done": true, "what": format!("put up the {name}"), "cubes": cubes }))
}

#[derive(Deserialize)]
struct Look {
    from: [f32; 3],
    at: [f32; 3],
}

/// Holds the camera at a place in the base looking at another, both in metres in its own
/// space, as `sandbox.look` does in the world's: a script's camera keeps to the base
/// wherever its claim set it down.
fn look(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Look { from, at } = parse_some(params)?;
    let frame = world.resource::<Site>().frame;
    let placed = |point: [f32; 3]| frame.transform_point(Vec3::from_array(point)).to_array();
    let handler = world
        .resource::<RemoteMethods>()
        .get("sandbox.look")
        .copied();
    let Some(RemoteMethodSystemId::Instant(id)) = handler else {
        return Err(refused("the camera cannot be held in this run"));
    };
    let asked = json!({ "at": placed(at), "from": placed(from) });
    world.run_system_with(id, Some(asked)).map_err(refused)?
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
        // Every act still being worked out taken in first, so a call sees the base as every
        // call before it left it.
        building.settle_all();
        run(&mut building, player)
    };
    state.apply(world);
    result
}

/// Acts as `by`, answering with what was done, or why not. In a benchmark, whose calls
/// nothing waits on, the act is worked out on a worker and swapped in when done
/// (`Building::act_later`), and its answer says only that it is under way; its outcome is
/// logged when it is taken in, as an act at once's is.
fn act_answer(building: &mut Building, by: Option<Entity>, what: &str, ops: Vec<Op>) -> Value {
    if building.benchmark() {
        building.act_later(by, what, ops);
        return json!({ "done": true, "what": what, "applying": true });
    }
    let result = building.act(by, what, &ops);
    answer(what, result)
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
    /// The box laid, from one cube or tile to another: not wanted by a roof `over` a space.
    #[serde(default)]
    min: [i32; 3],
    #[serde(default)]
    max: [i32; 3],
    kind: Option<String>,
    floor: Option<String>,
    style: Option<Style>,
    along_x: Option<bool>,
    /// For a roof: a tile in the space it covers, closed in by walls, its shape its own.
    over: Option<[i32; 3]>,
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
    let kits = world.resource::<terra_bases::BuildLook>().kits.clone();
    as_player(world, |building, player| {
        let base = &building.site.base;
        let ground = |column: [i32; 2]| building.ground_cm(column);
        let footings = plans::Footings::of(building.rules.footing, &ground, &building.rules.footed);
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
                let (footprint, along_x) = match lay.over {
                    Some(tile) => (
                        plans::room_footprint(base, IVec3::from_array(tile)),
                        lay.along_x,
                    ),
                    None => (
                        plans::rectangle(low, high),
                        Some(lay.along_x.unwrap_or(plans::ridge_along_x(low, high))),
                    ),
                };
                let storey = lay.over.map_or(low.y, |tile| tile[1]);
                let roof = kits
                    .as_ref()
                    .map_or(terra_build::Kind::FIRST_ROOF, |kits| kits.roof_over(kind));
                let style = lay.style.unwrap_or(Style::Gable);
                (
                    plans::roof(base, &footprint, storey, style, along_x, roof),
                    None,
                )
            }
            // From the foot cube `min`, up a storey toward `facing`.
            "flight" => {
                let facing = lay.facing.unwrap_or_default();
                let top = plans::flight_top(facing, lay.ramp.unwrap_or(false));
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
                    &building.rules.rules,
                    footings,
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
        let ground = |column: [i32; 2]| building.ground_cm(column);
        let footings = plans::Footings::of(building.rules.footing, &ground, &building.rules.footed);
        let ops = building.with_ground(plans::opening(&building.site.base, &cubes, footings));
        if ops.is_empty() {
            return Err(refused("it is open already"));
        }
        let what = if opening.door {
            "cut a doorway"
        } else {
            "cut a window"
        };
        Ok(act_answer(building, Some(player), what, ops))
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
            building.settle_all();
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
            let said = act_answer(building, Some(player), &work.what, work.ops.clone());
            let refused = said["done"] == json!(false);
            done.push(with_cubes(said, &work.ops));
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
            building.settle_all();
            let Some(work) = building.last_work(Some(player), IVec3::from_array(take.at), near)
            else {
                break;
            };
            let said = act_answer(building, Some(player), &work.what, work.ops.clone());
            let refused = said["done"] == json!(false);
            done.push(with_cubes(said, &work.ops));
            if refused {
                break;
            }
        }
        Ok(Value::Array(done))
    })
}

#[derive(Deserialize)]
struct Fit {
    at: [i32; 3],
    kind: Option<String>,
}

fn fit(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let fit: Fit = parse_some(params)?;
    let at = IVec3::from_array(fit.at);
    // A doorway, from its storey's floor, takes a door; a window, over a sill, shutters.
    let base = &world.resource::<Site>().base;
    let (low, _) = plans::hole(base, at).ok_or_else(|| refused("that is not an opening"))?;
    let hang = plans::hang_for(base, low);
    let builds = world.resource::<terra_survival::catalogue::Builds>();
    let fitting = match &fit.kind {
        Some(name) => builds.fittings().iter().find(|each| &each.name == name),
        None => builds.fittings().iter().find(|each| each.hang == hang),
    }
    .cloned()
    .ok_or_else(|| refused("there is no such door or shutters"))?;
    as_player(world, |building, player| {
        let prop = plans::fit(
            &building.site.base,
            at,
            fitting.kind,
            fitting.hang,
            &fitting.fits,
        )
        .map_err(|misfit| match misfit {
            plans::Misfit::NoHole => refused("that is not an opening"),
            plans::Misfit::Taken => refused("something is fitted there already"),
            plans::Misfit::Hang(Hang::Door) => refused("a doorway takes a door"),
            plans::Misfit::Hang(Hang::Shutters) => refused("a window takes shutters"),
            plans::Misfit::Size(..) => {
                refused(format!("{} fit holes {}", fitting.name, fitting.sizes()))
            }
        })?;
        let what = format!("fit {}", fitting.name.to_lowercase());
        let result = building.act(Some(player), &what, &[Op::FitProp(prop)]);
        Ok(answer(&what, result))
    })
}

#[derive(Deserialize)]
struct Door {
    at: [i32; 3],
    open: bool,
}

fn door(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let door: Door = parse_some(params)?;
    as_player(world, |building, player| {
        let at = building
            .site
            .base
            .prop_in(IVec3::from_array(door.at))
            .map(|prop| prop.min)
            .ok_or_else(|| refused("nothing is fitted there"))?;
        let what = if door.open { "open it" } else { "shut it" };
        let result = building.act(
            Some(player),
            what,
            &[Op::SetOpen {
                at,
                open: door.open,
            }],
        );
        Ok(answer(what, result))
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
    let laying = world.resource::<terra_bases::BuildLook>().laying;
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
        .filter(|kind| rules.hammer.contains(kind))
        .ok_or_else(|| refused(format!("the hammer holds no material called {kind}")))?;
    for (mut builder, mut hammer) in world
        .query_filtered::<(&mut Builder, &mut terra_bases::hammer::Hammer), (With<Player>, With<LocalPlayer>)>()
        .iter_mut(world)
    {
        builder.material = Some(picked);
        hammer.set_if_neq(builder.hammer());
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
        let blocks = building.rules.blocks.clone();
        let ops = plans::change(
            &building.site.base,
            &cubes,
            kind,
            &building.rules.rules,
            &building.rules.families,
            |kind| blocks.contains(&kind),
        );
        if ops.is_empty() {
            return Err(refused("there is nothing there to change"));
        }
        let what = format!("change to {}", rules.name(kind));
        Ok(act_answer(building, Some(player), &what, ops))
    })
}

/// Marks the built cubes of a brush's box round a cube to be carved out (D226).
fn carve(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Brushed = parse_some(params)?;
    as_player(world, |building, player| {
        let cubes = plans::brush(IVec3::from_array(asked.at), asked.brush.unwrap_or(1));
        let ground = |column: [i32; 2]| building.ground_cm(column);
        let footings = plans::Footings::of(building.rules.footing, &ground, &building.rules.footed);
        let ops = building.with_ground(plans::carve(&building.site.base, &cubes, footings));
        if ops.is_empty() {
            return Err(refused("there is nothing there to carve"));
        }
        Ok(act_answer(building, Some(player), "carve", ops))
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
        let said = act_answer(building, Some(player), "take out", ops.clone());
        Ok(with_cubes(said, &ops))
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
    // The room it is in, if it is air closed in (D261).
    let room = site.base.room(IVec3::from_array(at));
    Ok(match site.base.cube(IVec3::from_array(at)) {
        None => json!({ "at": at, "kind": Value::Null, "room": room }),
        Some(cube) => json!({
            "at": at,
            "kind": rules.name(cube.kind),
            "shape": format!("{:?}", cube.shape),
            "fill": format!("{:?}", cube.fill),
            "level": cube.level(),
            "built": cube.is_built(),
            "pending": format!("{:?}", cube.pending),
            "part": cube.part.map(|part| part.0),
            "room": room,
            "damage": site.base.damage(IVec3::from_array(at)),
            "health": rules.rules.toughness(cube.kind).health,
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
    let fragments = crate::loss::live(world);
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
    // Its roof cubes (D241): how many, of what shapes, how many built, and how many at each
    // stage, nothing in to trims on.
    let mut stages = [0usize; 1 + terra_build_layout::layout::roofs::ROOF_STAGES as usize];
    let mut shapes: std::collections::BTreeMap<String, usize> = Default::default();
    for (_, cube) in base.cubes().filter(|(_, cube)| cube.kind.is_roof()) {
        stages[usize::from(terra_build_layout::layout::roofs::stage_of(&cube))] += 1;
        let shape = format!("{:?}", cube.shape);
        let shape = shape.split('(').next().unwrap_or("").to_string();
        *shapes.entry(shape).or_default() += 1;
    }
    let roofs = json!({
        "cubes": stages.iter().sum::<usize>(),
        "built": stages[stages.len() - 1],
        "stages": stages,
        "shapes": shapes,
    });
    let props: Vec<Value> = base
        .props()
        .map(|prop| {
            let (low, high) = (IVec3::from_array(prop.min), IVec3::from_array(prop.max));
            let middle = (low.as_vec3() + (high + IVec3::ONE).as_vec3()) * 0.5 * CELL_M;
            json!({
                "kind": prop.kind.0,
                "hang": format!("{:?}", prop.hang),
                "leaves": prop.leaf_count(),
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
        "damaged": base.damaged().count(),
        "fragments": fragments,
        "room_cubes": base.room_cubes(),
        "rooms": base.room_count(),
        "parts": parts,
        "roofs": roofs,
        "props": props,
        "offcuts": offcuts,
        "since": site.since.len(),
        // Where this machine's player's feet are, in metres in the base's own space.
        "player": feet.map(|feet| frame.compute_affine().inverse().transform_point3(feet).to_array()),
    }))
}
