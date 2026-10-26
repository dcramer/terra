//! The theme's dressed kit laid over the cubes (D527, docs/dungeons.md#themes-d527), as
//! Skyrim's modular kits lay pieces over their grid: a barrel vault over each straight
//! corridor cell, a groin vault over every room bay and at a corridor's turns and crossings
//! (by three quarters), an arch over each doorway's head on its room's face, piers where a
//! hall's bays meet (a hall three cells or more both ways; a smaller room's bays span both
//! its cells) and pilasters where they meet its walls, and the loculi's dressed fronts with
//! what lies in them. Stairs are not vaulted yet. The vaults spring from `Kit::spring` and are crowned under the
//! theme's flat ceiling of cubes, which they hide; a pier stands on a column of trim cubes, so
//! it is walked round and the camera kept off it.

use super::{CELL, CUBE, EDGE, Frame, INNER, Kinds, MID, Piece, Stamp, hash, solid};
use crate::Dungeon;
use crate::place::{Cell, Rect, Side, Space};
use crate::theme::Kit;
use glam::{IVec3, Vec3};
use std::collections::HashSet;

/// A pier's column of cubes, floor to 2.5 m, under its capital.
const PIER_TOP: i32 = 4;
/// A loculi wall's height in cubes, 3.5 m: above its upper tier's lintel, under the vault.
const LOCULI_TOP: i32 = 6;
/// The niches' sills, cubes up: 0.5 m and 2 m; each 1 m high and 2 m across.
const TIERS: [i32; 2] = [1, 4];
/// A corridor's 3 m span over the groin's 4 m bay.
const CORRIDOR_BAY: f32 = 0.75;

pub(super) fn lay(dungeon: &Dungeon, kit: &Kit, kinds: &Kinds, out: &mut Stamp) {
    let spring = kit.spring / CUBE;
    let mut piers = HashSet::new();
    for cell in &dungeon.layout.cells {
        let f = Frame::new(cell.at, 0);
        let wall = |d: usize| cell.sides[d] == Side::Wall;
        let open = |d: usize| cell.sides[d] == Side::Open;
        match cell.space {
            Space::Corridor(_) => {
                let at = f.point(MID, spring, MID);
                let (model, facing, scale) = if wall(1) && wall(3) {
                    (&kit.barrel, 2, Vec3::ONE)
                } else if wall(0) && wall(2) {
                    (&kit.barrel, 1, Vec3::ONE)
                } else {
                    (&kit.groin, 2, Vec3::splat(CORRIDOR_BAY))
                };
                out.pieces.push(piece(model, at, facing, scale));
            }
            Space::Room(r) => {
                let bays = Bays::of(dungeon.layout.rooms[r]);
                // A merged bay is laid once, from the room's first cell on that axis.
                let first = [cell.at.x == bays.min.x, cell.at.z == bays.min.z];
                if !((bays.merged[0] && !first[0]) || (bays.merged[1] && !first[1])) {
                    // The bay from each wall's face, or from the open edge where the next
                    // bay's piers stand; across both cells where the bays merge.
                    let reach = |merged: bool, open: bool| match (merged, open) {
                        (true, _) => CELL + EDGE,
                        (false, true) => CELL,
                        (false, false) => EDGE,
                    };
                    let x = [if open(3) { 0 } else { 1 }, reach(bays.merged[0], open(1))];
                    let z = [if open(0) { 0 } else { 1 }, reach(bays.merged[1], open(2))];
                    let mid = |e: [i32; 2]| (e[0] + e[1]) as f32 * 0.5;
                    let span = |e: [i32; 2]| (e[1] - e[0]) as f32 * CUBE / 4.0;
                    out.pieces.push(piece(
                        &kit.groin,
                        f.point(mid(x), spring, mid(z)),
                        2,
                        Vec3::new(span(x), 1.0, span(z)),
                    ));
                }
                room_piers(cell, f, kit, kinds, bays, &mut piers, out);
            }
            Space::Stair(_) => continue,
        }
        // A doorway's arch on its room's face only: on a corridor's, the barrel runs to the
        // wall and frames the doorway itself, as a barrel corridor's end does; the ring there
        // (extrados 1.72 m round its 3 m centre) stood in the barrel's 1.5 m with a dark
        // crescent between (round 2's still).
        let corridor = matches!(cell.space, Space::Corridor(_));
        for d in 0..4 {
            if !corridor && matches!(cell.sides[d], Side::Door(_)) {
                let f = Frame::new(cell.at, d);
                let at = f.point(MID, 0.0, 1.0);
                out.pieces.push(piece(&kit.arch, at, f.side(2), Vec3::ONE));
            }
        }
    }
}

/// How a room is vaulted: in bays a cell square on free-standing piers where it is three
/// cells or more both ways, a hall; otherwise with no pier standing free, a room two cells
/// across vaulted in one bay across both, its ribs landing on the walls (the tour's camera,
/// string-pulled past piers in every room, had one within a metre in 3.8% of frames).
#[derive(Clone, Copy)]
struct Bays {
    min: IVec3,
    /// Piers stand free where four bays meet.
    hall: bool,
    /// Its bays run across both its cells in x, in z.
    merged: [bool; 2],
}

