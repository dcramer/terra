//! Trims round openings (docs/building-kits.md): where courses of long members are cut by
//! an opening of any shape, a frame of boards along its edges, jambs, a head and a sill.

use super::{Axis, Layout, Piece, Place, Walls, runs_of};
use std::collections::BTreeSet;
use terra_build::kit::Kit;
use terra_build::{CELL_M, Fill};

/// A frame of boards round every opening in a row, of whatever shape: a board along each
/// edge where an open cube meets the wall, edges in a line one board. Up the
/// sides they are jambs; over the top a head, reaching up into the first log above; under
/// it a sill, unless the opening is at the foot of its wall, as a doorway is.
pub(super) fn frames(walls: &Walls, kit: &Kit, axis: Axis, row: i32, layout: &mut Layout) {
    let mut holes: BTreeSet<(i32, i32)> = BTreeSet::new();
    for (at, _) in super::Scope::row_cubes(walls.base, axis, row) {
        let (_, along) = axis.row_of(at);
        if boarded(walls, at, axis) {
            holes.insert((along, at.y));
        }
    }
    jambs(walls, kit, (axis, row), &holes, layout);
    heads_and_sills(walls, kit, (axis, row), &holes, layout);
}

/// Up each side of the openings in a row (`holes`, along and layer): a jamb along every run
/// of hole cubes with wall beside them, a post where the kit dresses jambs with them (D215),
/// else a board.
fn jambs(
    walls: &Walls,
    kit: &Kit,
    (axis, row): (Axis, i32),
    holes: &BTreeSet<(i32, i32)>,
    layout: &mut Layout,
) {
    // A jamb whose wall beside it is itself an opening's over its top or under its foot (a
    // window beside a doorway, over the stub of wall between them) stops at that opening's
    // sill or stands on its head, as a jamb does under a head: the sill and the head lap
    // past their opening over the jamb, and a jamb run to the layer's face stood in them,
    // its end in the plane of their face. `wall` is the wall's run beside the jamb.
    let beside = |(wall, low, high): (i32, i32, i32), (foot, top): (f32, f32)| {
        (
            if holes.contains(&(wall, low - 1)) {
                head_top(kit, axis, low - 1)
            } else {
                foot
            },
            if holes.contains(&(wall, high + 1)) {
                sill_foot(kit, axis, high + 1)
            } else {
                top
            },
        )
    };
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
                // Down into the course under a window's sill too: the course its foot's
                // layer starts in ends at the post, so a post standing on the layer's floor
                // left a hole under it between that course's end and the sill. Half a course
                // and a little in, inside a round log or a squared one, clear of the planes of
                // a squared course's top and foot and of the head's and the sill's faces.
                let over = if solid(along, high + 1) {
                    kit.course * 0.5 + POST_PAST_M
                } else {
                    0.0
                };
                // Else a hair into what it stands on, as a corner post is, out of the plane of
                // a sill's or a split log's foot laid on the same.
                let under = if solid(along, low - 1) {
                    kit.course * 0.5 + POST_PAST_M
                } else {
                    super::runs::CORNER_POST_SUNK_M
                };
                let (foot, top) = beside(
                    (along + outward, low, high),
                    (
                        low as f32 * CELL_M - under,
                        (high + 1) as f32 * CELL_M + over,
                    ),
                );
                let mut post = super::runs::log(
                    axis.point(row, middle, foot),
                    axis.point(row, middle, top),
                    super::Split::Whole,
                    super::hash::of(&[row as u32, along as u32, low as u32, outward as u32, 11]),
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
            let (foot, top) = beside(
                (along + outward, low, high),
                (low as f32 * CELL_M, (high + 1) as f32 * CELL_M),
            );
            board(a, b, foot, top, 0.0);
        }
    }
}

