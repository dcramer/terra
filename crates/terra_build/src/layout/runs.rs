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

use super::roofs::Field;
use super::{Axis, Layout, Piece, Place, Split, Spot, TICKS, Walls, hash, metres, ticks, unit};
use crate::kit::Kit;
use crate::mesh::ROOF_THICK_M;
use crate::{CELL_M, Fill, STOREY_CUBES, storey_of, wall_layer};
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};

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
                layout.pieces.push(log(
                    axis.point(row, a, centre),
                    axis.point(row, b, centre),
                    split,
                    hash(&[seed, part as u32, (a * 100.0) as i32 as u32]),
                    chinking.filter(|(near, far)| far > near),
                ));
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

/// Under a pitched roof, a row's courses carry on up, each log as long as its foot is under
/// the roof and each end cut to the roof's slope over it, so they shorten toward the ridge
/// and close the gable to the roof, as a gable of logs is built.
pub(super) fn gables(
    walls: &Walls,
    kit: &Kit,
    fields: &BTreeMap<i32, Field>,
    axis: Axis,
    row: i32,
    (first, last): (i32, i32),
    layout: &mut Layout,
) {
    let radius = kit.radius();
    // Each storey whose walls' top layer this row reaches.
    let tops: BTreeSet<i32> = super::Scope::row_cubes(walls.base, axis, row)
        .into_iter()
        .map(|(at, _)| at)
        .filter(|&at| {
            at.y - wall_layer(storey_of(at.y)) == STOREY_CUBES - 1 && walls.runs(at, axis)
        })
        .map(|at| storey_of(at.y))
        .collect();
    for storey in tops {
        let Some(field) = fields.get(&storey) else {
            continue;
        };
        let eaves = field.top;
        // The roof's underside over the row's middle, so far along it.
        let under = |distance: f32| {
            let point = axis.point(row, distance, 0.0);
            field.under(glam::Vec2::new(point.x, point.z))
        };
        // The lower of the underside over the log's two faces, so a log along the eaves,
        // where the roof falls across the wall, is not laid through it.
        let lowest = |distance: f32| {
            let point = axis.point(row, distance, 0.0);
            [-1.0, 1.0]
                .map(|side| point + axis.across() * radius * side)
                .into_iter()
                .map(|p| field.under(glam::Vec2::new(p.x, p.z)))
                .try_fold(f32::MAX, |low, under| under.map(|u| low.min(u)))
        };
        let (from, to) = (first as f32 * CELL_M, (last + 1) as f32 * CELL_M);
        let samples = ((to - from) / GABLE_STEP_M).round() as i32;
        let at = |sample: i32| from + sample as f32 * GABLE_STEP_M;
        for step in 0.. {
            let centre = eaves + kit.course * (step as f32 + 0.5);
            let foot = centre - radius;
            // How far the underside is over the log's foot at each sample.
            let clear: Vec<Option<f32>> = (0..=samples)
                .map(|sample| lowest(at(sample)).map(|under| under - foot))
                .collect();
            let fits = |sample: i32| clear[sample as usize].is_some_and(|c| c > 0.0);
            // Where the roof comes down to the log's foot between two samples.
            let crossing = |inside: i32, outside: i32| {
                let (a, b) = (
                    clear[inside as usize].unwrap_or(0.0),
                    clear[outside as usize].unwrap_or(-1.0),
                );
                let t = (a / (a - b).max(1e-6)).clamp(0.0, 1.0);
                at(inside) + (at(outside) - at(inside)) * t
            };
            let mut spans: Vec<(f32, f32)> = Vec::new();
            let mut sample = 0;
            while sample <= samples {
                if !fits(sample) {
                    sample += 1;
                    continue;
                }
                let start = if sample == 0 {
                    from
                } else {
                    crossing(sample, sample - 1)
                };
                while sample < samples && fits(sample + 1) {
                    sample += 1;
                }
                let end = if sample == samples {
                    to
                } else {
                    crossing(sample, sample + 1)
                };
                spans.push((start, end));
                sample += 1;
            }
            spans.retain(|(a, b)| b - a > 0.15);
            if spans.is_empty() {
                break;
            }
            let seed = hash(&[axis as u32, row as u32, storey as u32, 100 + step as u32]);
            for (index, (a, b)) in spans.into_iter().enumerate() {
                // Each end cut to the slope where the roof comes down over it; sawn
                // square where the wall ends under it.
                let slope = |end: f32, inward: f32| {
                    let (Some(here), Some(there)) = (under(end), under(end + inward * 0.05)) else {
                        return 0;
                    };
                    if here >= centre + radius {
                        return 0;
                    }
                    let rise = ((there - here) / 0.05).max(0.0);
                    (rise * TICKS as f32).round() as i32
                };
                let mut piece = log(
                    axis.point(row, a, centre),
                    axis.point(row, b, centre),
                    Split::Whole,
                    hash(&[seed, index as u32]),
                    kit.chinking.then_some((0.0, b - a)),
                );
                if let Piece::Log { lean, .. } = &mut piece {
                    *lean = [slope(a, 1.0), slope(b, -1.0)];
                }
                layout.pieces.push(piece);
            }
        }
    }
}

/// Cuts back every log that would pass through a roof, its eaves too: the ends of the top
/// courses that run past a corner, under a hip's slope, end where the roof begins, as a
/// builder saws them. A log the roof cuts in the middle is left in two.
pub(super) fn under_roofs(fields: &BTreeMap<i32, Field>, kit: &Kit, layout: &mut Layout) {
    let radius = kit.radius() + kit.ragged();
    // Where the slab of a roof is at a point, its underside and its top, if one is there.
    let slab = |at: Vec3| {
        fields.values().find_map(|field| {
            let rise = field.rise(glam::Vec2::new(at.x, at.z))?;
            let top = field.top + rise;
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
            ..
        } = piece
        else {
            layout.pieces.push(piece);
            continue;
        };
        let (from, to) = (from.metres(), to.metres());
        let chinking = chinking.map(|(near, far)| (metres(near), metres(far)));
        let length = from.distance(to);
        if fields.is_empty() || from.y != to.y || length < 1e-3 {
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
            layout.pieces.push(log(
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
            ));
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
    use crate::{Base, BuildOp, Panel, RoofSection, RoofStyle, STOREY_M};

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
            let r = kit.radius();
            let laid = lay(&base, &kit);
            for piece in &laid.pieces {
                let Piece::Log { from, to, lean, .. } = *piece else {
                    continue;
                };
                let (from, to) = (from.metres(), to.metres());
                if lean != [0, 0] {
                    // A gable's log cut to the roof: its foot under the roof at each end,
                    // and the roof over its end rising the way it is cut.
                    for (end, cut) in [(from, lean[0]), (to, lean[1])] {
                        let rise = section.rise(glam::Vec2::new(end.x, end.z)).unwrap();
                        let under = STOREY_M + rise - ROOF_THICK_M;
                        assert!(end.y - r <= under + 0.01, "{style:?}: {end} under {under}");
                        assert!(cut == 0 || (cut as f32 / TICKS as f32 - 0.7).abs() < 0.01);
                    }
                    continue;
                }
                for i in 0..=20 {
                    let at = from.lerp(to, i as f32 / 20.0);
                    let Some(rise) = section.rise_beyond(glam::Vec2::new(at.x, at.z), kit.eaves)
                    else {
                        continue;
                    };
                    let top = STOREY_M + rise;
                    let r = kit.radius();
                    assert!(
                        at.y + r <= top - ROOF_THICK_M + 3e-3 || at.y - r >= top - 3e-3,
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
