//! Long members laid along runs (docs/building-kits.md, D178, D200): each wall of a
//! timber kind's cubes laid as logs or beams by the stacked pattern.
//!
//! A row of cubes running east and west lays whole courses, two to a cube; a row running
//! north and south lies half a course higher, with a split log at its foot and on top, so
//! where two meet their logs interlock without either knowing of the other. A cube with
//! rows both ways is a corner, a T or a cross, and its logs run through it to its outer
//! face, no further, so nothing leaves the grid; a row ending with nothing across it is cut
//! flush. Openings cut the courses through them, which end at their frame (`trims`). A
//! cube with nothing beside it either way is a post. Into a roof cube over a wall the
//! courses carry on up, shortening to the ridge, and any log a roof comes down over is cut
//! back (D241).

use super::roles::Role;
use super::roofs::Roofs;
use super::{Axis, Layout, Piece, Place, Split, Spot, TICKS, Walls, hash, metres, ticks, unit};
use crate::kit::{Kit, Kits};
use crate::{CELL_M, Fill};
use glam::{IVec3, Vec3};

/// How finely a gable's logs are fitted under its roof, in metres along the wall.
const GABLE_STEP_M: f32 = 0.02;
/// How far under a log's foot a roof is still one it may meet: one further down it stands
/// clear over.
const NEAR_ROOF_M: f32 = 1.0;

/// The courses of one row, each split into logs where it runs unbroken.
pub(super) fn courses(
    walls: &Walls,
    kit: &Kit,
    axis: Axis,
    row: i32,
    (first, last): (i32, i32),
    (lowest, highest): (i32, i32),
    layout: &mut Layout,
) {
    let per_cube = (CELL_M / kit.course).round() as i32;
    // A plate along the top course and a sill under the first, where the kit dresses its
    // walls' tops and feet with them (D215); posts up its openings' sides.
    let plate = kit.dress.member(Role::Crest).is_some();
    let sill = kit.dress.member(Role::Foot).is_some();
    let beside_hole = post_width(kit).unwrap_or(kit.frame);
    for course in lowest * per_cube - 1..=(highest + 1) * per_cube + 1 {
        let spots: Vec<Spot> = (first..=last)
            .map(|along| walls.spot(kit, axis, row, along, course))
            .collect();
        let centre = axis.centre(kit, course);
        let seed = hash(&[axis as u32, row as u32, course as u32]);
        let mut index = 0;
        while index < spots.len() {
            let Spot::Log(split) = spots[index] else {
                index += 1;
                continue;
            };
            let start = index;
            while index + 1 < spots.len() && spots[index + 1] == spots[start] {
                index += 1;
            }
            let (low, high) = (first + start as i32, first + index as i32);
            index += 1;
            // How each end finishes: cut at an opening's frame, run past a corner into the
            // open air, or flush with the cube.
            let layer = ((centre + 1e-3) / CELL_M).floor() as i32;
            let reach = |end: i32, outward: i32, which: u32| -> f32 {
                let beyond = end + outward;
                let next = if (first..=last).contains(&beyond) {
                    spots[(beyond - first) as usize]
                } else {
                    Spot::Nothing
                };
                let cube = axis.cube(row, end, layer.max(layer_of_split(centre, split)));
                let corner = walls.runs(cube, axis.other());
                let open = walls.open_beyond(axis.cube(row, beyond, cube.y));
                match next {
                    Spot::Hole => -beside_hole,
                    // Uneven, but never past the overhang: short of it by up to `ragged`.
                    Spot::Nothing if corner && open => {
                        kit.overhang - unit(hash(&[seed, end as u32, which])) * kit.ragged()
                    }
                    _ => 0.0,
                }
            };
            let from = low as f32 * CELL_M - reach(low, -1, 0);
            let to = (high + 1) as f32 * CELL_M + reach(high, 1, 1);
            // Daub between this course and the one under it, along the wall itself.
            let below = walls.spot(kit, axis, row, low, course - 1);
            let daub = kit.chinking && split != Split::Top && matches!(below, Spot::Log(_));
            let (wall_from, wall_to) = (low as f32 * CELL_M, (high + 1) as f32 * CELL_M);
            let limit = kit.longest + 2.0 * (kit.overhang + kit.ragged());
            let every = |course: i32, what: &dyn Fn(Spot) -> bool| {
                (low..=high).all(|along| what(walls.spot(kit, axis, row, along, course)))
            };
            let bottom = layer_of_split(centre, split);
            let role = if plate && every(course + 1, &|spot| spot == Spot::Nothing) {
                Some(Role::Crest)
            } else if sill
                && every(course - 1, &|spot| spot == Spot::Nothing)
                && (low..=high).all(|along| walls.ours(axis.cube(row, along, bottom - 1)).is_none())
            {
                Some(Role::Foot)
            } else {
                None
            };
            for (part, (a, b)) in joined(from, to, limit, course).into_iter().enumerate() {
                let chinking = daub.then(|| (wall_from.max(a) - a, wall_to.min(b) - a));
                let mut piece = log(
                    axis.point(row, a, centre),
                    axis.point(row, b, centre),
                    split,
                    hash(&[seed, part as u32, (a * 100.0) as i32 as u32]),
                    chinking.filter(|(near, far)| far > near),
                );
                set_role(&mut piece, role);
                layout.pieces.push(piece);
            }
        }
    }
}

