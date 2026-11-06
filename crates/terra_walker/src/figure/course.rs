//! The animation course (D631; its scorecard `cargo xtask anim`, tools/anim): the man's figure (male.glb, the game's own
//! clips, blends, sync groups, stride warping, foot lock and lift) posed headless with no GPU,
//! driven through every transition the game's figure makes, and measured by numbers alone
//! (`measure`): idle to jog and sprint and back, the speed changes between them, turns moving
//! and in place, a jump's take-off and landing standing and jogging, and into and out of every
//! move of the sword set from its guard. The body is moved as the mover moves a man (its
//! `movers.ron` accel, decel, turn and jump speed) over flat ground, its figure posed by the
//! game's systems as in play; each transition's frames are scored against the clips' own steady
//! state. It prints `scorecard\t<id>\t<value>` a criterion (tools/anim/criteria.json) and
//! `anim\t...` a transition, which the scorecard keeps in its log.

mod measure;

use super::*;
use bevy::animation::AnimationPlugin;
use measure::Frame;

const FPS: f32 = 60.0;
/// The joints a pop is read on: the body's, not its fingers'.
const JOINTS: [&str; 22] = [
    "pelvis",
    "spine_01",
    "spine_02",
    "spine_03",
    "neck_01",
    "Head",
    "clavicle_l",
    "upperarm_l",
    "lowerarm_l",
    "hand_l",
    "clavicle_r",
    "upperarm_r",
    "lowerarm_r",
    "hand_r",
    "thigh_l",
    "calf_l",
    "foot_l",
    "ball_l",
    "thigh_r",
    "calf_r",
    "foot_r",
    "ball_r",
];
/// How long after a transition starts its frames are its own, s: a blend (0.18 s), its foot's
/// settle and a step by the lock's rule.
const WINDOW_S: f32 = 0.9;
/// A ball moving more than this in a frame is moving (`tools/controller/transitions.ts`), m.
const MOVED_M: f32 = 0.005;
/// The body stands once slower than this, m/s.
const STOPPED_MPS: f32 = 0.05;

