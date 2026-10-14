//! Trims round openings (docs/building-kits.md): where courses of long members are cut by
//! an opening of any shape, a frame of boards along its edges, jambs, a head and a sill.

use super::{Axis, Layout, Piece, Place, Walls, runs_of};
use crate::kit::Kit;
use crate::{CELL_M, Fill};
use std::collections::BTreeSet;

/// A frame of boards round every opening in a row, of whatever shape: a board along each
/// edge where an open or glass cube meets the wall, edges in a line one board. Up the
/// sides they are jambs; over the top a head, reaching up into the first log above; under
/// it a sill, unless the opening is at the foot of its wall, as a doorway is.
pub(super) fn frames(walls: &Walls, kit: &Kit, axis: Axis, row: i32, layout: &mut Layout) {
    let mut holes: BTreeSet<(i32, i32)> = BTreeSet::new();
    for (at, cube) in super::Scope::row_cubes(walls.base, axis, row) {
        let (_, along) = axis.row_of(at);
        if super::drawn_fill(&cube) != Fill::Solid && walls.runs(at, axis) {
            holes.insert((along, at.y));
        }
    }
    // Posts up every opening's sides, where the kit dresses jambs with them (D215): from
    // the opening's foot to its head and on into the course over it.
    if let Some(width) = super::runs::post_width(kit) {
        let solid =
            |along: i32, layer: i32| walls.ours(axis.cube(row, along, layer)) == Some(Fill::Solid);
        for outward in [-1, 1] {
            let edges: BTreeSet<(i32, i32)> = holes
                .iter()
                .copied()
                .filter(|&(along, layer)| solid(along + outward, layer))
                .collect();
            for (along, low, high) in runs_of(&edges, |(along, layer)| (along, layer)) {
                let face = (along + i32::from(outward > 0)) as f32 * CELL_M;
                let middle = face + outward as f32 * width * 0.5;
                let over = if solid(along, high + 1) {
                    kit.course
                } else {
                    0.0
                };
                let mut post = super::runs::log(
                    axis.point(row, middle, low as f32 * CELL_M),
                    axis.point(row, middle, (high + 1) as f32 * CELL_M + over),
                    super::Split::Whole,
                    super::hash(&[row as u32, along as u32, low as u32, outward as u32, 11]),
                    None,
                );
                super::runs::set_role(&mut post, Some(super::roles::Role::Jamb));
                layout.pieces.push(post);
            }
        }
    }
    let posts = super::runs::post_width(kit).is_some();
    let solid =
        |along: i32, layer: i32| walls.ours(axis.cube(row, along, layer)) == Some(Fill::Solid);
    let per_cube = (CELL_M / kit.course).round() as i32;
    let depth = kit.radius() + 0.02;
    let across = axis.across();
    let mut board = |a: f32, b: f32, bottom: f32, top: f32, proud: f32| {
        let (low, high) = (axis.point(row, a, bottom), axis.point(row, b, top));
        let out = across * (depth + proud);
        layout.pieces.push(Piece::Board {
            min: Place::near((low - out).min(high + out)),
            max: Place::near((low - out).max(high + out)),
        });
    };
    // Up each side: a jamb along every run of hole cubes with wall beside them, unless a
    // post stands there.
    for outward in [-1, 1] {
        if posts {
            continue;
        }
        let edges: BTreeSet<(i32, i32)> = holes
            .iter()
            .copied()
            .filter(|&(along, layer)| solid(along + outward, layer))
            .collect();
        for (along, low, high) in runs_of(&edges, |(along, layer)| (along, layer)) {
            let face = (along + i32::from(outward > 0)) as f32 * CELL_M;
            let (a, b) = if outward > 0 {
                (face, face + kit.frame)
            } else {
                (face - kit.frame, face)
            };
            board(a, b, low as f32 * CELL_M, (high + 1) as f32 * CELL_M, 0.0);
        }
    }
    // Over the top and under the bottom: a head and a sill along every run of hole cubes
    // with wall over or under them.
    for upward in [1, -1] {
        let edges: BTreeSet<(i32, i32)> = holes
            .iter()
            .copied()
            .filter(|&(along, layer)| solid(along, layer + upward))
            .collect();
        for (layer, low, high) in runs_of(&edges, |(along, layer)| (layer, along)) {
            let (a, b) = (
                low as f32 * CELL_M - kit.frame,
                (high + 1) as f32 * CELL_M + kit.frame,
            );
            let near = layer * per_cube;
            if upward > 0 {
                let edge = (layer + 1) as f32 * CELL_M;
                // Up to the middle of the first course wholly above it.
                let head = (near - 2..near + 2 * per_cube + 2)
                    .map(|course| axis.centre(kit, course))
                    .find(|&centre| centre - kit.course * 0.5 >= edge - 1e-3)
                    .map_or(edge + 0.1, |centre| centre.max(edge + 0.06));
                board(a, b, edge, head, 0.01);
            } else {
                let edge = layer as f32 * CELL_M;
                let foot = (near - 2 * per_cube - 2..near + 2)
                    .rev()
                    .map(|course| axis.centre(kit, course))
                    .find(|&centre| centre + kit.course * 0.5 <= edge + 1e-3)
                    .map_or(edge - 0.1, |centre| centre.min(edge - 0.06));
                board(a, b, foot, edge, 0.03);
            }
        }
    }
}

