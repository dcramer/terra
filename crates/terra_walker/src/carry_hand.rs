//! Only the spear and the bow are carried by the rule now, and this layer goes with them: a
//! one-handed tool takes the `Carry_Tool` pose (its look's `carry_hold`, D584's holding as an
//! overlay from a clip), so nothing here moves it. Delete this module, `Layer::CarryHand`,
//! `Held::in_fist`/`at_wrist` and its twin `tools/characters/carry_hand.ts` once a long-tool
//! carry pose (animation.md's order of work, step 3) takes the spear and the bow.
//!
//! A tool carried by the rule (`character::carry_rule`) tips forward and down with the hand and
//! the arm, not by sliding its handle across the palm (D564). Its tilt, turned all about the
//! palm's middle, ran the handle 45 degrees slantwise across the palm, from under the index's
//! tip to under the heel of the hand: the fingers bent beside it, not round it (the developer's
//! still of the carried axe, 2026-10-28). So the look turns in the fist no more than `OBLIQUE`,
//! the slant a power grip holds a hammer's handle at, and the hand turns the rest at the wrist
//! (its ulnar deviation, which reaches some 45 degrees; Kapandji, "The Physiology of the
//! Joints", vol. 1), about the same axis, so the look points as the rule says.
//!
//! The rule tips the look from the hand, so it points down only while the forearm hangs: a jog
//! swings the forearm up to level and past, and the axe came up level, pointing ahead. Shipped
//! games carry a one-handed tool on its own arm overlay, the swing damped (ALS's overlay layers,
//! their arm curves; docs only); here, while a look is carried by the rule, the elbow is opened
//! in its own bend's plane until the forearm is at least `FOREARM_DOWN` below level, or the arm
//! is near straight, so the swing keeps its shoulder and the tool its tip. Its twin is
//! `tools/characters/carry_hand.ts`, which the move check lays over the carried clips
//! (`moves.ts --carry`); change both.

use bevy::{prelude::*, world_serialization::WorldInstanceReady};

use crate::bend::{Kept, find, in_body};
use crate::character::{Body, CarryHold, HAND_L, HAND_R, Held};

/// The most a carried look turns in the fist, degrees: a power grip holds a handle a little
/// slantwise across the palm. The slant takes the handle away from the little finger's
/// knuckle, which must bend further to reach it: 86 degrees square, 106 to 109 at 15, past its
/// 115 at 19 to 20, where on the male, heavy, strong and thin hands it misses and curls into
/// the palm (a wrap of 21 degrees; the base's and the female's smaller hands still reach). Less
/// in the fist is more at the wrist, which swings the fist back: at 10 a carried pickaxe went
/// 1.3 cm into a heavy body's thigh at the jog, at 15 it clears by 0.2. At 15 every finger of
/// every template wraps 95 degrees or more through every carried clip (grip.ts `WRAP_DEG` 80).
pub const OBLIQUE: f32 = 15.0;
/// While a look is carried by the rule, how far below level its forearm is kept at least,
/// degrees: the rule's tilt, from a hanging hand, then still points the look forward and down.
pub const FOREARM_DOWN: f32 = 45.0;
/// The least an elbow opened for it stays bent, degrees.
const ELBOW_MOST_OPEN: f32 = 10.0;

/// On a body whose model has both arms: each side's (left, right) hand socket, its arm's bones
/// and what was written over their clips.
#[derive(Component)]
pub struct CarryHands {
    sides: [Side; 2],
}

struct Side {
    socket: Entity,
    lower: Entity,
    hand: Entity,
    elbow: Kept,
    wrist: Kept,
}

pub fn plugin(app: &mut App) {
    app.add_observer(find_hands).add_systems(
        PostUpdate,
        // On the pose the clips, the holds and the fit gave; before the fists (which close in
        // the hand's frame, whatever turns it) and the arms kept off the hips (which count the
        // look where the hand carries it).
        tip_carried
            .in_set(crate::pose::Layer::CarryHand)
            .after(bevy::app::AnimationSystems)
            .after(crate::Fitted)
            .before(crate::grip::close_hands)
            .before(crate::arms::keep_off_hips)
            .before(TransformSystems::Propagate),
    );
}

fn find_hands(
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
    let side = |socket: &str, s: &str| {
        let [socket, lower, hand] = find(
            root,
            [socket, &format!("lowerarm_{s}"), &format!("hand_{s}")],
            &children,
            &names,
        )?;
        Some(Side {
            socket,
            lower,
            hand,
            elbow: Kept::default(),
            wrist: Kept::default(),
        })
    };
    if let (Some(left), Some(right)) = (side(HAND_L, "l"), side(HAND_R, "r")) {
        commands.entity(root).insert(CarryHands {
            sides: [left, right],
        });
    }
}