/// One stretch of the course: what the body is asked to do, for how long, and the transition
/// it starts, if it is one that is scored (its name and family).
#[derive(Clone, Default)]
struct Step {
    tag: Option<(String, &'static str)>,
    secs: f32,
    speed: f32,
    yaw: f32,
    jump: bool,
    act: Option<Act>,
    /// Stops what a layer asked (the fight's end).
    end_act: bool,
    /// The fight's feet: read in the body's frame (`feet::Kept::stance`).
    stance: bool,
}

fn go(secs: f32, speed: f32, yaw: f32) -> Step {
    Step {
        secs,
        speed,
        yaw,
        ..Default::default()
    }
}

fn tagged(mut step: Step, name: &str, family: &'static str) -> Step {
    step.tag = Some((name.to_string(), family));
    step
}

/// The course: every transition the game's figure makes, each from a steady state.
fn course(set: &terra_motion::moves::MoveSet) -> Vec<Step> {
    use terra_motion::gaits::{JOG, SPRINT};
    let mut s = vec![
        go(2.0, 0.0, 0.0),
        tagged(go(2.5, JOG, 0.0), "start.jog", "start"),
        tagged(go(2.0, 0.0, 0.0), "stop.jog", "stop"),
        tagged(go(2.5, SPRINT, 0.0), "start.sprint", "start"),
        tagged(go(2.0, 0.0, 0.0), "stop.sprint", "stop"),
        go(1.5, JOG, 0.0),
        tagged(go(2.0, SPRINT, 0.0), "jog.sprint", "gait"),
        // A steady gait, measured over the whole stretch: a loop that skates where no
        // transition hides it (the Sprint_Loop's, char-faults j3).
        tagged(go(3.0, SPRINT, 0.0), "steady.sprint", "steady"),
        tagged(go(2.0, JOG, 0.0), "sprint.jog", "gait"),
        tagged(go(3.0, JOG, 0.0), "steady.jog", "steady"),
        tagged(go(1.5, JOG, 90.0), "turn.jog.90", "turn"),
        tagged(go(1.5, JOG, 270.0), "turn.jog.180", "turn"),
        // A stop half a stride on from the one above: a stop's fade by its stride phase.
        go(0.29, JOG, 270.0),
        tagged(go(2.0, 0.0, 270.0), "stop.jog.half", "stop"),
        tagged(go(1.5, 0.0, 360.0), "turn.idle.90", "turn"),
        tagged(go(1.5, 0.0, 540.0), "turn.idle.180", "turn"),
        tagged(
            Step {
                jump: true,
                ..go(2.5, 0.0, 540.0)
            },
            "jump.idle",
            "jump",
        ),
        go(1.5, JOG, 540.0),
        tagged(
            Step {
                jump: true,
                ..go(2.5, JOG, 540.0)
            },
            "jump.jog",
            "jump",
        ),
        tagged(go(2.0, 0.0, 540.0), "stop.jog.landed", "stop"),
    ];
    let guard = || Act {
        blend: Duration::from_millis(180),
        ..Act::looping(&set.guard, 1.0)
    };
    s.push(tagged(
        Step {
            act: Some(guard()),
            stance: true,
            ..go(1.5, 0.0, 540.0)
        },
        "into.guard",
        "fight",
    ));
    let mut names: Vec<_> = set.moves.keys().collect();
    names.sort();
    for name in names {
        let each = &set.moves[name];
        let speed = each.speed.max(0.01);
        let mut act = Act::once(&each.clip, each.from, speed);
        act.blend = Duration::from_millis(110);
        let secs = ((each.done - each.from) / speed).clamp(0.2, 4.0);
        s.push(tagged(
            Step {
                act: Some(act),
                stance: true,
                ..go(secs, 0.0, 540.0)
            },
            &format!("into.{name}"),
            "fight",
        ));
        s.push(tagged(
            Step {
                act: Some(guard()),
                stance: true,
                ..go(1.2, 0.0, 540.0)
            },
            &format!("out.{name}"),
            "fight",
        ));
    }
    s.push(tagged(
        Step {
            end_act: true,
            ..go(2.0, 0.0, 540.0)
        },
        "out.guard",
        "fight",
    ));
    s
}

/// The body as the mover would move a man: speed and facing eased toward what is asked, a jump
/// rising and falling under gravity.
#[derive(Default)]
struct Mover {
    at: Vec3,
    speed: f32,
    yaw: f32,
    height: f32,
    rising: f32,
    airborne: f32,
}

impl Mover {
    fn tick(&mut self, step: &Step, kind: &terra_motion::mover::Kind, dt: f32) {
        let aloft = self.height > 0.0 || self.rising > 0.0;
        if !aloft {
            let rate = if step.speed > self.speed {
                kind.accel
            } else {
                kind.decel
            };
            let d = step.speed - self.speed;
            self.speed += d.clamp(-rate * dt, rate * dt);
            let turn = step.yaw - self.yaw;
            let most = kind.turn_deg_s * dt;
            self.yaw += turn.clamp(-most, most);
        }
        let facing = self.facing();
        self.at += facing * self.speed * dt;
        if aloft {
            self.rising -= 9.81 * dt;
            self.height += self.rising * dt;
            self.airborne += dt;
            if self.height <= 0.0 {
                self.height = 0.0;
                self.rising = 0.0;
                self.airborne = 0.0;
            }
        }
    }

