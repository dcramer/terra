//! The fight's record (`--fight-log <file.jsonl>`, D69): every counted frame of a run, one
//! line of JSON, so a tool judges the fight by numbers rather than by eye
//! (`tools/sandbox/duel.ts`). On each: every fighter's feet and facing, its move, how far
//! into it and its phase, whether a press may cut it short now, its guard, hit-stop, reeling
//! and being down, its pushbox, whether the other stands in its blow's reach (`terra_motion`'s
//! `Move::in_reach`), and where the balls of its feet are and how high over its feet; and
//! what happened on it: the script's step (`script`), each move the player asked for and
//! what came of it, and each contact, with the hit-stop the rules give it. Places and
//! directions are in the start's terms, metres east, up and south of it, as `--at`, `--cine`
//! and the staged log take them, so a camera can be set on the fight from the record (or
//! from the player's feet on the first frame, with no start); the first line gives up too.
//! The camera drawing the frame is in it as well (where it is, which way it looks and its
//! up, its vertical field of view and aspect), so a tool measures how the fight is framed.
//! The shown places, between ticks, as a person sees them. A player with no sword drawn is
//! written too outside a duel, as a walker (no move), so a walk is judged by the same numbers
//! (`tools/controller/yard.ts`, the controller's filmed course); each player says how far
//! below its body its figure is shown after a step up (`below`, `terra_walker::stepping`),
//! whether it stands on the ground and its speed.

use super::{Fighter, Set};
use crate::staged::Staged;
use bevy::prelude::*;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use terra_walker::bodies::Body;
use terra_walker::character;
use terra_walker::stepping::Stepping;
use terra_walker::{Motion, Player, Stamina};
use terra_world::planet::MainCamera;
use terra_world::shell::{Options, ScriptFrame};
use terra_world::start::Start;

/// The balls of the feet, where a planted foot meets the ground, as the gait check reads them.
const BALLS: [&str; 2] = ["ball_l", "ball_r"];

/// What happened this frame, put in by the systems it happened in.
#[derive(Resource, Default)]
pub struct Said {
    pub step: Option<String>,
    pub asked: Vec<Value>,
    pub contacts: Vec<Value>,
}

/// A move the player asked for, and what came of it (`stamina::ask`'s answer): started,
/// waiting for its window, or refused too tired.
pub fn asked(said: Option<&mut Said>, name: &str, got: Option<bool>) {
    let Some(said) = said else { return };
    let came = match got {
        Some(true) => "started",
        Some(false) => "waits",
        None => "too tired",
    };
    said.asked.push(json!({ "move": name, "got": came }));
}

#[derive(Resource)]
struct Out {
    file: BufWriter<File>,
    origin: Option<Vec3>,
    balls: HashMap<Entity, [Entity; 2]>,
    /// Whether a player with no sword drawn is written as a walker: not in a duel, whose
    /// record is the fight's alone (its checks would read the walk to the draw as one).
    walkers: bool,
}

pub fn plugin(app: &mut App) {
    let Some(options) = app.world().get_resource::<Options>() else {
        return;
    };
    let walkers = options.duel.is_none();
    let Some(path) = options.fight_log.clone() else {
        return;
    };
    let file = File::create(&path).unwrap_or_else(|problem| {
        panic!("--fight-log: could not write {}: {problem}", path.display())
    });
    app.insert_resource(Out {
        file: BufWriter::new(file),
        origin: None,
        balls: HashMap::new(),
        walkers,
    })
    .init_resource::<Said>()
    .add_systems(Last, write);
}

/// Metres to the millimetre, so a line stays short.
fn mm(v: Vec3) -> [f64; 3] {
    [r(v.x), r(v.y), r(v.z)]
}

/// A number to three places, written as it reads rather than as the nearest `f32`.
fn r(x: f32) -> f64 {
    (f64::from(x) * 1000.0).round() / 1000.0
}