/// Posts up every outside corner, where the kit dresses arrises with them (D215): a
/// squared post at the corner, a little proud of both faces, over the courses' ends.
pub(super) fn corner_posts(
    walls: &Walls,
    kit: &Kit,
    kits: &crate::kit::Kits,
    ours: &[IVec3],
    layout: &mut Layout,
) {
    let Some((_, scale, _)) = kit.dress.member(Role::Arris) else {
        return;
    };
    let width = kit.diameter * scale;
    let roles = super::roles::Roles::new(walls.base, walls.kind, kits);
    for &at in ours {
        for edge in roles.edges(at) {
            if edge.role != Role::Arris || roles.edges(at - IVec3::Y).contains(&edge) {
                continue;
            }
            let mut top = at;
            while roles.edges(top + IVec3::Y).contains(&edge) {
                top += IVec3::Y;
            }
            let mut corner = at.as_vec3() * CELL_M;
            let mut inward = Vec3::ZERO;
            for dir in [edge.side, edge.other] {
                if dir.sign() > 0 {
                    corner[dir.axis()] += CELL_M;
                }
                inward -= dir.step().as_vec3();
            }
            let middle = corner + inward * (width * 0.5 - CORNER_POST_PROUD_M);
            let mut post = log(
                Vec3::new(middle.x, at.y as f32 * CELL_M, middle.z),
                Vec3::new(middle.x, (top.y + 1) as f32 * CELL_M, middle.z),
                Split::Whole,
                hash(&[at.x as u32, at.y as u32, at.z as u32, edge.side as u32, 13]),
                None,
            );
            set_role(&mut post, Some(Role::Arris));
            layout.pieces.push(post);
        }
    }
}

/// How far a corner post stands out of the faces it covers: within an eighth of a cube
/// (D192).
const CORNER_POST_PROUD_M: f32 = 0.03;

/// How wide a post up an opening's side is, where the kit dresses jambs with one.
pub(super) fn post_width(kit: &Kit) -> Option<f32> {
    kit.dress
        .member(Role::Jamb)
        .map(|(_, scale, _)| kit.diameter * scale)
}

/// A long member laid for a role.
pub(super) fn set_role(piece: &mut Piece, to: Option<Role>) {
    if let Piece::Log { role, .. } = piece {
        *role = to;
    }
}

