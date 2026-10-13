//! Long members laid along runs (docs/building-kits.md, D178, D200): each wall of a
//! timber kind's cubes laid as logs or beams by the stacked pattern.
//!
//! A row of cubes running east and west lays whole courses, two to a cube; a row running
//! north and south lies half a course higher, with a split log at its foot and on top, so
//! where two meet their logs interlock without either knowing of the other. A cube with
//! rows both ways is a corner, a T or a cross, and its logs run through it to its outer
//! face, no further, so nothing leaves the grid; a row ending with nothing across it is cut
//! flush. Openings cut the courses through them, which end at their frame (`trims`). A
//! cube with nothing beside it either way is a post. Under a pitched roof the courses carry
//! on up, shortening to the ridge, and any log a roof would pass through is cut back.

use super::{Axis, Layout, Piece, Split, Spot, Walls, hash, unit};
use crate::kit::Kit;
use crate::mesh::ROOF_THICK_M;
use crate::{Base, CELL_M, Fill, STOREY_CUBES, STOREY_M, storey_of, wall_layer};
use glam::{IVec3, Vec3};
use std::collections::BTreeSet;

/// How finely a gable's logs are fitted under its roof, in metres along the wall.
const GABLE_STEP_M: f32 = 0.02;

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
                    Spot::Hole => -kit.frame,
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
            for (part, (a, b)) in joined(from, to, limit, course).into_iter().enumerate() {
                let chinking = daub.then(|| (wall_from.max(a) - a, wall_to.min(b) - a));
                layout.pieces.push(Piece::Log {
                    from: axis.point(row, a, centre),
                    to: axis.point(row, b, centre),
                    split,
                    seed: hash(&[seed, part as u32, (a * 100.0) as i32 as u32]),
                    chinking: chinking.filter(|(near, far)| far > near),
                });
            }
        }
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

/// Under a pitched roof, a row's courses carry on up, each log as long as fits under it,
/// so they shorten toward the ridge as a gable's do.
pub(super) fn gables(
    walls: &Walls,
    kit: &Kit,
    axis: Axis,
    row: i32,
    (first, last): (i32, i32),
    layout: &mut Layout,
) {
    let radius = kit.radius();
    // Each storey whose walls' top layer this row reaches.
    let tops: BTreeSet<i32> = walls
        .base
        .cubes()
        .map(|(at, _)| at)
        .filter(|&at| {
            axis.row_of(at).0 == row
                && at.y - wall_layer(storey_of(at.y)) == STOREY_CUBES - 1
                && walls.runs(at, axis)
        })
        .map(|at| storey_of(at.y))
        .collect();
    for storey in tops {
        let eaves = (storey + 1) as f32 * STOREY_M;
        let rise = |distance: f32| {
            let point = axis.point(row, distance, 0.0);
            walls
                .base
                .roof_rise(storey, glam::Vec2::new(point.x, point.z))
                .map(|rise| rise - ROOF_THICK_M)
        };
        let (from, to) = (first as f32 * CELL_M, (last + 1) as f32 * CELL_M);
        for step in 0.. {
            let centre = kit.course * (step as f32 + 0.5);
            let needed = centre + radius * 0.9;
            let mut spans: Vec<(f32, f32)> = Vec::new();
            let mut open: Option<f32> = None;
            let samples = ((to - from) / GABLE_STEP_M).round() as i32;
            for sample in 0..=samples {
                let distance = from + sample as f32 * GABLE_STEP_M;
                let fits = rise(distance).is_some_and(|rise| rise >= needed);
                match (fits, open) {
                    (true, None) => open = Some(distance),
                    (false, Some(start)) => {
                        spans.push((start, distance - GABLE_STEP_M));
                        open = None;
                    }
                    _ => {}
                }
            }
            if let Some(start) = open {
                spans.push((start, to));
            }
            spans.retain(|(a, b)| b - a > 0.15);
            if spans.is_empty() {
                break;
            }
            let seed = hash(&[axis as u32, row as u32, storey as u32, 100 + step as u32]);
            for (index, (a, b)) in spans.into_iter().enumerate() {
                layout.pieces.push(Piece::Log {
                    from: axis.point(row, a, eaves + centre),
                    to: axis.point(row, b, eaves + centre),
                    split: Split::Whole,
                    seed: hash(&[seed, index as u32]),
                    chinking: kit.chinking.then_some((0.0, b - a)),
                });
            }
        }
    }
}