type Them<'a> = (
    Entity,
    &'a GlobalTransform,
    &'a Fighter,
    Has<Player>,
    Option<&'a Body>,
    Option<&'a Motion>,
    Option<&'a Staged>,
    Option<&'a Stamina>,
    Option<&'a terra_walker::feet::Kept>,
);

/// A player walking with no sword drawn: not a fighter, written as a walker.
type Walkers<'a> = (
    Entity,
    &'a GlobalTransform,
    Option<&'a Motion>,
    Option<&'a Stepping>,
    Option<&'a terra_walker::feet::Kept>,
);

/// What a player's walk adds to its line: its figure's lag after a step up, whether it
/// stands on the ground, and its speed, m/s.
fn walking(line: &mut Value, motion: Option<&Motion>, stepping: Option<&Stepping>) {
    line["below"] = json!(r(stepping.map_or(0.0, |s| s.below)));
    if let Some(motion) = motion {
        line["on_ground"] = json!(motion.on_ground);
        line["speed"] = json!(r(motion.velocity.length()));
    }
}

fn write(
    frame: Option<Res<ScriptFrame>>,
    set: Res<Set>,
    mut out: ResMut<Out>,
    mut said: ResMut<Said>,
    fighters: Query<Them<'_>>,
    walkers: Query<Walkers<'_>, (With<Player>, Without<Fighter>)>,
    steps: Query<&Stepping>,
    bones: Query<&GlobalTransform>,
    children: Query<&Children>,
    names: Query<&Name>,
    start: Option<Res<Start>>,
    camera: Option<Single<(&GlobalTransform, &Projection), With<MainCamera>>>,
) {
    let Some(frame) = frame.filter(|frame| frame.is_changed()) else {
        return;
    };
    let set = &set.0;
    let seen: Vec<_> = fighters.iter().collect();
    // Before the sword is drawn there is no fighter, only the script's steps and a walker.
    let player = seen.iter().find(|each| each.3).map(|each| each.1);
    let walk = out.walkers;
    let player = player.or(walkers.iter().find(|_| walk).map(|each| each.1));
    let up = player.map_or(Vec3::Y, |player| player.translation().normalize());
    let first = out.origin.is_none() && player.is_some();
    if let Some(player) = player.filter(|_| first) {
        out.origin = Some(player.translation());
    }
    let origin = out.origin.unwrap_or_default();
    let terms = start
        .map_or(Transform::from_translation(origin), |start| start.0)
        .compute_affine()
        .inverse();
    let local = |at: Vec3| mm(terms.transform_point3(at));
    let mut bodies = Vec::new();
    for &(entity, place, fighter, is_player, body, motion, staged, pool, kept) in &seen {
        let feet = place.translation();
        let facing = motion
            .map(|motion| motion.facing)
            .or(staged.map(Staged::facing))
            .unwrap_or(Vec3::Z);
        // The other side's nearest: whether it stands in this one's blow's reach.
        let foe = seen
            .iter()
            .filter(|other| other.3 != is_player)
            .map(|other| (other.1.translation() - feet).reject_from(up))
            .min_by(|a, b| a.length().total_cmp(&b.length()));
        let now = fighter.runner.now(set);
        let t = fighter.runner.t();
        let reach = match (now, foe) {
            (Some(each), Some(to)) => {
                let off = facing.reject_from(up).angle_between(to).to_degrees();
                each.in_reach(to.length(), if off.is_nan() { 0.0 } else { off })
            }
            _ => false,
        };
        let balls = balls_of(
            &mut out, entity, feet, up, &local, &children, &names, &bones,
        );
        bodies.push(json!({
            "who": if is_player { "you" } else { "partner" },
            "feet": local(feet),
            "facing": mm(terms.transform_vector3(facing)),
            "move": fighter.runner.name(),
            "t": r(t),
            "phase": now.map(|each| format!("{:?}", each.phase(t))),
            "cancels": now.is_some_and(|each| each.cancels(t)),
            "reach": reach,
            "guard": fighter.guard_s.is_some(),
            "stop_s": r(fighter.stop_s),
            "stagger_s": r(fighter.stagger_s),
            "down": fighter.down_s.is_some(),
            "free": fighter.free(),
            "radius": body.map(|body| r(body.radius)),
            "health": r(fighter.health),
            "stamina": pool.map(|pool| r(pool.left)),
            "balls": balls,
            "kept": kept.map(|kept| kept.states()),
            "lock": kept.map(|kept| kept.locks().map(|[curve, alpha]| [r(curve), r(alpha)])),
            "why": kept.map(|kept| kept.whys()),
            "gated": kept.and_then(|kept| kept.gated()),
        }));
        if is_player {
            let last = bodies.last_mut().expect("just pushed");
            walking(last, motion, steps.get(entity).ok());
        }
    }
    for (entity, place, motion, stepping, kept) in walkers.iter().filter(|_| walk) {
        let feet = place.translation();
        let facing = motion.map_or(Vec3::Z, |motion| motion.facing);
        // A walker's balls over its figure's root as shown, so a step's smoothing is no dip.
        let root = feet - up * stepping.map_or(0.0, |s| s.below);
        let balls = balls_of(
            &mut out, entity, root, up, &local, &children, &names, &bones,
        );
        let mut line = json!({
            "who": "you",
            "feet": local(feet),
            "facing": mm(terms.transform_vector3(facing)),
            "move": null,
            "t": 0.0,
            "phase": null,
            "cancels": false,
            "reach": false,
            "guard": false,
            "stop_s": 0.0,
            "stagger_s": 0.0,
            "down": false,
            "free": true,
            "radius": null,
            "balls": balls,
            "kept": kept.map(|kept| kept.states()),
            "lock": kept.map(|kept| kept.locks().map(|[curve, alpha]| [r(curve), r(alpha)])),
            "why": kept.map(|kept| kept.whys()),
            "gated": kept.and_then(|kept| kept.gated()),
        });
        walking(&mut line, motion, stepping);
        bodies.push(line);
    }
    let mut line = json!({
        "frame": frame.0,
        "fighters": bodies,
        "step": said.step.take(),
        "asked": std::mem::take(&mut said.asked),
        "contacts": std::mem::take(&mut said.contacts),
    });
    if let Some((place, Projection::Perspective(lens))) = camera.as_deref().map(|c| (c.0, c.1)) {
        line["camera"] = json!({
            "at": local(place.translation()),
            "looking": mm(terms.transform_vector3(*place.forward())),
            "up": mm(terms.transform_vector3(*place.up())),
            "vfov": r(lens.fov.to_degrees()),
            "aspect": r(lens.aspect_ratio),
        });
    }
    if first {
        line["up"] = json!(mm(terms.transform_vector3(up)));
    }
    let written = writeln!(out.file, "{line}").and_then(|()| out.file.flush());
    if let Err(problem) = written {
        warn!("Melee: the fight's record could not be written: {problem}");
    }
}

/// Where the balls of a body's feet are, in the start's terms, and how high over `feet`, the
/// bones found once a body.
#[allow(clippy::too_many_arguments)]
fn balls_of(
    out: &mut Out,
    entity: Entity,
    feet: Vec3,
    up: Vec3,
    local: &dyn Fn(Vec3) -> [f64; 3],
    children: &Query<&Children>,
    names: &Query<&Name>,
    bones: &Query<&GlobalTransform>,
) -> Vec<Value> {
    if let std::collections::hash_map::Entry::Vacant(slot) = out.balls.entry(entity)
        && let [Some(l), Some(r)] = BALLS.map(|name| character::find(entity, name, children, names))
    {
        slot.insert([l, r]);
    }
    out.balls
        .get(&entity)
        .map(|pair| {
            pair.iter()
                .filter_map(|bone| bones.get(*bone).ok())
                .map(|bone| {
                    let at = bone.translation();
                    json!({ "at": local(at), "up": r((at - feet).dot(up)) })
                })
                .collect()
        })
        .unwrap_or_default()
}
