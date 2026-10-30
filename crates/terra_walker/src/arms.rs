//! It stays until the jog is retaken (animation.md's order of work, step 2): the jog's arms
//! are a formula's (D584's listed fault), and without this layer its empty left hand goes 3.6
//! cm into the female's thigh (moves.ts, Jog_Fwd_Loop at 0.40 s; bar 2). A carry pose does
//! not replace it, since a static pose on an empty hand would freeze the arm's swing. Then
//! measure the empty hands without `--arms` and delete it with its twin `arms.ts`.
//!
//! The hanging hands kept off the hips (D535): after every clip and hold has posed a body,
//! an upper arm whose hand hangs by the hip closer to the body's middle than `OUT` times the
//! hip joint's own distance from it is turned out from the body at the shoulder just enough to
//! clear it. A clip made on one body (the library's idle and walk on the mannequin) puts the
//! fingers into another's thigh; turning the upper arm out clears them, as the staged figures'
//! bends do (`staged::pose`) and as shipped engines fit a clip to a body after it (Unreal's
//! "Transform (Modify) Bone" in an animation graph, ALS's arm spacing for wider bodies).
//!
//! It reads the pose, so it holds over whichever idle or walk plays, a bought one too, with no
//! clip changed: a hand already clear (the guard's, a blow's in front or above) is left as the
//! clip has it. What the hand holds counts as the hand does, as ALS's arm spacing counts a held
//! item: an axe's haft carried past the thigh turns the arm out until the haft clears it, by a
//! few points that bound the look (`Reach`), kept `HELD` further out than the hand's own, so
//! each item keeps its carried tilt and no clip changes. The composed pose's `film.pose.arms_in_body` row measures it (D530). Its twin
//! is `tools/characters/arms.ts`, which the move check lays over the walker's clips
//! (`moves.ts --arms`); change both.

use bevy::{prelude::*, world_serialization::WorldInstanceReady};

use crate::bend::{Kept, find, in_body};
use crate::character::{Body, HAND_L, HAND_R, Held, look_points};

/// How far from the body's middle a hanging hand is kept, in hip joints' distances from it:
/// twice puts the fingers a hand's breadth outside the thigh's joint, clear of the thigh's
/// flesh on every template (the duel's record: fingers 10 cm beyond the hip joint sit 2 cm
/// out of the male template's thigh, at 5-7 cm they were 5-6 cm in).
pub const OUT: f32 = 2.0;
/// Over the hip joint, metres, a hand is wholly by the hip up to `LOW` and not at all from
/// `HIGH`; ahead of or behind it, wholly within `NEAR` and not at all from `FAR`.
const LOW: f32 = 0.12;
const HIGH: f32 = 0.22;
const NEAR: f32 = 0.20;
const FAR: f32 = 0.32;
/// How much further out than the hand, metres, what it holds is kept: `OUT`'s line is a hip
/// joint's, and a look's points (surface, where the hand's are joints) carried down the thigh
/// met the heavy and female templates' flesh 1 to 2.4 cm inside it; 4 cm clears every carried
/// look on every template (the move check, item-attach job 5), 2 cm left the hammer at 0.6.
pub const HELD: f32 = 0.04;
/// The most an arm is turned out, degrees.
pub const MOST: f32 = 25.0;

/// The bones of a side the check reads, in this order: the upper arm, the hand and the points
/// of it the move check measures (skin.ts `armPoints`), and the thigh.
const LEFT: [&str; 7] = [
    "upperarm_l",
    "hand_l",
    "middle_01_l",
    "middle_02_l",
    "index_02_l",
    "thumb_02_l",
    "thigh_l",
];
const RIGHT: [&str; 7] = [
    "upperarm_r",
    "hand_r",
    "middle_01_r",
    "middle_02_r",
    "index_02_r",
    "thumb_02_r",
    "thigh_r",
];

/// How many slices along its handle a held look's reach takes.
pub const SLICES: usize = 8;

/// On a body with arms and legs: its pelvis, each side's bones (`LEFT`, `RIGHT`), its hand
/// sockets (left, right) if it has them, and what was written over each upper arm.
#[derive(Component)]
pub struct Arms {
    pelvis: Entity,
    sides: [[Entity; 7]; 2],
    sockets: Option<[Entity; 2]>,
    kept: [Kept; 2],
}

