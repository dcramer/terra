//! How enclosed a town's streets are, as a street reads from in it: how far apart the house
//! fronts stand across it, how much of each side is built up, and how wide the gaps between
//! neighbours are. The measures behind a packed town such as Kingdom Come's Rattay, whose
//! lanes are a few metres between fronts with the houses joined along them.

use super::{Town, inside};
use glam::Vec2;

/// How far along a street's line it is sampled.
const STEP_M: f32 = 0.5;
/// How far out from the line to its side a front is looked for.
const REACH_M: f32 = 12.0;
/// How fine the look out is.
const PROBE_M: f32 = 0.1;

/// A town's streets as measured from them (only where they are built up on either side).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Enclosure {
    /// Front to front across a street, in metres, at the middle of the places built up on
    /// both sides; and the narrowest and widest tenth.
    pub between_fronts: [f32; 3],
    /// Of a built-up stretch of a street's side, the share with a front on it.
    pub built_up: f32,
    /// The gaps between neighbouring fronts along a street's side, in metres, at the middle,
    /// and how many there are.
    pub gap: f32,
    pub gaps: usize,
    /// How much of the square's edge has a front within a few metres of it.
    pub square_fronted: f32,
}

/// How far out from `at` toward `out` the first building stands, if within reach.
fn front(town: &Town, at: Vec2, out: Vec2) -> Option<f32> {
    let mut t = 0.0;
    while t <= REACH_M {
        let p = at + out * t;
        if town.footprints.iter().flatten().any(|quad| inside(p, quad)) {
            return Some(t);
        }
        t += PROBE_M;
    }
    None
}

/// Points every `STEP_M` along a line, with the line's way there.
fn along(points: &[Vec2]) -> Vec<(Vec2, Vec2)> {
    let mut out = Vec::new();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = a.distance(b);
        if length < 1e-3 {
            continue;
        }
        let way = (b - a) / length;
        let mut t = 0.0;
        while t < length {
            out.push((a + way * t, way));
            t += STEP_M;
        }
    }
    out
}

fn quantile(values: &mut [f32], q: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f32::total_cmp);
    values[((values.len() - 1) as f32 * q).round() as usize]
}

/// Measures how enclosed `town`'s streets and square are.
pub fn enclosure(town: &Town) -> Enclosure {
    let mut widths = Vec::new();
    let mut gaps = Vec::new();
    let (mut sides, mut built) = (0usize, 0usize);
    for street in &town.streets {
        let samples = along(&street.points);
        let found: Vec<[Option<f32>; 2]> = samples
            .iter()
            .map(|&(at, way)| {
                [1.0, -1.0].map(|side| {
                    front(town, at, way.perp() * side).filter(|&t| t <= street.wide * 0.5 + 6.0)
                })
            })
            .collect();
        for side in 0..2 {
            // A side's built-up stretch: from its first front to its last.
            let hits: Vec<usize> = (0..found.len())
                .filter(|&i| found[i][side].is_some())
                .collect();
            let (Some(&first), Some(&last)) = (hits.first(), hits.last()) else {
                continue;
            };
            let mut run = 0usize;
            for one in &found[first..=last] {
                sides += 1;
                if one[side].is_some() {
                    built += 1;
                    if run > 0 {
                        gaps.push(run as f32 * STEP_M);
                    }
                    run = 0;
                } else {
                    run += 1;
                }
            }
        }
        widths.extend(
            found
                .iter()
                .filter_map(|[a, b]| Some(a.as_ref()? + b.as_ref()?)),
        );
    }
    let outline = &town.square.outline;
    let middle = outline.iter().copied().sum::<Vec2>() / outline.len() as f32;
    let edge: Vec<Vec2> = outline
        .iter()
        .copied()
        .chain(outline.first().copied())
        .collect();
    let rim = along(&edge);
    let fronted = rim
        .iter()
        .filter(|&&(at, way)| {
            let out = way.perp() * -(middle - at).dot(way.perp()).signum();
            front(town, at, out).is_some_and(|t| t <= 4.0)
        })
        .count();
    Enclosure {
        between_fronts: [
            quantile(&mut widths, 0.5),
            quantile(&mut widths, 0.1),
            quantile(&mut widths, 0.9),
        ],
        built_up: if sides > 0 {
            built as f32 / sides as f32
        } else {
            0.0
        },
        gap: quantile(&mut gaps, 0.5),
        gaps: gaps.len(),
        square_fronted: if rim.is_empty() {
            0.0
        } else {
            fronted as f32 / rim.len() as f32
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::town::{Paths, Square, SquareShape, Street};

    /// A street 4 m wide along x with a row of 6 m houses each side, fronts 1 m off its
    /// edges and 2 m apart along it: 6 m front to front, a third of each side a gap.
    fn row() -> Town {
        let house = |x: f32, side: f32| {
            let (near, far) = (3.0 * side, 9.0 * side);
            [
                Vec2::new(x, near.min(far)),
                Vec2::new(x + 6.0, near.min(far)),
                Vec2::new(x + 6.0, near.max(far)),
                Vec2::new(x, near.max(far)),
            ]
        };
        let mut footprints = Vec::new();
        for i in 0..5 {
            for side in [1.0, -1.0] {
                footprints.push(vec![house(i as f32 * 8.0, side)]);
            }
        }
        Town {
            square: Square {
                shape: SquareShape::Long,
                outline: vec![
                    Vec2::new(-30.0, -5.0),
                    Vec2::new(-20.0, -5.0),
                    Vec2::new(-20.0, 5.0),
                    Vec2::new(-30.0, 5.0),
                ],
                along: Vec2::X,
                across: Vec2::Y,
            },
            streets: vec![Street {
                points: vec![Vec2::new(-10.0, 0.0), Vec2::new(50.0, 0.0)],
                wide: 4.0,
            }],
            buildings: Vec::new(),
            footprints,
            homes: Vec::new(),
            yards: Vec::new(),
            lanes: Vec::new(),
            things: Vec::new(),
            fences: Vec::new(),
            paths: Paths::default(),
        }
    }

    #[test]
    fn a_row_measures_as_built() {
        let measured = enclosure(&row());
        assert!(
            (measured.between_fronts[0] - 6.0).abs() < 0.3,
            "{measured:?}"
        );
        // Each side runs 38 m from its first front to its last, 30 of them built (a sample
        // either end of each house more, sampled every half metre).
        assert!(
            (measured.built_up - 30.0 / 38.0).abs() < 0.08,
            "{measured:?}"
        );
        assert!((measured.gap - 2.0).abs() <= 1.0, "{measured:?}");
        assert_eq!(measured.square_fronted, 0.0);
    }
}
