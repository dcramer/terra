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
                // Every other course a hair shorter: courses overlap a little, one on the
                // next, so their sawn ends in one plane overlap there and fight.
                let stagger = END_STAGGER_M * course.rem_euclid(2) as f32;
                (match next {
                    Spot::Hole => -beside_hole,
                    // Ending against another kind under an end board (`trims::ends`): its
                    // sawn end inside the board, not in the plane of the board's face.
                    _ if walls.board_against(axis.cube(row, beyond, cube.y)) => -kit.frame * 0.6,
                    // Uneven, but never past the overhang: short of it by up to `ragged`.
                    Spot::Nothing if corner && open => {
                        kit.overhang - unit(hash(&[seed, end as u32, which])) * kit.ragged()
                    }
                    _ => 0.0,
                }) - stagger
            };
            // Each end on a tick of the course's own parity, in from where it reaches, so the
            // ends of two courses one on the next, their ragged reach however it falls, are
            // never in one plane.
            let parity = |metres: f32, inward: i32| {
                let t = ticks(metres);
                let t = if (t - course).rem_euclid(2) == 0 {
                    t
                } else {
                    t + inward
                };
                super::metres(t)
            };
            let from = parity(low as f32 * CELL_M - reach(low, -1, 0), 1);
            let to = parity((high + 1) as f32 * CELL_M + reach(high, 1, 1), -1);
            let (wall_from, wall_to) = (low as f32 * CELL_M, (high + 1) as f32 * CELL_M);
            let limit = kit.longest + 2.0 * (kit.overhang + kit.ragged());
            let bottom = layer_of_split(centre, split);
            // Joints at places fixed along the row, a log's length apart and staggered
            // course to course, so a log is laid the same wherever its row ends.
            let span = (kit.longest - JOINT_CLEAR_M).max(CELL_M);
            let phase = span
                * (0.5 * course.rem_euclid(2) as f32
                    + 0.25 * unit(hash(&[axis as u32, row as u32, 17])));
            for (a, b) in joined(from, to, limit, span, phase) {
                // Each log by the cubes it lies along, so it is the same whatever else of
                // the row is laid: daub under it where it lies on a course of logs, and
                // a plate or a sill where it has nothing over it, or under it.
                let first = ((a / CELL_M).floor() as i32).clamp(low, high);
                let last = ((b / CELL_M).ceil() as i32 - 1).clamp(low, high);
                let daub = kit.chinking
                    && split != Split::Top
                    && (first..=last).any(|along| {
                        matches!(walls.spot(kit, axis, row, along, course - 1), Spot::Log(_))
                    });
                let every = |course: i32, what: &dyn Fn(Spot) -> bool| {
                    (first..=last).all(|along| what(walls.spot(kit, axis, row, along, course)))
                };
                let role = if plate && every(course + 1, &|spot| spot == Spot::Nothing) {
                    Some(Role::Crest)
                } else if sill
                    && every(course - 1, &|spot| spot == Spot::Nothing)
                    && (first..=last)
                        .all(|along| walls.ours(axis.cube(row, along, bottom - 1)).is_none())
                {
                    Some(Role::Foot)
                } else {
                    None
                };
                let chinking = daub.then(|| (wall_from.max(a) - a, wall_to.min(b) - a));
                let mut piece = log(
                    axis.point(row, a, centre),
                    axis.point(row, b, centre),
                    split,
                    hash(&[seed, 0, (a * 100.0) as i32 as u32]),
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
                // Its foot a hair into what it stands on and its top a hair under the courses',
                // out of the planes of their feet and tops.
                Vec3::new(
                    middle.x,
                    at.y as f32 * CELL_M - CORNER_POST_SUNK_M,
                    middle.z,
                ),
                Vec3::new(
                    middle.x,
                    (top.y + 1) as f32 * CELL_M - CORNER_POST_SUNK_M,
                    middle.z,
                ),
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
/// How far a corner post's foot is let into what it stands on.
const CORNER_POST_SUNK_M: f32 = 0.012;

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
    // Where logs end, and how many end there: two, a joint in a course.
    let mut ends: std::collections::HashMap<[i32; 3], u8> = Default::default();
    for piece in &pieces {
        if let Piece::Log { from, to, .. } = piece
            && from.0[1] == to.0[1]
        {
            *ends.entry(from.0).or_default() += 1;
            *ends.entry(to.0).or_default() += 1;
        }
    }
    let mut blocking = Layout::default();
    for piece in pieces {
        let Piece::Log {
            from,
            to,
            split,
            seed,
            chinking,
            role,
            lean,
            ..
        } = piece
        else {
            layout.pieces.push(piece);
            continue;
        };
        let (from, to) = (from.metres(), to.metres());
        let length = from.distance(to);
        // An upright, a post up a gable's last cube alone: cut level under the roof over
        // it to its slope (`Piece::Log`'s lean, for an upright), as a gable's logs are.
        if from.x == to.x && from.z == to.z && length > 1e-3 {
            let (bottom, top) = (from.y.min(to.y), from.y.max(to.y));
            let half = crate::timber::section(kit, role).0 * 0.5;
            let under = |dx: f32, dz: f32| roofs.under_over(from.x + dx, from.z + dz, bottom);
            let (Some(middle), Some(east), Some(west), Some(south), Some(north)) = (
                under(0.0, 0.0),
                under(half, 0.0),
                under(-half, 0.0),
                under(0.0, half),
                under(0.0, -half),
            ) else {
                layout.pieces.push(piece);
                continue;
            };
            let slope = [(east - west) / (2.0 * half), (south - north) / (2.0 * half)];
            // Its top where the roof is highest over its width, cut down to the slope.
            let highest = middle + (slope[0].abs() + slope[1].abs()) * half;
            if highest >= top {
                layout.pieces.push(piece);
            } else if middle > bottom + 0.05 {
                let mut cut = log(from.with_y(bottom), from.with_y(highest), split, seed, None);
                set_role(&mut cut, role);
                if let Piece::Log { lean, .. } = &mut cut {
                    *lean = slope.map(|s| libm::roundf(s * TICKS as f32) as i32);
                }
                layout.pieces.push(cut);
            }
            continue;
        }
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
        let joint = |end: Vec3| ends.get(&Place::near(end).0).copied().unwrap_or(0) > 1;
        eave_blocking(
            &roofs,
            kit,
            &Dropped {
                from,
                along,
                across,
                length,
                joints: [joint(from), joint(to)],
                // A row along the north and south leaves the corners where a row along
                // the east and west crosses it to that one's blocking, so the two are
                // never laid in one another.
                unfit: &|i| {
                    let p = (from + along * at(i)).with_y(foot + 0.01);
                    let cube = (p / CELL_M).floor().as_ivec3();
                    let ours = |c: IVec3| {
                        base.cube(c)
                            .is_some_and(|c| c.kind == kit.kind() && c.fill == crate::Fill::Solid)
                    };
                    let crossed =
                        along.x.abs() < 0.5 && (ours(cube + IVec3::X) || ours(cube - IVec3::X));
                    (!fits(i) && !crossed).then(|| at(i))
                },
                samples,
            },
            &mut blocking,
        );
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
            let leans = [
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
            // An end cut to the slope is cut from where the roof meets its foot: where the
            // log stops short of that, at its stretch's end (a gable's step, where the course
            // turns to a half log), its cut starts so far over its foot, so it follows the
            // roof's underside rather than standing under it.
            let lift_at = |end: f32, rise: i32| {
                let Some(here) = under(end) else {
                    return 0;
                };
                // Not where it runs past a corner into the open: there the roof is its
                // verge's, which the log's end clears.
                let inward = if end < length * 0.5 { 0.01 } else { -0.01 };
                let at = (from + along * (end + inward)).with_y(foot + 0.01);
                let inside = base.cube((at / CELL_M).floor().as_ivec3()).is_some();
                if rise <= 0 || here <= foot + 1e-3 || !inside {
                    return 0;
                }
                ticks((here - foot - SLOPE_SHORT_M).clamp(0.0, 2.0 * radius))
            };
            let lifts = [lift_at(a, leans[0]), lift_at(b, leans[1])];
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
            if let Piece::Log {
                lean: cut_lean,
                lift: cut_lift,
                ..
            } = &mut cut
            {
                *cut_lean = leans;
                *cut_lift = lifts;
            }
            layout.pieces.push(cut);
        }
    }
    layout.pieces.extend(blocking.pieces);
}

/// A log an eave comes down over, for [`eave_blocking`].
struct Dropped<'a> {
    from: Vec3,
    along: Vec3,
    across: Vec3,
    length: f32,
    /// Whether each end, at `from` and at the other, is a joint with the next log of its
    /// course, rather than the end of its stretch.
    joints: [bool; 2],
    /// For each of so many samples along it, where along it is, if it does not fit there.
    unfit: &'a dyn Fn(i32) -> Option<f32>,
    samples: i32,
}