    fn facing(&self) -> Vec3 {
        let yaw = self.yaw.to_radians();
        Vec3::new(libm::sinf(yaw), 0.0, -libm::cosf(yaw))
    }
}

/// A headless app with the figure's own plugins: its clips, the foot lock, the pose layers and
/// the lift, loading the man's file.
fn app() -> App {
    let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin {
            file_path: assets.to_string_lossy().into_owned(),
            ..default()
        },
        ImagePlugin::default(),
        bevy::mesh::MeshPlugin,
        bevy::scene::ScenePlugin,
        bevy::world_serialization::WorldSerializationPlugin,
        TransformPlugin,
        AnimationPlugin,
        terra_world::gltf_gate::GatedGltfPlugin::default(),
    ));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_secs_f32(1.0 / FPS),
    ));
    app.init_resource::<Loading>();
    plugin(&mut app);
    crate::feet::plugin(&mut app);
    crate::pose::plugin(&mut app);
    crate::lift::plugin(&mut app);
    app.finish();
    app.cleanup();
    app
}

/// The joints by name under `root`.
fn joints(world: &mut World, root: Entity) -> HashMap<String, Entity> {
    let mut q = world.query::<(Entity, &Name)>();
    let mut children = world.query::<&Children>();
    let mut out = HashMap::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Ok((e, name)) = q.get(world, e) {
            out.insert(name.as_str().to_string(), e);
        }
        if let Ok(c) = children.get(world, e) {
            stack.extend(c.iter());
        }
    }
    out
}

/// The leg clips playing (`whole`, `under` and one-armed nodes; not the arms' holds and swings)
/// and each one's weight and speed.
fn legs(moves: &Moves, player: &AnimationPlayer) -> Vec<(String, f32, f32, f32)> {
    let name_of = |node: AnimationNodeIndex| {
        moves.clips.iter().find_map(|(name, n)| {
            let all = n
                .whole
                .iter()
                .chain(&n.under)
                .chain(n.under_one.iter().flatten());
            all.copied().any(|i| i == node).then(|| name.clone())
        })
    };
    player
        .playing_animations()
        .filter_map(|(node, a)| {
            let name = name_of(*node)?;
            (a.weight() > 1e-3).then(|| (name, a.weight(), a.speed(), a.seek_time()))
        })
        .collect()
}

