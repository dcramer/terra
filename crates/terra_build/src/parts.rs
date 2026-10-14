//! Parts that touch and agree are one (D260): a part groups cubes for the interface, the
//! order they go in and what they cost, and nothing else, so cubes laid by hand beside cubes
//! of their kind laid on the same terms join their part, two such parts a cube joins become
//! one, and a part taken apart into pieces that no longer touch is split, each piece its own
//! part on the same terms. Building cube by cube then needs a part per connected mass of a
//! kind, not per click, and a section's palette of 255 parts (`cubes`) is never the limit.
//!
//! These only choose; the moves are [`Op::Regroup`], checked and undone like any other.

use crate::Base;
use crate::cost::Terms;
use crate::cubes::{Kind, PartId};
use crate::ops::Op;
use crate::support::FACES;
use glam::IVec3;
use std::collections::{BTreeMap, BTreeSet};

impl Base {
    /// The part cubes of a kind laid on these terms join, and the operations that merge
    /// into it every other part they touch that agrees, to apply before they are laid: the
    /// lowest numbered of the parts of the kind and terms beside them. `None` if they touch
    /// none, and so are a part of their own.
    pub fn joining(&self, cubes: &[IVec3], kind: Kind, terms: &Terms) -> Option<(PartId, Vec<Op>)> {
        let laying: BTreeSet<[i32; 3]> = cubes.iter().map(|at| at.to_array()).collect();
        let mut touching = BTreeSet::new();
        for &at in cubes {
            for step in FACES {
                let beside = at + step;
                if laying.contains(&beside.to_array()) {
                    continue;
                }
                let Some(id) = self.cube(beside).and_then(|cube| cube.part) else {
                    continue;
                };
                if self
                    .part(id)
                    .is_some_and(|part| part.kind == kind && part.terms == *terms)
                {
                    touching.insert(id);
                }
            }
        }
        let into = *touching.first()?;
        let mut merges: BTreeMap<PartId, Vec<[i32; 3]>> = BTreeMap::new();
        for (at, cube) in self.cubes() {
            if let Some(id) = cube.part.filter(|id| *id != into && touching.contains(id)) {
                merges.entry(id).or_default().push(at.to_array());
            }
        }
        let merges = merges
            .into_values()
            .map(|cubes| Op::Regroup { cubes, part: into })
            .collect();
        Some((into, merges))
    }

    /// The operations that split the parts taking cubes away leaves in pieces, to apply once
    /// they are gone: each piece of a part that no longer touches the rest, face to face,
    /// its own part from the base's next number on; the piece with the first cube in the
    /// sorted order keeps the part's number.
    pub fn splitting(&self, taken: &[IVec3]) -> Vec<Op> {
        let gone: BTreeSet<[i32; 3]> = taken.iter().map(|at| at.to_array()).collect();
        let parts: BTreeSet<PartId> = taken
            .iter()
            .filter_map(|&at| self.cube(at).and_then(|cube| cube.part))
            .collect();
        if parts.is_empty() {
            return Vec::new();
        }
        let mut left: BTreeMap<PartId, BTreeSet<[i32; 3]>> = BTreeMap::new();
        for (at, cube) in self.cubes() {
            if let Some(id) = cube.part.filter(|id| parts.contains(id))
                && !gone.contains(&at.to_array())
            {
                left.entry(id).or_default().insert(at.to_array());
            }
        }
        let mut next = self.next_part().0;
        let mut ops = Vec::new();
        for cubes in left.values() {
            for piece in pieces(cubes).into_iter().skip(1) {
                ops.push(Op::Regroup {
                    cubes: piece,
                    part: PartId(next),
                });
                next += 1;
            }
        }
        ops
    }
}

/// Cubes in pieces that touch face to face, each sorted, in the order of their first cubes.
fn pieces(cubes: &BTreeSet<[i32; 3]>) -> Vec<Vec<[i32; 3]>> {
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut out = Vec::new();
    for &start in cubes {
        if !seen.insert(start) {
            continue;
        }
        let mut piece = vec![start];
        let mut frontier = vec![start];
        while let Some(at) = frontier.pop() {
            for step in FACES {
                let next = (IVec3::from_array(at) + step).to_array();
                if cubes.contains(&next) && seen.insert(next) {
                    piece.push(next);
                    frontier.push(next);
                }
            }
        }
        piece.sort();
        out.push(piece);
    }
    out
}