/// The layer a split log's cubes are on: the one its kept half lies in.
fn layer_of_split(centre: f32, split: Split) -> i32 {
    let layer = |height: f32| (height / CELL_M).floor() as i32;
    match split {
        Split::Top => layer(centre + 1e-3),
        Split::Bottom => layer(centre - 1e-3),
        Split::Whole => layer(centre + 1e-3),
    }
}

/// Cuts every log a roof comes down over at the roof's underside, its eaves too: a log
/// carried up into a roof cube over its wall, as a gable's courses are (`Walls::ours`), is
/// as long as its foot is under the roof, each end cut to the slope over it, so the courses
/// shorten toward the ridge and close the gable to the roof; and the ends of the top
/// courses that run past a corner, under the eaves, end where the roof begins, as a builder
/// saws them. A log the roof cuts in the middle is left in two.
pub(super) fn under_roofs(base: &crate::Base, kits: &Kits, kit: &Kit, layout: &mut Layout) {
    // Every roof near any of the logs, whole, so a log is cut the same whichever section
    // lays it.
    let logs = layout.pieces.iter().filter_map(|piece| match piece {
        Piece::Log { from, to, .. } => Some([from.0, to.0]),
        _ => None,
    });
    let Some((low, high)) = logs
        .flatten()
        .fold(None, |bounds: Option<(IVec3, IVec3)>, p| {
            let at = IVec3::from_array(p.map(|t| t.div_euclid(TICKS)));
            Some(bounds.map_or((at, at), |(low, high)| (low.min(at), high.max(at))))
        })
    else {
        return;
    };
    let near = super::Scope {
        low: low - IVec3::splat(2),
        high: high + IVec3::splat(2),
    };
    let roofs = Roofs::whole(base, kits, near);
    if roofs.is_empty() {
        return;
    }
    let radius = kit.radius();
    let reach = radius + kit.ragged();
    let pieces = std::mem::take(&mut layout.pieces);
    for piece in pieces {
        let Piece::Log {
            from,
            to,
            split,
            seed,
            chinking,
            role,
            lean,
        } = piece
        else {
            layout.pieces.push(piece);
            continue;
        };
        let (from, to) = (from.metres(), to.metres());
        let length = from.distance(to);
        if from.y != to.y || length < 1e-3 {
            layout.pieces.push(piece);
            continue;
        }
        let chinking = chinking.map(|(near, far)| (metres(near), metres(far)));
        let along = (to - from) / length;
        let across = Vec3::Y.cross(along).normalize_or(Vec3::X);
        let foot = from.y - radius;
        // The roof's underside over the log's middle line, of the lowest roof over it or
        // a little under its foot.
        let under = |at: f32| {
            let p = from + along * at;
            roofs.under_over(p.x, p.z, foot - NEAR_ROOF_M)
        };
        // How far the roof's underside is over the log's foot, over its middle line and
        // either face, the lowest, so a log along the eaves, where the roof falls across it,
        // is not laid through it: none where no roof is near, or the log stands clear over
        // it, as a wall built on a roof does.
        let clearance = |at: f32| -> Option<f32> {
            let p = from + along * at;
            // A log carried up into a roof's cube is under that roof, never over it.
            let in_roof = base
                .cube((p.with_y(foot + 0.01) / CELL_M).floor().as_ivec3())
                .is_some_and(|cube| cube.kind.is_roof());
            let (mut lowest, mut over, mut any) = (f32::MAX, !in_roof, false);
            let (mut least, mut most) = (f32::MAX, f32::MIN);
            for side in [-1.0, 0.0, 1.0] {
                let q = p + across * reach * side;
                let Some(top) = roofs.top_over(q.x, q.z, foot - NEAR_ROOF_M) else {
                    continue;
                };
                any = true;
                over &= foot >= top - 1e-3;
                lowest = lowest.min(top - foot);
                (least, most) = (least.min(top), most.max(top));
            }
            // Where the roof falls across the log, as along the eaves, the log has no end
            // to cut to it there: it fits whole under the roof's deck or not at all. Where
            // it falls along it, as up a gable, its end is cut to the slope (`lean`).
            if most - least > 0.01 {
                lowest -= 2.0 * radius;
            }
            (any && !over).then_some(lowest)
        };
        let samples = libm::ceilf(length / GABLE_STEP_M) as i32;
        let at = |i: i32| length * i as f32 / samples as f32;
        let clear: Vec<Option<f32>> = (0..=samples).map(|i| clearance(at(i))).collect();
        let fits = |i: i32| clear[i as usize].is_none_or(|c| c > 0.0);
        if (0..=samples).all(|i| fits(i) && clear[i as usize].is_none_or(|c| c > 2.0 * radius)) {
            layout.pieces.push(piece);
            continue;
        }
        // Where the roof comes down to the log's foot between two samples.
        let crossing = |inside: i32, outside: i32| {
            let (a, b) = (
                clear[inside as usize].unwrap_or(1.0),
                clear[outside as usize].unwrap_or(-1.0),
            );
            let t = (a / (a - b).max(1e-6)).clamp(0.0, 1.0);
            at(inside) + (at(outside) - at(inside)) * t
        };
        let mut spans: Vec<(f32, f32)> = Vec::new();
        let mut i = 0;
        while i <= samples {
            if !fits(i) {
                i += 1;
                continue;
            }
            let start = if i == 0 { 0.0 } else { crossing(i, i - 1) };
            while i < samples && fits(i + 1) {
                i += 1;
            }
            let end = if i == samples {
                length
            } else {
                crossing(i, i + 1)
            };
            spans.push((start, end));
            i += 1;
        }
        // Each end cut to the slope where the roof comes down over it: how many ticks the
        // underside rises a cube in from it; sawn square where it is clear of the roof.
        let slope = |end: f32, inward: f32| {
            let (Some(here), Some(there)) = (under(end), under(end + inward * 0.05)) else {
                return 0;
            };
            if here >= foot + 2.0 * radius {
                return 0;
            }
            let rise = ((there - here) / 0.05).max(0.0);
            libm::roundf(rise * TICKS as f32) as i32
        };
        for (index, (a, b)) in spans.into_iter().enumerate() {
            if b - a < 0.15 {
                continue;
            }
            let mut cut = log(
                from + along * a,
                from + along * b,
                split,
                if index == 0 {
                    seed
                } else {
                    hash(&[seed, index as u32])
                },
                chinking.and_then(|(c0, c1)| {
                    let (c0, c1) = (c0.max(a) - a, c1.min(b) - a);
                    (c1 > c0).then_some((c0, c1))
                }),
            );
            set_role(&mut cut, role);
            if let Piece::Log { lean: cut_lean, .. } = &mut cut {
                *cut_lean = [
                    if a < 1e-4 {
                        lean[0].max(slope(a, 1.0))
                    } else {
                        slope(a, 1.0)
                    },
                    if b > length - 1e-4 {
                        lean[1].max(slope(b, -1.0))
                    } else {
                        slope(b, -1.0)
                    },
                ];
            }
            layout.pieces.push(cut);
        }
    }
}

