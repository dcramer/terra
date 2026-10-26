//! How far a point of a foot stands over the world's ground straight under it, for the walk
//! record (`feet.ground`, `tools/controller/yard.ts`): the planet's field, dug or not, and the
//! boxes the last tick gathered round the players (a base's cubes, props), sampled down a line
//! along the body's up. A path of its own, not the foot placement's probe, so the record judges
//! the figure's feet by the world rather than by what the feet were told (D455's foot IK).

use bevy::{ecs::system::SystemParam, math::DVec3, prelude::*};
use terra_walker::movement::Movers;
use terra_world::planet::{HeightMemo, Planet};

/// The line asked down: from so far over the point to so far under it, m (a foot over a
/// tread a step lower, or sunk into one a step higher, is found).
const ABOVE_M: f64 = 0.7;
const BELOW_M: f64 = 0.7;
/// The line's samples this far apart, m (thinner than any floor a foot stands on), and the
/// crossing found to within so much.
const SAMPLE_M: f64 = 0.02;
const WITHIN_M: f64 = 0.0005;
/// How many of the land's lattice points the memo keeps before it forgets them.
const MEMO_MOST: usize = 1024;

/// How far `at` stands over the ground under it along `up`, m, negative sunk into it: the
/// first crossing into the ground (`distance` from positive to not) going down the line from
/// ABOVE_M over `at` to BELOW_M under it. None if the line never goes into the ground (over a
/// drop).
pub fn gap(mut distance: impl FnMut(DVec3) -> f32, at: DVec3, up: DVec3) -> Option<f32> {
    let along = |t: f64| at + up * t;
    let steps = ((ABOVE_M + BELOW_M) / SAMPLE_M).ceil() as usize;
    let mut was: Option<f64> = None;
    for i in 0..=steps {
        let t = ABOVE_M - i as f64 * SAMPLE_M;
        let outside = distance(along(t)) > 0.0;
        match (was, outside) {
            (_, true) => was = Some(t),
            (Some(over), false) => {
                let (mut hi, mut lo) = (over, t);
                while hi - lo > WITHIN_M {
                    let mid = (hi + lo) * 0.5;
                    if distance(along(mid)) > 0.0 {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                return Some((-(hi + lo) * 0.5) as f32);
            }
            (None, false) => {}
        }
    }
    None
}

/// The world's ground under a point: ask it with `gap`.
#[derive(SystemParam)]
pub struct Ground<'w, 's> {
    planet: Option<Res<'w, Planet>>,
    movers: Option<Res<'w, Movers>>,
    memo: Local<'s, HeightMemo>,
}

impl Ground<'_, '_> {
    /// How far `at` stands over the world's ground along `up` (`gap`); none with no planet.
    pub fn gap(&mut self, at: Vec3, up: Vec3) -> Option<f32> {
        let planet = self.planet.as_deref()?;
        self.memo.keep_within(MEMO_MOST);
        let memo = &mut *self.memo;
        let blocks = self.movers.as_deref().map_or(&[][..], Movers::blocks);
        let distance = |p: DVec3| {
            let boxes = blocks.iter().map(|block| block.distance(p.as_vec3()));
            boxes.fold(planet.distance_in(p, memo), f32::min)
        };
        gap(distance, at.as_dvec3(), up.as_dvec3())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Level ground at y 0, and a tread 0.5 m higher east of x 0.
    fn treads(p: DVec3) -> f32 {
        let lower = p.y;
        let upper = if p.x >= 0.0 { p.y - 0.5 } else { f64::INFINITY };
        lower.min(upper) as f32
    }

    #[test]
    fn a_point_over_the_ground_or_sunk_into_it() {
        let at = |x: f64, y: f64| gap(treads, DVec3::new(x, y, 0.0), DVec3::Y).unwrap();
        assert!((at(-1.0, 0.05) - 0.05).abs() < 1e-3);
        // A foot left at the lower tread's height over the upper is sunk into it.
        assert!((at(0.2, 0.05) + 0.45).abs() < 1e-3);
        assert!((at(0.2, 0.53) - 0.03).abs() < 1e-3);
    }

    #[test]
    fn over_a_drop_there_is_no_ground() {
        let drop = |p: DVec3| (p.y + 2.0) as f32;
        assert_eq!(gap(drop, DVec3::ZERO, DVec3::Y), None);
    }
}
