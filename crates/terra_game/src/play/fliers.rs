//! Creatures that fly (D382): a flier body plan (docs/creatures.md), its kinds data in
//! `assets/data/fliers.ron`, the first a dragon. A flier is moved as shipped games and films
//! move a big one (research/2026-10-17-large-flier-motion.md, after
//! research/2026-10-15-dragons-and-fire.md):
//!
//! - **Along a planned course** ([`plan`]): a smooth curve through the points it is told,
//!   flown at the speed its turns and climbs allow, gathering and shedding speed as
//!   something heavy does, so it never skids round a corner or leaps to speed.
//! - **Banked into its turns** by the coordinated turn's law (tan φ = v² κ / g, within its
//!   most), read a little ahead so it rolls before it turns, eased as a heavy body rolls;
//!   pitched with its climb, and nose up as it brakes, flaring into a hover.
//! - **Its head leading and its tail trailing**: its neck turns into a turn and up into a
//!   climb ahead of its body, and its tail swings out behind, each bone a little after the
//!   one before it, laid over its clips after they play ([`spine`]).
//! - **Beating its wings to climb, gather speed or slow, and gliding** as it comes down or
//!   cruises, in bouts of a few beats; its clips blended by weight rather than switched, its
//!   beats quicker the harder it works, and its body lifted with each downstroke.
//! - **Hovering**, which nothing so big really can, short and with deep beats; **and
//!   breathing fire** in a strafing pass or hovering, after rearing its head back for a
//!   moment, the tell, the breath a cone of flame from its mouth that sets alight what it
//!   meets (`build::fire::BreathesFire`).
//!
//! Until monsters think (creatures.md), what it does is told it: over the remote,
//! `flight.spawn`, `flight.path`, `flight.hover`, `flight.breathe` and `flight.roar`, so a
//! script flies it as an AI later will.

mod plan;
mod spine;

use crate::build::fire::BreathesFire;
use crate::remote::{method, refused};
use bevy::{
    ecs::system::SystemState,
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
    transform::TransformSystems,
    world_serialization::{WorldAsset, WorldInstanceReady},
};
use plan::{Plan, Wings};
use serde::Deserialize;
use serde_json::{Value, json};
use spine::{Spine, chain, spine};

/// A kind of flier, as `fliers.ron` has it. The file says what each part means.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct FlierDef {
    pub name: String,
    pub model: (String, f32),
    pub clips: FlierClips,
    pub cruise_mps: f32,
    pub hover_mps: f32,
    pub accel_mps2: f32,
    pub brake_mps2: f32,
    pub most_bank_deg: f32,
    pub bank_s: f32,
    pub turn_deg_s: f32,
    pub flare_deg: f32,
    pub neck: (String, u8),
    pub tail: (String, u8),
    pub windup_s: f32,
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

impl FlierDef {
    fn wings(&self) -> Wings {
        Wings {
            slowest: self.hover_mps,
            most_bank: self.most_bank_deg.to_radians(),
            accel: self.accel_mps2,
            brake: self.brake_mps2,
        }
    }
}

const FILE: &str = "fliers.ron";
/// How long, in seconds, a clip's weight takes to come most of the way to what it should be:
/// between beating and gliding, and into or out of a hover or a breath.
const BLEND_S: f32 = 0.3;
const BLEND_HOVER_S: f32 = 0.55;
/// How fast a flier eases into a hover, a share a second.
const SETTLE: f32 = 1.6;
const GRAVITY: f32 = 9.81;
/// How far ahead along its course it reads the turn it banks for, in seconds: it rolls
/// before it turns.
const ROLL_LEAD_S: f32 = 0.35;
/// The fastest its heading may swing, in radians a second, whatever its course does.
const MOST_TURN: f32 = 1.6;
/// Below what speed, in metres a second, it beats as it hovers, and from what it starts to.
const HOVERING_MPS: [f32; 2] = [4.0, 9.0];
/// How much it must work (`Flier::effort`) to beat its wings, and how little to glide; in
/// between it cruises, gliding with a bout of beats every so often: so many seconds of
/// glide, then so many of beats.
const BEATS_FROM: f32 = 0.6;
const GLIDES_BELOW: f32 = 0.3;
const CRUISE_BOUT_S: [f32; 2] = [3.4, 2.2];
/// How far its body rises with each downstroke at its full beat, in metres.
const HEAVE_M: f32 = 0.35;
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
    /// Along a planned course, so far into it, then hovering facing a point.
    Path {
        plan: Plan,
        seconds: f32,
        then_face: Vec3,
    },
    /// Holding at a point, facing another.
    Hover { at: Vec3, face: Vec3 },
}