/// A cube of ours with none beside it either way is a post: an upright log up each
/// stack of them.
pub(super) fn posts(walls: &Walls, ours: &[IVec3], layout: &mut Layout) {
    let alone = |at: IVec3| {
        walls.ours(at) == Some(Fill::Solid) && !walls.runs(at, Axis::X) && !walls.runs(at, Axis::Z)
    };
    for &at in ours {
        // From the bottom of each stack.
        if !alone(at) || alone(at - IVec3::Y) {
            continue;
        }
        let mut top = at;
        while alone(top + IVec3::Y) {
            top += IVec3::Y;
        }
        let middle = |layer: i32| {
            Vec3::new(
                (at.x as f32 + 0.5) * CELL_M,
                layer as f32 * CELL_M,
                (at.z as f32 + 0.5) * CELL_M,
            )
        };
        layout.pieces.push(log(
            middle(at.y),
            middle(top.y + 1),
            Split::Whole,
            hash(&[at.x as u32, at.y as u32, at.z as u32, 7]),
            None,
        ));
    }
}

/// A long member from one point to another in metres, placed in the grid, with the daub
/// under it so far along it from `from`, in metres.
pub(super) fn log(
    from: Vec3,
    to: Vec3,
    split: Split,
    seed: u32,
    chinking: Option<(f32, f32)>,
) -> Piece {
    let (start, end) = (Place::near(from), Place::near(to));
    // The daub measured from where the log starts in the grid, and no further than it runs.
    let length = start.metres().distance(end.metres());
    let shift = (from - start.metres()).dot((to - from).normalize_or_zero());
    let along = |at: f32| ticks((at + shift).clamp(0.0, length));
    Piece::Log {
        from: start,
        to: end,
        split,
        seed,
        chinking: chinking.map(|(near, far)| (along(near), along(far))),
        lean: [0, 0],
        role: None,
    }
}

