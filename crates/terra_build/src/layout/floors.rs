//! Decks (docs/building-kits.md, construction-001's floor cases): a timber kind's floors and
//! flat roofs, every cube of its kind not in a wall, laid as a real deck is: sawn boards
//! side by side across the top, two to a cube's width and as long as a tree gives, their
//! joints staggered and over a joist; a rim beam along every side left open, so a floor's
//! edge reads as timber; and joists under it, one down the middle of each row of cubes,
//! where its underside is in the air. Where a floor meets a wall or a footing, its boards
//! stop at the cube between them and the rim is left out, as the wall or footing covers it.

use super::{Layout, Piece, Place, Random, hash, ticks};
use crate::kit::Kit;
use crate::{Base, CELL_M, Fill};
use glam::{IVec3, Vec3};
use std::collections::BTreeSet;

/// How thick a board is, and how far its top stands over its cube's, clear of the relief of
/// a stone footing it runs onto.
const BOARD_THICK_M: f32 = 0.04;
const BOARD_PROUD_M: f32 = 0.008;
/// The gap between boards side by side and end to end.
const GAP_M: f32 = 0.006;
/// How wide a rim beam is, and a joist and how deep.
const RIM_WIDE_M: f32 = 0.08;
/// How deep each beam of a rim is at most: a deeper rim is built up of beams stacked.
const RIM_COURSE_M: f32 = 0.24;
const JOIST_WIDE_M: f32 = 0.1;
const JOIST_DEEP_M: f32 = 0.2;
/// How far either way of the middle of a cube the line between its two boards falls, as a
/// fraction of the cube.
const SPLIT: f32 = 0.08;

/// The four ways along the floor.
const SIDES: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];

/// Lays a deck over every cube of the kit's kind laid as one (`Walls::cell`): its floors and
/// flat roofs, and the tops of its solid masses.
pub(super) fn decks(walls: &super::Walls, kit: &Kit, scope: super::Scope, layout: &mut Layout) {
    let (base, kind) = (walls.base, kit.kind());
    let deck: BTreeSet<[i32; 3]> = base
        .cubes()
        .filter(|(at, cube)| cube.kind == kind && walls.cell(*at) == Some(super::Cell::Deck))
        .map(|(at, _)| at.to_array())
        .collect();
    regions(base, kit, &deck, CELL_M, scope, layout);
}

/// Lays a deck over each region of cubes joined side by side on a layer, its top so high
/// over the layer's foot: a cube's height, or half for halves.
pub(super) fn regions(
    base: &Base,
    kit: &Kit,
    deck: &BTreeSet<[i32; 3]>,
    height: f32,
    scope: super::Scope,
    layout: &mut Layout,
) {
    let longest = kit.longest.max(CELL_M);
    let mut seen = BTreeSet::new();
    for &start in deck {
        if seen.contains(&start) {
            continue;
        }
        // The cubes joined side by side on its layer: one deck.
        let mut region = BTreeSet::from([start]);
        let mut next = vec![IVec3::from_array(start)];
        seen.insert(start);
        while let Some(at) = next.pop() {
            for side in SIDES {
                let beside = (at + side).to_array();
                if deck.contains(&beside) && seen.insert(beside) {
                    region.insert(beside);
                    next.push(at + side);
                }
            }
        }
        // Only a deck that reaches the scope is laid, but all of it.
        let cells = region.iter().map(|&at| IVec3::from_array(at));
        let low = cells.clone().reduce(IVec3::min).unwrap_or_default();
        let high = cells.reduce(IVec3::max).unwrap_or_default();
        if !scope.reaches(low, high) {
            continue;
        }
        // Boards run on under a wall as far as its members' faces, where it stands on
        // something else beside the deck, as a footing.
        let under_wall = ((CELL_M - kit.diameter) * 0.5).max(0.0);
        lay_deck(base, &region, longest, under_wall, height, layout);
    }
}

