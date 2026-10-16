//! Creatures that fly (D382): a flier body plan (docs/creatures.md), its kinds data in
//! `assets/data/fliers.ron`, the first a dragon. A flier is moved as shipped games move
//! their dragons (research/2026-10-15-dragons-and-fire.md): along a flight path, a spline
//! through points, at its cruising speed, rolling into each turn by the bank its speed and
//! the turn's tightness ask for (tan φ = v² κ / g, held within its most) before it yaws,
//! climbing with its wings beating and gliding when it comes down; then hovering, which
//! nothing so big really can, short and with deep beats; and breathing fire, in a strafing
//! pass (fly-by) or hovering, the breath a cone of flame from its mouth that sets alight what
//! it meets (`build::fire::BreathesFire`). Until monsters think (creatures.md), what it does
//! is told it: over the remote, `flight.spawn`, `flight.path`, `flight.hover`,
//! `flight.breathe` and `flight.roar`, so a script flies it as an AI later will.

use crate::build::fire::BreathesFire;
use crate::remote::{method, refused};
use bevy::{
    ecs::system::SystemState,
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
    world_serialization::{WorldAsset, WorldInstanceReady},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

/// A kind of flier, as `fliers.ron` has it. The file says what each part means.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct FlierDef {
    pub name: String,
    pub model: (String, f32),
    pub clips: FlierClips,
    pub cruise_mps: f32,
    pub hover_mps: f32,
    pub most_bank_deg: f32,
    pub bank_deg_s: f32,
    pub turn_deg_s: f32,
    pub mouth: String,
    pub breath: BreathDef,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct FlierClips {
    pub fly: String,
    pub glide: String,
    pub hover: String,
    pub breathe: String,
    pub fly_breathe: String,
    pub roar: String,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct BreathDef {
    pub reach_m: f32,
    pub spread: f32,
    pub heat: f32,
}

const FILE: &str = "fliers.ron";
/// How long a clip takes to blend into the next.
const BLEND: Duration = Duration::from_millis(450);
/// How far apart the points a path is laid out through are, in metres.
const PATH_STEP_M: f32 = 0.5;
/// How fast a flier eases into a hover, a share a second.
const SETTLE: f32 = 1.6;
const GRAVITY: f32 = 9.81;

/// The kinds, read once.
#[derive(Resource)]
struct Kinds {
    defs: Vec<FlierDef>,
    models: Vec<Handle<Gltf>>,
    ready: Vec<
        Option<(
            Handle<WorldAsset>,
            Handle<AnimationGraph>,
            [AnimationNodeIndex; 6],
        )>,
    >,
}

/// What a flier is doing.
#[derive(Clone, Debug)]
enum Flight {
    /// Along a path laid out through points, so far along it, then hovering facing a point.
    Path {
        points: Vec<Vec3>,
        along: f32,
        speed: f32,
        then_face: Vec3,
    },
    /// Holding at a point, facing another.
    Hover { at: Vec3, face: Vec3 },
}

/// A flier: its kind, what it is doing, how it is banked, its speed now, whether it is
/// breathing fire and at what, and when it last roared.
#[derive(Component)]
pub struct Flier {
    kind: usize,
    flight: Flight,
    bank: f32,
    speed: f32,
    breathing: bool,
    aim: Option<Vec3>,
    roar_s: f32,
}

/// On a flier's animation player: whose, and what it plays.
#[derive(Component)]
struct Flaps {
    flier: Entity,
    clips: [AnimationNodeIndex; 6],
    playing: Option<AnimationNodeIndex>,
}

/// On the entity at a flier's mouth that breathes, looking out of it: whose.
#[derive(Component)]
struct Mouth(Entity);

pub fn plugin(app: &mut App) {
    let defs: Vec<FlierDef> = terra_world::data::read(FILE);
    app.insert_resource(Kinds {
        models: Vec::new(),
        ready: vec![None; defs.len()],
        defs,
    })
    .add_systems(Startup, load)
    .add_systems(Update, (prepare, fly, animate, breathe).chain());
    method(app, "flight.spawn", spawn);
    method(app, "flight.path", path);
    method(app, "flight.hover", hover);
    method(app, "flight.breathe", breath);
    method(app, "flight.roar", roar);
}

fn load(assets: Res<AssetServer>, mut kinds: ResMut<Kinds>) {
    kinds.models = kinds
        .defs
        .iter()
        .map(|def| assets.load(def.model.0.clone()))
        .collect();
}

/// Each kind's scene and clips, once its model is in.
fn prepare(
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut kinds: ResMut<Kinds>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    for i in 0..kinds.defs.len() {
        if kinds.ready[i].is_some() || !assets.is_loaded_with_dependencies(&kinds.models[i]) {
            continue;
        }
        let Some(gltf) = gltfs.get(&kinds.models[i]) else {
            continue;
        };
        let def = &kinds.defs[i];
        let Some(scene) = gltf.default_scene.clone() else {
            error!("{} has no scene.", def.name);
            continue;
        };
        let c = &def.clips;
        let clips: Option<Vec<_>> = [
            &c.fly,
            &c.glide,
            &c.hover,
            &c.breathe,
            &c.fly_breathe,
            &c.roar,
        ]
        .iter()
        .map(|name| gltf.named_animations.get(name.as_str()).cloned())
        .collect();
        let Some(clips) = clips else {
            error!("{} is missing a clip.", def.name);
            continue;
        };
        let (graph, nodes) = AnimationGraph::from_clips(clips);
        kinds.ready[i] = Some((
            scene,
            graphs.add(graph),
            [nodes[0], nodes[1], nodes[2], nodes[3], nodes[4], nodes[5]],
        ));
    }
}

/// A Catmull-Rom spline through the points, laid out a step apart.
fn lay_out(points: &[Vec3]) -> Vec<Vec3> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let at = |i: isize| points[i.clamp(0, points.len() as isize - 1) as usize];
    let mut out = vec![points[0]];
    for i in 0..points.len() as isize - 1 {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        let steps = ((p2 - p1).length() / PATH_STEP_M).ceil().max(1.0) as usize;
        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            out.push(
                0.5 * ((2.0 * p1)
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3),
            );
        }
    }
    out
}

