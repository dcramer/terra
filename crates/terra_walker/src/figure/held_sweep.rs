//! The held sweep: the man's figure (male.glb and the game's own clips, as the animation
//! course poses it, with no GPU) holding each look in turn, driven through every clip it has
//! at frame times a game meets (none, a hitch, a pause's clamp), its clip changed every few
//! frames and its look swapped and put away mid-clip; after every frame every bone is finite,
//! its rotation a unit quaternion and its scale its rest's, and each hand within an arm's reach
//! of its shoulder. What it holds: the developer's still of 2026-11-09, the right arm's skin
//! stretched tens of metres from a crouched body with the hammer in hand.

use super::*;
use crate::character::{HAND_L, HAND_R};
use terra_gen::hash;

/// The looks swept, each a file of `assets/models/held`.
const LOOKS: [&str; 6] = [
    "hammer",
    "stone_axe",
    "knife",
    "arming_sword",
    "basket",
    "torch",
];
/// The frame times drawn from, s: none, next to none, a fast and a usual frame, a slow one, a
/// hitch and a pause (the last two over the engine's clamp of a frame, 0.25 s).
const FRAME_S: [f64; 7] = [0.0, 1.0e-6, 1.0 / 240.0, 1.0 / 60.0, 0.1, 1.0, 30.0];
/// The runs a look is swept by, each its own seed, and the frames of a run.
const RUNS: u32 = 4;
const FRAMES: u32 = 700;
/// The farthest a hand is from its upper arm's joint, m: the man's arm is 0.6 m.
const REACH_M: f32 = 2.0;

/// One run's random draws, in order.
struct Draws(u32, u32);

impl Draws {
    fn next(&mut self, below: u32) -> u32 {
        self.1 += 1;
        hash::of(&[self.0, self.1]) % below
    }
}

/// What a bone must still be: its name, itself and its scale at rest.
type Bones = Vec<(String, Entity, Vec3)>;

/// Every named thing under the figure with a transform, before anything is held.
fn bones(world: &mut World, root: Entity) -> Bones {
    let mut named = world.query::<(&Name, &Transform)>();
    let mut children = world.query::<&Children>();
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Ok((name, t)) = named.get(world, e) {
            out.push((name.as_str().to_string(), e, t.scale));
        }
        if let Ok(c) = children.get(world, e) {
            stack.extend(c.iter());
        }
    }
    out
}

/// What is wrong with the pose as it stands, if anything: the first bone that is not finite,
/// not a unit turn or not its rest's size, or a hand off its arm.
fn wrong(world: &World, bones: &Bones) -> Option<String> {
    let mut at = std::collections::HashMap::new();
    for (name, bone, rest) in bones {
        let Some(t) = world.get::<Transform>(*bone) else {
            continue;
        };
        let turn = t.rotation.length();
        if !t.translation.is_finite() {
            return Some(format!("{name}: translation {:?}", t.translation));
        }
        if !t.rotation.is_finite() || (turn - 1.0).abs() > 1.0e-3 {
            return Some(format!(
                "{name}: rotation {:?} of length {turn}",
                t.rotation
            ));
        }
        if !t.scale.is_finite() || (t.scale - *rest).abs().max_element() > 1.0e-3 {
            return Some(format!("{name}: scale {:?}, at rest {rest:?}", t.scale));
        }
        let Some(g) = world.get::<GlobalTransform>(*bone) else {
            continue;
        };
        if !g.affine().is_finite() {
            return Some(format!("{name}: in the world {:?}", g.affine()));
        }
        at.insert(name.as_str(), g.translation());
    }
    for (upper, hand) in [("upperarm_r", "hand_r"), ("upperarm_l", "hand_l")] {
        let far = at[upper].distance(at[hand]);
        if far > REACH_M {
            return Some(format!("{hand}: {far} m from {upper}"));
        }
    }
    None
}

/// The headless figure's app with what holds a look in a hand as the game does.
fn app() -> App {
    let mut app = course::app();
    app.add_observer(crate::character::held_ready)
        .add_observer(crate::strap::strap)
        .add_systems(
            PostUpdate,
            crate::strap::follow_fist
                .after(bevy::app::AnimationSystems)
                .after(crate::Fitted)
                .before(bevy::transform::TransformSystems::Propagate),
        );
    crate::gaze::plugin(&mut app);
    app
}