/// Where a roof falls across a log it comes down over, as along the eaves, and the log does
/// not fit under it, the log is left out; this blocks the space it leaves, from its foot up
/// to the roof's underside, with boards, as a builder blocks between the rafters over a
/// wall's plate (eave blocking), so the wall closes to the roof rather than stopping a
/// course short of it with a slot into the room. Laid from the log alone, so it is the same
/// whatever else is laid (D281): none where the course under it is left out too, whose
/// blocking reaches as high; and those of a row along the north and south set in a little
/// further than those along the east and west, so where two cross at a corner none of their
/// faces share a plane.
fn eave_blocking(roofs: &Roofs, kit: &Kit, log: &Dropped, layout: &mut Layout) {
    let radius = kit.radius();
    let inset = if log.along.x.abs() > 0.5 {
        BLOCKING_SET_BACK_M
    } else {
        BLOCKING_SET_BACK_M + CROSSING_M
    };
    // A little inside the log's width, so its sides are not in the planes of the members'
    // faces under it, and its foot a little into the course under it.
    let half = radius - inset;
    let foot = log.from.y - radius;
    let step = log.length / log.samples.max(1) as f32;
    // The roof's underside over the log's width where it falls across it, its lowest.
    let gap = |at: f32| -> Option<f32> {
        let p = log.from + log.along * at;
        let mut tops = [0.0f32; 3];
        for (top, side) in tops.iter_mut().zip([-1.0, 0.0, 1.0]) {
            let q = p + log.across * half * side;
            *top = roofs.top_over(q.x, q.z, foot - NEAR_ROOF_M)?;
        }
        let (least, most) = (
            tops.iter().copied().fold(f32::MAX, f32::min),
            tops.iter().copied().fold(f32::MIN, f32::max),
        );
        (most - least > 0.01 && most > foot + MIN_BLOCKING_M).then_some(least)
    };
    let mut i = 0;
    while i <= log.samples {
        let Some(top) = (log.unfit)(i).and_then(gap) else {
            i += 1;
            continue;
        };
        let (start, mut low) = (i, top);
        while i < log.samples
            && let Some(next) = (log.unfit)(i + 1).and_then(gap)
        {
            low = low.min(next);
            i += 1;
        }
        let a = (start as f32 - 0.5) * step;
        let b = (i as f32 + 0.5) * step;
        i += 1;
        // The course under it left out too: its blocking covers this one's.
        if low - (foot - kit.course) <= 2.0 * radius {
            continue;
        }
        // Its ends a little in from the log's, out of the plane of the sawn ends under it,
        // but for a joint, where the next log's blocking carries on.
        let (a, b) = (
            if log.joints[0] {
                a.max(0.0)
            } else {
                a.max(inset)
            },
            if log.joints[1] {
                b.min(log.length)
            } else {
                b.min(log.length - inset)
            },
        );
        if b - a < MIN_BLOCKING_M {
            continue;
        }
        // In lengths split where the roof over it creases (a valley's or a hip's line, a
        // ridge), each a plane along, so its top cut between its corners follows the roof.
        let crease = |t: f32| {
            [-1.0, 0.0, 1.0].iter().any(|&side| {
                let u = |t: f32| {
                    let q = log.from + log.along * t + log.across * half * side;
                    roofs.top_over(q.x, q.z, foot - NEAR_ROOF_M).unwrap_or(foot)
                };
                (u(t - CREASE_STEP_M) - 2.0 * u(t) + u(t + CREASE_STEP_M)).abs() > 1e-3
            })
        };
        let mut cuts = vec![a];
        let mut t = a + CREASE_STEP_M;
        while t < b - CREASE_STEP_M {
            if crease(t) && t - cuts.last().copied().unwrap_or(a) > MIN_BLOCKING_M {
                cuts.push(t);
                t += CREASE_STEP_M * 2.0;
            } else {
                t += CREASE_STEP_M;
            }
        }
        cuts.push(b);
        // And in two across, so a crease along it is followed too.
        for (span, sides) in cuts
            .windows(2)
            .flat_map(|span| [(span, [-1.0, 0.0]), (span, [0.0, 1.0])])
        {
            let ends = [
                log.from + log.along * span[0],
                log.from + log.along * span[1],
            ];
            let mut min = Vec3::splat(f32::MAX);
            let mut max = Vec3::splat(f32::MIN);
            for end in ends {
                for side in sides {
                    let q = end + log.across * half * side;
                    min = min.min(q.with_y(foot - inset));
                    max = max.max(q.with_y(low - (inset - BLOCKING_SET_BACK_M)));
                }
            }
            // Its faces on every fourth tick, those of a row along the north and south two
            // ticks off them, so two crossing never share a plane, whatever roof they are
            // under.
            let crossing = i32::from(log.along.x.abs() <= 0.5) * 2;
            let snap = |p: Vec3| {
                let mut place = Place::near(p);
                for t in &mut place.0 {
                    *t = (*t - crossing).div_euclid(4) * 4 + crossing;
                }
                place
            };
            let (min, max) = (snap(min), snap(max));
            // Its top cut to the roof's underside over each corner, a hair under it, those
            // of a row along the north and south a little lower still, as their sides are
            // set in.
            let under = |x: i32, z: i32| {
                let p = Place([x, 0, z]).metres();
                let roof = roofs.top_over(p.x, p.z, foot - NEAR_ROOF_M).unwrap_or(foot);
                ticks((roof - (inset - BLOCKING_SET_BACK_M) - BLOCKING_UNDER_M).max(foot))
            };
            let top = [
                under(min.0[0], min.0[2]),
                under(max.0[0], min.0[2]),
                under(max.0[0], max.0[2]),
                under(min.0[0], max.0[2]),
            ];
            let board = Piece::Blocking {
                min,
                max: Place([
                    max.0[0],
                    top.iter().copied().min().unwrap_or(max.0[1]),
                    max.0[2],
                ]),
                top,
            };
            // A whole course's two halves, split at a layer, and courses left out one over
            // another, block the same space: one block, down to the lowest foot.
            let same = layout.pieces.iter_mut().find_map(|piece| match piece {
                Piece::Blocking {
                    min: m,
                    max: x,
                    top: t,
                } if [m.0[0], m.0[2], x.0[0], x.0[2]]
                    == [min.0[0], min.0[2], max.0[0], max.0[2]]
                    && *t == top =>
                {
                    Some(m)
                }
                _ => None,
            });
            match same {
                Some(m) => m.0[1] = m.0[1].min(min.0[1]),
                None => layout.pieces.push(board),
            }
        }
    }
}
/// How finely eave blocking looks for where the roof over it creases.
const CREASE_STEP_M: f32 = 0.02;
/// How far under a roof's underside eave blocking's top is cut.
const BLOCKING_UNDER_M: f32 = 0.003;
/// How far eave blocking is set back from a log's widest.
const BLOCKING_SET_BACK_M: f32 = 0.03;
/// How much further in a row's blocking along the north and south is set than one along the
/// east and west, so where they cross none of their faces share a plane.
const CROSSING_M: f32 = 0.005;