/// Where along a laid-out path so many metres along it is, and its way there.
fn along(path: &[Vec3], metres: f32) -> (Vec3, Vec3, bool) {
    let mut left = metres.max(0.0);
    for pair in path.windows(2) {
        let length = pair[0].distance(pair[1]);
        if left <= length && length > 0.0 {
            let way = (pair[1] - pair[0]) / length;
            return (pair[0] + way * left, way, false);
        }
        left -= length;
    }
    let n = path.len();
    let way = if n >= 2 {
        (path[n - 1] - path[n - 2]).normalize_or(Vec3::Z)
    } else {
        Vec3::Z
    };
    (path[n - 1], way, true)
}

/// Each flier moved by what it is doing: along its path at its speed, easing to a hover at
/// its end, banked into its turns; or holding at its hover, turned to face what it faces.
fn fly(time: Res<Time>, kinds: Res<Kinds>, mut fliers: Query<(&mut Flier, &mut Transform)>) {
    let step = time.delta_secs();
    if step <= 0.0 {
        return;
    }
    for (mut flier, mut transform) in &mut fliers {
        let def = &kinds.defs[flier.kind];
        let here = transform.translation;
        let up = here.normalize_or(Vec3::Y);
        let mut bank_wanted = 0.0;
        let next = match &mut flier.flight {
            Flight::Path {
                points,
                along: done,
                speed,
                then_face,
            } => {
                let length: f32 = points.windows(2).map(|p| p[0].distance(p[1])).sum();
                // Slowing into its hover over its last stretch.
                let left = (length - *done).max(0.0);
                let slow = (left / 40.0).clamp(0.0, 1.0);
                let v = def.hover_mps + (*speed - def.hover_mps) * slow;
                *done += v * step;
                let (at, way, ended) = along(points, *done);
                let (_, ahead, _) = along(points, *done + v * 0.6);
                // The turn's tightness, on the level: how fast its way turns a metre.
                let level = |w: Vec3| w.reject_from(up).normalize_or_zero();
                let turn = level(way).cross(level(ahead)).dot(up);
                let curvature = turn / (v * 0.6).max(0.1);
                bank_wanted = libm::atanf(v * v * curvature / GRAVITY);
                let face = *then_face;
                if ended {
                    Some((at, way, Some(Flight::Hover { at, face }), v))
                } else {
                    Some((at, way, None, v))
                }
            }
            Flight::Hover { at, face } => {
                let to = *at;
                let toward = (*face - here)
                    .reject_from(up)
                    .normalize_or(transform.forward().as_vec3());
                let eased = here + (to - here) * (1.0 - libm::expf(-SETTLE * step));
                Some((eased, toward, None, def.hover_mps * 0.3))
            }
        };
        let Some((at, way, then, v)) = next else {
            continue;
        };
        if let Some(then) = then {
            flier.flight = then;
        }
        flier.speed = v;
        let most = def.most_bank_deg.to_radians();
        let rate = def.bank_deg_s.to_radians() * step;
        let bank = flier.bank + (bank_wanted.clamp(-most, most) - flier.bank).clamp(-rate, rate);
        flier.bank = bank;
        // Its heading turned toward its way no faster than it turns.
        let wanted = Transform::from_translation(at).looking_to(way, up).rotation;
        let turned = transform.rotation.slerp(
            wanted,
            (def.turn_deg_s.to_radians() * step
                / transform.rotation.angle_between(wanted).max(1e-4))
            .min(1.0),
        );
        transform.translation = at;
        // Rolled into the turn: a right turn (way turning clockwise seen from above) rolls
        // its right wing down.
        let level = Transform::from_translation(at)
            .looking_to(turned * Vec3::NEG_Z, up)
            .rotation;
        transform.rotation = level * Quat::from_rotation_z(bank);
    }
}