/// Drives the man through the course and records a frame a tick.
type Tag = (String, &'static str, usize, usize);

fn run() -> (Vec<Frame>, Vec<Tag>, Vec<(Vec3, Vec3)>) {
    let mut app = app();
    let text =
        std::fs::read_to_string(terra_world::data::assets().join("data/moves/sword.shielded.ron"))
            .expect("the sword set's file");
    let set = terra_motion::moves::MoveSet::read(&text).expect("the sword set reads");
    let steps = course(&set);
    let kind = crate::movement::man();
    let body = app
        .world_mut()
        .spawn((
            Player,
            Motion::facing(Vec3::NEG_Z),
            Stance::default(),
            Acting::default(),
            on_world(Vec3::ZERO, Vec3::NEG_Z).0,
        ))
        .id();
    let start = std::time::Instant::now();
    let animator = loop {
        app.update();
        let mut q = app.world_mut().query_filtered::<Entity, With<Animated>>();
        if let Some(e) = q.iter(app.world()).next() {
            break e;
        }
        assert!(
            start.elapsed().as_secs() < 120,
            "the man's figure never stood: male.glb did not load"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    let named = joints(app.world_mut(), body);
    let find = |n: &str| *named.get(n).unwrap_or_else(|| panic!("the man has no {n}"));
    let turns: Vec<Entity> = JOINTS.iter().map(|n| find(n)).collect();
    let balls = [find("ball_l"), find("ball_r")];
    let dt = 1.0 / FPS;
    let mut mover = Mover::default();
    let mut frames = Vec::new();
    let mut places = Vec::new();
    let mut tags = Vec::new();
    for step in &steps {
        let first = frames.len();
        if let Some(act) = &step.act {
            app.world_mut()
                .get_mut::<Acting>(body)
                .unwrap()
                .play(act.clone());
        }
        if step.end_act {
            app.world_mut().get_mut::<Acting>(body).unwrap().clip = None;
        }
        if step.jump && mover.height <= 0.0 {
            mover.rising = kind.jump_speed;
            mover.height = 1e-4;
        }
        let n = (step.secs * FPS).round() as usize;
        for _ in 0..n {
            mover.tick(step, &kind, dt);
            let facing = mover.facing();
            let (place, tilt) = on_world(mover.at + Vec3::Y * mover.height, facing);
            let world = app.world_mut();
            {
                let mut motion = world.get_mut::<Motion>(body).unwrap();
                motion.velocity = tilt * facing * mover.speed;
                motion.rising = mover.rising;
                motion.on_ground = mover.height <= 0.0;
                motion.airborne = mover.airborne;
                motion.facing = tilt * facing;
            }
            *world.get_mut::<Transform>(body).unwrap() = place;
            if step.stance {
                world.get_mut::<crate::feet::Kept>(body).unwrap().stance = true;
            }
            app.update();
            frames.push(record(&mut app, body, animator, &turns, balls, &mover));
            places.push((mover.at, facing));
        }
        if let Some((name, family)) = &step.tag {
            tags.push((name.clone(), *family, first, frames.len()));
        }
    }
    (frames, tags, places)
}

/// The radius of the world the course stands on, m (the world's tests', `terra_world`'s
/// climate): the game's up is a body's place from the planet's centre at the origin (the foot
/// lock's, `feet.rs`), so a course on a plane through the origin had its up lying along the
/// ground once the body left the origin, and the lock gated every plant as aloft (anim-blend
/// j4: rests 45 cm under the root, the feet free through the whole course).
const WORLD_M: f32 = 2000.0;

/// The mover's place on its flat course (`at`, up `Y`) and facing as the game has them on the
/// world: on the sphere of `WORLD_M` above it, turned so its up is that place's; and the turn
/// from the course's frame to the world's there.
fn on_world(at: Vec3, facing: Vec3) -> (Transform, Quat) {
    let up = Vec3::new(at.x, WORLD_M, at.z).normalize();
    let tilt = Quat::from_rotation_arc(Vec3::Y, up);
    let place = Transform::from_translation(up * (WORLD_M + at.y))
        .with_rotation(tilt * Transform::IDENTITY.looking_to(facing, Vec3::Y).rotation);
    (place, tilt)
}

/// A point of the world (`on_world`) back in the course's flat frame, the body at `at` there.
fn off_world(point: Vec3, at: Vec3) -> Vec3 {
    let (place, tilt) = on_world(at, Vec3::NEG_Z);
    at + tilt.inverse() * (point - place.translation)
}

/// The figure as posed this frame.
fn record(
    app: &mut App,
    body: Entity,
    animator: Entity,
    turns: &[Entity],
    balls: [Entity; 2],
    mover: &Mover,
) -> Frame {
    let world = app.world();
    let moves = world.resource::<Moves>();
    let player = world.get::<AnimationPlayer>(animator).unwrap();
    let animated = world.get::<Animated>(animator).unwrap();
    let clip = animated
        .playing
        .as_ref()
        .map_or(String::new(), |(c, ..)| c.clone());
    let playing = legs(moves, player);
    let seek = playing
        .iter()
        .filter(|(n, ..)| *n == clip)
        .map(|p| p.3)
        .fold(0.0, f32::max);
    let solo = match playing.as_slice() {
        [(name, w, ..)] if *w > 0.999 => Some(name.clone()),
        _ => None,
    };
    let size = crate::pace::size(
        world.get::<GlobalTransform>(animator).unwrap(),
        world.get::<Transform>(animator).unwrap(),
    );
    let speed = mover.speed;
    let (mut pushed, mut weight) = (0.0, 0.0);
    for (name, w, rate, _) in &playing {
        let pace = moves.paces.get(name).copied().unwrap_or(0.0);
        let warp = if *name == clip && pace > 0.0 {
            crate::pace::gait(speed, pace, size).warp
        } else {
            1.0
        };
        pushed += w * pace * size * rate.abs() * warp;
        weight += w;
    }
    let kept = world.get::<crate::feet::Kept>(body).unwrap();
    if let Some(why) = kept.gated() {
        panic!(
            "the man's feet cannot be kept playing {clip} ({why}): the course's slide reads \
             contact from the clips' baked lock curves (D455, D631), so every clip it plays \
             needs its markers baked (the character skill's export); do not read contact from \
             the ball's height instead (anim-blend j3)"
        );
    }
    let lock = kept.locks().map(|[curve, _]| curve);
    Frame {
        t: world.resource::<Time>().elapsed_secs(),
        clip,
        seek,
        solo,
        turns: turns
            .iter()
            .map(|e| world.get::<Transform>(*e).unwrap().rotation)
            .collect(),
        balls: balls.map(|e| {
            let ball = world.get::<GlobalTransform>(e).unwrap().translation();
            off_world(ball, mover.at + Vec3::Y * mover.height)
        }),
        speed,
        stride: if weight > 0.0 { pushed / weight } else { 0.0 },
        aloft: mover.height > 0.0,
        lock,
    }
}

/// Where each ball stands in the body's level frame (right, ahead), the body at `at` facing
/// `facing`.
fn stance_of(frame: &Frame, at: Vec3, facing: Vec3) -> [Vec2; 2] {
    let right = facing.cross(Vec3::Y);
    frame
        .balls
        .map(|b| Vec2::new((b - at).dot(right), (b - at).dot(facing)))
}

/// A transition's frames for `cargo xtask anim --trace <name>` (the log keeps them all), over
/// its whole step: a line a frame of its time in, the clip and how far into it, the feet's lock
/// curves, the body's speed, how far each ball moved over the ground that frame (the stop's
/// settle and a slide read from it) and its three worst joints' pops (`measure::pop` of that
/// frame alone), so a row's worst is read as a curve without a rebuild to print one.
fn trace(
    name: &str,
    frames: &[Frame],
    kicks: &[Vec<f32>],
    steady: &std::collections::HashMap<String, Vec<f32>>,
    clips: &[&str],
    (from, to): (usize, usize),
) {
    for i in from..to {
        let mut joints: Vec<(f32, usize)> = (0..JOINTS.len())
            .map(|j| {
                let own = clips
                    .iter()
                    .filter_map(|c| steady.get(*c).map(|s| s[j]))
                    .fold(measure::FLOOR_DEG_S, f32::max);
                (kicks[i][j] / own, j)
            })
            .collect();
        joints.sort_by(|a, b| b.0.total_cmp(&a.0));
        let worst: Vec<String> = joints[..3]
            .iter()
            .map(|(p, j)| format!("{} {p:.2}", JOINTS[*j]))
            .collect();
        let f = &frames[i];
        let moved = (0..2).map(|b| {
            let d = f.balls[b] - frames[i.saturating_sub(1)].balls[b];
            Vec2::new(d.x, d.z).length() * 100.0
        });
        let moved: Vec<String> = moved.map(|cm| format!("{cm:.1}")).collect();
        println!(
            "trace\t{name}\t+{:.3}\t{} {:.3}{}\tlock {:.2} {:.2}\tspeed {:.2}\tballs {} cm\t{}",
            (i - from) as f32 / FPS,
            f.clip,
            f.seek,
            if f.solo.is_some() { "" } else { " (blend)" },
            f.lock[0],
            f.lock[1],
            f.speed,
            moved.join(" "),
            worst.join(", "),
        );
    }
}

/// The man driven through every transition the game's figure makes, each scored by its worst
/// pop, slide and glide against its clips' own steady state, a stop by where its feet end
/// against the idle's stance and how long after the body they settle, and each loop's seam;
/// printed a criterion a line for `cargo xtask anim` (tools/anim/scorecard.ts).
#[test]
#[ignore = "the animation course: cargo xtask anim runs it (seconds in release)"]
fn the_animation_course_measures_the_mans_transitions() {
    let (frames, tags, ats) = run();
    let dt = 1.0 / FPS;
    let kicks = measure::kicks(&frames, dt);
    let steady = measure::steady(&frames, &kicks);
    let mut worst: HashMap<String, f32> = HashMap::new();
    let mut put = |id: String, v: f32| {
        let w = worst.entry(id).or_insert(0.0);
        *w = w.max(v);
    };
    // The idle's own stance, at the end of the first stretch, the body still.
    let still = (2.0 * FPS) as usize - 1;
    let idle = stance_of(&frames[still], ats[still].0, ats[still].1);
    // Each ball's height there, standing (the slide's ground).
    let stand = frames[still].balls.map(|b| b.y);
    for (name, family, from, end) in &tags {
        // A steady stretch is its whole; a transition its first WINDOW_S.
        let to = match *family {
            "steady" => *end,
            _ => (*from + (WINDOW_S * FPS) as usize).min(*end),
        };
        let before = frames[from.saturating_sub(1)].clip.as_str();
        let after = frames[to - 1].clip.as_str();
        let (pop, joint, at_frame) = measure::pop(&kicks, &steady, &[before, after], (*from, to));
        let slide = measure::slide(&frames, stand, (*from, to));
        let glide = measure::glide(&frames, (*from, to));
        put(format!("pop.{family}"), pop);
        put(format!("slide.{family}"), slide);
        if *family != "fight" {
            // A start's is its root's speed against its clip's own (a distance-matched start
            // plays its clip by the body's travel, Lyra's).
            let id = match *family {
                "start" => "start.root".to_string(),
                _ => format!("glide.{family}"),
            };
            put(id, glide);
        }
        let mut extra = String::new();
        if *family == "stop" {
            // Where the feet end against the idle's stance, and how long after the body
            // stood still they still move.
            let last = *end - 1;
            let ends = stance_of(&frames[last], ats[last].0, ats[last].1);
            let off = ends
                .iter()
                .zip(&idle)
                .map(|(a, b)| (*a - *b).length())
                .fold(0.0, f32::max);
            let stood = (*from..*end)
                .find(|&i| frames[i].speed < STOPPED_MPS)
                .unwrap_or(*end);
            let moved = (stood.max(*from + 1)..*end)
                .rfind(|&i| {
                    (0..2).any(|f| {
                        let d = frames[i].balls[f] - frames[i - 1].balls[f];
                        Vec2::new(d.x, d.z).length() > MOVED_M
                    })
                })
                .map_or(0.0, |i| (i - stood) as f32 * dt);
            put("stop.rest".into(), off);
            put("stop.settle".into(), moved);
            extra = format!("\trest {off:.3} m\tsettle {moved:.2} s");
        }
        println!(
            "anim\t{name}\t{family}\t{before} -> {after}\tpop {pop:.2} ({} at +{:.2} s)\tslide {:.1} cm\tglide {glide:.2} m/s{extra}",
            JOINTS[joint],
            (at_frame - from) as f32 * dt,
            slide * 100.0,
        );
        trace(
            name,
            &frames,
            &kicks,
            &steady,
            &[before, after],
            (*from, *end),
        );
    }
    // Each loop's seams: the frames it wraps, against its own steady state.
    let mut seams: HashMap<String, (f32, usize)> = HashMap::new();
    for i in 3..frames.len() {
        if measure::wraps(&frames, i)
            && let Some(clip) = &frames[i].solo
        {
            let (p, j, _) = measure::pop(&kicks, &steady, &[clip.as_str()], (i, i + 1));
            let seam = seams.entry(clip.clone()).or_insert((0.0, 0));
            if p > seam.0 {
                *seam = (p, j);
            }
            put("seam.loops".into(), p);
        }
    }
    for (clip, (p, j)) in &seams {
        println!(
            "anim\tseam.{clip}\tseam\t{clip}\tpop {p:.2} ({})",
            JOINTS[*j]
        );
    }
    let mut ids: Vec<_> = worst.into_iter().collect();
    ids.sort_by(|a, b| a.0.cmp(&b.0));
    for (id, v) in ids {
        println!("scorecard\t{id}\t{v:.4}");
    }
}