/// Over the top and under the bottom of the openings in a row (`holes`, along and layer): a
/// head and a sill, and a head under a roof's cube over the top.
fn heads_and_sills(
    walls: &Walls,
    kit: &Kit,
    (axis, row): (Axis, i32),
    holes: &BTreeSet<(i32, i32)>,
    layout: &mut Layout,
) {
    let solid =
        |along: i32, layer: i32| walls.ours(axis.cube(row, along, layer)) == Some(Fill::Solid);
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
    // Under a roof's cube over the top, the roof's cube is the head (docs/building-voxels.md,
    // gap 6): a head board along the opening's top under each, cut a hair under the roof's
    // deck, which comes down to the opening's top at an eave.
    let roofed = |along: i32, layer: i32| {
        let at = axis.cube(row, along, layer);
        walls
            .base
            .cube(at)
            .filter(|c| c.kind.is_roof() && c.fill != Fill::Open)
            .and_then(|c| super::roofs::underside(at, c).map(|under| (at, under)))
    };
    let mut heads = Vec::new();
    let edges: BTreeSet<(i32, i32)> = holes
        .iter()
        .copied()
        .filter(|&(along, layer)| !solid(along, layer + 1) && roofed(along, layer + 1).is_some())
        .collect();
    for (layer, low, high) in runs_of(&edges, |(along, layer)| (layer, along)) {
        for along in low..=high {
            let Some((over, under)) = roofed(along, layer + 1) else {
                continue;
            };
            let edge = (layer + 1) as f32 * CELL_M;
            let a = along as f32 * CELL_M - if along == low { kit.frame } else { 0.0 };
            let b = (along + 1) as f32 * CELL_M + if along == high { kit.frame } else { 0.0 };
            let out = across * (depth + 0.01);
            let (min, max) = (
                (axis.point(row, a, edge) - out).min(axis.point(row, b, edge) + out),
                (axis.point(row, a, edge) - out).max(axis.point(row, b, edge) + out),
            );
            let cut = |x: f32, z: f32| {
                let deck = super::roofs::gable_cut(over, &under, glam::Vec3::new(x, 0.0, z));
                super::ticks((deck - END_BOARD_UNDER_M).min(edge + HEAD_UNDER_ROOF_M))
            };
            let top = [
                cut(min.x, min.z),
                cut(max.x, min.z),
                cut(max.x, max.z),
                cut(min.x, max.z),
            ];
            if top.iter().any(|&t| super::metres(t) > edge + 0.01) {
                heads.push(Piece::Blocking {
                    min: Place::near(min),
                    max: Place::near(max.with_y(edge)),
                    top,
                });
            }
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
            if upward > 0 {
                let edge = (layer + 1) as f32 * CELL_M;
                board(a, b, edge, head_top(kit, axis, layer), 0.01);
            } else {
                let edge = layer as f32 * CELL_M;
                board(a, b, sill_foot(kit, axis, layer), edge, 0.03);
            }
        }
    }
    layout.pieces.extend(heads);
}

/// Whether a cube of a wall running along `axis` is an opening framed with boards: open,
/// and with no door or shutters fitted, which are framed by their own frame.
fn boarded(walls: &Walls, at: glam::IVec3, axis: Axis) -> bool {
    walls
        .base
        .cube(at)
        .is_some_and(|cube| super::drawn_fill(&cube) != Fill::Solid)
        && walls.runs(at, axis)
        && !super::framed(walls.base, at)
}

/// The top of the head board over an opening whose top layer is `layer`: the middle of the
/// first course wholly above it.
fn head_top(kit: &Kit, axis: Axis, layer: i32) -> f32 {
    let per_cube = (CELL_M / kit.course).round() as i32;
    let (near, edge) = (layer * per_cube, (layer + 1) as f32 * CELL_M);
    (near - 2..near + 2 * per_cube + 2)
        .map(|course| axis.centre(kit, course))
        .find(|&centre| centre - kit.course * 0.5 >= edge - 1e-3)
        .map_or(edge + 0.1, |centre| centre.max(edge + 0.06))
}

/// The foot of the sill under an opening whose bottom layer is `layer`: the middle of the
/// first course wholly under it.
fn sill_foot(kit: &Kit, axis: Axis, layer: i32) -> f32 {
    let per_cube = (CELL_M / kit.course).round() as i32;
    let (near, edge) = (layer * per_cube, layer as f32 * CELL_M);
    (near - 2 * per_cube - 2..near + 2)
        .rev()
        .map(|course| axis.centre(kit, course))
        .find(|&centre| centre + kit.course * 0.5 <= edge + 1e-3)
        .map_or(edge - 0.1, |centre| centre.min(edge - 0.06))
}

