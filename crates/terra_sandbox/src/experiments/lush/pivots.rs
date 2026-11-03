//! Wind in levels (D597) for the lush forest's own trees: their limbs and clumps handed to
//! the world's pivots (`terra_world::grown::pivots`), which every grown tree takes its
//! pivots from, so the lush forest and the world sway alike.

use super::Tree;
use bevy::prelude::*;
use terra_world::grown::pivots::{self, Clump, Line, Pivots};
pub(super) use terra_world::grown::pivots::{ATTRIBUTE_PIVOT, ATTRIBUTE_TWIG, NO_PIVOT};

fn clumps(tree: &Tree) -> Vec<Clump> {
    tree.tips
        .iter()
        .enumerate()
        .map(|(i, &(at, size))| Clump {
            at,
            size,
            along: tree.along.get(i).copied().unwrap_or(Vec3::ZERO),
        })
        .collect()
}

/// Each limb's and clump's pivot, from the tree's grown limbs.
pub(super) fn of(tree: &Tree) -> Pivots {
    let limbs: Vec<Line> = tree
        .limbs
        .iter()
        .map(|limb| Line {
            points: &limb.points,
            depth: limb.depth,
        })
        .collect();
    pivots::of(&limbs, &clumps(tree))
}

/// A material's specialising for meshes carrying pivots: the world's.
pub(super) use terra_world::grown::pivots::specialize;