/// A flier: its kind and what it is doing; how it is banked and flared, and how fast each
/// is changing; its speed and how fast that changes; how fast it turns and pitches; how
/// hard it is working its wings; whether it is breathing fire, or rearing to, and at what;
/// and how long it has left to roar.
#[derive(Component)]
pub struct Flier {
    kind: usize,
    flight: Flight,
    bank: f32,
    bank_rate: f32,
    flare: f32,
    speed: f32,
    accel: f32,
    turn_rate: f32,
    pitch_rate: f32,
    effort: f32,
    heave: f32,
    /// Its heading, climb and all, without its bank or flare; none until it first flies.
    heading: Option<Quat>,
    /// How much of a hover its clips are in, last frame: its flare gives way to the hover's
    /// own tilt.
    hovering: f32,
    breathing: bool,
    /// Seconds left of rearing back before the breath comes, while it wants to breathe.
    windup: f32,
    wants_breath: bool,
    aim: Option<Vec3>,
    roar_s: f32,
}

impl Flier {
    fn new(kind: usize, flight: Flight) -> Flier {
        Flier {
            kind,
            flight,
            bank: 0.0,
            bank_rate: 0.0,
            flare: 0.0,
            speed: 0.0,
            accel: 0.0,
            turn_rate: 0.0,
            pitch_rate: 0.0,
            effort: 0.0,
            heave: 0.0,
            heading: None,
            hovering: 1.0,
            breathing: false,
            windup: 0.0,
            wants_breath: false,
            aim: None,
            roar_s: 0.0,
        }
    }

    /// How far into rearing back it is, 0 to 1.
    fn rearing(&self, def: &FlierDef) -> f32 {
        if self.wants_breath && !self.breathing && def.windup_s > 0.0 {
            1.0 - self.windup / def.windup_s
        } else {
            0.0
        }
    }
}