/// Each flier's clip by what it is doing: beating its wings to climb or slow, gliding as it
/// comes down or runs level fast, hovering, breathing as it flies or hovers, roaring.
fn animate(
    time: Res<Time>,
    mut fliers: Query<(&mut Flier, &Transform)>,
    mut flapping: Query<(&mut Flaps, &mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let step = time.delta_secs();
    for (mut flaps, mut player, mut transitions) in &mut flapping {
        let Ok((mut flier, transform)) = fliers.get_mut(flaps.flier) else {
            continue;
        };
        flier.roar_s = (flier.roar_s - step).max(0.0);
        let [fly, glide, hover, breathe, fly_breathe, roar] = flaps.clips;
        let up = transform.translation.normalize_or(Vec3::Y);
        let climb = transform.forward().as_vec3().dot(up);
        let wanted = match &flier.flight {
            Flight::Hover { .. } if flier.roar_s > 0.0 => roar,
            Flight::Hover { .. } if flier.breathing => breathe,
            Flight::Hover { .. } => hover,
            Flight::Path { .. } if flier.breathing => fly_breathe,
            Flight::Path { .. }
                if climb < -0.04
                    || (climb < 0.05 && flier.speed > 18.0 && flier.bank.abs() > 0.2) =>
            {
                glide
            }
            Flight::Path { .. } => fly,
        };
        if flaps.playing == Some(wanted) {
            continue;
        }
        flaps.playing = Some(wanted);
        transitions.play(&mut player, wanted, BLEND).repeat();
    }
}

/// The breath out of each flier's mouth: on while it breathes, aimed at what it breathes at
/// within a turn of its head.
fn breathe(
    fliers: Query<(&Flier, &Transform)>,
    mut mouths: Query<(&Mouth, &GlobalTransform, &mut BreathesFire)>,
) {
    for (mouth, at, mut breath) in &mut mouths {
        let Ok((flier, body)) = fliers.get(mouth.0) else {
            continue;
        };
        breath.on = flier.breathing;
        // At what it is told to breathe at, its head turned to it; else ahead of it and down,
        // as a strafing breath goes, by its body's way rather than its head's, which the clips
        // swing.
        let up = body.translation.normalize_or(Vec3::Y);
        let ahead = body
            .forward()
            .as_vec3()
            .reject_from(up)
            .normalize_or(body.forward().as_vec3());
        let down = (ahead * libm::cosf(BREATH_DOWN) - up * libm::sinf(BREATH_DOWN)).normalize();
        breath.aim = Some(
            flier
                .aim
                .unwrap_or(at.translation() + down * breath.reach_m),
        );
    }
}

/// How far below its way a flier breathes when it is told no target, in radians.
const BREATH_DOWN: f32 = 0.6;

#[derive(Deserialize)]
struct Spawn {
    kind: String,
    at: [f32; 3],
    facing: Option<[f32; 3]>,
    #[serde(default)]
    base: bool,
}

/// A point the remote gives, in the world, or in the player's base's own metres.
fn placed(world: &World, point: [f32; 3], base: bool) -> Result<Vec3, bevy::remote::BrpError> {
    let point = Vec3::from_array(point);
    if !base {
        return Ok(point);
    }
    crate::build::base_frame(world)
        .map(|frame| frame.transform_point(point))
        .ok_or_else(|| refused("there is no base"))
}

/// Brings a flier of a kind into the world, hovering at a point.
fn spawn(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Spawn = parse_some(params)?;
    let at = placed(world, asked.at, asked.base)?;
    let face = match asked.facing {
        Some(facing) => placed(world, facing, asked.base)?,
        None => at + at.normalize().any_orthonormal_vector(),
    };
    let kinds = world.resource::<Kinds>();
    let Some(kind) = kinds.defs.iter().position(|def| def.name == asked.kind) else {
        return Err(refused(format!("there is no flier called {}", asked.kind)));
    };
    let Some((scene, graph, clips)) = kinds.ready[kind].clone() else {
        return Err(refused(format!("the {} is not loaded yet", asked.kind)));
    };
    let def = kinds.defs[kind].clone();
    let up = at.normalize();
    let flier = world
        .spawn((
            Name::new(format!("Flier: {}", def.name)),
            // What it flies over runs from it.
            super::Frightful { within_m: 70.0 },
            Flier {
                kind,
                flight: Flight::Hover { at, face },
                bank: 0.0,
                speed: 0.0,
                breathing: false,
                aim: None,
                roar_s: 0.0,
            },
            Transform::from_translation(at).looking_to(
                (face - at)
                    .reject_from(up)
                    .normalize_or(up.any_orthonormal_vector()),
                up,
            ),
            Visibility::default(),
        ))
        .id();
    let mouth_name = def.mouth.clone();
    let breath = def.breath;
    world
        .spawn((
            bevy::world_serialization::WorldAssetRoot(scene),
            // The models face +Z; things here face -Z.
            Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                .with_scale(Vec3::splat(def.model.1)),
            ChildOf(flier),
        ))
        .observe(
            move |ready: On<WorldInstanceReady>,
                  mut commands: Commands,
                  children: Query<&Children>,
                  players: Query<(), With<AnimationPlayer>>,
                  names: Query<&Name>| {
                if let Some(animator) = children
                    .iter_descendants(ready.entity)
                    .find(|&child| players.contains(child))
                {
                    commands.entity(animator).insert((
                        AnimationGraphHandle(graph.clone()),
                        AnimationTransitions::new(),
                        Flaps {
                            flier,
                            clips,
                            playing: None,
                        },
                    ));
                }
                // The breath looks out of the mouth: its socket's +Z, as the model's front.
                if let Some(mouth) = children.iter_descendants(ready.entity).find(|&child| {
                    names
                        .get(child)
                        .is_ok_and(|name| name.as_str() == mouth_name)
                }) {
                    commands.spawn((
                        Name::new("Breath"),
                        Mouth(flier),
                        BreathesFire {
                            reach_m: breath.reach_m,
                            spread: breath.spread,
                            heat: breath.heat,
                            on: false,
                            aim: None,
                        },
                        Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
                        ChildOf(mouth),
                    ));
                }
            },
        );
    info!("Flight: a {} at {at}.", def.name);
    Ok(json!({ "done": true, "entity": flier.to_bits() }))
}

/// The flier of a kind, the first if there are several.
fn flier_of(world: &mut World, kind: Option<&str>) -> Result<Entity, bevy::remote::BrpError> {
    let names: Vec<String> = world
        .resource::<Kinds>()
        .defs
        .iter()
        .map(|def| def.name.clone())
        .collect();
    let mut state = SystemState::<Query<(Entity, &Flier)>>::new(world);
    let fliers = state.get(world).map_err(|why| refused(format!("{why}")))?;
    fliers
        .iter()
        .find(|(_, flier)| kind.is_none_or(|kind| names[flier.kind] == kind))
        .map(|(entity, _)| entity)
        .ok_or_else(|| refused("there is no flier"))
}

#[derive(Deserialize)]
struct Path {
    kind: Option<String>,
    through: Vec<[f32; 3]>,
    speed: Option<f32>,
    facing: Option<[f32; 3]>,
    #[serde(default)]
    base: bool,
}

/// Flies a flier along a path through points from where it is, at its cruising speed or
/// the one given, to hover at the last facing a point (on along its way if not given).
fn path(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Path = parse_some(params)?;
    let entity = flier_of(world, asked.kind.as_deref())?;
    let mut points = vec![
        world
            .get::<Transform>(entity)
            .map_or(Vec3::ZERO, |t| t.translation),
    ];
    for point in &asked.through {
        points.push(placed(world, *point, asked.base)?);
    }
    let last = *points.last().unwrap();
    let face = match asked.facing {
        Some(facing) => placed(world, facing, asked.base)?,
        None => last + (last - points[points.len().saturating_sub(2)]),
    };
    let kind = world.get::<Flier>(entity).map_or(0, |flier| flier.kind);
    let speed = asked
        .speed
        .unwrap_or(world.resource::<Kinds>().defs[kind].cruise_mps);
    let laid = lay_out(&points);
    let length: f32 = laid.windows(2).map(|p| p[0].distance(p[1])).sum();
    if let Some(mut flier) = world.get_mut::<Flier>(entity) {
        flier.flight = Flight::Path {
            points: laid,
            along: 0.0,
            speed,
            then_face: face,
        };
    }
    info!("Flight: a path of {length:.0} m at {speed:.0} m/s.");
    Ok(json!({ "done": true, "metres": length, "seconds": length / speed }))
}

#[derive(Deserialize)]
struct Hover {
    kind: Option<String>,
    at: [f32; 3],
    facing: [f32; 3],
    #[serde(default)]
    base: bool,
}

/// Holds a flier hovering at a point facing another, easing there from where it is.
fn hover(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Hover = parse_some(params)?;
    let entity = flier_of(world, asked.kind.as_deref())?;
    let (at, face) = (
        placed(world, asked.at, asked.base)?,
        placed(world, asked.facing, asked.base)?,
    );
    if let Some(mut flier) = world.get_mut::<Flier>(entity) {
        flier.flight = Flight::Hover { at, face };
    }
    Ok(json!({ "done": true }))
}

#[derive(Deserialize)]
struct Breath {
    kind: Option<String>,
    on: bool,
    at: Option<[f32; 3]>,
    #[serde(default)]
    base: bool,
}

/// Starts or stops a flier breathing fire, aimed at a point if given.
fn breath(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Breath = parse_some(params)?;
    let entity = flier_of(world, asked.kind.as_deref())?;
    let aim = asked
        .at
        .map(|at| placed(world, at, asked.base))
        .transpose()?;
    if let Some(mut flier) = world.get_mut::<Flier>(entity) {
        flier.breathing = asked.on;
        flier.aim = aim;
    }
    info!(
        "Flight: breathing {}.",
        if asked.on { "fire" } else { "no more" }
    );
    Ok(json!({ "done": true }))
}

#[derive(Deserialize)]
struct Roar {
    kind: Option<String>,
    seconds: Option<f32>,
}

/// A hovering flier roars, for its roar's length or as long as given.
fn roar(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Roar = parse_some(params)?;
    let entity = flier_of(world, asked.kind.as_deref())?;
    if let Some(mut flier) = world.get_mut::<Flier>(entity) {
        flier.roar_s = asked.seconds.unwrap_or(2.7);
    }
    Ok(json!({ "done": true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// A world with the fliers' kinds and the clock stepping a sixtieth of a second.
    fn world() -> World {
        let mut world = World::new();
        let defs: Vec<FlierDef> = terra_world::data::read(FILE);
        world.insert_resource(Kinds {
            models: Vec::new(),
            ready: vec![None; defs.len()],
            defs,
        });
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
        world.insert_resource(time);
        world
    }

    #[test]
    fn a_flier_banks_into_a_turn_its_inside_wing_down_and_flies_level_out_of_it() {
        let mut world = world();
        let up = Vec3::Y;
        let start = Vec3::new(0.0, 2000.0, 0.0);
        // North (-z), then a right turn east (+x), then on east.
        let points = vec![
            start,
            start + Vec3::new(0.0, 0.0, -80.0),
            start + Vec3::new(60.0, 0.0, -140.0),
            start + Vec3::new(200.0, 0.0, -140.0),
        ];
        let flier = world
            .spawn((
                Flier {
                    kind: 0,
                    flight: Flight::Path {
                        points: lay_out(&points),
                        along: 0.0,
                        speed: 24.0,
                        then_face: start,
                    },
                    bank: 0.0,
                    speed: 0.0,
                    breathing: false,
                    aim: None,
                    roar_s: 0.0,
                },
                Transform::from_translation(start).looking_to(Vec3::NEG_Z, up),
            ))
            .id();
        let mut lowest_right = f32::INFINITY;
        for _ in 0..60 * 9 {
            world.run_system_once(fly).unwrap();
            let t = world.get::<Transform>(flier).unwrap();
            lowest_right = lowest_right.min(t.right().dot(up));
        }
        // In the right turn its right wing went down, well into a bank.
        assert!(
            lowest_right < -0.3,
            "its right wing went no lower than {lowest_right}"
        );
        // Out of it, flying east, it is level again.
        let t = world.get::<Transform>(flier).unwrap();
        assert!(
            t.right().dot(up).abs() < 0.1,
            "still banked {}",
            t.right().dot(up)
        );
        assert!(
            t.forward().dot(Vec3::X) > 0.9,
            "not heading east: {}",
            t.forward()
        );
    }

    #[test]
    fn the_fliers_file_reads_and_every_kind_has_its_model() {
        let defs: Vec<FlierDef> = terra_world::data::read(FILE);
        assert!(!defs.is_empty());
        for def in &defs {
            assert!(
                terra_world::data::assets().join(&def.model.0).exists(),
                "{} has no model {}",
                def.name,
                def.model.0
            );
            assert!(
                def.cruise_mps > def.hover_mps
                    && def.most_bank_deg > 0.0
                    && def.most_bank_deg < 80.0
            );
        }
    }

    #[test]
    fn a_path_runs_through_its_points_and_along_it_ends_at_the_last() {
        let points = [
            Vec3::ZERO,
            Vec3::new(40.0, 0.0, 0.0),
            Vec3::new(40.0, 0.0, 40.0),
        ];
        let laid = lay_out(&points);
        for point in points {
            assert!(
                laid.iter().any(|p| p.distance(point) < 1e-3),
                "{point} not on the path"
            );
        }
        let length: f32 = laid.windows(2).map(|p| p[0].distance(p[1])).sum();
        assert!(length > 80.0 && length < 90.0, "{length}");
        let (end, _, ended) = along(&laid, length + 5.0);
        assert!(ended && end.distance(points[2]) < 1e-3);
        let (mid, way, ended) = along(&laid, 20.0);
        assert!(!ended && (mid.x - 20.0).abs() < 1.0 && way.x > 0.9);
    }
}
