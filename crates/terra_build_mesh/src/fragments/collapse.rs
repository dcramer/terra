//! How what was left hanging comes down (D305), by rule for every structure, as masonry does
//! (research/2026-10-18-how-structures-collapse.md):
//!
//! - **Support goes from the bottom up.** Its blocks fall together, the cracks running up
//!   through it a moment apart (`burst`'s stagger), so the lowest courses land first and
//!   break, and what is over them comes down onto their heap ([`super::Rubble`]).
//! - **It tips only when its support gave way on one side and it is slender** ([`falls`]):
//!   a tower whose foot went evenly crumbles where it stood, its shell peeling outward as it
//!   goes, as an imploded building's walls do; a squat block slumps.
//! - **Tipping, it breaks up within a few degrees.** Masonry has almost no tensile strength:
//!   a tall body hinged at its foot bends hardest a third of the way up (the falling
//!   chimney), so it cracks there and at its courses within its material's angle
//!   ([`super::Look::cracks`]), each block flung on as it was going and turning as the whole
//!   was, so they part in the air rather than go over as one.

use super::{Breaks, Fragment, GRAVITY, Shape, unit};
use crate::fragments::Breaking;
use glam::{IVec3, Quat, Vec3};
use terra_build::cubes::Kind;
use terra_build::{CELL_M, Cube};
use terra_gen::hash;

/// A tall fall tipping before it breaks up ([`collapse`]): the edge of its foot it turns
/// about, the axis along that edge, how far over it has gone and how fast it is going over,
/// in radians and radians a second; where the fragment's middle was from that edge and how
/// it was turned when it stood; how tall what tips is, which sets how quickly it goes over;
/// and how far over this fragment cracks off it and flies on its own.
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

/// How what was left hanging comes down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Falls {
    /// Where it stood, onto its own heap.
    InPlace,
    /// Over the way its support gave way, on the level.
    Tips { way: Vec3 },
}

/// From how many cubes tall, and how many in all, a fall can tip; how much taller than deep
/// along the way it would go it must be (a squat block slumps); how far off its foot's
/// middle what gave way under it must be, as a share of its foot's half depth that way, to
/// count as one-sided; and how many cubes under its lowest what gave way may be.
const TIPS_TALL: i32 = 8;
const TIPS_FROM: usize = 120;
const SLENDER: f32 = 1.5;
const UNEVEN: f32 = 0.3;
const FOOT_CUBES: i32 = 2;

/// How what was left hanging of `fell` comes down, from where its support gave way:
/// `destroyed` says which of it harm broke.
pub fn falls(fell: &[(IVec3, Cube)], destroyed: &dyn Fn(IVec3) -> bool) -> Falls {
    let hanging: Vec<IVec3> = fell
        .iter()
        .filter(|(at, cube)| !destroyed(*at) && !cube.kind.is_roof())
        .map(|(at, _)| *at)
        .collect();
    if hanging.len() < TIPS_FROM {
        return Falls::InPlace;
    }
    let (low, high) = bounds(&hanging);
    let tall = high.y - low.y + 1;
    if tall < TIPS_TALL {
        return Falls::InPlace;
    }
    // Its foot, and what gave way under it.
    let level = |at: &IVec3| Vec3::new(at.x as f32 + 0.5, 0.0, at.z as f32 + 0.5);
    let foot: Vec<Vec3> = hanging
        .iter()
        .filter(|at| at.y < low.y + FOOT_CUBES)
        .map(level)
        .collect();
    let gave: Vec<Vec3> = fell
        .iter()
        .filter(|(at, _)| destroyed(*at) && at.y < low.y + FOOT_CUBES && at.y >= low.y - FOOT_CUBES)
        .filter(|(at, _)| {
            at.x >= low.x - 1 && at.x <= high.x + 1 && at.z >= low.z - 1 && at.z <= high.z + 1
        })
        .map(|(at, _)| level(at))
        .collect();
    if gave.is_empty() || foot.is_empty() {
        return Falls::InPlace;
    }
    let middle = |points: &[Vec3]| points.iter().sum::<Vec3>() / points.len() as f32;
    let off = middle(&gave) - middle(&foot);
    let Some(way) = off.try_normalize() else {
        return Falls::InPlace;
    };
    let along = |way: Vec3| {
        let (least, most) = foot.iter().fold((f32::MAX, f32::MIN), |(least, most), at| {
            (least.min(at.dot(way)), most.max(at.dot(way)))
        });
        most - least + 1.0
    };
    let deep = along(way);
    if off.length() < UNEVEN * deep * 0.5 || (tall as f32) < SLENDER * deep {
        return Falls::InPlace;
    }
    Falls::Tips { way }
}