/// A held look's reach, in its own frame (the grip at the origin, the handle along +Y): the
/// points that bound it (`reach`), which count in its arm's spacing as the hand's own do.
#[derive(Component, Default, Debug)]
pub struct Reach(pub Vec<Vec3>);

/// The few points that bound a held look whose points are `points`, in its frame: cut into
/// `SLICES` slices along +Y, of each slice the points farthest along -X, +X, -Z and +Z.
pub fn reach(points: &[Vec3]) -> Vec<Vec3> {
    let Some(lo) = points.iter().map(|p| p.y).reduce(f32::min) else {
        return Vec::new();
    };
    let span = points.iter().map(|p| p.y).fold(lo, f32::max) - lo;
    let mut slices = [[None::<Vec3>; 4]; SLICES];
    for &p in points {
        let k = if span > 0.0 {
            (((p.y - lo) / span * SLICES as f32) as usize).min(SLICES - 1)
        } else {
            0
        };
        let s = &mut slices[k];
        let farther = [
            s[0].is_none_or(|q| p.x < q.x),
            s[1].is_none_or(|q| p.x > q.x),
            s[2].is_none_or(|q| p.z < q.z),
            s[3].is_none_or(|q| p.z > q.z),
        ];
        for (each, farther) in s.iter_mut().zip(farther) {
            if farther {
                *each = Some(p);
            }
        }
    }
    slices.into_iter().flatten().flatten().collect()
}

pub fn plugin(app: &mut App) {
    app.add_observer(find_arms)
        .add_observer(held_reach)
        .add_systems(
            PostUpdate,
            keep_off_hips
                .in_set(crate::pose::Layer::Arms)
                .after(bevy::app::AnimationSystems)
                .after(crate::Fitted)
                .before(TransformSystems::Propagate),
        );
}

fn find_arms(
    ready: On<WorldInstanceReady>,
    bodies: Query<(), With<Body>>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut commands: Commands,
) {
    let root = ready.entity;
    if !bodies.contains(root) {
        return;
    }
    let (Some([pelvis]), Some(left), Some(right)) = (
        find(root, ["pelvis"], &children, &names),
        find(root, LEFT, &children, &names),
        find(root, RIGHT, &children, &names),
    ) else {
        return;
    };
    commands.entity(root).insert(Arms {
        pelvis,
        sides: [left, right],
        sockets: find(root, [HAND_L, HAND_R], &children, &names),
        kept: [Kept::default(); 2],
    });
}

/// Once a held look has loaded, its reach from its meshes' points, each placed by its nodes'
/// transforms under the look's root.
fn held_reach(
    ready: On<WorldInstanceReady>,
    held: Query<(), With<Held>>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    transforms: Query<&Transform>,
    meshes3d: Query<&Mesh3d>,
    meshes: Res<Assets<Mesh>>,
    mut commands: Commands,
) {
    let root = ready.entity;
    if !held.contains(root) {
        return;
    }
    let points = look_points(root, &children, &parents, &transforms, &meshes3d, &meshes);
    commands.entity(root).insert(Reach(reach(&points)));
}

/// How far, radians, to turn out an arm hanging from a shoulder at `shoulder` (in the body,
/// the side's sign `side`: +1 the left, -1 the right) whose hand's points are `points`, beside
/// a hip joint at `hip` on a pelvis at `middle`: the most any point needs to reach `OUT` hip
/// widths from the middle, by how much it is by the hip, over its drop from the shoulder.
pub fn turn_out(side: f32, shoulder: Vec3, points: &[Vec3], hip: Vec3, middle: Vec3) -> f32 {
    use terra_gen::smoothstep;
    let hip_half = side * (hip.x - middle.x);
    if hip_half <= 0.0 {
        return 0.0;
    }
    let mut most = 0.0_f32;
    for p in points {
        let out = side * (p.x - middle.x);
        let short = OUT * hip_half - out;
        if short <= 0.0 {
            continue;
        }
        let by = smoothstep(HIGH, LOW, p.y - hip.y) * smoothstep(FAR, NEAR, (p.z - hip.z).abs());
        let drop = (shoulder.y - p.y).max(0.15);
        most = most.max(short * by / drop);
    }
    side * most.min(MOST.to_radians())
}