/// A deck's boards, rims and joists over one region of cubes on a layer.
fn lay_deck(
    base: &Base,
    region: &BTreeSet<[i32; 3]>,
    longest: f32,
    under_wall: f32,
    height: f32,
    layout: &mut Layout,
) {
    let cells: Vec<IVec3> = region.iter().map(|&at| IVec3::from_array(at)).collect();
    let low = cells.iter().copied().reduce(IVec3::min).unwrap_or_default();
    let high = cells.iter().copied().reduce(IVec3::max).unwrap_or_default();
    let y = low.y;
    let top = y as f32 * CELL_M + height;
    // Boards along the longer way, joists across them.
    let along_x = high.x - low.x >= high.z - low.z;
    let (along, across) = if along_x {
        (IVec3::X, IVec3::Z)
    } else {
        (IVec3::Z, IVec3::X)
    };
    let cell = |a: i32, b: i32| along * a + across * b + IVec3::Y * y;
    let (a_low, b_low) = (low.dot(along), low.dot(across));
    let (a_high, b_high) = (high.dot(along), high.dot(across));
    let covered = |at: IVec3| {
        base.cube(at + IVec3::Y)
            .is_some_and(|c| c.fill != Fill::Open)
    };
    // A cube beside the deck under a wall: the boards run on over it, under the wall.
    let wall_foot = |at: IVec3| {
        !region.contains(&at.to_array())
            && base.cube(at).is_some_and(|c| c.fill == Fill::Solid)
            && base.in_wall(at + IVec3::Y)
    };
    let board_y = top + BOARD_PROUD_M - BOARD_THICK_M * 0.5;
    let point =
        |a: f32, b: f32, height: f32| along.as_vec3() * a + across.as_vec3() * b + Vec3::Y * height;
    // Boards: two across each row of cubes, along every run of it whose top is open.
    for b in b_low..=b_high {
        let mut row = Random(hash(&[0xdec, b as u32, y as u32, along_x as u32]));
        let split = 0.5 + SPLIT * row.signed();
        let runs = runs(a_low..=a_high, |a| {
            region.contains(&cell(a, b).to_array()) && !covered(cell(a, b))
        });
        for (board, (from, to)) in [(0.0, split), (split, 1.0)].into_iter().enumerate() {
            for &(first, last) in &runs {
                // The outer board of a row along a wall runs on under it.
                let beside = if board == 0 { b - 1 } else { b + 1 };
                let wider = if (first..=last).all(|a| wall_foot(cell(a, beside))) {
                    under_wall
                } else {
                    0.0
                };
                let (from_m, to_m) = if board == 0 {
                    (
                        b as f32 * CELL_M + from * CELL_M - wider,
                        (b as f32 + to) * CELL_M,
                    )
                } else {
                    ((b as f32 + from) * CELL_M, (b as f32 + to) * CELL_M + wider)
                };
                let middle = (from_m + to_m) * 0.5;
                let wide = to_m - from_m - GAP_M;
                let (start_out, end_out) = (
                    if wall_foot(cell(first - 1, b)) {
                        under_wall
                    } else {
                        0.0
                    },
                    if wall_foot(cell(last + 1, b)) {
                        under_wall
                    } else {
                        0.0
                    },
                );
                // Joints over a joist, staggered from the boards beside it.
                let seed = hash(&[0xb0a, b as u32, board as u32, y as u32, first as u32]);
                let mut lengths = Random(seed);
                let mut at = first;
                let mut piece = 0u32;
                while at <= last {
                    let most = ((longest / CELL_M) as i32).max(1);
                    let span = if piece == 0 {
                        1 + (lengths.unit() * most as f32) as i32
                    } else {
                        most - (lengths.unit() * 2.0) as i32
                    }
                    .clamp(1, most);
                    let end = (at + span - 1).min(last);
                    let start_m = at as f32 * CELL_M + GAP_M * 0.5
                        - if at == first { start_out } else { 0.0 };
                    let end_m = (end + 1) as f32 * CELL_M - GAP_M * 0.5
                        + if end == last { end_out } else { 0.0 };
                    layout.pieces.push(Piece::Plank {
                        from: Place::near(point(start_m, middle, board_y)),
                        to: Place::near(point(end_m, middle, board_y)),
                        wide: ticks(wide),
                        high: ticks(BOARD_THICK_M),
                        seed: hash(&[seed, piece]),
                    });
                    at = end + 1;
                    piece += 1;
                }
            }
        }
    }
    // Rims along every side of it left open, the length of each run of that side.
    let beam_high = height - BOARD_THICK_M;
    for side in SIDES {
        let runs_along = if side.dot(along) != 0 { across } else { along };
        let open = |at: IVec3| {
            region.contains(&at.to_array())
                && base
                    .cube(at + side)
                    .is_none_or(|cube| cube.fill == Fill::Open)
        };
        let lines: BTreeSet<i32> = cells.iter().map(|at| at.dot(side.abs())).collect();
        for line in lines {
            let first = cells.iter().map(|at| at.dot(runs_along)).min().unwrap_or(0);
            let last = cells.iter().map(|at| at.dot(runs_along)).max().unwrap_or(0);
            let at_of = |k: i32| side.abs() * line + runs_along * k + IVec3::Y * y;
            for (from, to) in runs(first..=last, |k| open(at_of(k))) {
                let face = (line as f32 + 0.5 + side.dot(side.abs()) as f32 * 0.5) * CELL_M
                    - side.dot(side.abs()) as f32 * RIM_WIDE_M * 0.5;
                // In beams a course deep, stacked, as a deep rim is built up.
                let beams = ((beam_high / RIM_COURSE_M).round() as i32).max(1);
                let deep = beam_high / beams as f32;
                for beam in 0..beams {
                    let height = y as f32 * CELL_M + deep * (beam as f32 + 0.5);
                    let on = |k: f32| {
                        side.abs().as_vec3() * face + runs_along.as_vec3() * k + Vec3::Y * height
                    };
                    layout.pieces.push(Piece::Plank {
                        from: Place::near(on(from as f32 * CELL_M)),
                        to: Place::near(on((to + 1) as f32 * CELL_M)),
                        wide: ticks(RIM_WIDE_M),
                        high: ticks(deep - GAP_M),
                        seed: hash(&[
                            0x41b,
                            line as u32,
                            from as u32,
                            side.x as u32,
                            side.z as u32,
                            beam as u32,
                        ]),
                    });
                }
            }
        }
    }
    // Joists down the middle of each row of cubes across the boards, under the cubes whose
    // underside is in the air.
    let hangs = |at: IVec3| {
        region.contains(&at.to_array()) && base.cube(at - IVec3::Y).is_none() && !base.grounded(at)
    };
    for a in a_low..=a_high {
        for (first, last) in runs(b_low..=b_high, |b| hangs(cell(a, b))) {
            let middle = (a as f32 + 0.5) * CELL_M;
            let height = top - BOARD_THICK_M - JOIST_DEEP_M * 0.5;
            layout.pieces.push(Piece::Plank {
                from: Place::near(point(middle, first as f32 * CELL_M, height)),
                to: Place::near(point(middle, (last + 1) as f32 * CELL_M, height)),
                wide: ticks(JOIST_WIDE_M),
                high: ticks(JOIST_DEEP_M),
                seed: hash(&[0x1015, a as u32, first as u32, y as u32]),
            });
        }
    }
}