/// A stretch of a course in logs no longer than `longest`, the joins staggered from one
/// course to the next so no seam runs up the wall.
fn joined(start: f32, end: f32, longest: f32, course: i32) -> Vec<(f32, f32)> {
    let length = end - start;
    if length <= longest {
        return vec![(start, end)];
    }
    let count = (length / longest).ceil();
    let each = length / count;
    // Every other course moves its joins along by half a log, with a short log at each end.
    let shift = if course.rem_euclid(2) == 1 {
        each * 0.5
    } else {
        0.0
    };
    let mut cuts = vec![start];
    let mut at = start + if shift > 0.0 { shift } else { each };
    while at < end - 0.3 {
        cuts.push(at);
        at += each;
    }
    cuts.push(end);
    cuts.windows(2).map(|pair| (pair[0], pair[1])).collect()
}

#[cfg(test)]
mod tests {
    use super::super::lay;
    use super::super::tests::{cabin, kit, logs, wall};
    use super::*;
    use crate::kit::Kits;
    use crate::roofer::{self, Style};
    use crate::{Base, BuildOp, Cube, Kind, Panel, STOREY_M};
    use std::collections::BTreeSet;

    /// The cabin under a roof of cubes the roofer lays, its gables of logs.
    fn roofed_cabin(style: Style) -> Base {
        let mut base = cabin();
        base.apply(BuildOp::Fill {
            min: [1, 5, 1],
            max: [6, 5, 4],
            cube: None,
        });
        let footprint: BTreeSet<[i32; 2]> =
            (0..8).flat_map(|x| (0..6).map(move |z| [x, z])).collect();
        let plan = roofer::over(&footprint, 6, style, None);
        for (at, shape) in plan.roof {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube {
                    shape,
                    ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
                }),
            });
        }
        for at in plan.gables {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube::new(kit().kind(), Fill::Solid)),
            });
        }
        base
    }

    #[test]
    fn a_room_has_a_log_a_course_on_every_wall_and_stays_inside_its_cubes() {
        let kit = kit();
        let courses = kit.courses().0 as usize;
        let layout = lay(&cabin(), &kit);
        let logs = logs(&layout);
        // Walls running east and west lay a log a course; the others one fewer and a
        // split log at the foot and on top.
        assert_eq!(logs.len(), 2 * courses + 2 * (courses + 1), "{logs:?}");
        assert_eq!(logs.iter().filter(|l| l.2 != Split::Whole).count(), 4);
        // The south wall, the row of cubes from z = 2.5 to 3, runs through both corners
        // to their outer faces, a little short of them here and there, and no further.
        let south: Vec<_> = logs
            .iter()
            .filter(|(from, to, _)| from.z == 2.75 && to.z == 2.75)
            .collect();
        assert_eq!(south.len(), courses);
        for (from, to, _) in south {
            let short = kit.ragged + 1e-4;
            assert!(
                from.x >= 0.0 && from.x < short && to.x <= 4.0 && to.x > 4.0 - short,
                "{from} {to}"
            );
        }
        // Nothing leaves the cubes: four metres by three.
        for (from, to, _) in &logs {
            for end in [from, to] {
                assert!(
                    (0.0..=4.0).contains(&end.x) && (0.0..=3.0).contains(&end.z),
                    "{end}"
                );
            }
        }
    }

    #[test]
    fn walls_either_way_never_share_a_course() {
        let logs = logs(&lay(&cabin(), &kit()));
        let height = |run_x: bool| -> Vec<i32> {
            logs.iter()
                .filter(|(from, to, _)| (from.z == to.z) == run_x)
                .map(|(from, _, _)| (from.y * 1000.0).round() as i32)
                .collect()
        };
        let (east_west, north_south) = (height(true), height(false));
        assert!(east_west.iter().all(|y| !north_south.contains(y)));
    }

    #[test]
    fn a_thick_wall_shows_logs_along_its_faces_and_sawn_ends_only_at_its_ends() {
        for thick in 2..=3 {
            let mut base = Base::default();
            base.apply(BuildOp::Fill {
                min: [0, 0, 0],
                max: [9, crate::STOREY_CUBES - 1, thick - 1],
                cube: Some(Cube::new(kit().kind(), Fill::Solid)),
            });
            let logs = logs(&lay(&base, &kit()));
            // Logs run along both its faces...
            for face in [0.25, (thick as f32 - 0.5) * CELL_M] {
                assert!(
                    logs.iter()
                        .any(|(from, to, _)| from.z == face && to.z == face && to.x - from.x > 1.0),
                    "{thick} thick: no log along the face at {face}"
                );
            }
            // ...and none across it but at its two ends, so no face but an end shows a
            // log's sawn end.
            for (from, to, _) in &logs {
                if from.z != to.z {
                    assert!(
                        from.x < CELL_M || from.x > 9.0 * CELL_M,
                        "{thick} thick: a log across it at {from} {to}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_free_end_is_cut_flush() {
        let mut base = Base::default();
        wall(&mut base, (0..4).map(|x| [x, 0, 0]), Panel::Solid);
        let logs = logs(&lay(&base, &kit()));
        assert!(!logs.is_empty());
        for (from, to, _) in logs {
            assert!(
                from.x.abs() < 1e-4 && (to.x - 2.0).abs() < 1e-4,
                "{from} {to}"
            );
        }
    }

    #[test]
    fn a_gable_end_is_logs_that_shorten_toward_the_ridge() {
        let base = roofed_cabin(Style::Gable);
        // The west gable, on the row of cubes along x = 0, above the walls: its whole logs,
        // the split ones at each step's top being cut short under the roof.
        let gable: Vec<_> = logs(&lay(&base, &kit()))
            .into_iter()
            .filter(|(from, to, split)| {
                from.x == 0.25 && to.x == 0.25 && from.y > STOREY_M && *split == Split::Whole
            })
            .map(|(from, to, _)| (from.y, from.distance(to)))
            .collect();
        assert!(gable.len() >= 3, "{gable:?}");
        assert!(
            gable
                .windows(2)
                .all(|pair| pair[1].0 > pair[0].0 && pair[1].1 < pair[0].1),
            "{gable:?}"
        );
        // Under the roof: the ridge is 1.5 m from the eaves, rising 1.5 m.
        assert!(gable.iter().all(|(y, _)| *y < STOREY_M + 1.5));
    }

    #[test]
    fn no_log_passes_through_a_roof() {
        let kit = kit();
        for style in [Style::Hip, Style::Gable, Style::LeanTo] {
            let base = roofed_cabin(style);
            let roofs = Roofs::whole(&base, &Kits(vec![kit.clone()]), super::super::Scope::ALL);
            let r = kit.radius();
            let laid = lay(&base, &kit);
            for piece in &laid.pieces {
                let Piece::Log { from, to, lean, .. } = *piece else {
                    continue;
                };
                let (from, to) = (from.metres(), to.metres());
                if from.y != to.y {
                    continue;
                }
                if lean != [0, 0] {
                    // A gable's log cut to the roof: its foot under the roof at each end,
                    // and the roof over its end rising the way it is cut, a cube a cube.
                    for (end, cut) in [(from, lean[0]), (to, lean[1])] {
                        let under = roofs.under_over(end.x, end.z, end.y - r - 1.0).unwrap();
                        assert!(end.y - r <= under + 0.01, "{style:?}: {end} under {under}");
                        assert!(cut == 0 || (cut - TICKS).abs() <= 2, "{style:?}: {cut}");
                    }
                    continue;
                }
                for i in 0..=20 {
                    let at = from.lerp(to, i as f32 / 20.0);
                    let Some(top) = roofs.top_over(at.x, at.z, at.y - r) else {
                        continue;
                    };
                    assert!(
                        at.y + r <= top + 3e-3,
                        "{style:?}: a log at {at} meets the roof at {top}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_long_wall_is_joined_with_the_seams_staggered() {
        let mut base = Base::default();
        wall(&mut base, (0..20).map(|x| [x, 0, 0]), Panel::Solid);
        let kit = kit();
        let logs = logs(&lay(&base, &kit));
        let limit = kit.longest + 2.0 * (kit.overhang + kit.ragged);
        assert!(
            logs.iter()
                .all(|(from, to, _)| from.distance(*to) <= limit + 1e-4)
        );
        let seams = |y: f32| -> Vec<i32> {
            logs.iter()
                .filter(|(from, _, _)| (from.y - y).abs() < 1e-3)
                .map(|(from, _, _)| (from.x * 100.0).round() as i32)
                .collect()
        };
        let (first, second) = (seams(kit.course * 0.5), seams(kit.course * 1.5));
        assert!(first.len() > 1 && second.len() > 1);
        assert!(first[1..].iter().all(|x| !second.contains(x)));
    }

    #[test]
    fn a_column_of_cubes_alone_is_a_post() {
        let mut base = Base::default();
        wall(&mut base, [[0, 0, 0]], Panel::Solid);
        let logs = logs(&lay(&base, &kit()));
        assert_eq!(
            logs,
            vec![(
                Vec3::new(0.25, 0.0, 0.25),
                Vec3::new(0.25, 3.0, 0.25),
                Split::Whole
            )]
        );
    }

    #[test]
    fn every_log_stays_inside_the_cubes_of_its_run() {
        // An L, a T and a cross, each log within the cubes of the row it runs along,
        // between their ends, and no further across than the wall's own cube.
        let mut base = Base::default();
        wall(&mut base, (0..8).map(|x| [x, 0, 0]), Panel::Solid);
        wall(&mut base, (1..6).map(|z| [0, 0, z]), Panel::Solid);
        wall(&mut base, (1..6).map(|z| [4, 0, z]), Panel::Solid);
        wall(&mut base, (0..8).map(|x| [x, 0, 3]), Panel::Solid);
        let cubes: BTreeSet<[i32; 2]> = base.cubes().map(|(at, _)| [at.x, at.z]).collect();
        for (from, to, _) in logs(&lay(&base, &kit())) {
            for i in 0..=40 {
                let at = from.lerp(to, i as f32 / 40.0);
                // A hair inside, as a log's ends sit on its cubes' faces.
                let inward = (to - from).normalize() * if i == 0 { 1e-3 } else { -1e-3 };
                let p = if i == 0 || i == 40 { at + inward } else { at };
                let column = [(p.x / CELL_M).floor() as i32, (p.z / CELL_M).floor() as i32];
                assert!(
                    cubes.contains(&column),
                    "{from} {to} leaves its cubes at {p}"
                );
            }
        }
    }
}
