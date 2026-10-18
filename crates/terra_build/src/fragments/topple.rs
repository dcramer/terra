//! A tall fall tipping over as one before it breaks up (D305): a tower or a wall section whose
//! foot gives way leans out on the edge of its foot, slowly and then faster, and its blocks let
//! go of it, the top first, each flung on as it was going and breaking where it lands.

use super::{Fragment, GRAVITY, Shape, unit};
use crate::{Base, Cube};
use glam::{IVec3, Quat, Vec3};
use terra_gen::hash;

/// A tall fall tipping over as one before it breaks up ([`topple`]): the edge of its foot it
/// turns about, the axis along that edge, how far over it has gone and how fast it is
/// going over, in radians and radians a second; where the fragment's middle was from that
/// edge and how it was turned when it stood; how tall what tips is, which sets how quickly
/// it goes over; and how far over this fragment lets go of it and flies on its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hinge {
    pub pivot: Vec3,
    pub axis: Vec3,
    pub angle: f32,
    pub rate: f32,
    pub from: Vec3,
    pub stood: Quat,
    pub tall: f32,
    pub lets_go: f32,
}

impl Hinge {
    /// Turns it over by a step as a tall body hinged at its foot falls (θ'' = 3g sin θ / 2L),
    /// returning where the fragment's middle is and how it is turned.
    pub(super) fn step(&mut self, dt: f32) -> (Vec3, Quat) {
        let pull = 1.5 * GRAVITY / self.tall.max(1.0);
        self.rate += pull * libm::sinf(self.angle) * dt;
        self.angle += self.rate * dt;
        let over = Quat::from_axis_angle(self.axis, self.angle);
        (self.pivot + over * self.from, over * self.stood)
    }

    /// How fast a point at `at` on what tips is moving.
    pub(super) fn velocity(&self, at: Vec3) -> Vec3 {
        (self.axis * self.rate).cross(at - self.pivot)
    }
}

/// From how many cubes tall, and how many in all, what was left hanging tips over as one
/// before it breaks up ([`topple`]); how far over it is when it starts and how fast it is
/// going, in radians and radians a second; how far over its top lets go of it and its foot
/// at most.
const TOPPLES_TALL: i32 = 8;
const TOPPLES_FROM: usize = 120;
const TOPPLE_START: f32 = 0.03;
const TOPPLE_KICK: f32 = 0.12;
const LETS_GO_TOP: f32 = 0.3;
const LETS_GO_FOOT: f32 = 1.1;

/// A tall fall tips over as one, as a tower or a wall section does when its foot gives way:
/// the blocks it came down as ([`burst`]'s whole ones, still waiting their turn) hinged on the
/// edge of its foot on the side it leans to, going over slowly and then faster, its top
/// letting go first and the rest after, each block flung on as it was going and breaking
/// where it lands. It leans the way `lean` says (on the level; away from its base's middle,
/// say), or if that says nothing, from where it broke, or a way of its own.
pub fn topple(made: &mut [Fragment], fell: &[(IVec3, Cube)], lean: Vec3) {
    let hanging: Vec<usize> = (0..made.len())
        .filter(|&i| made[i].shape == Shape::Whole && made[i].hinge.is_none())
        .collect();
    if fell.len() < TOPPLES_FROM || hanging.is_empty() {
        return;
    }
    let (low, high) = fell.iter().fold(
        (IVec3::splat(i32::MAX), IVec3::splat(i32::MIN)),
        |(low, high), (at, _)| (low.min(*at), high.max(*at)),
    );
    if high.y - low.y + 1 < TOPPLES_TALL {
        return;
    }
    let middle = hanging.iter().map(|&i| made[i].at).sum::<Vec3>() / hanging.len() as f32;
    let bottom = hanging
        .iter()
        .map(|&i| made[i].at.y - made[i].size.y * 0.5)
        .fold(f32::MAX, f32::min);
    let top = hanging
        .iter()
        .map(|&i| made[i].at.y + made[i].size.y * 0.5)
        .fold(f32::MIN, f32::max);
    let seed = hash::of(&[low.x as u32, low.y as u32, low.z as u32, fell.len() as u32]);
    let own = unit(&[seed, 1]) * core::f32::consts::TAU;
    let way = lean.with_y(0.0).try_normalize().unwrap_or(Vec3::new(
        libm::cosf(own),
        0.0,
        libm::sinf(own),
    ));
    // The edge of its foot on that side: as far that way as any of it reaches.
    let reach = hanging
        .iter()
        .map(|&i| (made[i].at - middle).dot(way) + made[i].size.max_element() * 0.5)
        .fold(0.0f32, f32::max);
    let pivot = Vec3::new(middle.x, bottom, middle.z) + way * reach;
    let axis = way.cross(Vec3::NEG_Y).normalize();
    let tall = top - bottom;
    let start = TOPPLE_START * (0.7 + 0.6 * unit(&[seed, 2]));
    for &i in &hanging {
        let piece = &mut made[i];
        let height = ((piece.at.y - bottom) / tall.max(0.1)).clamp(0.0, 1.0);
        let lets_go = LETS_GO_TOP
            + (LETS_GO_FOOT - LETS_GO_TOP) * (1.0 - height)
            + 0.12 * (unit(&[piece.seed, 77]) - 0.5);
        piece.hinge = Some(Hinge {
            pivot,
            axis,
            angle: start,
            rate: TOPPLE_KICK,
            from: piece.at - pivot,
            stood: piece.turn,
            tall,
            lets_go,
        });
        // All of it goes together, at once.
        piece.wait = 0.0;
        piece.velocity = Vec3::ZERO;
        piece.spin = Vec3::ZERO;
    }
}

