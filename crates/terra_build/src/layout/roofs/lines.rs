//! A roof's lines: its creases (ridges, hips and valleys) and its edges (eaves, verges and
//! flashing), found square by square and joined end to end into the long pieces laid
//! along them.

use super::{Beside, RoofPart, Roofs, SIDES, STEP_M, Segment, crease, flashing, is_flashing};
use crate::CELL_M;
use crate::layout::Place;
use glam::{IVec3, Vec2, Vec3};
use std::collections::BTreeMap;

/// The lines of a roof's squares, before they are joined: each square's creases between its
/// own triangles and with the squares beside it, and its edges, an eave, a verge or flashing
/// where nothing carries on; and apart, the creases under something standing on the roof,
/// whose frame runs on under it, as it carries what stands there, but whose caps do not.
pub(super) fn found(roofs: &Roofs, ours: &dyn Fn(usize) -> bool) -> (Vec<Segment>, Vec<Segment>) {
    let count = roofs.squares.len();
    let mut segments: Vec<Segment> = Vec::new();
    // Creases under something standing on the roof: the frame under them runs on, as it
    // carries what stands there, but their caps do not.
    let mut hidden: Vec<Segment> = Vec::new();
    for q in (0..count).filter(|&q| ours(q)) {
        let triangles: [[Vec3; 3]; 4] = [0, 1, 2, 3].map(|side| roofs.triangle(q, side));
        let centroid = |t: &[Vec3; 3]| Place::near((t[0] + t[1] + t[2]) / 3.0);
        let covered = roofs.squares[q].covered;
        // Its diagonals, between its own triangles.
        for side in 0..4 {
            let (a, b) = (&triangles[side], &triangles[(side + 1) % 4]);
            let (p, r) = (a[0], a[2]);
            if let Some(part) = crease(a, b, p, r) {
                let segment = Segment {
                    part,
                    from: Place::near(p),
                    to: Place::near(r),
                    sides: [centroid(a), centroid(b)],
                    square: q,
                };
                if covered {
                    hidden.push(segment);
                } else {
                    segments.push(segment);
                }
            }
        }
        // Its sides: shared with the square beyond, or an edge of the roof.
        for (side, triangle) in triangles.iter().enumerate() {
            let (step, _, _) = SIDES[side];
            let out = Vec3::new(step[0] as f32, 0.0, step[1] as f32) * STEP_M;
            let (p, r) = (triangle[1], triangle[2]);
            let part = match roofs.beside[q][side] {
                Beside::Square(other) => {
                    // Each shared side once, from the square to its west or north.
                    if side == 0 || side == 3 {
                        continue;
                    }
                    let theirs = roofs.triangle(other, (side + 2) % 4);
                    let crease = crease(triangle, &theirs, p, r).map(|part| Segment {
                        part,
                        from: Place::near(p),
                        to: Place::near(r),
                        sides: [centroid(triangle), centroid(&theirs)],
                        square: q,
                    });
                    match (covered, roofs.squares[other].covered) {
                        (false, false) => segments.extend(crease),
                        (true, true) => hidden.extend(crease),
                        // Where something stands on one and not the other, the covering
                        // runs on under its foot, flashed along it from the open side.
                        (mine, _) => {
                            hidden.extend(crease);
                            let (open, toward, square) = if mine {
                                (theirs, -out, other)
                            } else {
                                (*triangle, out, q)
                            };
                            segments.push(Segment {
                                part: flashing(&open),
                                from: Place::near(p),
                                to: Place::near(r),
                                sides: [centroid(&open), Place::near((p + r) * 0.5 + toward)],
                                square,
                            });
                        }
                    }
                    continue;
                }
                // Under what stands on it, against what stands beside it: nothing shows.
                Beside::Against if covered => continue,
                Beside::Against => flashing(triangle),
                Beside::Open if (p.y - r.y).abs() < 1e-4 => RoofPart::Eave,
                Beside::Open => RoofPart::Verge,
            };
            segments.push(Segment {
                part,
                from: Place::near(p),
                to: Place::near(r),
                sides: [centroid(triangle), Place::near((p + r) * 0.5 + out)],
                square: q,
            });
        }
    }
    (segments, hidden)
}

