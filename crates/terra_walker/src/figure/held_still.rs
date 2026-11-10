//! The held arm, still and smooth: the man's figure (as the held sweep stands him, with no
//! GPU) with a look in a hand, read two ways. Stilled: with no clip posing him again (his
//! player's clips stopped) his arm bones stay where the pose layers put them, however many
//! layers turn one bone (`carry_arm`, `arms` and `shield_arm` each keep their own
//! `bend::Kept` of the upper arm), rather than each turning the other's turn again a frame.
//! Idle: his arm is as smooth with a tool carried as with his hands empty, by each bone's
//! change of angular velocity from one frame to the next. What they hold: the developer's
//! "my dude is also a little jittery in the idle animation?" of 2026-11-09.

use super::held_sweep::{Bones, stood, turn, waited};
use super::*;
use terra_gen::hash;

/// The arm bones the pose layers turn, right then left.
const ARM_BONES: [&str; 8] = [
    "clavicle_r",
    "upperarm_r",
    "lowerarm_r",
    "hand_r",
    "clavicle_l",
    "upperarm_l",
    "lowerarm_l",
    "hand_l",
];
/// The frames a stilled arm is read over, and those before them in which a give still easing
/// when the clips stopped comes to rest.
const STILL: u32 = 600;
const SETTLE: u32 = 60;
/// The most a stilled bone turns, degrees: 1e-4 rad.
const STILL_DEG: f32 = 0.0058;
/// How many times an empty-handed idle's worst arm bone a carried idle's may be.
const SMOOTH: f32 = 2.0;

/// The man stood with `looks` in his hands (right, left), playing `clip` (none: as he stands).
fn man(looks: [Option<&str>; 2], clip: Option<&str>) -> (App, Bones) {
    let (mut app, body, bones, sockets, ..) = stood();
    let server = app.world().resource::<AssetServer>().clone();
    for (look, socket) in looks.into_iter().zip(sockets) {
        let Some(look) = look else { continue };
        let model = server.load(
            bevy::gltf::GltfAssetLabel::Scene(0).from_asset(crate::character::held_look(look)),
        );
        waited(&mut app, look, &mut |_| {
            server.is_loaded_with_dependencies(&model)
        });
        let mut commands = app.world_mut().commands();
        crate::character::hold(&mut commands, model, socket);
        app.world_mut().flush();
        waited(&mut app, look, &mut |world| {
            world.get::<CarryHold>(socket).is_some()
        });
    }
    if let Some(clip) = clip {
        let act = Act::looping(clip, 1.0);
        app.world_mut().get_mut::<Acting>(body).unwrap().play(act);
    }
    (app, bones)
}

fn bone(bones: &Bones, name: &str) -> Entity {
    bones.iter().find(|b| b.0 == name).unwrap().1
}

fn turns(app: &App, bones: &Bones) -> Vec<Quat> {
    ARM_BONES
        .iter()
        .map(|name| {
            let bone = bone(bones, name);
            app.world().get::<Transform>(bone).unwrap().rotation
        })
        .collect()
}

/// The man a second into `clip` with `looks` in hand, then no clip posing him again: the arm
/// bone that turns furthest over `STILL` frames from where it stood once settled, and by how
/// much, degrees. `planted` turns his right upper arm a hundredth of a degree more each
/// frame, as a layer that took its own turn for the clip's pose would.
fn stilled(looks: [Option<&str>; 2], clip: &str, planted: bool) -> (&'static str, f32) {
    let (mut app, bones) = man(looks, Some(clip));
    for _ in 0..60 {
        app.update();
    }
    let world = app.world_mut();
    let mut players = world.query::<&mut AnimationPlayer>();
    for mut player in players.iter_mut(world) {
        player.stop_all();
    }
    for _ in 0..SETTLE {
        app.update();
    }
    let first = turns(&app, &bones);
    let mut worst = ("", 0.0_f32);
    for _ in 0..STILL {
        if planted {
            let upper = bone(&bones, "upperarm_r");
            let mut t = app.world_mut().get_mut::<Transform>(upper).unwrap();
            t.rotation *= Quat::from_rotation_x(0.01_f32.to_radians());
        }
        app.update();
        for (k, now) in turns(&app, &bones).into_iter().enumerate() {
            // By the turn between them: `angle_between`'s arc cosine reads no turn under
            // 0.04 degrees.
            let by = (now * first[k].inverse()).xyz().length();
            let by = (2.0 * libm::asinf(by.min(1.0))).to_degrees();
            if by > worst.1 {
                worst = (ARM_BONES[k], by);
            }
        }
    }
    worst
}

