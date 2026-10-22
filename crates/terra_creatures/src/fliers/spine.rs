//! A flier's neck and tail laid over its clips (D382): its head leading into each turn and
//! climb, its tail trailing, and its head reared back before it breathes.

use super::{Flier, Kinds, ease};
use bevy::prelude::*;

/// How far its head leads into a turn, in seconds of the turn, and most; how far its tail
/// swings out behind, the same; how long each tail bone takes to follow the one before.
const NECK_LEAD_S: f32 = 0.55;
const NECK_MOST: f32 = 0.65;
const TAIL_LAG_S: f32 = 0.5;
const TAIL_MOST: f32 = 0.7;
const TAIL_FOLLOW_S: f32 = 0.09;
/// How far it rears its head back before it breathes, in radians.
const REAR: f32 = 0.5;

/// On a flier's model: its neck's and tail's bones, each with the axes in its parent's space
/// that are the body's up and its left at rest, and how far each is turned and trailing.
#[derive(Component)]
pub struct Spine {
    pub flier: Entity,
    pub neck: Vec<(Entity, Vec3, Vec3)>,
    pub tail: Vec<(Entity, Vec3, Vec3)>,
    pub neck_turn: Vec2,
    pub neck_rate: Vec2,
    pub tail_turn: Vec<Vec2>,
}

/// Each flier's neck and tail over its clips: its head turned into its turn and up into its
/// climb ahead of its body, reared back before it breathes and thrown forward as the breath
/// comes; its tail swinging out behind each turn and lagging each pitch, each bone a little
/// after the one before it, as a whip follows.
pub fn spine(
    time: Res<Time>,
    kinds: Res<Kinds>,
    fliers: Query<&Flier>,
    mut spines: Query<&mut Spine>,
    mut bones: Query<&mut Transform, Without<Flier>>,
) {
    let step = time.delta_secs();
    if step <= 0.0 {
        return;
    }
    for mut spine in &mut spines {
        let Ok(flier) = fliers.get(spine.flier) else {
            continue;
        };
        let def = &kinds.defs[flier.kind];
        let yaw = (flier.turn_rate * NECK_LEAD_S).clamp(-NECK_MOST, NECK_MOST);
        let pitch = (-flier.pitch_rate * NECK_LEAD_S).clamp(-NECK_MOST, NECK_MOST)
            - REAR * flier.rearing(def);
        let Spine {
            neck_turn,
            neck_rate,
            ..
        } = &mut *spine;
        // Lightly under-damped, so the throw forward as the breath comes overshoots.
        for (value, rate, target) in [
            (&mut neck_turn.x, &mut neck_rate.x, yaw),
            (&mut neck_turn.y, &mut neck_rate.y, pitch),
        ] {
            let w = 9.0;
            *rate += (w * w * (target - *value) - 1.1 * w * *rate) * step;
            *value += *rate * step;
        }
        let tail_wanted = Vec2::new(
            (-flier.turn_rate * TAIL_LAG_S).clamp(-TAIL_MOST, TAIL_MOST),
            (flier.pitch_rate * TAIL_LAG_S).clamp(-TAIL_MOST, TAIL_MOST),
        );
        let follow = ease(TAIL_FOLLOW_S, step);
        let mut before = tail_wanted;
        for turn in &mut spine.tail_turn {
            *turn += (before - *turn) * follow;
            before = *turn;
        }
        let n = spine.neck.len().max(1) as f32;
        let neck_share = |i: usize| 2.0 * (i as f32 + 1.0) / (n * (n + 1.0));
        for (i, (bone, up, left)) in spine.neck.iter().enumerate() {
            if let Ok(mut t) = bones.get_mut(*bone) {
                let share = neck_share(i);
                // Pitch about its left: a turn about the left axis by a positive angle lowers
                // the nose, so up into a climb is the negative.
                t.rotation = Quat::from_axis_angle(*up, spine.neck_turn.x * share)
                    * Quat::from_axis_angle(*left, -spine.neck_turn.y * share)
                    * t.rotation;
            }
        }
        let m = spine.tail.len().max(1) as f32;
        for ((bone, up, left), turn) in spine.tail.iter().zip(&spine.tail_turn) {
            if let Ok(mut t) = bones.get_mut(*bone) {
                t.rotation = Quat::from_axis_angle(*up, turn.x * 1.5 / m)
                    * Quat::from_axis_angle(*left, turn.y * 1.5 / m)
                    * t.rotation;
            }
        }
    }
}

/// Each bone of a chain (`Neck1`, `Neck2`, ...) under a model, from its root out, with the
/// body's up and left at rest in its parent's space.
pub fn chain(
    root: Entity,
    (name, count): &(String, u8),
    children: &Query<&Children>,
    names: &Query<&Name>,
    parents: &Query<&ChildOf>,
    transforms: &Query<&Transform>,
) -> Vec<(Entity, Vec3, Vec3)> {
    // How a node is turned in the model, through its parents up to the model's root.
    let turned = |mut node: Entity| {
        let mut rotation = Quat::IDENTITY;
        while node != root {
            if let Ok(t) = transforms.get(node) {
                rotation = t.rotation * rotation;
            }
            let Ok(parent) = parents.get(node) else {
                break;
            };
            node = parent.parent();
        }
        rotation
    };
    (1..=*count)
        .filter_map(|i| {
            let want = format!("{name}{i}");
            let bone = children
                .iter_descendants(root)
                .find(|&child| names.get(child).is_ok_and(|n| n.as_str() == want))?;
            let parent = parents.get(bone).ok()?.parent();
            // The model faces +Z with +Y up, so its left is +X.
            let inverse = turned(parent).inverse();
            Some((bone, inverse * Vec3::Y, inverse * Vec3::X))
        })
        .collect()
}