/// Whether a crease's end meets another crease, at its end or along it, as a wing's ridge
/// meets the main ridge at a T, or a hip the ridge's end.
pub(super) fn crease_met(segment: &Segment, end: Place, all: &[Segment]) -> bool {
    let crease =
        |part: RoofPart| matches!(part, RoofPart::Ridge | RoofPart::Hip | RoofPart::Valley);
    let p = end.metres();
    all.iter().any(|other| {
        if std::ptr::eq(other, segment) || !crease(other.part) {
            return false;
        }
        let (a, b) = (other.from.metres(), other.to.metres());
        // Not the next piece of its own line, jointed to it.
        let along = (segment.to.metres() - segment.from.metres()).normalize_or_zero();
        if along.dot((b - a).normalize_or_zero()).abs() > 0.99 {
            return false;
        }
        let t = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
        (a + (b - a) * t).distance(p) < 0.01
    })
}

/// Whether a line's end meets another line of the roof there, and so takes no cap: a verge's
/// top under its ridge's cap, two verges at an apex, whose own module covers the joint; a
/// hip's or a valley's foot at the eaves, where it is mitred to them (`roofing`) rather
/// than capped square across the line and standing out past them. An eave meets nothing it
/// should stop at.
pub(super) fn edge_met(segment: &Segment, end: Place, all: &[Segment]) -> bool {
    let near =
        |a: Place, b: Place| (0..3).all(|i| (a.0[i] - b.0[i]).abs() <= crate::layout::TICKS / 16);
    // A crease's end at something standing on the roof dies into it under the flashing,
    // uncapped: a cap there stood a knob against the wall's face.
    let crease = matches!(
        segment.part,
        RoofPart::Ridge | RoofPart::Hip | RoofPart::Valley
    );
    if crease
        && all.iter().any(|other| {
            let (a, b) = (other.from.metres(), other.to.metres());
            let p = end.metres();
            let t = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
            is_flashing(other.part) && (a + (b - a) * t).distance(p) < 0.01
        })
    {
        return true;
    }
    if matches!(segment.part, RoofPart::Hip | RoofPart::Valley) {
        return all.iter().any(|other| {
            other.part == RoofPart::Eave && (near(other.from, end) || near(other.to, end))
        });
    }
    if segment.part != RoofPart::Verge {
        return false;
    }
    all.iter().any(|other| {
        !std::ptr::eq(other, segment)
            && matches!(
                other.part,
                RoofPart::Ridge | RoofPart::Hip | RoofPart::Verge
            )
            && (near(other.from, end) || near(other.to, end))
    })
}

/// A ridge a cube wide, the flat top of whole cubes along a roof's middle where its span
/// is odd, capped as one: its two ridges, level, as long as each other, a cube apart and at
/// one height, are one cap along its middle as wide as it, over the slopes either side.
pub(super) fn ridges_a_cube_wide(mut segments: Vec<Segment>) -> Vec<Segment> {
    let wide = crate::layout::TICKS;
    let mut gone = vec![false; segments.len()];
    let mut merged = Vec::new();
    for i in 0..segments.len() {
        let a = &segments[i];
        if gone[i] || a.part != RoofPart::Ridge || a.from.0[1] != a.to.0[1] {
            continue;
        }
        for j in i + 1..segments.len() {
            let b = &segments[j];
            if gone[j] || b.part != RoofPart::Ridge || b.from.0[1] != a.from.0[1] {
                continue;
            }
            let across = IVec3::from_array(b.from.0) - IVec3::from_array(a.from.0);
            let along = IVec3::from_array(a.to.0) - IVec3::from_array(a.from.0);
            let same = IVec3::from_array(b.to.0) - IVec3::from_array(b.from.0) == along;
            let square = across.y == 0
                && across.x * along.x + across.z * along.z == 0
                && across.x.abs() + across.z.abs() == wide;
            if !(same && square) {
                continue;
            }
            // The side of each away from the other.
            let away = |s: &Segment, from: IVec3| {
                *s.sides
                    .iter()
                    .max_by_key(|p| {
                        let d = IVec3::from_array(p.0) - from;
                        (d.x * across.x + d.z * across.z).abs()
                    })
                    .expect("two sides")
            };
            let middle =
                |p: Place, q: Place| Place([0, 1, 2].map(|k| (p.0[k] + q.0[k]).div_euclid(2)));
            merged.push(Segment {
                part: RoofPart::Ridge,
                from: middle(a.from, b.from),
                to: middle(a.to, b.to),
                sides: [
                    away(a, IVec3::from_array(b.from.0)),
                    away(b, IVec3::from_array(a.from.0)),
                ],
                square: a.square,
            });
            gone[i] = true;
            gone[j] = true;
            break;
        }
    }
    let mut out: Vec<Segment> = segments
        .drain(..)
        .zip(gone)
        .filter(|(_, gone)| !gone)
        .map(|(segment, _)| segment)
        .collect();
    out.extend(merged);
    out
}