/// Where another kind's wall comes in square against the side of one of the kit's, as a
/// stone or earth wall's end against a log end wall at a corner: a board up the joint,
/// filling the space the cube holds between the members' face and the other's, which a
/// cube's wall of logs, narrower than its cube, leaves as a slot through the corner. From
/// the foot of each stack of such cubes to its top.
pub(super) fn sides(walls: &Walls, kit: &Kit, ours: &[glam::IVec3], layout: &mut Layout) {
    let face = kit.radius();
    for axis in [Axis::X, Axis::Z] {
        let across = axis.other().step();
        for outward in [-1, 1] {
            let meets = |at: glam::IVec3| {
                let other = at + across * outward;
                walls.ours(at) == Some(Fill::Solid)
                    && walls.runs(at, axis)
                    && !walls.runs(at, axis.other())
                    && walls.base.cube(other).is_some_and(|cube| {
                        cube.kind != walls.kind
                            && cube.kind != terra_build::Kind::PLAIN
                            && cube.fill == Fill::Solid
                            && !cube.kind.is_roof()
                            && walls.base.in_wall(other)
                    })
            };
            // Where the wall ends in the cube with an end board over its ends, the board up
            // the side joint stops against the end board's inside face, as a filler butts
            // against the trim: run on to the cube's face, it crossed the end board, their
            // tops and feet in one plane. A stack is split where that changes.
            let stops = |at: glam::IVec3| [-1, 1].map(|end| ended(walls, at, axis, end));
            let joins =
                |at: glam::IVec3, next: glam::IVec3| meets(next) && stops(next) == stops(at);
            for &at in ours {
                if !meets(at) || joins(at, at - glam::IVec3::Y) {
                    continue;
                }
                let mut top = at;
                while joins(top, top + glam::IVec3::Y) {
                    top += glam::IVec3::Y;
                }
                let [before, after] = stops(at).map(|s| {
                    if s {
                        kit.frame * END_BOARD_WIDE
                    } else {
                        SIDE_BOARD_IN_M
                    }
                });
                let (row, along) = axis.row_of(at);
                let middle = axis.point(row, 0.0, 0.0).dot(axis.across());
                let sign = outward as f32;
                let (a, b) = (
                    middle + sign * (face - SIDE_BOARD_INTO_M),
                    middle + sign * CELL_M * 0.5,
                );
                let point = |across_m: f32, along_m: f32, height: f32| {
                    let mut p = axis.point(row, along_m, height);
                    p += axis.across() * (across_m - middle);
                    p
                };
                let low = point(
                    a.min(b),
                    along as f32 * CELL_M + before,
                    at.y as f32 * CELL_M + END_BOARD_IN_M,
                );
                let high = point(
                    a.max(b),
                    (along + 1) as f32 * CELL_M - after,
                    (top.y + 1) as f32 * CELL_M - END_BOARD_IN_M,
                );
                layout.pieces.push(Piece::Board {
                    min: Place::near(low.min(high)),
                    max: Place::near(low.max(high)),
                });
            }
        }
    }
}

/// How far into the members a board up a side joint reaches, past their face.
const SIDE_BOARD_INTO_M: f32 = 0.03;
/// How far in from its cube's faces along the wall a board up a side joint stops: past
/// where every other course's logs stop short (`runs::END_STAGGER_M`).
const SIDE_BOARD_IN_M: f32 = 0.01;

/// How far in from its stack's foot and top an end board stops.
const END_BOARD_IN_M: f32 = 0.005;
/// How wide an end board is, in the kit's frame boards.
const END_BOARD_WIDE: f32 = 1.2;

/// Whether a cube of the kit's wall running along `axis` ends at its face toward `outward`
/// against a wall the kits say it ends at with a board (D232).
fn carried(walls: &Walls, at: glam::IVec3, axis: Axis, outward: i32) -> bool {
    walls.ours(at) == Some(Fill::Solid)
        && walls.runs(at, axis)
        && walls.board_against(at + axis.step() * outward)
}

/// Whether an end board stands up a cube's end toward `outward`: where it ends with a board,
/// but not in a roof's cube the wall is carried up into, where the board is cut to the
/// roof's deck (docs/building-voxels.md, gap 1): a board standing to the cube's top came up
/// through the roof's covering beside a chimney.
fn ended(walls: &Walls, at: glam::IVec3, axis: Axis, outward: i32) -> bool {
    carried(walls, at, axis, outward) && !walls.base.cube(at).is_some_and(|c| c.kind.is_roof())
}

/// How tall a head board under a roof's cube stands at most, where the roof is high enough.
const HEAD_UNDER_ROOF_M: f32 = 0.12;
/// How far under a roof's deck an end board carried up into the roof is cut.
const END_BOARD_UNDER_M: f32 = 0.004;