/// How far, radians, to open an elbow whose upper arm runs `upper` and forearm `fore` (in the
/// body, +y up) for the forearm to point at least `FOREARM_DOWN` below level, no further than
/// `ELBOW_MOST_OPEN` short of straight; and the axis, in the body, to turn the forearm about
/// by minus that. None for a forearm low enough already, or an arm too straight to bend in a
/// plane.
pub fn elbow_open(upper: Vec3, fore: Vec3) -> Option<(Vec3, f32)> {
    let low = -FOREARM_DOWN.to_radians();
    let rise = |v: Vec3| libm::asinf((v.y / v.length()).clamp(-1.0, 1.0));
    if rise(fore) <= low {
        return None;
    }
    let axis = upper.cross(fore).try_normalize()?;
    let bent = libm::acosf((upper.dot(fore) / (upper.length() * fore.length())).clamp(-1.0, 1.0));
    let most = (bent - ELBOW_MOST_OPEN.to_radians()).max(0.0);
    let step = 0.5_f32.to_radians();
    let mut open = 0.0;
    while open < most && rise(Quat::from_axis_angle(axis, -open) * fore) > low {
        open += step;
    }
    Some((axis, open.min(most)))
}

/// Each hand carrying a look by the rule: its arm opened at the elbow while the forearm rises
/// (`elbow_open`), then the hand turned at the wrist by the look's tilt past the fist's share
/// (`Held::at_wrist`), each as far as the look has turned toward carried.
pub fn tip_carried(
    mut bodies: Query<(Entity, &mut CarryHands)>,
    sockets: Query<(Option<&Children>, Has<CarryHold>)>,
    held: Query<&Held>,
    parents: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
) {
    for (root, mut hands) in &mut bodies {
        for side in &mut hands.sides {
            let Ok((children, posed)) = sockets.get(side.socket) else {
                continue;
            };
            let look = children.and_then(|c| c.iter().find_map(|each| held.get(each).ok()));
            let Some(look) = look.filter(|look| !posed && look.carried() > 0.0) else {
                continue;
            };
            let share = look.carried();
            // The elbow, as the clip posed it this frame (one left as written is left so).
            if let Ok(now) = transforms.get(side.lower).map(|t| t.rotation)
                && side.elbow.clip(now) == now
                && let Ok(up) = parents.get(side.lower).map(ChildOf::parent)
            {
                let at = |bone| Vec3::from(in_body(bone, root, &parents, &transforms).translation);
                let (shoulder, elbow, wrist) = (at(up), at(side.lower), at(side.hand));
                if let Some((axis, open)) = elbow_open(elbow - shoulder, wrist - elbow) {
                    let parent = in_body(up, root, &parents, &transforms)
                        .to_scale_rotation_translation()
                        .1;
                    let turn = Quat::from_axis_angle(axis, -open * share);
                    let written = side.elbow.turn(now, parent, turn);
                    if let Ok(mut t) = transforms.get_mut(side.lower) {
                        t.rotation = written;
                    }
                }
            }
            // The wrist, about the socket's X in the hand's frame, where the look tips.
            let (Ok(now), Ok(socket)) = (
                transforms.get(side.hand).map(|t| t.rotation),
                transforms.get(side.socket).map(|t| t.rotation),
            ) else {
                continue;
            };
            if side.wrist.clip(now) != now {
                continue;
            }
            let deviation = Quat::from_axis_angle(socket * Vec3::X, look.at_wrist().to_radians());
            let written = side
                .wrist
                .turn(now, Quat::IDENTITY, now * deviation * now.inverse());
            if let Ok(mut t) = transforms.get_mut(side.hand) {
                t.rotation = written;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A jog's forearm swung up ahead is lowered until it points `FOREARM_DOWN` below level,
    /// its upper arm left; a hanging one is left; one that cannot get there opens only so far.
    #[test]
    fn a_carried_tool_keeps_its_forearm_down_and_its_elbow_bent() {
        let rise = |v: Vec3| libm::asinf(v.y / v.length()).to_degrees();
        // Upper arm down and a little ahead, forearm level ahead (a jog's swing forward).
        let upper = Vec3::new(0.0, -0.28, 0.08);
        let fore = Vec3::new(0.0, 0.0, 0.26);
        let (axis, open) = elbow_open(upper, fore).expect("a level forearm is lowered");
        let lowered = Quat::from_axis_angle(axis, -open) * fore;
        assert!(
            (rise(lowered) + FOREARM_DOWN).abs() < 1.0,
            "{}",
            rise(lowered)
        );
        assert!((lowered.length() - fore.length()).abs() < 1e-4);
        // Lowered toward the upper arm, in their plane: the elbow less bent, ahead still.
        assert!(lowered.angle_between(upper) < fore.angle_between(upper));
        assert!(lowered.z > 0.0 && lowered.x.abs() < 1e-4);
        // Hanging already: left.
        assert!(elbow_open(upper, Vec3::new(0.0, -0.25, 0.05)).is_none());
        // An upper arm swung up ahead: the forearm can only come down to near straight.
        let raised = Vec3::new(0.0, 0.0, 0.28);
        let (axis, open) = elbow_open(raised, Vec3::new(0.0, 0.2, 0.15)).expect("bent");
        let most = Quat::from_axis_angle(axis, -open) * Vec3::new(0.0, 0.2, 0.15);
        let left = most.angle_between(raised).to_degrees();
        assert!((left - ELBOW_MOST_OPEN).abs() < 1.0, "{left}");
    }
}