/// The runs of a range where `on` holds, as their first and last.
fn runs(range: std::ops::RangeInclusive<i32>, on: impl Fn(i32) -> bool) -> Vec<(i32, i32)> {
    let mut found: Vec<(i32, i32)> = Vec::new();
    for k in range {
        if !on(k) {
            continue;
        }
        match found.last_mut() {
            Some((_, last)) if *last + 1 == k => *last = k,
            _ => found.push((k, k)),
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::super::lay;
    use super::super::tests::{cabin, kit, textured};
    use super::*;
    use crate::{BuildOp, Kind};

    fn planks(layout: &Layout) -> Vec<(Vec3, Vec3, i32, i32)> {
        layout
            .pieces
            .iter()
            .filter_map(|piece| match *piece {
                Piece::Plank {
                    from,
                    to,
                    wide,
                    high,
                    ..
                } => Some((from.metres(), to.metres(), wide, high)),
                _ => None,
            })
            .collect()
    }

    /// How much of each cube's top a deck's boards cover, by cube.
    fn boarded(layout: &Layout) -> std::collections::BTreeMap<[i32; 3], f32> {
        let mut cover = std::collections::BTreeMap::new();
        for (from, to, wide, high) in planks(layout) {
            let (wide, high_m) = (super::super::metres(wide), super::super::metres(high));
            if high != ticks(BOARD_THICK_M) {
                continue;
            }
            let along = (to - from).normalize();
            let across = Vec3::Y.cross(along).abs();
            let y = ((from.y + high_m) / CELL_M).floor() as i32 - 1;
            let steps = (from.distance(to) / 0.05).round() as i32;
            for s in 0..steps {
                let p = from + along * (0.025 + s as f32 * 0.05);
                for c in [-0.45, -0.15, 0.15, 0.45] {
                    let q = p + across * (wide * c);
                    let at = (q / CELL_M).floor().as_ivec3().with_y(y);
                    *cover.entry(at.to_array()).or_insert(0.0) += 0.05 * wide * 0.25;
                }
            }
        }
        cover
    }

    #[test]
    fn a_floor_is_boarded_over_every_open_cube_and_nowhere_else() {
        let base = cabin();
        let layout = lay(&base, &textured());
        let cover = boarded(&layout);
        let deck: BTreeSet<[i32; 3]> = base
            .cubes()
            .filter(|(at, _)| !base.in_wall(*at))
            .filter(|(at, _)| base.cube(*at + IVec3::Y).is_none())
            .map(|(at, _)| at.to_array())
            .collect();
        // Over every cube of floor and flat roof not under a wall, its top nearly all
        // boarded, but for the gaps between boards; nowhere else.
        assert_eq!(cover.keys().copied().collect::<BTreeSet<_>>(), deck);
        for (at, area) in &cover {
            let whole = CELL_M * CELL_M;
            assert!(
                *area > whole * 0.93 && *area < whole * 1.01,
                "{at:?}: {area}"
            );
        }
        // No board longer than a tree gives.
        assert!(
            planks(&layout)
                .iter()
                .all(|(from, to, ..)| from.distance(*to) <= kit().longest + 1e-4)
        );
    }

    #[test]
    fn a_floor_inside_stone_footings_runs_on_under_its_log_walls_to_their_faces() {
        let mut base = cabin();
        base.lay_footings(Kind(2), |_| 0);
        let layout = lay(&base, &textured());
        let boards: Vec<(Vec3, Vec3, f32)> = planks(&layout)
            .into_iter()
            .filter(|p| p.3 == ticks(BOARD_THICK_M))
            .map(|(from, to, wide, _)| (from, to, super::super::metres(wide)))
            .collect();
        // Inside the walls, 3 m by 2, and under the logs as far as their faces.
        let reach = (CELL_M - kit().diameter) * 0.5;
        let (low, high) = boards.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(low, high), &(from, to, wide)| {
                let across = Vec3::Y.cross((to - from).normalize()).abs() * wide * 0.5;
                (
                    low.min(from - across).min(to - across),
                    high.max(from + across).max(to + across),
                )
            },
        );
        assert!((low.x - (CELL_M - reach)).abs() < 0.01, "{low}");
        assert!((high.x - (7.0 * CELL_M + reach)).abs() < 0.01, "{high}");
        assert!((low.z - (CELL_M - reach)).abs() < 0.01, "{low}");
        assert!((high.z - (5.0 * CELL_M + reach)).abs() < 0.01, "{high}");
    }

    #[test]
    fn a_deck_out_in_the_open_has_rims_round_it_and_joists_under_it_in_the_air() {
        let mut base = Base::default();
        // A deck three metres by two, a storey up.
        base.apply(BuildOp::SetFloor {
            min: [0, 1, 0],
            max: [5, 1, 3],
            laid: true,
            kind: kit().kind(),
        });
        let layout = lay(&base, &textured());
        let all = planks(&layout);
        let rims: Vec<_> = all.iter().filter(|p| p.2 == ticks(RIM_WIDE_M)).collect();
        let joists: Vec<_> = all.iter().filter(|p| p.2 == ticks(JOIST_WIDE_M)).collect();
        // A rim along each of its four sides, their outer faces on its edges, two beams deep.
        assert_eq!(rims.len(), 4 * 2, "{rims:?}");
        let rim_length: f32 = rims.iter().map(|(f, t, ..)| f.distance(*t)).sum();
        assert!((rim_length - 2.0 * 2.0 * (3.0 + 2.0)).abs() < 1e-4);
        // A joist under each row of cubes along the boards, across the deck.
        assert_eq!(joists.len(), 6);
        // Everything within its cubes.
        for (from, to, wide, high) in &all {
            let piece = Piece::Plank {
                from: Place::near(*from),
                to: Place::near(*to),
                wide: *wide,
                high: *high,
                seed: 0,
            };
            let (min, max) = piece.bounds().unwrap();
            assert!(min.cmpge(Vec3::new(0.0, 2.5, 0.0) - 2e-3).all(), "{min}");
            // But for the boards standing a little proud.
            let top = 3.0 + BOARD_PROUD_M;
            assert!(max.cmple(Vec3::new(3.0, top, 2.0) + 2e-3).all(), "{max}");
        }
    }
}