/// Cuts back every log that would pass through a roof, its eaves too: the ends of the top
/// courses that run past a corner, under a hip's slope, end where the roof begins, as a
/// builder saws them. A log the roof cuts in the middle is left in two.
pub(super) fn under_roofs(base: &Base, kit: &Kit, layout: &mut Layout) {
    let radius = kit.radius() + kit.ragged();
    // Where the slab of a roof is at a point, its underside and its top, if one is there.
    let slab = |at: Vec3| {
        base.sections().iter().find_map(|section| {
            let eaves = (section.storey() + 1) as f32 * STOREY_M;
            let rise = section.rise_beyond(glam::Vec2::new(at.x, at.z), kit.eaves)?;
            let top = eaves + rise;
            (at.y + radius > top - ROOF_THICK_M && at.y - radius < top).then_some(())
        })
    };
    let pieces = std::mem::take(&mut layout.pieces);
    for piece in pieces {
        let Piece::Log {
            from,
            to,
            split,
            seed,
            chinking,
        } = piece
        else {
            layout.pieces.push(piece);
            continue;
        };
        let length = from.distance(to);
        if base.sections().is_empty() || from.y != to.y || length < 1e-3 {
            layout.pieces.push(piece);
            continue;
        }
        let along = (to - from) / length;
        let samples = (length / GABLE_STEP_M).ceil() as i32;
        let clear: Vec<bool> = (0..=samples)
            .map(|i| slab(from + along * (length * i as f32 / samples as f32)).is_none())
            .collect();
        if clear.iter().all(|&c| c) {
            layout.pieces.push(piece);
            continue;
        }
        // The runs of it that are clear of every roof.
        let mut start = None;
        let mut spans = Vec::new();
        for (i, &c) in clear.iter().enumerate() {
            let at = length * i as f32 / samples as f32;
            match (c, start) {
                (true, None) => start = Some(at),
                (false, Some(a)) => {
                    spans.push((a, at - length / samples as f32));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(a) = start {
            spans.push((a, length));
        }
        for (index, (a, b)) in spans.into_iter().enumerate() {
            if b - a < 0.15 {
                continue;
            }
            layout.pieces.push(Piece::Log {
                from: from + along * a,
                to: from + along * b,
                split,
                seed: if index == 0 {
                    seed
                } else {
                    hash(&[seed, index as u32])
                },
                chinking: chinking.and_then(|(c0, c1)| {
                    let (c0, c1) = (c0.max(a) - a, c1.min(b) - a);
                    (c1 > c0).then_some((c0, c1))
                }),
            });
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
        layout.pieces.push(Piece::Log {
            from: middle(at.y),
            to: middle(top.y + 1),
            split: Split::Whole,
            seed: hash(&[at.x as u32, at.y as u32, at.z as u32, 7]),
            chinking: None,
        });
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
    use crate::{BuildOp, Panel, RoofSection, RoofStyle};

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
        let mut base = cabin();
        base.apply(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [7, 0, 5],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        // The west gable, on the row of cubes along x = 0, above the walls.
        let gable: Vec<_> = logs(&lay(&base, &kit()))
            .into_iter()
            .filter(|(from, to, _)| from.x == 0.25 && to.x == 0.25 && from.y > STOREY_M)
            .map(|(from, to, _)| (from.y, from.distance(to)))
            .collect();
        assert!(gable.len() >= 3, "{gable:?}");
        assert!(
            gable
                .windows(2)
                .all(|pair| pair[1].0 > pair[0].0 && pair[1].1 < pair[0].1)
        );
        // Under the roof: the ridge is 1.5 m from the eaves, rising 1.05 m.
        assert!(gable.iter().all(|(y, _)| *y < STOREY_M + 1.05));
    }

    #[test]
    fn no_log_passes_through_a_roof() {
        let kit = kit();
        for style in [RoofStyle::Hip, RoofStyle::Gable, RoofStyle::LeanTo] {
            let mut base = cabin();
            base.apply(BuildOp::AddRoof(RoofSection {
                min: [0, 0, 0],
                max: [7, 0, 5],
                style,
                along_x: true,
                pitch: 0.7,
            }));
            let section = base.sections()[0];
            let logs = logs(&lay(&base, &kit));
            for (from, to, _) in &logs {
                for i in 0..=20 {
                    let at = from.lerp(*to, i as f32 / 20.0);
                    let Some(rise) = section.rise_beyond(glam::Vec2::new(at.x, at.z), kit.eaves)
                    else {
                        continue;
                    };
                    let top = STOREY_M + rise;
                    let r = kit.radius();
                    assert!(
                        at.y + r <= top - ROOF_THICK_M + 1e-3 || at.y - r >= top - 1e-3,
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