/// Where a wall of the kit's kind ends against a wall of another kind, as logs end flush
/// against stone (D179): an end board up the joint, over the logs' sawn ends, from the
/// foot of each stack of such cubes to its top, where the kits say it ends there with a
/// board (D232: the other runs through it, or neither does).
pub(super) fn ends(
    walls: &Walls,
    kit: &Kit,
    kits: &crate::kit::Kits,
    ours: &[glam::IVec3],
    layout: &mut Layout,
) {
    let other = |at: glam::IVec3| {
        walls.base.cube(at).is_some_and(|cube| {
            cube.kind != walls.kind
                && cube.kind != crate::Kind::PLAIN
                && cube.fill == Fill::Solid
                && walls.base.in_wall(at)
                && kits.end(walls.kind, cube.kind) == crate::kit::Joint::Board
        })
    };
    let ends_at = |at: glam::IVec3, axis: Axis, outward: i32| {
        walls.ours(at) == Some(Fill::Solid)
            && walls.runs(at, axis)
            && other(at + axis.step() * outward)
    };
    for axis in [Axis::X, Axis::Z] {
        for outward in [-1, 1] {
            for &at in ours {
                // From the foot of each stack.
                if !ends_at(at, axis, outward) || ends_at(at - glam::IVec3::Y, axis, outward) {
                    continue;
                }
                let mut top = at;
                while ends_at(top + glam::IVec3::Y, axis, outward) {
                    top += glam::IVec3::Y;
                }
                let (row, along) = axis.row_of(at);
                let face = (along + i32::from(outward > 0)) as f32 * CELL_M;
                let (a, b) = (face, face - outward as f32 * kit.frame * 1.2);
                let out = axis.across() * (kit.radius() + 0.03);
                let low = axis.point(row, a.min(b), at.y as f32 * CELL_M) - out;
                let high = axis.point(row, a.max(b), (top.y + 1) as f32 * CELL_M) + out;
                layout.pieces.push(Piece::Board {
                    min: Place::near(low.min(high)),
                    max: Place::near(low.max(high)),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{boards, cabin, kit, logs, wall};
    use super::super::{Layout, lay};
    use super::*;
    use crate::{BuildOp, Cube, DOOR_HEIGHT_M, Panel, STOREY_CUBES};
    use glam::Vec3;

    #[test]
    fn a_door_cuts_the_courses_it_crosses_and_is_framed() {
        let mut base = cabin();
        // A door a metre wide in the south wall.
        wall(&mut base, [[3, 0, 5], [4, 0, 5]], Panel::Door);
        let kit = kit();
        let layout = lay(&base, &kit);
        let south: Vec<_> = logs(&layout)
            .into_iter()
            .filter(|(from, to, _)| from.z == 2.75 && to.z == 2.75)
            .collect();
        // Every course the door crosses is two logs, ending at the jambs.
        let crossed = south
            .iter()
            .filter(|(from, _, _)| from.y < DOOR_HEIGHT_M)
            .count();
        let whole = south.len() - crossed;
        assert_eq!(crossed % 2, 0);
        assert_eq!((crossed / 2 + whole) as i32, kit.courses().0);
        for (from, to, _) in &south {
            let clear = to.x <= 1.5 - kit.frame + 1e-4 || from.x >= 2.5 + kit.frame - 1e-4;
            assert!(clear || from.y > DOOR_HEIGHT_M, "{from} {to}");
        }
        // Two jambs and a head; a doorway needs no sill.
        assert_eq!(boards(&layout), 3);
    }

    #[test]
    fn a_window_has_a_sill_and_a_cube_carved_out_is_framed_too() {
        let mut base = cabin();
        wall(&mut base, [[0, 0, 2]], Panel::WINDOW);
        let kit = kit();
        let before = lay(&base, &kit);
        // Two jambs, a head and a sill.
        assert_eq!(boards(&before), 4);
        // One cube a metre and a half up the north wall, carved out.
        base.apply(BuildOp::Fill {
            min: [3, 3, 0],
            max: [3, 3, 0],
            cube: Some(Cube::new(kit.kind(), Fill::Open)),
        });
        let after = lay(&base, &kit);
        assert_eq!(boards(&after), 8);
        // Every course through it is cut, so the north wall has a log more for each.
        let north = |layout: &Layout| {
            logs(layout)
                .iter()
                .filter(|(from, to, _)| from.z == 0.25 && to.z == 0.25)
                .count()
        };
        let through = (CELL_M / kit.course).round() as usize;
        assert_eq!(north(&after), north(&before) + through);
    }

    #[test]
    fn logs_ending_against_stone_have_an_end_board_up_the_joint() {
        let mut base = crate::Base::default();
        let stone = crate::Kind(2);
        wall(&mut base, (0..6).map(|x| [x, 0, 0]), Panel::Solid);
        // Its east end a stone pillar.
        base.apply(BuildOp::Fill {
            min: [5, 0, 0],
            max: [5, STOREY_CUBES - 1, 0],
            cube: Some(Cube::new(stone, Fill::Solid)),
        });
        let layout = lay(&base, &kit());
        let ends: Vec<(Vec3, Vec3)> = layout
            .pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Board { min, max } => Some((min.metres(), max.metres())),
                _ => None,
            })
            .collect();
        // One board, the storey's height, its east face at the stone's face.
        assert_eq!(ends.len(), 1, "{ends:?}");
        let (a, b) = ends[0];
        assert!(
            (b.x - 2.5).abs() < 0.01 && a.x < 2.5 && a.x > 2.3,
            "{a} {b}"
        );
        assert!(
            a.y.abs() < 0.01 && (b.y - crate::STOREY_M).abs() < 0.01,
            "{a} {b}"
        );
    }
}