/// One run: the figure with `look` in its right hand, through `FRAMES` frames of draws from
/// `seed`. What went wrong, with the frame and what led to it, or nothing.
fn run(look: &str, seed: u32) -> Option<String> {
    let (mut app, body, bones, sockets, clips, models) = stood();
    let hold = |app: &mut App, k: usize, socket: Entity| {
        let model = models[k].clone();
        let mut commands = app.world_mut().commands();
        let held = crate::character::hold(&mut commands, model, socket);
        app.world_mut().flush();
        held
    };
    let first = LOOKS.iter().position(|l| *l == look).unwrap();
    let mut held = [Some(hold(&mut app, first, sockets[0])), None];
    waited(&mut app, look, &mut |world| {
        world.get::<CarryHold>(sockets[0]).is_some()
    });
    swept(
        &mut app,
        (look, seed),
        (body, &bones, sockets, &clips),
        &mut held,
        &hold,
    )
}

/// Updates the app until `done`.
fn waited(app: &mut App, what: &str, done: &mut dyn FnMut(&mut World) -> bool) {
    let start = std::time::Instant::now();
    while !done(app.world_mut()) {
        app.update();
        assert!(start.elapsed().as_secs() < 120, "{what} never loaded");
        std::thread::sleep(Duration::from_millis(1));
    }
}

type Models = Vec<Handle<bevy::world_serialization::WorldAsset>>;

/// The app with the man stood in it and every look loaded: his body, bones, hand sockets
/// (right, left), clips by name and the looks' models.
fn stood() -> (App, Entity, Bones, [Entity; 2], Vec<String>, Models) {
    let mut app = app();
    let body = app
        .world_mut()
        .spawn((
            Player,
            Motion::facing(Vec3::NEG_Z),
            Stance::default(),
            Acting::default(),
            course::on_world(Vec3::ZERO, Vec3::NEG_Z).0,
        ))
        .id();
    waited(&mut app, "male.glb", &mut |world| {
        let mut q = world.query_filtered::<(), With<Animated>>();
        q.iter(world).next().is_some()
    });
    app.update();
    let bones = bones(app.world_mut(), body);
    let find = |n: &str| bones.iter().find(|b| b.0 == n).map(|b| b.1);
    let sockets = [find(HAND_R).unwrap(), find(HAND_L).unwrap()];
    let moves = {
        let world = app.world_mut();
        let mut q = world.query::<&Animated>();
        q.iter(world).next().unwrap().moves.clone()
    };
    let mut clips: Vec<String> = moves.clips.keys().cloned().collect();
    clips.sort();
    let server = app.world().resource::<AssetServer>().clone();
    let models: Models = LOOKS
        .iter()
        .map(|l| {
            server.load(
                bevy::gltf::GltfAssetLabel::Scene(0).from_asset(crate::character::held_look(l)),
            )
        })
        .collect();
    waited(&mut app, "a held look", &mut |_| {
        models.iter().all(|m| server.is_loaded_with_dependencies(m))
    });
    (app, body, bones, sockets, clips, models)
}