/// The least space under a roof worth blocking.
const MIN_BLOCKING_M: f32 = 0.02;
/// How far under the roof a log's slope cut lifted over its foot starts.
const SLOPE_SHORT_M: f32 = 0.006;

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
        lift: [0, 0],
        role: None,
    }
}

/// A stretch of a course in logs: whole if it is no longer than `limit`, else jointed at
/// every place `span` apart from `phase` along the row that is more than a hand's width
/// from either end, as a builder joins logs where they end, wherever the stretch starts.
/// Where the joints fall depends on nothing but the row and the course, so a log is the
/// same whichever part of a wall is laid (D281), and an edit reaches a log's length along it.
fn joined(start: f32, end: f32, limit: f32, span: f32, phase: f32) -> Vec<(f32, f32)> {
    if end - start <= limit {
        return vec![(start, end)];
    }
    let mut cuts = vec![start];
    let mut k = ((start + JOINT_CLEAR_M - phase) / span).floor() + 1.0;
    loop {
        let at = phase + k * span;
        if at >= end - JOINT_CLEAR_M {
            break;
        }
        if at > start + JOINT_CLEAR_M {
            cuts.push(at);
        }
        k += 1.0;
    }
    cuts.push(end);
    cuts.windows(2).map(|pair| (pair[0], pair[1])).collect()
}

