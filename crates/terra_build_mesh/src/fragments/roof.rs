//! A roof left hanging as it stood (D305): each of its cubes is a slab along its slope, drawn
//! where the roof was until its covering is shed, so a burning tower's roof does not vanish
//! the frame what held it gave way, its pitch, flames and shadow gone at once (castle film
//! r29, 14.23 s and the keep's 23.03 s). The slab then falls with the bits shed off it, as
//! a roof's boarding comes down in panels with its shingles round them.

use super::{Fragment, Laid, SHRINK, Shape};
use glam::{IVec3, Mat3, Quat, Vec3};
use terra_build::CELL_M;
use terra_build::cubes::{Cube, Shape as CubeShape};

/// How thick a slab of roof is, and how far over its cube's surface its top stands: about
/// the covering's thickness, where the drawing lifts it (`roofing`).
const THICK_M: f32 = 0.16;
const LIFT_M: f32 = 0.05;

/// The plane a roof cube's covering lay in: a point on it, its way out (up the roof's
/// normal) and its way up the slope (level for a flat one).
pub(super) struct Pitch {
    pub on: Vec3,
    pub out: Vec3,
    pub up_slope: Vec3,
}

/// Where a roof cube at `at` was covered, from its shape: a ramp's plane through its middle,
/// rising toward its side; a corner's toward the corner between its two sides; a whole or
/// half cube's top.
pub(super) fn pitch(at: IVec3, cube: &Cube) -> Pitch {
    let middle = (at.as_vec3() + 0.5) * CELL_M;
    let rising = |toward: Vec3| {
        let toward = toward.normalize_or(Vec3::X);
        Pitch {
            on: middle,
            out: (Vec3::Y - toward).normalize(),
            up_slope: (toward + Vec3::Y).normalize(),
        }
    };
    let level = |top: f32| Pitch {
        on: middle.with_y(at.y as f32 * CELL_M + top),
        out: Vec3::Y,
        up_slope: Vec3::X,
    };
    match cube.shape {
        CubeShape::Slope(side)
        | CubeShape::Stair(side)
        | CubeShape::InvertedSlope(side)
        | CubeShape::InvertedStair(side) => rising(side.step().as_vec3()),
        CubeShape::OuterCorner(side) | CubeShape::InnerCorner(side) => {
            rising((side.step() + side.turned(1).step()).as_vec3())
        }
        CubeShape::Half => level(CELL_M * 0.5),
        _ => level(CELL_M),
    }
}

/// The slab standing in for a roof cube's covering until `wait` has passed: as long as its
/// slope runs across the cube, as wide as the cube, its top on the covering; laid where it
/// is so it is drawn plain until it moves ([`Fragment::stands`]).
pub(super) fn slab(at: IVec3, cube: &Cube, seed: u32, wait: f32) -> Fragment {
    let pitch = pitch(at, cube);
    let along = pitch.up_slope;
    let across = along.cross(pitch.out).normalize();
    let turn = Quat::from_mat3(&Mat3::from_cols(along, pitch.out, across)).normalize();
    let long = CELL_M / along.with_y(0.0).length().max(0.5);
    let size = Vec3::new(long, THICK_M, CELL_M) * SHRINK;
    let middle = pitch.on + pitch.out * (LIFT_M - THICK_M * 0.5);
    Fragment {
        kind: cube.kind,
        shape: Shape::Whole,
        at: middle,
        velocity: Vec3::ZERO,
        turn,
        spin: across * 0.8,
        size,
        wait,
        still: false,
        fill: 1.0,
        seed,
        hinge: None,
        body: None,
        sawn: [true; 2],
        laid: Some(Laid { at: middle, turn }),
    }
}

/// A bit shed off a roof cube, put in its slab, so what is shed is not seen through the
/// roof standing in for it, or hanging in the air above it, before it goes.
pub(super) fn onto(piece: &mut Fragment, pitch: &Pitch) {
    let above = (piece.at - pitch.on).dot(pitch.out);
    piece.at += pitch.out * (LIFT_M - THICK_M * 0.5 - above);
}
