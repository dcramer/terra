//! The carried arm's swing (D584, docs/animation.md step 3): an arm the hold layer lays a carry
//! pose on (`figure`'s one-arm hold) swings as the stride's own arm does, at the pose's share.
//! Each stride loop has, for each arm, an additive clip baked from it offline
//! (`tools/characters/additive.ts`: the arm's rotations less the loop's mean pose of them); the
//! graph adds it over the pose the clips and holds blend (`layer::graph_named`'s additive node),
//! so the swing is part of the clip blend and not a layer after it. It plays in step with its
//! loop (the same seek time) at the loop's weight times the pose's share (its set's `swing`, the
//! clips file's extras): a carried hammer swings half as far as an empty hand, a lit torch a
//! quarter. Ported from ALS v4's overlay layering, whose `Arm_R_Add` curve sets per overlay pose
//! how much of the locomotion's arm is added over it.

use crate::layer::Nodes;
use bevy::{gltf::GltfNode, platform::collections::HashMap, prelude::*};

/// An arm's swing playing over a loop: the loop's node, the swing's, and the pose's share.
pub(crate) struct Swing {
    main: AnimationNodeIndex,
    node: AnimationNodeIndex,
    share: f32,
}

/// Each carry pose's share of the stride's arm swing, by the pose's name: the clips file's
/// extras `swing`, which `additive.ts` writes from the set. A pose not named swings none.
pub fn shares(gltf: &Gltf, nodes: &Assets<GltfNode>) -> HashMap<String, f32> {
    gltf.nodes
        .iter()
        .filter_map(|node| nodes.get(node)?.extras.as_ref())
        .find_map(|extras| {
            let value: serde_json::Value = serde_json::from_str(&extras.value).ok()?;
            serde_json::from_value(value.get("swing")?.clone()).ok()
        })
        .unwrap_or_default()
}

/// Starts the swing of each arm (left, right) carrying in a pose (`posed`) over `main`, which
/// has just started as copy `copy` of the clip of `nodes`, at that arm's pose's share; a clip
/// with no swing for the arm starts none.
pub(crate) fn start(
    swings: &mut Vec<Swing>,
    animator: &mut AnimationPlayer,
    (nodes, copy, main): (&Nodes, usize, AnimationNodeIndex),
    posed: [bool; 2],
    share: [f32; 2],
) {
    for arm in 0..2 {
        let Some(node) = nodes.swing[copy][arm] else {
            continue;
        };
        swings.retain(|s| s.node != node);
        if !posed[arm] || share[arm] <= 0.0 {
            animator.stop(node);
            continue;
        }
        animator.start(node).repeat().set_speed(0.0).set_weight(0.0);
        swings.push(Swing {
            main,
            node,
            share: share[arm],
        });
    }
}

/// Each swing at its loop's seek time, with its share of the loop's weight as the transitions
/// fade it, and stopped with it.
pub(crate) fn ride(swings: &mut Vec<Swing>, animator: &mut AnimationPlayer) {
    swings.retain(|s| {
        let Some((weight, at)) = animator
            .animation(s.main)
            .map(|a| (a.weight(), a.seek_time()))
        else {
            animator.stop(s.node);
            return false;
        };
        match animator.animation_mut(s.node) {
            Some(swing) => {
                swing.set_weight(weight * s.share).seek_to(at);
                true
            }
            None => false,
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A swing follows its loop: its share of the loop's weight, the loop's seek time, and it
    /// stops when the loop does; an arm not carrying in a pose, or at no share, starts none.
    #[test]
    fn a_swing_rides_its_loop_at_its_share_and_stops_with_it() {
        let mut graph = AnimationGraph::new();
        let clip = Handle::<AnimationClip>::default();
        let mut add = || graph.add_clip(clip.clone(), 1.0, graph.root);
        let (main, left, right) = (add(), add(), add());
        let nodes = Nodes {
            whole: [main; 2],
            under: [main; 2],
            over: main,
            under_one: [[main; 2]; 2],
            over_one: [main; 2],
            swing: [[Some(left), Some(right)], [None, None]],
        };
        let mut animator = AnimationPlayer::default();
        animator.start(main).set_weight(0.6).seek_to(0.25);
        let mut swings = Vec::new();
        start(
            &mut swings,
            &mut animator,
            (&nodes, 0, main),
            [false, true],
            [0.5, 0.5],
        );
        assert!(
            animator.animation(left).is_none(),
            "the left arm carries nothing"
        );
        ride(&mut swings, &mut animator);
        let swing = animator.animation(right).unwrap();
        assert!((swing.weight() - 0.3).abs() < 1e-6 && swing.seek_time() == 0.25);
        animator.stop(main);
        ride(&mut swings, &mut animator);
        assert!(swings.is_empty() && animator.animation(right).is_none());
        start(
            &mut swings,
            &mut animator,
            (&nodes, 0, main),
            [true, true],
            [0.0, 0.0],
        );
        assert!(swings.is_empty(), "no share, no swing");
    }
}