/// How far round a layout's scope a roof's squares are worked on, in cubes: past the reach
/// of a square's pieces, its eaves included.
pub(super) const NEAR_SCOPE: i32 = crate::cubes::SECTION + 4;

/// Segments joined end to end along each line into long pieces, as a builder runs a board
/// or a ridge's cap on, jointed where a line passes a section's edge but for within two
/// metres of its ends, so a long piece is the same whichever part of the roof is laid and an edit
/// reaches no further than a section along it (D281).
pub(super) fn join(segments: Vec<Segment>) -> Vec<Segment> {
    // By what each is, the line it is on in plan and its height where that line crosses
    // the plan's origin, then along it. Heights are to the nearest tick, so a line's slope
    // is compared, not its direction in ticks.
    type Line = (RoofPart, [i64; 2], i64, i64);
    // A run of a line's segments end to end: how far it reaches, the run as one, by whose
    // slope the next is joined, and its segments with where each starts and ends along it.
    type Run = (i64, Segment, Vec<(i64, i64, Segment)>);
    let slope = |s: &Segment| {
        let (a, b) = (s.from.metres(), s.to.metres());
        (b.y - a.y) / Vec2::new(b.x - a.x, b.z - a.z).length().max(1e-6)
    };
    let mut lines: BTreeMap<Line, Vec<(i64, i64, Segment)>> = BTreeMap::new();
    for mut segment in segments {
        let plan = |p: Place| glam::I64Vec2::new(i64::from(p.0[0]), i64::from(p.0[2]));
        let mut d = plan(segment.to) - plan(segment.from);
        let g = i64::from(gcd(d.x.abs() as i32, d.y.abs() as i32).max(1));
        d /= g;
        if d.x < 0 || (d.x == 0 && d.y < 0) {
            d = -d;
            std::mem::swap(&mut segment.from, &mut segment.to);
        }
        let (a, b) = (plan(segment.from), plan(segment.to));
        // Its height where it would cross the line's start, to the centimetre.
        let along = a.as_vec2().dot(d.as_vec2().normalize_or_zero())
            * (CELL_M / crate::layout::TICKS as f32);
        let level =
            libm::roundf((segment.from.metres().y - slope(&segment) * along) * 100.0) as i64;
        let key = (segment.part, d.to_array(), a.perp_dot(d), level);
        lines
            .entry(key)
            .or_default()
            .push((a.dot(d), b.dot(d), segment));
    }
    // Each line's runs of segments end to end, then each run jointed.
    let mut out = Vec::new();
    for ((_, d, _, _), mut on_line) in lines {
        // A line is measured along it in ticks times its direction's length.
        let clear =
            (JOINT_CLEAR as f32 * glam::Vec2::new(d[0] as f32, d[1] as f32).length()) as i64;
        on_line.sort_by_key(|(start, end, _)| (*start, *end));
        let mut runs: Vec<Run> = Vec::new();
        for (start, end, segment) in on_line {
            match runs.last_mut() {
                Some((reach, joined, run))
                    if start <= *reach && (slope(joined) - slope(&segment)).abs() < 0.02 =>
                {
                    if end > *reach {
                        *reach = end;
                        joined.to = segment.to;
                    }
                    run.push((start, end, segment));
                }
                _ => runs.push((end, segment.clone(), vec![(start, end, segment)])),
            }
        }
        for (reach, _, run) in runs {
            let first = run[0].0;
            let mut current: Option<(i64, Segment)> = None;
            let mut last_start = first;
            for (start, end, segment) in run {
                let crossed = start.div_euclid(JOINT_EVERY) > last_start.div_euclid(JOINT_EVERY)
                    && start - first >= clear
                    && reach - start >= clear;
                last_start = start;
                match &mut current {
                    Some((joined_reach, joined)) if !crossed => {
                        if end > *joined_reach {
                            *joined_reach = end;
                            joined.to = segment.to;
                        }
                    }
                    _ => {
                        if let Some((_, done)) = current.take() {
                            out.push(done);
                        }
                        current = Some((end, segment));
                    }
                }
            }
            if let Some((_, done)) = current {
                out.push(done);
            }
        }
    }
    out
}

/// Where a long piece of a roof is jointed, along its line: at every section's edge, in the
/// units a line is measured in (ticks along it, across both axes for a diagonal), but for
/// within two metres, in ticks, of its ends, so no short piece is left at an end.
pub(super) const JOINT_EVERY: i64 = (crate::cubes::SECTION * crate::layout::TICKS) as i64;
pub(super) const JOINT_CLEAR: i64 = 4 * crate::layout::TICKS as i64;

pub(super) fn gcd(a: i32, b: i32) -> i32 {
    if b == 0 { a } else { gcd(b, a % b) }
}
