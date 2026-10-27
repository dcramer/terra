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
//! clip has it. The composed pose's `film.pose.arms_in_body` row measures it (D530). Its twin
//! is `tools/characters/arms.ts`, which the move check lays over the walker's clips
//! (`moves.ts --arms`); change both.

use bevy::{prelude::*, world_serialization::WorldInstanceReady};

use crate::bend::{Kept, find, in_body};
use crate::character::Body;

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

/// On a body with arms and legs: its pelvis, each side's bones (`LEFT`, `RIGHT`), and what was
/// written over each upper arm.
#[derive(Component)]
pub struct Arms {
    pelvis: Entity,
    sides: [[Entity; 7]; 2],
    kept: [Kept; 2],
}

pub fn plugin(app: &mut App) {
    app.add_observer(find_arms).add_systems(
        PostUpdate,
        keep_off_hips
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
        kept: [Kept::default(); 2],
    });
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

/// Each body's hanging hands turned out off its hips.
pub fn keep_off_hips(
    mut bodies: Query<(Entity, &mut Arms)>,
    parents: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
) {
    for (root, mut arms) in &mut bodies {
        let arms = &mut *arms;
        let middle = in_body(arms.pelvis, root, &parents, &transforms).translation;
        for (k, (side, bones)) in [1.0, -1.0].into_iter().zip(arms.sides).enumerate() {
            let [upper, hand, middle_01, middle_02, index, thumb, thigh] = bones;
            let at = |bone| Vec3::from(in_body(bone, root, &parents, &transforms).translation);
            let (wrist, knuckle) = (at(hand), at(middle_01));
            let points = [(wrist + knuckle) / 2.0, at(middle_02), at(index), at(thumb)];
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
}