/// Where a wall of the kit's kind ends against a wall of another kind, as logs end flush
/// against stone (D179): an end board up the joint, over the logs' sawn ends, from the
/// foot of each stack of such cubes to its top, where the kits say it ends there with a
/// board (D232: the other runs through it, or neither does).
pub(super) fn ends(walls: &Walls, kit: &Kit, ours: &[glam::IVec3], layout: &mut Layout) {
    let carried = |at: glam::IVec3, axis: Axis, outward: i32| carried(walls, at, axis, outward);
    let ends_at = |at: glam::IVec3, axis: Axis, outward: i32| ended(walls, at, axis, outward);
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
                let (a, b) = (face, face - outward as f32 * kit.frame * END_BOARD_WIDE);
                let out = axis.across() * (kit.radius() + 0.03);
                // Its foot and top a hair in from its stack's, out of the planes of a sill's
                // underside and a split log's flat top there. Over an opening it stands on
                // the head, and under one it stops at the sill's foot, as a jamb stops under
                // a head: the head and the sill lap past the opening over the logs' ends, and
                // a board from the opening's edge ran through them in the plane of their face.
                let foot = if boarded(walls, at - glam::IVec3::Y, axis) {
                    head_top(kit, axis, at.y - 1)
                } else {
                    at.y as f32 * CELL_M + END_BOARD_IN_M
                };
                let crown = if boarded(walls, top + glam::IVec3::Y, axis) {
                    sill_foot(kit, axis, top.y + 1)
                } else {
                    (top.y + 1) as f32 * CELL_M - END_BOARD_IN_M
                };
                let low = axis.point(row, a.min(b), foot) - out;
                let high = axis.point(row, a.max(b), crown) + out;
                if crown > foot + 0.01 {
                    layout.pieces.push(Piece::Board {
                        min: Place::near(low.min(high)),
                        max: Place::near(low.max(high)),
                    });
                }
                // On up into the roof's cube over the stack the wall is carried up into, its
                // top cut a hair under the roof's deck.
                let over = top + glam::IVec3::Y;
                if let Some(cube) = walls
                    .base
                    .cube(over)
                    .filter(|_| carried(over, axis, outward))
                    && let Some(under) = super::roofs::underside(over, cube)
                {
                    let floor = over.y as f32 * CELL_M + END_BOARD_IN_M;
                    let (min, max) = (low.min(high).with_y(floor), low.max(high).with_y(floor));
                    let cut = |x: f32, z: f32| {
                        let h = super::roofs::gable_cut(over, &under, glam::Vec3::new(x, 0.0, z));
                        super::ticks(h - END_BOARD_UNDER_M)
                    };
                    let top = [
                        cut(min.x, min.z),
                        cut(max.x, min.z),
                        cut(max.x, max.z),
                        cut(min.x, max.z),
                    ];
                    if top.iter().any(|&t| super::metres(t) > floor + 0.01) {
                        layout.pieces.push(Piece::Blocking {
                            min: Place::near(min),
                            max: Place::near(max),
                            top,
                        });
                    }
                }
            }
        }
    }
}

/// How far past the middle of the course over an opening's head, or under its sill, a post
/// up its side reaches.
const POST_PAST_M: f32 = 0.04;

#[cfg(test)]
mod tests {
    use super::super::tests::{boards, cabin, kit, logs, wall};
    use super::super::{Layout, lay};
    use super::*;
    use glam::Vec3;
    use terra_build::{BuildOp, Cube, DOOR_HEIGHT_M, Panel, STOREY_CUBES};