/// The frames of a run, on the man as he stands with his first look in hand.
fn swept(
    app: &mut App,
    (look, seed): (&str, u32),
    (body, bones, sockets, clips): (Entity, &Bones, [Entity; 2], &[String]),
    held: &mut [Option<Entity>; 2],
    hold: &dyn Fn(&mut App, usize, Entity) -> Entity,
) -> Option<String> {
    let mut draws = Draws(seed, 0);
    let mut said: Vec<String> = Vec::new();
    let mut left = 0;
    let (mut at, mut speed) = (Vec3::ZERO, 0.0_f32);
    for frame in 0..FRAMES {
        if left == 0 {
            left = 1 + draws.next(14);
            let world = app.world_mut();
            *world.get_mut::<Stance>(body).unwrap() = Stance::default();
            {
                let mut motion = world.get_mut::<Motion>(body).unwrap();
                motion.airborne = 0.0;
                motion.on_ground = true;
            }
            speed = 0.0;
            let what = match draws.next(14) {
                0..=4 => {
                    let clip = &clips[draws.next(clips.len() as u32) as usize];
                    let mut act = Act::looping(clip, [1.0, 1.0, 0.0, 3.0][draws.next(4) as usize]);
                    act.once = draws.next(2) == 0;
                    if draws.next(4) == 0 {
                        act.hold = Some(clips[draws.next(clips.len() as u32) as usize].clone());
                    }
                    if draws.next(4) == 0 {
                        act.blend = Duration::ZERO;
                    }
                    let said = format!("act {clip} x{} hold {:?}", act.speed, act.hold);
                    world.get_mut::<Acting>(body).unwrap().play(act);
                    said
                }
                5 => {
                    world.get_mut::<Acting>(body).unwrap().clip = None;
                    "stand".into()
                }
                6 => {
                    world.get_mut::<Acting>(body).unwrap().clip = None;
                    speed = [1.2, 3.0, 7.0][draws.next(3) as usize];
                    format!("go {speed}")
                }
                7 => {
                    world.get_mut::<Acting>(body).unwrap().clip = None;
                    let mut motion = world.get_mut::<Motion>(body).unwrap();
                    motion.airborne = 0.3;
                    motion.on_ground = false;
                    "fall".into()
                }
                8 => {
                    world.get_mut::<Acting>(body).unwrap().clip = None;
                    world.get_mut::<Stance>(body).unwrap().dodging = 0.6;
                    "roll".into()
                }
                9 => {
                    world.get_mut::<Acting>(body).unwrap().clip = None;
                    world.get_mut::<Stance>(body).unwrap().attacking = 0.5;
                    "attack".into()
                }
                10 => {
                    world.get_mut::<Acting>(body).unwrap().clip = None;
                    let mut stance = world.get_mut::<Stance>(body).unwrap();
                    if draws.next(2) == 0 {
                        stance.guarding = true;
                        "guard".into()
                    } else {
                        stance.flinch = 0.4;
                        "flinch".into()
                    }
                }
                11 => {
                    world.get_mut::<Stance>(body).unwrap().since_acting = 0.0;
                    "use".into()
                }
                _ => {
                    // A look put away, taken up or swapped, in either hand.
                    let hand = draws.next(2) as usize;
                    if let Some(look) = held[hand].take() {
                        world.entity_mut(look).despawn();
                    }
                    if draws.next(3) == 0 {
                        format!("hand {hand} emptied")
                    } else {
                        let k = draws.next(LOOKS.len() as u32) as usize;
                        held[hand] = Some(hold(app, k, sockets[hand]));
                        format!("hand {hand} takes {}", LOOKS[k])
                    }
                }
            };
            said.push(format!("frame {frame}: {what}"));
        }
        left -= 1;
        let dt = FRAME_S[draws.next(FRAME_S.len() as u32) as usize];
        at += Vec3::NEG_Z * speed * (dt as f32).min(0.25);
        let world = app.world_mut();
        world.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs_f64(dt),
        ));
        {
            let mut motion = world.get_mut::<Motion>(body).unwrap();
            motion.velocity = Vec3::NEG_Z * speed;
            if !motion.on_ground {
                motion.airborne += dt as f32;
            }
        }
        *world.get_mut::<Transform>(body).unwrap() = course::on_world(at, Vec3::NEG_Z).0;
        app.update();
        if let Some(wrong) = wrong(app.world(), bones) {
            let led = said.iter().rev().take(6).rev().cloned().collect::<Vec<_>>();
            return Some(format!(
                "{look}, seed {seed}, frame {frame}, frame time {dt} s: {wrong}\n  after {}",
                led.join("\n        ")
            ));
        }
    }
    None
}

#[test]
fn no_clip_look_or_frame_time_leaves_a_bone_that_is_not_a_unit_turn() {
    let cases: Vec<(&str, u32)> = LOOKS
        .iter()
        .flat_map(|look| (0..RUNS).map(move |seed| (*look, seed)))
        .collect();
    let failed: Vec<String> = terra_gen::par::map(&cases, |(look, seed)| run(look, *seed))
        .into_iter()
        .flatten()
        .collect();
    assert!(
        failed.is_empty(),
        "{} of {} runs left a bone the skin cannot be drawn on:\n{}",
        failed.len(),
        cases.len(),
        failed.join("\n")
    );
}

/// The check catches each fault it is for (D632): a planted bad bone is named.
#[test]
fn a_planted_bad_bone_is_caught() {
    let planted: [(&str, fn(&mut Transform)); 5] = [
        ("rotation", |t| {
            t.rotation = Quat::from_xyzw(0.0, 0.0, 0.0, 3.0)
        }),
        ("rotation", |t| t.rotation = Quat::NAN),
        ("translation", |t| t.translation.x = f32::INFINITY),
        ("scale", |t| t.scale = Vec3::splat(40.0)),
        ("hand_r", |t| t.translation.y = -30.0),
    ];
    for (said, plant) in planted {
        let mut app = App::new();
        app.add_plugins(TransformPlugin);
        let world = app.world_mut();
        let mut bones = Bones::new();
        let mut parent = None;
        for name in ["upperarm_l", "hand_l", "upperarm_r", "hand_r"] {
            let mut bone = world.spawn((Name::new(name), Transform::from_xyz(0.0, -0.3, 0.0)));
            let bone = match parent {
                Some(parent) => bone.insert(ChildOf(parent)).id(),
                None => bone.id(),
            };
            parent = Some(bone);
            bones.push((name.into(), bone, Vec3::ONE));
        }
        app.update();
        assert_eq!(wrong(app.world(), &bones), None, "a sound pose is refused");
        plant(&mut app.world_mut().get_mut::<Transform>(bones[3].1).unwrap());
        app.update();
        let found = wrong(app.world(), &bones).unwrap_or_default();
        assert!(found.contains(said), "a planted {said} read: {found:?}");
    }
}