/// With no clip posing the body again, every layer's turn of an arm bone is from the pose its
/// clip left, once: a tool in the right hand, a shield or a torch in the left, standing
/// crouched (the legs folded, so a hanging arm is given at the shoulder and elbow) or jogging.
#[test]
fn an_arm_no_clip_poses_again_stays_where_its_layers_put_it() {
    let _turn = turn();
    let mut cases = Vec::new();
    for clip in ["Crouch_Idle_Loop", "Crouch_Fwd_Loop", "Jog_Fwd_Loop"] {
        for right in ["hammer", "stone_axe", "knife", "arming_sword", "basket"] {
            cases.push(([Some(right), None], clip));
        }
        for left in ["round_shield", "heater_shield", "torch"] {
            cases.push(([Some("hammer"), Some(left)], clip));
            cases.push(([None, Some(left)], clip));
        }
        cases.push(([None, None], clip));
    }
    let moved: Vec<String> = terra_gen::par::map(&cases, |(looks, clip)| {
        let (bone, by) = stilled(*looks, clip, false);
        (by > STILL_DEG).then(|| format!("{looks:?} in {clip}: {bone} turned {by} degrees"))
    })
    .into_iter()
    .flatten()
    .collect();
    assert!(
        moved.is_empty(),
        "an arm bone no clip posed again went on turning (a layer turned its own or another's \
         turn again, `bend::Kept`):\n{}",
        moved.join("\n")
    );
    // The check catches what it is for (D632): a turn laid on again each frame is named.
    let (bone, by) = stilled([Some("hammer"), None], "Crouch_Idle_Loop", true);
    assert!(
        bone == "upperarm_r" && by > 1.0,
        "a planted drift read {bone} {by}"
    );
}

/// A frame's time, s: even at 60 a second, or uneven as a real frame's is (a fifth either
/// way, one in twenty twice as long).
fn frame_s(uneven: bool, frame: u32) -> f64 {
    if !uneven {
        return 1.0 / 60.0;
    }
    let long = if hash::of(&[7, frame]).is_multiple_of(20) {
        2.0
    } else {
        1.0
    };
    long * (0.8 + 0.4 * f64::from(hash::unit(hash::of(&[11, frame])))) / 60.0
}

/// Shakes the right upper arm a tenth of a degree either way on alternate frames.
fn shake(mut frame: Local<u32>, names: Query<&Name>, mut bones: Query<(Entity, &mut Transform)>) {
    *frame += 1;
    let by = if frame.is_multiple_of(2) {
        0.1_f32
    } else {
        -0.1
    };
    for (bone, mut t) in &mut bones {
        if names.get(bone).is_ok_and(|n| n.as_str() == "upperarm_r") {
            t.rotation *= Quat::from_rotation_x(by.to_radians());
        }
    }
}

/// The man idle for ten seconds with `looks` in hand: the greatest change of angular velocity
/// from one frame to the next of any arm bone's own turn, degrees a frame (of a sixtieth of
/// a second), and of either hand's place in the world, mm a frame.
fn jerk(looks: [Option<&str>; 2], uneven: bool, shaken: bool) -> (f32, f32) {
    let (mut app, bones) = man(looks, None);
    if shaken {
        app.add_systems(
            PostUpdate,
            shake
                .after(crate::arms::keep_off_hips)
                .before(bevy::transform::TransformSystems::Propagate),
        );
    }
    let hands = ["hand_r", "hand_l"].map(|name| bone(&bones, name));
    let places =
        |app: &App| hands.map(|h| app.world().get::<GlobalTransform>(h).unwrap().translation());
    let step = |app: &mut App, frame: u32| -> f32 {
        let dt = frame_s(uneven, frame);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs_f64(dt),
        ));
        app.update();
        dt as f32
    };
    for frame in 0..90 {
        step(&mut app, frame);
    }
    let (mut turned, mut placed) = (turns(&app, &bones), places(&app));
    let mut turning: Vec<Option<Vec3>> = vec![None; ARM_BONES.len()];
    let mut going: [Option<Vec3>; 2] = [None; 2];
    let (mut deg, mut mm) = (0.0_f32, 0.0_f32);
    for frame in 90..690 {
        let per = 1.0 / 60.0 / step(&mut app, frame);
        let now = turns(&app, &bones);
        for k in 0..now.len() {
            let mut d = now[k] * turned[k].inverse();
            if d.w < 0.0 {
                d = -d;
            }
            let w = d.to_scaled_axis() * per.to_degrees();
            if let Some(was) = turning[k] {
                deg = deg.max((w - was).length());
            }
            turning[k] = Some(w);
        }
        turned = now;
        let at = places(&app);
        for k in 0..2 {
            let v = (at[k] - placed[k]) * 1000.0 * per;
            if let Some(was) = going[k] {
                mm = mm.max((v - was).length());
            }
            going[k] = Some(v);
        }
        placed = at;
    }
    (deg, mm)
}

/// Standing idle with a tool carried, a shield strapped or a torch held, the arms are as
/// smooth as with the hands empty, at an even frame time and an uneven one.
#[test]
fn a_carried_idle_is_as_smooth_as_an_empty_handed_one() {
    let _turn = turn();
    let looks = [
        [None, None],
        [Some("hammer"), None],
        [Some("hammer"), Some("round_shield")],
        [None, Some("torch")],
    ];
    let cases: Vec<([Option<&str>; 2], bool, bool)> = looks
        .into_iter()
        .flat_map(|l| [(l, false, false), (l, true, false)])
        .chain([([Some("hammer"), None], false, true)])
        .collect();
    let read = terra_gen::par::map(&cases, |&(looks, uneven, shaken)| {
        jerk(looks, uneven, shaken)
    });
    for (k, (looks, uneven, shaken)) in cases.iter().enumerate() {
        let empty = read[usize::from(*uneven)];
        let (deg, mm) = read[k];
        let smooth = deg <= SMOOTH * empty.0 && mm <= SMOOTH * empty.1;
        assert!(
            smooth != *shaken,
            "{looks:?}, uneven {uneven}, shaken {shaken}: the arms' worst change of angular \
             velocity is {deg} degrees a frame and the hands' {mm} mm a frame; empty-handed \
             {} and {}",
            empty.0,
            empty.1
        );
    }
}
