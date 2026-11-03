//! The mountains' slopes blended between every so many of the ground's lattice points, not
//! measured at each (D604).
//!
//! A mountain's ground point asks the ridges' height 13 times: once where it is, 6 times for
//! the slope its gullies follow (over `SLOPE_STEP_M` either side along each axis) and 6 for
//! the broad slope its faces step by (over `FACES_SLOPE_M`). Those slopes are differences
//! over tens and hundreds of metres, smooth over a metre, so the 12 asks at each of the
//! first view's metre-apart points were some 80% of shaping it. Measured instead at every
//! `SLOPE_EVERY`th and `BROAD_EVERY`th lattice point and blended bilinearly, as a terrain's
//! derived maps (its normals, its flow) are kept on a coarser grid than its heights, each is
//! measured once for every ground point round it; a coarse view's points, which sit on every
//! so many lattice points already, are measured where they are.

use super::{GeneratedPlanet, cube_direction};
use crate::mountains::{FACES_SLOPE_M, Range, SLOPE_STEP_M};
use glam::Vec3;

/// Every how many of the ground's lattice points the gullies' slope is measured.
pub(super) const SLOPE_EVERY: u32 = 8;
/// Every how many the faces' broad slope is.
pub(super) const BROAD_EVERY: u32 = 32;

/// The points of a face's row of `lattice` cells either side of `i` that a slope is measured
/// at, and how far `i` is between them: every `every`th, counted from both of the row's ends,
/// so along a face's edge its points are its neighbour's whichever way the two count.
pub(super) fn bracket(i: u32, lattice: u32, every: u32) -> (u32, u32, f32) {
    let below =
        (i / every * every).max(lattice.saturating_sub((lattice - i).div_ceil(every) * every));
    let above = i
        .div_ceil(every)
        .saturating_mul(every)
        .min(lattice - (lattice - i) / every * every);
    if above == below {
        (below, below, 0.0)
    } else {
        (below, above, (i - below) as f32 / (above - below) as f32)
    }
}

impl GeneratedPlanet {
    /// The direction of a lattice point.
    pub(super) fn lattice_direction(&self, (face, i, j): (u8, u32, u32)) -> Vec3 {
        let cells = self.lattice as f32;
        let u = |k: u32| k as f32 / cells * 2.0 - 1.0;
        cube_direction(face, u(i), u(j))
    }

    /// The ridges' rise round a lattice point over `step_m` (`Range::gradient`), blended from
    /// the points every `every`th it is measured at.
    fn slope_at(
        &self,
        range: &Range,
        (face, i, j): (u8, u32, u32),
        every: u32,
        step_m: f32,
    ) -> Vec3 {
        let held = if every == SLOPE_EVERY {
            &self.known.slopes
        } else {
            &self.known.broad
        };
        let measured = |i: u32, j: u32| {
            held.get_or((face, i, j), || {
                range.gradient(
                    self.lattice_direction((face, i, j)) * self.spec.radius_m,
                    step_m,
                )
            })
        };
        let (i0, i1, fi) = bracket(i, self.lattice, every);
        let (j0, j1, fj) = bracket(j, self.lattice, every);
        let row = |j: u32| {
            let low = measured(i0, j);
            if fi == 0.0 {
                low
            } else {
                low.lerp(measured(i1, j), fi)
            }
        };
        let low = row(j0);
        if fj == 0.0 {
            low
        } else {
            low.lerp(row(j1), fj)
        }
    }

    /// The mountains' two slopes at the lattice point nearest a direction, each across the
    /// ground in metres a metre: the gullies' and the faces' broad one, each as the land takes
    /// it and as measured at the point itself; none off the ranges. For the measure of how far
    /// the one strays from the other (`terra_land`'s `slopes` test, D604).
    pub fn mountain_slopes(&self, direction: Vec3) -> Option<[(Vec3, Vec3); 2]> {
        let range = self.range.as_ref()?;
        let (face, s, t) = super::cube_coordinates(direction);
        let cells = self.lattice as f32;
        let index = |u: f32| (((u + 1.0) * 0.5 * cells).round() as u32).min(self.lattice);
        let at = (face, index(s), index(t));
        let up = self.lattice_direction(at);
        let point = up * self.spec.radius_m;
        if range.inside(point) <= 0.0 {
            return None;
        }
        let level = |rise: Vec3| rise - up * rise.dot(up);
        Some(
            [(SLOPE_EVERY, SLOPE_STEP_M), (BROAD_EVERY, FACES_SLOPE_M)].map(|(every, step_m)| {
                (
                    level(self.slope_at(range, at, every, step_m)),
                    level(range.gradient(point, step_m)),
                )
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::bracket;

    #[test]
    fn a_rows_slope_points_are_every_so_many_from_both_ends_and_bracket_every_point() {
        for (lattice, every) in [(47_124, 8), (47_124, 32), (100, 8), (5, 8), (64, 8)] {
            let points: Vec<u32> = (0..=lattice)
                .filter(|&i| i % every == 0 || (lattice - i) % every == 0)
                .collect();
            for i in 0..=lattice.min(3000) {
                let (low, high, f) = bracket(i, lattice, every);
                // The nearest slope points either side, and the same counted from the far end.
                let below = *points.iter().rev().find(|&&p| p <= i).unwrap();
                let above = *points.iter().find(|&&p| p >= i).unwrap();
                assert_eq!(
                    (low, high),
                    (below, above),
                    "{i} of {lattice} every {every}"
                );
                assert!((0.0..1.0).contains(&f));
                let (l, h, g) = bracket(lattice - i, lattice, every);
                assert_eq!((l, h), (lattice - high, lattice - low));
                if low != high {
                    assert!((g - (1.0 - f)).abs() < 1e-6);
                }
            }
        }
    }
}