/// How near a stretch's end a joint is not made, in metres: no stub of a log.
const JOINT_CLEAR_M: f32 = 0.3;
/// How much shorter every other course is at each end, so the sawn ends of courses one on
/// another are never in one plane where they overlap.
const END_STAGGER_M: f32 = 0.006;

/// How far along a row, in cubes, laying a log depends on its cubes: a log is at most a
/// stretch's limit long, and where its stretch is jointed depends on its stretch's ends,
/// that far again. An edit reaches no further along a row than this (D281).
pub(crate) fn reach_cubes(kit: &Kit) -> i32 {
    let limit = kit.longest + 2.0 * (kit.overhang + kit.ragged());
    (2.0 * limit / CELL_M).ceil() as i32 + 2
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
            let short = kit.ragged + END_STAGGER_M + 1e-3;
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
    fn a_free_end_is_cut_flush_every_other_course_a_hair_short() {
        let mut base = Base::default();
        wall(&mut base, (0..4).map(|x| [x, 0, 0]), Panel::Solid);
        let logs = logs(&lay(&base, &kit()));
        assert!(!logs.is_empty());
        for (from, to, _) in logs {
            assert!(
                from.x.abs() < END_STAGGER_M + 1e-3 && (to.x - 2.0).abs() < END_STAGGER_M + 1e-3,
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
    fn a_wall_under_an_eave_is_closed_to_the_roof() {
        // Under a hip the roof falls across every wall's top, so the course carried up into
        // the roof's cubes is left out; the space between the courses under it and the
        // roof is blocked with boards, with no two sharing a face.
        let kit = kit();
        let base = roofed_cabin(Style::Hip);
        let roofs = Roofs::whole(&base, &Kits(vec![kit.clone()]), super::super::Scope::ALL);
        let laid = lay(&base, &kit);
        let boards: Vec<(Vec3, Vec3)> = laid
            .pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Blocking { .. } => p.bounds(),
                _ => None,
            })
            .collect();
        let logs = logs(&laid);
        // Along the middle of each wall's line, from the top of the highest log there up to
        // the roof's underside over its outer face, where the roof comes lowest, something
        // stands: a log or a board.
        let middle = CELL_M * 0.5;
        let face = kit.radius() - 0.04;
        for (x, z, outward) in [
            (2.0, middle, -Vec3::Z),
            (2.0, 3.0 - middle, Vec3::Z),
            (middle, 1.5, -Vec3::X),
            (4.0 - middle, 1.5, Vec3::X),
        ] {
            let outer = Vec3::new(x, 0.0, z) + outward * face;
            let under = roofs
                .under_over(outer.x, outer.z, 2.0)
                .expect("a roof over the wall");
            let mut y = 2.0;
            while y < under - 0.02 {
                let p = Vec3::new(x, y, z);
                let in_log = logs.iter().any(|(from, to, _)| {
                    let (low, high) = (from.min(*to), from.max(*to));
                    let near =
                        |a: f32, lo: f32, hi: f32| a >= lo - kit.radius() && a <= hi + kit.radius();
                    near(p.x, low.x, high.x)
                        && near(p.z, low.z, high.z)
                        && (p.y - from.y).abs() < kit.radius() * 0.9
                });
                let in_board = boards
                    .iter()
                    .any(|(lo, hi)| p.cmpge(*lo).all() && p.cmple(*hi).all());
                assert!(
                    in_log || in_board,
                    "nothing at {p}, under the roof at {under}"
                );
                y += 0.01;
            }
        }
        // No two boards in one another with a face in the same plane.
        for (i, a) in boards.iter().enumerate() {
            for b in &boards[i + 1..] {
                let overlap = a.0.cmplt(b.1).all() && b.0.cmplt(a.1).all();
                if overlap {
                    for axis in 0..3 {
                        assert!(
                            (a.0[axis] - b.0[axis]).abs() > 1e-3
                                && (a.1[axis] - b.1[axis]).abs() > 1e-3,
                            "{a:?} and {b:?} share a face"
                        );
                    }
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
