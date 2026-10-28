//! A kit's dress drawn (D215, research/role-pieces.md): the pieces a layout lays by each
//! exposed edge's role (`layout::roles`) made into surfaces, and the masks that ground and
//! weather what a kit draws.
//!
//! **Profiles** (`Piece::Trim`): a section drawn in Blender (`tools/kits`) swept along one
//! cube's edge, a stone a cube long: mitred where its run turns round a corner, out or
//! back, with a chamfered joint where it runs on into the next cube's, capped where it
//! stops, run on past a stopped end as far as the kit says (a lintel bearing on the wall
//! either side), raised and run down into the ground where a foot steps with it.
//!
//! **Corner stones** (`Piece::Corner`): a dressed block for each course up an upright edge,
//! made from the material's own wall (D215 as amended): a bevelled block of the size its
//! pattern's numbers give, standing a little proud of both faces, cut from its own texture
//! and height map, the inside of one of its largest stones (`Relief::stones`) wrapped round
//! the block's faces; quoins long and short by turns, jamb stones long and short along the
//! wall. Nothing modelled is shared, so every block material's corners match its wall.
//!
//! Profiles are textured from the world's planes, as the cubes' surface is, each stone a
//! part of the texture of its own and a little lighter or darker, in the kit's `Dressed`
//! material.
//!
//! Profiles are [`profiles`], corner stones [`corners`], and the masks that ground and
//! weather what a kit draws [`masks`].

mod corners;
mod masks;
mod profiles;

use crate::mesh::{Indexed, Material, Surface, Surfaces, planar};
use glam::{IVec3, Mat3, Vec3};
use profiles::stone_look;
use std::collections::BTreeMap;
use terra_build::kit::{Dress, Kit, Masks};
use terra_build::{Base, CELL_M, Fill};
use terra_build_layout::layout::{self, Dir, End, Piece, TICKS};
use terra_gen::hash;

pub(crate) use masks::{faceted, lift_at, smooth};
pub use masks::{ground_at, mask, roof_mask, sheltered};
pub use profiles::{Bed, bed, trim_triangles};

pub use terra_build::pieces::{Module, Pieces, Stone};

/// How tall a course of dressed stone is: two to a cube.
pub const COURSE_M: f32 = CELL_M * 0.5;

/// How far along a band a point one metre over its line moves where it is mitred to a
/// ramp's raking band: the tangent of half the 45 degrees between them.
const BEND_MITRE: f32 = 0.414_213_56;

/// How wide and deep the chamfered joint between two stones of a band is.
const JOINT_M: f32 = 0.012;
const JOINT_DEPTH_M: f32 = 0.009;
/// How far a soffit hangs under the underside it runs along.
const SOFFIT_DROP_M: f32 = 0.004;
/// How far back a square end's part set into the wall is cut, for each metre it is in.
const SQUARE_RECESS: f32 = 0.2;
/// And a piece's whose part set in is all out of sight: a little past the diagonal.
const BURIED_RECESS: f32 = 1.05;
/// How far short of the cube's depth an opening's head or sill is sunk behind its piece.
const REVEAL_SHORT_M: f32 = 0.002;
/// How far past its half-cube step into what rises behind it a cut shape's band reaches.
const BAND_PAST_M: f32 = 0.004;
/// How far short of its square end a border stops.
const BORDER_SHORT_M: f32 = 0.003;
/// How far a corner stone may run along a face it shares with the cube's other edge's
/// stones: to half the cube, short of a joint, so the two meet and never pass.
const SHARED_REACH_M: f32 = CELL_M * 0.5 - JOINT_M * 0.5;

/// Faces meeting at more than about 50 degrees are shaded apart (docs/art-style.md).
const CREASE: f32 = 0.64;

/// Draws a dressed piece into the kit's `Dressed` material, a corner stone into its wall's
/// own (`Masonry`), cut from `wall`, its height map, where it is given; nothing if the kit
/// has no pieces for its role.
pub fn draw(
    piece: &Piece,
    kit: &Kit,
    wall: Option<&crate::relief::Relief>,
    surfaces: &mut Surfaces,
) {
    if let Piece::Corner {
        cube,
        role,
        faces,
        courses,
        shared,
        seed,
    } = *piece
    {
        corners::draw(
            cube,
            (role, faces, courses, shared, seed),
            kit,
            wall,
            surfaces,
        );
        return;
    }
    let Some(pieces) = kit.dress.made.as_deref() else {
        return;
    };
    // A profile in a material of its own, as an earth wall's timber lintel; else dressed stone.
    let material = match *piece {
        Piece::Trim { role, .. } | Piece::Band { role, .. } => match kit.dress.roles.get(&role) {
            Some(Dress::Profile {
                material: Some(material),
                ..
            }) => *material,
            _ => Material::Dressed,
        },
        _ => Material::Dressed,
    };
    let grain = matches!(material, Material::Hewn | Material::Log);
    let metres = kit
        .textures
        .get(&material)
        .map_or([1.2, 1.2], |texture| texture.metres);
    let mut made = Indexed::default();
    let drawn = match piece {
        Piece::Trim { .. } => profiles::trim(piece, kit, pieces, (grain, metres), &mut made),
        Piece::Band { .. } => profiles::band(piece, kit, pieces, (grain, metres), &mut made),
        _ => false,
    };
    if !drawn {
        return;
    }
    surfaces.of(material).add(made);
}

/// A way's unit vector.
fn unit(dir: Dir) -> Vec3 {
    dir.step().as_vec3()
}

#[cfg(test)]
mod tests;