/// Each body's hanging hands, and what they hold, turned out off its hips.
pub fn keep_off_hips(
    mut bodies: Query<(Entity, &mut Arms)>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    reaches: Query<&Reach>,
    mut transforms: Query<&mut Transform>,
) {
    let mut points = Vec::new();
    for (root, mut arms) in &mut bodies {
        let arms = &mut *arms;
        let middle = in_body(arms.pelvis, root, &parents, &transforms).translation;
        for (k, (side, bones)) in [1.0, -1.0].into_iter().zip(arms.sides).enumerate() {
            let [upper, hand, middle_01, middle_02, index, thumb, thigh] = bones;
            let at = |bone| Vec3::from(in_body(bone, root, &parents, &transforms).translation);
            let (wrist, knuckle) = (at(hand), at(middle_01));
            points.clear();
            points.extend([(wrist + knuckle) / 2.0, at(middle_02), at(index), at(thumb)]);
            // What the hand holds: each look on its socket, by its reach where it is now.
            let holding = arms
                .sockets
                .map(|s| s[k])
                .and_then(|s| children.get(s).ok());
            for look in holding.into_iter().flatten() {
                if let Ok(Reach(reach)) = reaches.get(*look) {
                    let place = in_body(*look, root, &parents, &transforms);
                    // Kept `HELD` further out than the hand: counted that much nearer the middle.
                    let nearer = Vec3::X * side * HELD;
                    points.extend(reach.iter().map(|&p| place.transform_point3(p) - nearer));
                }
            }
            let Ok(up) = parents.get(upper) else {
                continue;
            };
            let parent = in_body(up.parent(), root, &parents, &transforms)
                .to_scale_rotation_translation()
                .1;
            let Ok(now) = transforms.get(upper).map(|t| t.rotation) else {
                continue;
            };
            // Still as turned last frame, no clip posed it again: it stays so.
            let clip = arms.kept[k].clip(now);
            if clip != now {
                continue;
            }
            let angle = turn_out(side, at(upper), &points, at(thigh), middle.into());
            let written = arms.kept[k].turn(clip, parent, Quat::from_rotation_z(angle));
            if let Ok(mut t) = transforms.get_mut(upper) {
                t.rotation = written;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand hanging straight down from a shoulder 0.2 m out, 0.6 m below it, by a hip joint
    /// 0.1 m out: at 0.2 m out it is as far as `OUT` keeps it.
    #[test]
    fn a_hand_hanging_against_the_thigh_is_turned_out_until_clear_and_a_clear_one_is_left() {
        let middle = Vec3::new(0.0, 1.0, 0.0);
        let hip = Vec3::new(0.1, 0.94, 0.0);
        let shoulder = Vec3::new(0.15, 1.45, 0.0);
        let hanging = |x: f32| Vec3::new(x, 0.9, 0.05);
        // Against the thigh: turned out (left arm: +z) until the hand is 0.2 m out.
        let angle = turn_out(1.0, shoulder, &[hanging(0.15)], hip, middle);
        let drop = shoulder.y - hanging(0.15).y;
        let swung = Quat::from_rotation_z(angle) * (hanging(0.15) - shoulder) + shoulder;
        assert!(angle > 0.0);
        assert!(swung.x >= 0.195, "{swung} after {angle} over {drop}");
        // The right side mirrors it.
        let right = turn_out(
            -1.0,
            shoulder * Vec3::new(-1.0, 1.0, 1.0),
            &[hanging(-0.15)],
            hip * Vec3::new(-1.0, 1.0, 1.0),
            middle,
        );
        assert!((right + angle).abs() < 1e-6);
        // Already clear, above the hip or well ahead of it (a guard, a blow): left.
        assert_eq!(turn_out(1.0, shoulder, &[hanging(0.25)], hip, middle), 0.0);
        let raised = Vec3::new(0.12, 1.3, 0.1);
        assert_eq!(turn_out(1.0, shoulder, &[raised], hip, middle), 0.0);
        let ahead = Vec3::new(0.12, 0.9, 0.4);
        assert_eq!(turn_out(1.0, shoulder, &[ahead], hip, middle), 0.0);
    }

    /// A haft 4 cm thick and 8 long, one slice to a centimetre: each slice's four extremes.
    #[test]
    fn a_looks_reach_is_each_slices_extremes() {
        let mut points = Vec::new();
        for y in 0..8 {
            for (x, z) in [
                (-0.02, 0.0),
                (0.02, 0.0),
                (0.0, -0.01),
                (0.0, 0.03),
                (0.0, 0.0),
            ] {
                points.push(Vec3::new(x, y as f32 * 0.01, z));
            }
        }
        let got = reach(&points);
        assert_eq!(got.len(), 4 * SLICES);
        assert!(
            !got.contains(&Vec3::new(0.0, 0.03, 0.0)),
            "a middle point kept"
        );
        assert!(got.contains(&Vec3::new(0.0, 0.07, 0.03)));
        assert!(reach(&[]).is_empty());
    }

    /// A body of bare bones: shoulders 0.15 m out, hands hanging 0.21 m out (clear of `OUT`'s
    /// 0.2), hip joints 0.1 m out; the left hand holding, if `held`, a 0.3 m haft that runs
    /// from the grip in, down and forward past the thigh. Each upper arm's turn after
    /// `keep_off_hips`, and the haft's points in the body before it.
    fn turned(held: bool) -> ([f32; 2], Vec<Vec3>) {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let root = world.spawn(Transform::default()).id();
        let at = |world: &mut World, parent: Entity, place: Vec3| {
            world
                .spawn((Transform::from_translation(place), ChildOf(parent)))
                .id()
        };
        let pelvis = at(&mut world, root, Vec3::new(0.0, 1.0, 0.0));
        let haft = Quat::from_rotation_arc(Vec3::Y, Vec3::new(-0.5, -0.6, 0.6).normalize());
        let along: Vec<Vec3> = (0..=30).map(|k| Vec3::Y * k as f32 * 0.01).collect();
        let mut sides = [[root; 7]; 2];
        let mut sockets = [root; 2];
        for (k, side) in [1.0, -1.0].into_iter().enumerate() {
            let shoulder = Vec3::new(0.15 * side, 1.45, 0.0);
            let upper = at(&mut world, root, shoulder);
            let hand = at(
                &mut world,
                upper,
                Vec3::new(0.21 * side, 0.9, 0.05) - shoulder,
            );
            let [m1, m2, index, thumb] = [(); 4].map(|_| at(&mut world, hand, Vec3::ZERO));
            let thigh = at(&mut world, root, Vec3::new(0.1 * side, 0.94, 0.0));
            sides[k] = [upper, hand, m1, m2, index, thumb, thigh];
            sockets[k] = at(&mut world, hand, Vec3::ZERO);
        }
        if held {
            world.spawn((
                Transform::from_rotation(haft),
                ChildOf(sockets[0]),
                Reach(reach(&along)),
            ));
        }
        world.entity_mut(root).insert(Arms {
            pelvis,
            sides,
            sockets: Some(sockets),
            kept: [Kept::default(); 2],
        });
        world.run_system_once(keep_off_hips).unwrap();
        let angle = |k: usize| {
            let r = world.get::<Transform>(sides[k][0]).unwrap().rotation;
            r.to_euler(EulerRot::ZYX).0
        };
        let hand = Vec3::new(0.21, 0.9, 0.05);
        (
            [angle(0), angle(1)],
            along.iter().map(|&p| hand + haft * p).collect(),
        )
    }

    #[test]
    fn a_haft_carried_across_the_thigh_turns_its_arm_out_until_it_clears() {
        // Holding nothing, the clear hands are left as the clip has them.
        assert_eq!(turned(false).0, [0.0, 0.0]);
        // Holding the haft, the left arm turns out (+z) until the haft's points by the hip are
        // `OUT` hip widths and `HELD` from the middle; the right, holding nothing, is left.
        let ([left, right], haft) = turned(true);
        assert!(left > 0.0 && left <= MOST.to_radians(), "{left}");
        assert_eq!(right, 0.0);
        let shoulder = Vec3::new(0.15, 1.45, 0.0);
        let mut least = f32::MAX;
        for p in haft.into_iter().filter(|p| p.y < 0.94 + LOW && p.z < NEAR) {
            let swung = Quat::from_rotation_z(left) * (p - shoulder) + shoulder;
            least = least.min(swung.x);
            assert!(swung.x >= 0.19 + HELD, "{p} only to {swung} after {left}");
        }
        // Held only as far as a hand would be, it would stop short of `HELD` beyond.
        assert!(
            least < 0.2 + HELD + 0.02,
            "turned further than it needs: {least}"
        );
    }
}