impl Base {
    /// A box holding every cube, in cubes, to the sections they are in, if there are any.
    pub fn bounds(&self) -> Option<(IVec3, IVec3)> {
        self.cubes.bounds()
    }
}

#[cfg(test)]
mod tests {
    use super::super::burst;
    use super::super::tests::{chunks, cube};
    use super::*;
    use crate::Shape as CubeShape;

    #[test]
    fn a_tall_wall_whose_foot_gives_way_tips_over_as_one_the_way_it_leans_then_breaks_up() {
        // A wall 30 cubes long, 3 thick and 24 high along x, its foot destroyed.
        let wall: Vec<(IVec3, Cube)> = (0..24)
            .flat_map(|y| (0..30).flat_map(move |x| (0..3).map(move |z| IVec3::new(x, y, z))))
            .map(|at| (at, cube(CubeShape::Whole)))
            .collect();
        let foot = |at: IVec3| at.y == 0;
        let mut parts = burst(&wall, &foot, &chunks, 1.0, 3000);
        topple(&mut parts, &wall, Vec3::Z);
        let top = |parts: &[Fragment]| {
            parts
                .iter()
                .filter(|p| p.shape == Shape::Whole)
                .max_by(|a, b| a.at.y.total_cmp(&b.at.y))
                .copied()
                .unwrap()
        };
        let before = top(&parts);
        let floor = |_: Vec3| 0.0;
        let solid = |_: Vec3| false;
        let mut together = None;
        for step in 0..60 * 5 {
            for part in &mut parts {
                part.step(1.0 / 60.0, &floor, &solid);
            }
            // Early on, still all one: its top and its middle go over at the same angle.
            if step == 60 {
                let hinged: Vec<_> = parts.iter().filter_map(|p| p.hinge).collect();
                assert!(hinged.len() > 10, "{} hinged", hinged.len());
                together = Some(
                    hinged
                        .iter()
                        .all(|h| (h.angle - hinged[0].angle).abs() < 1e-4),
                );
            }
        }
        assert_eq!(together, Some(true), "it did not go over as one");
        // Its top went over the way it leaned, a long way out past its foot, and came down.
        let fell_out = parts
            .iter()
            .filter(|p| p.shape != Shape::Whole || p.hinge.is_none())
            .map(|p| p.at.z)
            .fold(f32::MIN, f32::max);
        assert!(fell_out > 6.0, "the farthest piece is {fell_out} m out");
        assert!(parts.iter().all(|p| p.hinge.is_none()), "some never let go");
        assert!(parts.iter().all(|p| p.at.y < before.at.y - 3.0));
        // Not the other way.
        let back = parts.iter().map(|p| p.at.z).fold(f32::MAX, f32::min);
        assert!(back > -2.5, "a piece went {back} m the wrong way");
    }
}
