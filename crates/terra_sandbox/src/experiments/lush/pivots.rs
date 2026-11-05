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

/// Each limb's and clump's pivot, from the tree's grown limbs; a modelled tree's (a veteran
/// oak's, with no limbs grown) from the masses of its crown, each turning from its fork, the
/// lowest clump's height up its bole.
pub(super) fn of(tree: &Tree) -> Pivots {
    if tree.limbs.iter().all(|limb| limb.depth == 0) {
        let low = tree
            .tips
            .iter()
            .map(|(at, _)| at.y)
            .fold(f32::MAX, f32::min);
        let fork = Vec3::Y * if low < f32::MAX { low } else { tree.trunk_m };
        return pivots::of_masses(fork, &tree.masses, &tree.mass_of, &clumps(tree));
    }
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