/// How fast, at most, in metres a second, a block crumbling in place is thrown out from the
/// middle of what fell, at its shell and at its top, as an imploded wall peels outward;
/// how much spin it is left with, a share of what it was thrown with.
const PEELS_OUT: f32 = 1.4;
const PEEL_SPIN: f32 = 0.5;
/// How far over a tipping fall starts and how fast it is going, in radians and radians a
/// second; how much its blocks' crack angles vary, as a share either way.
const TIP_START: f32 = 0.03;
const TIP_KICK: f32 = 0.15;
const CRACKS_VARY: f32 = 0.3;

/// What was left hanging, as `burst` made it, set coming down as its support says
/// ([`falls`]): crumbling where it stood, or tipping and cracking up within its
/// material's angle. `breaks` says how each kind breaks.
pub fn collapse(
    made: &mut [Fragment],
    fell: &[(IVec3, Cube)],
    destroyed: &dyn Fn(IVec3) -> bool,
    breaks: &dyn Fn(Kind) -> Breaks,
) {
    let hanging: Vec<usize> = (0..made.len())
        .filter(|&i| made[i].shape == Shape::Whole && made[i].hinge.is_none())
        .collect();
    if hanging.len() < 2 {
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
    let tall = (top - bottom).max(CELL_M);
    match falls(fell, destroyed) {
        Falls::InPlace => {
            // How far out its shell reaches from its middle, on the level.
            let reach = hanging
                .iter()
                .map(|&i| (made[i].at - middle).with_y(0.0).length())
                .fold(CELL_M, f32::max);
            for &i in &hanging {
                let piece = &mut made[i];
                let out = (piece.at - middle).with_y(0.0);
                let shell = (out.length() / reach).clamp(0.0, 1.0);
                let height = ((piece.at.y - bottom) / tall).clamp(0.0, 1.0);
                let jitter = 0.6 + 0.8 * unit(&[piece.seed, 31]);
                let peel = PEELS_OUT * shell * (0.3 + 0.7 * height) * jitter;
                piece.velocity = out.normalize_or_zero() * peel;
                piece.spin *= PEEL_SPIN;
            }
        }
        Falls::Tips { way } => {
            // The edge of its foot on that side: as far that way as any of it reaches.
            let reach = hanging
                .iter()
                .map(|&i| (made[i].at - middle).dot(way) + made[i].size.max_element() * 0.5)
                .fold(0.0f32, f32::max);
            let pivot = Vec3::new(middle.x, bottom, middle.z) + way * reach;
            let axis = way.cross(Vec3::NEG_Y).normalize();
            let seed = hash::of(&[hanging.len() as u32, bottom.to_bits()]);
            let start = TIP_START * (0.7 + 0.6 * unit(&[seed, 2]));
            for &i in &hanging {
                let piece = &mut made[i];
                let height = ((piece.at.y - bottom) / tall).clamp(0.0, 1.0);
                // It bends hardest a third of the way up: what is over that cracks off
                // first, the stub under it a little later.
                let cracks = breaks(piece.kind).look().cracks;
                let stub = if height < 1.0 / 3.0 { 1.6 } else { 1.0 };
                let vary = 1.0 + CRACKS_VARY * (2.0 * unit(&[piece.seed, 77]) - 1.0);
                piece.hinge = Some(Hinge {
                    pivot,
                    axis,
                    angle: start,
                    rate: TIP_KICK,
                    from: piece.at - pivot,
                    stood: piece.turn,
                    tall,
                    lets_go: start + cracks * stub * vary,
                });
                // All of it goes together, at once.
                piece.wait = 0.0;
                piece.velocity = Vec3::ZERO;
                piece.spin = Vec3::ZERO;
            }
        }
    }
}

fn bounds(cubes: &[IVec3]) -> (IVec3, IVec3) {
    cubes.iter().fold(
        (IVec3::splat(i32::MAX), IVec3::splat(i32::MIN)),
        |(low, high), at| (low.min(*at), high.max(*at)),
    )
}

#[cfg(test)]
#[path = "collapse_tests.rs"]
mod tests;