    #[test]
    fn a_jamb_post_with_nothing_of_its_kind_under_it_stands_a_hair_into_what_it_stands_on() {
        // A doorway at the foot of a wall standing on nothing of its kind, as on a stone
        // footing: the posts up its sides stand a hair into what is under them, as a corner
        // post does, out of the plane of the foot's sill laid on the same; standing on it,
        // the two feet fought where they met (the village's squared timber L-shaped house).
        let kit = terra_build::testing::shipped("hewn.ron");
        let mut base = terra_build::Base::default();
        for x in 0..6 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 0],
                panel: Some(if x == 2 { Panel::Door } else { Panel::Solid }),
                kind: kit.kind(),
            });
        }
        let feet: Vec<f32> = lay(&base, &kit)
            .pieces
            .iter()
            .filter_map(|piece| match piece {
                Piece::Log {
                    from,
                    to,
                    role: Some(super::super::roles::Role::Jamb),
                    ..
                } => Some(from.metres().y.min(to.metres().y)),
                _ => None,
            })
            .collect();
        assert_eq!(feet.len(), 2, "a post up each side: {feet:?}");
        for foot in feet {
            assert!(foot < -0.005, "a post's foot at {foot}");
        }
    }

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
        // A door fitted in it comes with its own frame: the kit's boards round it go.
        let rules = terra_build::Rules::default();
        let door = terra_build::Prop {
            min: [3, 0, 5],
            max: [4, 3, 5],
            opens: [0, 0, 1],
            ..terra_build::Prop::default()
        };
        base.act(
            &terra_build::ops::Op::FitProp(door),
            &terra_build::ops::Context::free(&rules),
        )
        .unwrap();
        let fitted = lay(&base, &kit);
        assert_eq!(boards(&fitted), 0);
        // And the courses it crosses run on to end inside that frame, which reaches
        // `OVERLAP_M` into the wall: no gap between them and it to see through.
        let overlap = crate::fitting::OVERLAP_M;
        for (from, to, _) in logs(&fitted) {
            if from.z != 2.75 || to.z != 2.75 || from.y > DOOR_HEIGHT_M {
                continue;
            }
            let ends_west = to.x < 2.0 && to.x > 1.5 - overlap && to.x <= 1.5 + 1e-4;
            let ends_east = from.x > 2.0 && from.x < 2.5 + overlap && from.x >= 2.5 - 1e-4;
            assert!(ends_west || ends_east, "{from} {to}");
        }
    }

    #[test]
    fn a_window_has_a_sill_and_a_cube_carved_out_is_framed_too() {
        let mut base = cabin();
        wall(&mut base, [[0, 0, 2]], Panel::WINDOW);
        let kit = kit();
        let before = lay(&base, &kit);
        // Two jambs, a head and a sill.
        assert_eq!(boards(&before), 4);
        // Shutters fitted over it come with their own frame: the kit's boards round it go.
        let mut fitted = base.clone();
        let shutters = terra_build::Prop {
            min: [0, 2, 2],
            max: [0, 3, 2],
            opens: [-1, 0, 0],
            hang: crate::fitting::Hang::Shutters,
            ..terra_build::Prop::default()
        };
        let rules = terra_build::Rules::default();
        fitted
            .act(
                &terra_build::ops::Op::FitProp(shutters),
                &terra_build::ops::Context::free(&rules),
            )
            .unwrap();
        assert_eq!(boards(&lay(&fitted, &kit)), 0);
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
        let mut base = terra_build::Base::default();
        let stone = terra_build::Kind(2);
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
            a.y.abs() < 0.01 && (b.y - terra_build::STOREY_M).abs() < 0.01,
            "{a} {b}"
        );
        // The logs' sawn ends inside it, not in the plane of its face against the stone.
        for (from, to, _) in logs(&layout) {
            for end in [from.x, to.x] {
                assert!(
                    (end - b.x).abs() > 0.01,
                    "a log ends at the board's face: {end}"
                );
            }
        }
    }

    #[test]
    fn a_stone_wall_ending_against_a_log_walls_side_is_met_by_a_board() {
        // A log wall along x, and a stone wall coming in square against its south side: the
        // logs, narrower than their cubes, leave a slot between their face and the stone's
        // end, which a board up the joint fills, from the logs' face to the cube's.
        let mut base = terra_build::Base::default();
        let stone = terra_build::Kind(2);
        wall(&mut base, (0..6).map(|x| [x, 0, 0]), Panel::Solid);
        base.apply(BuildOp::Fill {
            min: [2, 0, 1],
            max: [2, STOREY_CUBES - 1, 3],
            cube: Some(Cube::new(stone, Fill::Solid)),
        });
        let kit = kit();
        let layout = lay(&base, &kit);
        let side: Vec<(Vec3, Vec3)> = layout
            .pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Board { min, max } => Some((min.metres(), max.metres())),
                _ => None,
            })
            .filter(|(a, b)| a.x > 0.99 && b.x < 1.51 && b.z > 0.45)
            .collect();
        assert_eq!(side.len(), 1, "{side:?}");
        let (a, b) = side[0];
        let face = CELL_M * 0.5 + kit.radius();
        assert!(a.z < face && (b.z - CELL_M).abs() < 1e-3, "{a} {b}");
        assert!(
            (a.x - 1.0).abs() < 0.02 && (b.x - 1.5).abs() < 0.02,
            "{a} {b}"
        );
        assert!(a.y < 0.01 && b.y > terra_build::STOREY_M - 0.01, "{a} {b}");
    }
}
