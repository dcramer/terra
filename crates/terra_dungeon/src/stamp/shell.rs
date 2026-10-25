//! The shell: every cell's floor and ceiling slabs, a wall on each walled side, doorways and a
//! stair's ways as openings in the wall (`Fill::Open`, so building still closes each space),
//! and the stairs. Positions are a `Frame`'s: a side's wall is the north edge of the cell seen
//! from that side; a stair is drawn rising north and turned to its `up`.

use super::{Frame, Kinds, opening, shaped, solid};
use crate::place::{Cell, Layout, Side, Space, Stair};
use terra_build::{BuildOp, Facing, Shape};

/// How much of a slab a cell has.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Slab {
    Whole,
    /// Over the stair's last three columns, at the top cell's front.
    Front,
    None,
}

pub(super) fn lay(layout: &Layout, kinds: &Kinds, ops: &mut Vec<BuildOp>) {
    // Every ceiling before any floor: a room's floor laid over the ceiling under it is that
    // ceiling, in the floors' kind.
    for floor in [false, true] {
        for cell in &layout.cells {
            slabs(layout, cell, floor, kinds, ops);
        }
    }
    for cell in &layout.cells {
        for (d, &side) in cell.sides.iter().enumerate() {
            wall(Frame::new(cell.at, d), side, kinds, ops);
        }
    }
    for stair in &layout.stairs {
        steps(stair, kinds, ops);
        if stair.edge.is_none() {
            surface(stair, kinds, ops);
        }
    }
}

/// The floor slab under a cell (`floor`), in the floors' kind, or the ceiling slab over it,
/// in the ceilings' (the next level's floor where a room is over it); a stair opens both
/// over its rise: no ceiling over the lower cells but the top's front, no floor in the well
/// and only the front of the landing's. The surface stair has no ceiling.
fn slabs(layout: &Layout, cell: &Cell, floor: bool, kinds: &Kinds, ops: &mut Vec<BuildOp>) {
    let laying = floor;
    let (floor, ceiling, turn) = match cell.space {
        Space::Stair(s) => {
            let stair = &layout.stairs[s];
            let i = stair.cells.iter().position(|&c| c == cell.at).unwrap_or(0);
            let (floor, ceiling) = match (i, stair.edge.is_none()) {
                (_, true) | (0, _) => (Slab::Whole, Slab::None),
                (1, _) => (Slab::Whole, Slab::Front),
                (2, _) => (Slab::None, Slab::Whole),
                _ => (Slab::Front, Slab::Whole),
            };
            (floor, ceiling, stair.up)
        }
        _ => (Slab::Whole, Slab::Whole, 0),
    };
    let f = Frame::new(cell.at, turn);
    let (slab, y, kind) = if laying {
        (floor, -1, kinds.floor)
    } else {
        (ceiling, 7, kinds.ceiling)
    };
    match slab {
        Slab::Whole => f.fill(ops, [0, y, 0], [5, y, 5], solid(kind)),
        Slab::Front => f.fill(ops, [0, y, 0], [5, y, 2], solid(kind)),
        Slab::None => {}
    }
}

/// A doorway's opening in cubes, across and up, between its jambs and under its lintel.
pub(super) const DOORWAY: [i32; 2] = [4, 5];

/// A side of a cell, seen from it (its north edge in the frame): a wall a cube thick from the
/// floor slab to the ceiling slab; a doorway 4 cubes wide and 5 high with trim jambs and
/// lintel; a stair's way 4 wide and 6 high.
fn wall(f: Frame, side: Side, kinds: &Kinds, ops: &mut Vec<BuildOp>) {
    if side == Side::Open {
        return;
    }
    f.fill(ops, [0, -1, 0], [5, 7, 0], solid(kinds.wall));
    match side {
        Side::Door(_) => {
            let [wide, high] = DOORWAY;
            f.fill(ops, [1, 0, 0], [wide, high - 1, 0], opening(kinds.wall));
            f.fill(ops, [0, 0, 0], [0, high, 0], solid(kinds.trim));
            f.fill(
                ops,
                [wide + 1, 0, 0],
                [wide + 1, high, 0],
                solid(kinds.trim),
            );
            f.fill(ops, [1, high, 0], [wide, high, 0], solid(kinds.trim));
        }
        Side::Way => f.fill(ops, [1, 0, 0], [4, 5, 0], opening(kinds.wall)),
        _ => {}
    }
}

/// A stair's twelve columns across its two lower cells, rising north in its frame: two flat,
/// eight steps each a cube up (a stair cube on top, two quarter-metre treads), two at the
/// upper floor; four cubes wide between the side walls.
fn steps(stair: &Stair, kinds: &Kinds, ops: &mut Vec<BuildOp>) {
    for c in 2..12 {
        let cell = stair.cells[if c < 6 { 0 } else { 1 }];
        let f = Frame::new(cell, stair.up);
        let z = 5 - c % 6;
        if c >= 10 {
            f.fill(ops, [1, 0, z], [4, 7, z], solid(kinds.floor));
            continue;
        }
        let top = c - 2;
        if top > 0 {
            f.fill(ops, [1, 0, z], [4, top - 1, z], solid(kinds.floor));
        }
        f.fill(
            ops,
            [1, top, z],
            [4, top, z],
            shaped(kinds.floor, Shape::Stair(Facing::North)),
        );
    }
}

/// The stair up to the surface: its walls carried two cubes above ground as a parapet, round
/// its back too, and an arch of trim over its top.
fn surface(stair: &Stair, kinds: &Kinds, ops: &mut Vec<BuildOp>) {
    let last = stair.cells.len() - 1;
    for (i, &cell) in stair.cells.iter().enumerate() {
        let f = Frame::new(cell, stair.up);
        f.fill(ops, [0, 8, 0], [0, 9, 5], solid(kinds.wall));
        f.fill(ops, [5, 8, 0], [5, 9, 5], solid(kinds.wall));
        if i == 0 {
            f.fill(ops, [0, 8, 5], [5, 9, 5], solid(kinds.wall));
        }
        if i == last {
            f.fill(ops, [0, 8, 0], [0, 13, 0], solid(kinds.trim));
            f.fill(ops, [5, 8, 0], [5, 13, 0], solid(kinds.trim));
            f.fill(ops, [0, 13, 0], [5, 13, 0], solid(kinds.trim));
        }
    }
}