impl Bays {
    fn of(room: Rect) -> Bays {
        let hall = room.size.x >= 3 && room.size.y >= 3;
        Bays {
            min: room.min,
            hall,
            merged: [!hall && room.size.x == 2, !hall && room.size.y == 2],
        }
    }
}

/// The piers at a room cell's corners: a pier where four of its bays meet, a pilaster where
/// two meet at a wall (half of it in the wall), a quarter in a corner, none in a corner by a
/// doorway (it would stand in the way), and none against a wall where the hall is a single
/// bay across from it; each once.
fn room_piers(
    cell: &Cell,
    f: Frame,
    kit: &Kit,
    kinds: &Kinds,
    bays: Bays,
    placed: &mut HashSet<IVec3>,
    out: &mut Stamp,
) {
    // Each corner: its north or south side, its east or west side, and where it is.
    for (ns, ew, cx, cz) in [
        (0, 3, 0, 0),
        (0, 1, CELL, 0),
        (2, 1, CELL, CELL),
        (2, 3, 0, CELL),
    ] {
        let (a, b) = (cell.sides[ns], cell.sides[ew]);
        // In from a walled side to the wall's face.
        let ix = if ew == 3 { 1 } else { -1 };
        let iz = if ns == 0 { 1 } else { -1 };
        let door = |s: Side| matches!(s, Side::Door(_) | Side::Way);
        // The room runs on past the cell away from a wall: a pilaster or a corner's pier only
        // where the hall is two bays or more across from that wall, so a gallery a cell wide
        // keeps its 3 m clear and its ribs land on the walls' imposts (the tour's camera
        // looked into a pilaster within a metre in 3.8% of frames).
        let across = |d: usize| cell.sides[(d + 2) % 4] == Side::Open;
        // The cube standing in front of a wall's face at `at`, in from it by `inward`.
        let front = |at: i32, inward: i32| if inward > 0 { at } else { at - 1 };
        let (x, z, xs, zs) = match (a == Side::Open, b == Side::Open) {
            (true, true) if !bays.hall => continue,
            (true, true) => (cx, cz, [cx - 1, cx], [cz - 1, cz]),
            // Within a merged bay no pilaster.
            (true, false) if !across(ew) || bays.merged[1] => continue,
            (false, true) if !across(ns) || bays.merged[0] => continue,
            (false, false) if !(across(ns) && across(ew)) => continue,
            // Along an east or west wall.
            (true, false) => {
                let x = cx + ix;
                (x, cz, [front(x, ix); 2], [cz - 1, cz])
            }
            (false, true) => {
                let z = cz + iz;
                (cx, z, [cx - 1, cx], [front(z, iz); 2])
            }
            (false, false) if door(a) || door(b) => continue,
            (false, false) => {
                let (x, z) = (cx + ix, cz + iz);
                (x, z, [front(x, ix); 2], [front(z, iz); 2])
            }
        };
        let key = f.cube(x, 0, z);
        if !placed.insert(key) {
            continue;
        }
        f.fill(
            &mut out.ops,
            [xs[0], 0, zs[0]],
            [xs[1], PIER_TOP, zs[1]],
            solid(kinds.trim),
        );
        out.pieces.push(piece(
            &kit.pier,
            f.point(x as f32, 0.0, z as f32),
            2,
            Vec3::ONE,
        ));
    }
}

/// A loculi wall (`Class::Alcove`) in the kit's way: its north wall thickened a cube to
/// 3.5 m, two tiers of niches 2 m across and 1 m high cut half a metre into it, each with its
/// dressed front and, in two of three, its dead.
pub(super) fn loculi(f: Frame, cell: &Cell, kit: &Kit, kinds: &Kinds, out: &mut Stamp) {
    f.fill(
        &mut out.ops,
        [1, 0, 1],
        [INNER, LOCULI_TOP, 1],
        solid(kinds.wall),
    );
    for (tier, y) in TIERS.into_iter().enumerate() {
        f.fill(&mut out.ops, [2, y, 1], [5, y + 1, 1], None);
        let at = f.point(MID, y as f32, 2.0);
        out.pieces.push(piece(&kit.niche, at, f.side(2), Vec3::ONE));
        let pick = hash(cell.at, 430 + tier as u32) as usize % (kit.remains.len() + 1);
        if let Some(remains) = kit.remains.get(pick) {
            let at = f.point(MID, y as f32, 1.5);
            out.pieces.push(piece(remains, at, f.side(2), Vec3::ONE));
        }
    }
}

fn piece(model: &str, at: Vec3, facing: usize, scale: Vec3) -> Piece {
    Piece {
        model: model.to_string(),
        at,
        facing,
        scale,
    }
}