/// On a flier's animation player: whose, what it plays and at what weight each, and how long
/// it has been in its bout of cruising.
#[derive(Component)]
struct Flaps {
    flier: Entity,
    clips: [AnimationNodeIndex; 6],
    weights: [f32; 6],
    beating: bool,
    bout_s: f32,
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
    .add_systems(Update, (prepare, fly, animate, breathe).chain())
    .add_systems(
        PostUpdate,
        spine
            .after(bevy::app::AnimationSystems)
            .before(TransformSystems::Propagate),
    );
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

/// A critically damped spring's step toward a target, its time to settle about `settle_s`.
fn spring(value: &mut f32, rate: &mut f32, target: f32, settle_s: f32, step: f32) {
    let w = 4.0 / settle_s.max(1e-3);
    *rate += (w * w * (target - *value) - 2.0 * w * *rate) * step;
    *value += *rate * step;
}

/// A share of the way to a target, easing over `settle_s`.
fn ease(settle_s: f32, step: f32) -> f32 {
    1.0 - libm::expf(-step / settle_s.max(1e-3))
}

/// The signed angle on the level from one way to another, a turn to the left above nought.
fn level_angle(from: Vec3, to: Vec3, up: Vec3) -> f32 {
    let (a, b) = (
        from.reject_from(up).normalize_or_zero(),
        to.reject_from(up).normalize_or_zero(),
    );
    libm::atan2f(a.cross(b).dot(up), a.dot(b))
}

/// Each flier moved by what it is doing: along its course at its planned speed, banked into
/// its turns and flared as it brakes, easing to a hover at its end; or holding at its hover,
/// turned to face what it faces. Then its rearing to breathe counted down.
fn fly(time: Res<Time>, kinds: Res<Kinds>, mut fliers: Query<(&mut Flier, &mut Transform)>) {
    let step = time.delta_secs();
    if step <= 0.0 {
        return;
    }
    for (mut flier, mut transform) in &mut fliers {
        let def = &kinds.defs[flier.kind];
        let up = transform.translation.normalize_or(Vec3::Y);
        let was = flier
            .heading
            .map_or(transform.forward().as_vec3(), |heading| {
                heading * Vec3::NEG_Z
            });
        let heave = flier.heave;
        let most_turn = match flier.flight {
            Flight::Hover { .. } => def.turn_deg_s.to_radians(),
            Flight::Path { .. } => MOST_TURN,
        };
        let (at, way, bank_wanted, speed, accel, then) = match &mut flier.flight {
            Flight::Path {
                plan,
                seconds,
                then_face,
            } => {
                *seconds += step;
                let now = plan.at(*seconds);
                let ahead = plan.at(*seconds + ROLL_LEAD_S);
                let bank = libm::atanf(ahead.speed * ahead.speed * ahead.bend / GRAVITY);
                let then = now.ended.then_some(Flight::Hover {
                    at: now.at,
                    face: *then_face,
                });
                (now.at, now.way, bank, now.speed, now.accel, then)
            }
            Flight::Hover { at, face } => {
                let here = transform.translation - up * heave;
                let toward = (*face - here).reject_from(up).normalize_or(was);
                let eased = here + (*at - here) * (1.0 - libm::expf(-SETTLE * step));
                let way = (eased - here) / step;
                (
                    eased,
                    toward,
                    0.0,
                    way.length().min(def.hover_mps),
                    0.0,
                    None,
                )
            }
        };
        if let Some(then) = then {
            flier.flight = then;
        }
        flier.accel = accel;
        flier.speed = speed;
        // Its bank eased toward the turn's, a heavy body slow to roll.
        let most = def.most_bank_deg.to_radians();
        let (mut bank, mut bank_rate) = (flier.bank, flier.bank_rate);
        spring(
            &mut bank,
            &mut bank_rate,
            bank_wanted.clamp(-most, most),
            def.bank_s,
            step,
        );
        (flier.bank, flier.bank_rate) = (bank, bank_rate);
        // Nose up as it brakes, flaring its wings against the air; the hover's own tilt
        // takes over from it.
        let flare_wanted = (-accel / def.brake_mps2.max(0.1)).clamp(0.0, 1.0)
            * def.flare_deg.to_radians()
            * (1.0 - flier.hovering);
        flier.flare += (flare_wanted - flier.flare) * ease(0.4, step);
        // Its heading along its way, swung no faster than a body so big swings.
        let level_was = Transform::IDENTITY.looking_to(was, up).rotation;
        let wanted = Transform::IDENTITY.looking_to(way, up).rotation;
        let angle = level_was.angle_between(wanted);
        let heading = level_was.slerp(wanted, (most_turn * step / angle.max(1e-4)).min(1.0));
        let forward = heading * Vec3::NEG_Z;
        flier.heading = Some(heading);
        let turned = level_angle(was, forward, up) / step;
        flier.turn_rate += (turned - flier.turn_rate) * ease(0.12, step);
        let pitch_was = libm::asinf(was.dot(up).clamp(-1.0, 1.0));
        let pitch_now = libm::asinf(forward.dot(up).clamp(-1.0, 1.0));
        flier.pitch_rate += ((pitch_now - pitch_was) / step - flier.pitch_rate) * ease(0.15, step);
        // A right turn (its way turning clockwise seen from above) rolls its right wing down.
        transform.translation = at + up * flier.heave;
        transform.rotation =
            heading * Quat::from_rotation_x(flier.flare) * Quat::from_rotation_z(flier.bank);
        // How hard it works its wings: to climb, to gather speed, and to stay up slow.
        let climb = speed * forward.dot(up);
        let slow = ((def.cruise_mps * 0.6 - speed) / (def.cruise_mps * 0.3)).clamp(0.0, 1.0);
        flier.effort = 0.4 + climb / 4.0 + accel / 2.0 + slow * f32::from(accel > -0.5);
        if flier.wants_breath && !flier.breathing {
            flier.windup -= step;
            if flier.windup <= 0.0 {
                flier.breathing = true;
            }
        }
    }
}

/// Each flier's clips weighed by what it is doing: beating its wings to climb, gather speed
/// or slow, gliding as it comes down, brakes or cruises, with a bout of beats every so often
/// in a cruise; hovering when slow; breathing as it flies or hovers; roaring. Its beats come
/// quicker the harder it works, and lift its body on each downstroke.
fn animate(
    time: Res<Time>,
    mut fliers: Query<&mut Flier>,
    mut flapping: Query<(&mut Flaps, &mut AnimationPlayer)>,
) {
    let step = time.delta_secs();
    for (mut flaps, mut player) in &mut flapping {
        let Ok(mut flier) = fliers.get_mut(flaps.flier) else {
            continue;
        };
        flier.roar_s = (flier.roar_s - step).max(0.0);
        let effort = flier.effort;
        flaps.bout_s += step;
        let beating = if effort > BEATS_FROM {
            true
        } else if effort < GLIDES_BELOW || flier.accel < -1.0 {
            false
        } else {
            // Cruising: gliding, with a bout of beats every so often.
            let [glide_s, beat_s] = CRUISE_BOUT_S;
            let bout = if flaps.beating { beat_s } else { glide_s };
            if flaps.bout_s > bout {
                !flaps.beating
            } else {
                flaps.beating
            }
        };
        if beating != flaps.beating {
            flaps.beating = beating;
            flaps.bout_s = 0.0;
        }
        let breath = flier.breathing || flier.wants_breath;
        let mut wanted = [0.0f32; 6];
        let [fly, glide, hover, breathe, fly_breathe, roar] = [0, 1, 2, 3, 4, 5];
        match flier.flight {
            Flight::Hover { .. } if flier.roar_s > 0.0 => wanted[roar] = 1.0,
            Flight::Hover { .. } if breath => wanted[breathe] = 1.0,
            Flight::Hover { .. } => wanted[hover] = 1.0,
            Flight::Path { .. } if breath => wanted[fly_breathe] = 1.0,
            Flight::Path { .. } => {
                let [low, high] = HOVERING_MPS;
                let t = ((high - flier.speed) / (high - low)).clamp(0.0, 1.0);
                let hovering = t * t * (3.0 - 2.0 * t);
                let beat = if beating { 1.0 } else { 0.0 };
                wanted[fly] = (1.0 - hovering) * beat;
                wanted[glide] = (1.0 - hovering) * (1.0 - beat);
                wanted[hover] = hovering;
            }
        }
        let into_hover = wanted[hover] + wanted[breathe] + wanted[roar] > 0.5;
        let share = ease(if into_hover { BLEND_HOVER_S } else { BLEND_S }, step);
        for (weight, wanted) in flaps.weights.iter_mut().zip(wanted) {
            *weight += (wanted - *weight) * share;
            if *weight < 0.002 && wanted == 0.0 {
                *weight = 0.0;
            }
        }
        let total: f32 = flaps.weights.iter().sum();
        let weights = flaps.weights.map(|w| w / total.max(1e-4));
        flier.hovering = weights[hover] + weights[breathe] + weights[roar];
        for (clip, weight) in flaps.clips.iter().zip(weights) {
            if weight <= 0.0 {
                if player.is_playing_animation(*clip) {
                    player.stop(*clip);
                }
                continue;
            }
            if !player.is_playing_animation(*clip) {
                player.play(*clip).repeat();
            }
            if let Some(active) = player.animation_mut(*clip) {
                active.set_weight(weight);
            }
        }
        // Harder strokes come quicker.
        let pace = 0.9 + 0.3 * effort.clamp(0.0, 1.0);
        if let Some(active) = player.animation_mut(flaps.clips[fly]) {
            active.set_speed(pace);
        }
        // Lifted on each downstroke (the clip's first 55%), by how much it beats.
        let phase = player
            .animation(flaps.clips[fly])
            .map_or(0.0, |active| active.seek_time() / FLY_CLIP_S);
        let s = stroke(phase.fract(), 0.55);
        flier.heave = HEAVE_M * weights[fly] * libm::sinf(std::f32::consts::TAU * (s - 0.05));
    }
}

/// How long the beating clip is, in seconds (`tools/creatures/dragon.py`).
const FLY_CLIP_S: f32 = 1.1;

/// A beat's phase from its clip's time, half of it the downstroke, which takes `down` of the
/// beat, as the clips are made.
fn stroke(t: f32, down: f32) -> f32 {
    if t < down {
        0.5 * t / down
    } else {
        0.5 + 0.5 * (t - down) / (1.0 - down)
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
pub(super) fn placed(
    world: &World,
    point: [f32; 3],
    base: bool,
) -> Result<Vec3, bevy::remote::BrpError> {
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
            Flier::new(kind, Flight::Hover { at, face }),
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
    let (neck, tail) = (def.neck.clone(), def.tail.clone());
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
                  names: Query<&Name>,
                  parents: Query<&ChildOf>,
                  transforms: Query<&Transform>| {
                if let Some(animator) = children
                    .iter_descendants(ready.entity)
                    .find(|&child| players.contains(child))
                {
                    commands.entity(animator).insert((
                        AnimationGraphHandle(graph.clone()),
                        Flaps {
                            flier,
                            clips,
                            weights: [0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
                            beating: false,
                            bout_s: 0.0,
                        },
                    ));
                }
                let neck = chain(
                    ready.entity,
                    &neck,
                    &children,
                    &names,
                    &parents,
                    &transforms,
                );
                let tail = chain(
                    ready.entity,
                    &tail,
                    &children,
                    &names,
                    &parents,
                    &transforms,
                );
                commands.entity(ready.entity).insert(Spine {
                    flier,
                    tail_turn: vec![Vec2::ZERO; tail.len()],
                    neck,
                    tail,
                    neck_turn: Vec2::ZERO,
                    neck_rate: Vec2::ZERO,
                });
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

/// Plans a flier's course from where it is through points, to hover at the last facing a
/// point: on from the way it heads now, or the way it faces hovering, at the speed it has.
fn plan_from(
    world: &World,
    entity: Entity,
    through: &[Vec3],
    cruise: Option<f32>,
) -> Option<(Plan, f32)> {
    let flier = world.get::<Flier>(entity)?;
    let here = world.get::<Transform>(entity)?.translation;
    let up = here.normalize_or(Vec3::Y);
    let here = here - up * flier.heave;
    let def = &world.resource::<Kinds>().defs[flier.kind];
    let way = match &flier.flight {
        Flight::Path { plan, seconds, .. } => plan.at(*seconds).way,
        Flight::Hover { face, .. } => (*face - here).reject_from(up),
    };
    let cruise = cruise.unwrap_or(def.cruise_mps);
    let planned = plan::plan(here, way, flier.speed, through, cruise, &def.wings(), up);
    Some((planned, cruise))
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

/// Flies a flier along a course through points from where it is, at its cruising speed or
/// the one given, to hover at the last facing a point (on along its way if not given).
fn path(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Path = parse_some(params)?;
    let entity = flier_of(world, asked.kind.as_deref())?;
    let mut through = Vec::new();
    for point in &asked.through {
        through.push(placed(world, *point, asked.base)?);
    }
    let Some(&last) = through.last() else {
        return Err(refused("a path needs a point"));
    };
    let before = through
        .len()
        .checked_sub(2)
        .map(|i| through[i])
        .or_else(|| world.get::<Transform>(entity).map(|t| t.translation))
        .unwrap_or(last);
    let face = match asked.facing {
        Some(facing) => placed(world, facing, asked.base)?,
        None => last + (last - before),
    };
    let Some((planned, speed)) = plan_from(world, entity, &through, asked.speed) else {
        return Err(refused("there is no flier"));
    };
    let (metres, seconds) = (planned.metres(), planned.seconds());
    if let Some(mut flier) = world.get_mut::<Flier>(entity) {
        flier.flight = Flight::Path {
            plan: planned,
            seconds: 0.0,
            then_face: face,
        };
    }
    info!("Flight: a course of {metres:.0} m at up to {speed:.0} m/s, {seconds:.1} s.");
    Ok(json!({ "done": true, "metres": metres, "seconds": seconds }))
}

#[derive(Deserialize)]
struct Hover {
    kind: Option<String>,
    at: [f32; 3],
    facing: [f32; 3],
    #[serde(default)]
    base: bool,
}

/// Holds a flier hovering at a point facing another: easing there if it is slow, else
/// flying there and braking into the hover, as a flier must.
fn hover(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Hover = parse_some(params)?;
    let entity = flier_of(world, asked.kind.as_deref())?;
    let (at, face) = (
        placed(world, asked.at, asked.base)?,
        placed(world, asked.facing, asked.base)?,
    );
    let flying = world.get::<Flier>(entity).is_some_and(|flier| {
        let def = &world.resource::<Kinds>().defs[flier.kind];
        matches!(flier.flight, Flight::Path { .. }) && flier.speed > def.hover_mps + 0.5
    });
    let flight = if flying {
        let Some((planned, _)) = plan_from(world, entity, &[at], None) else {
            return Err(refused("there is no flier"));
        };
        Flight::Path {
            plan: planned,
            seconds: 0.0,
            then_face: face,
        }
    } else {
        Flight::Hover { at, face }
    };
    if let Some(mut flier) = world.get_mut::<Flier>(entity) {
        flier.flight = flight;
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

/// Starts or stops a flier breathing fire, aimed at a point if given: it rears back first,
/// for its kind's wind-up, and the breath comes after.
fn breath(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Breath = parse_some(params)?;
    let entity = flier_of(world, asked.kind.as_deref())?;
    let aim = asked
        .at
        .map(|at| placed(world, at, asked.base))
        .transpose()?;
    let windup = world.get::<Flier>(entity).map_or(0.0, |flier| {
        world.resource::<Kinds>().defs[flier.kind].windup_s
    });
    if let Some(mut flier) = world.get_mut::<Flier>(entity) {
        if asked.on && !flier.wants_breath {
            flier.windup = windup;
            flier.breathing = windup <= 0.0;
        }
        if !asked.on {
            flier.breathing = false;
        }
        flier.wants_breath = asked.on;
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
mod tests;
