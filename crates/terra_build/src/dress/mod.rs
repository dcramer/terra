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

use crate::kit::{Dress, Kit, Masks};
use crate::layout::{self, Dir, End, Piece, TICKS, hash};
use crate::mesh::{Indexed, Material, Surface, Surfaces, planar};
use crate::{Base, CELL_M, Fill};
use glam::{IVec3, Mat3, Vec3};
use profiles::stone_look;
use std::collections::BTreeMap;

pub use masks::{ground_at, mask, roof_mask, sheltered};
pub(crate) use masks::{lift_at, smooth};
pub use profiles::{Bed, bed, trim_triangles};

/// A kit's pieces, as `tools/kits/pieces.py` writes them from their Blender source.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Pieces {
    /// Sections by name, as points (out, up) in metres from the edge they are swept along:
    /// out from its face, up from its line.
    pub profiles: BTreeMap<String, Vec<[f32; 2]>>,
    /// Modules by name (D258, `modules`): lengths of mesh repeated along a line.
    pub modules: BTreeMap<String, Module>,
}

/// A module (D258): a length of mesh modelled along +X from 0 to `length`, Y up from what
/// it sits on and Z across, laid along a line in whole copies (`modules::lay`); several
/// variants, picked a copy at a time by its seed, each also at a far budget; and a cap,
/// modelled past +X's end, closing an end where its line stops.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Module {
    pub length: f32,
    /// Its contract (D290, D303): its real size, along (X), up (Y) and across (Z), in
    /// metres, from its origin, its anchor, where the layout seats its line (a roof's deck or
    /// covering, a wall's face); the model it was made from, if modelled, which is this size
    /// wherever it is used; and its named sockets in its own frame (`start` and `end`).
    pub dims: [f32; 3],
    pub model: Option<String>,
    pub sockets: BTreeMap<String, [f32; 3]>,
    /// Whether it is its pattern's, shared by its materials and drawn in the pattern's atlas
    /// (`Material::Trim`), rather than a material's own (`Material::Piece`); set as read.
    #[serde(skip)]
    pub shared: bool,
    /// Over a crease, how far either side its up turns from one plane's to the other's
    /// (`modules::Line::blend`).
    pub blend: f32,
    pub near: Vec<Stone>,
    pub far: Vec<Stone>,
    pub cap: Option<Stone>,
}

impl Module {
    /// How big its section is, up (Y) and across (Z), as modelled: what a member scaled to
    /// its frame's size is scaled from.
    pub fn size(&self) -> [f32; 2] {
        let mut low = [f32::MAX; 2];
        let mut high = [f32::MIN; 2];
        for p in self.near.iter().flat_map(|m| &m.positions) {
            for (k, axis) in [1, 2].into_iter().enumerate() {
                low[k] = low[k].min(p[axis]);
                high[k] = high[k].max(p[axis]);
            }
        }
        [0, 1].map(|k| (high[k] - low[k]).max(0.0))
    }

    /// The most triangles one copy has near, its cap's apart.
    pub fn triangles(&self) -> usize {
        self.near
            .iter()
            .map(|m| m.triangles.len())
            .max()
            .unwrap_or(0)
    }
}

/// A mesh of a module's: triangles over shared corners.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Stone {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    /// A modelled piece's places in its kit's atlas (`Material::Piece`), a corner each, V
    /// down from the top; none for a piece made from numbers, mapped by where it lies.
    pub uvs: Vec<[f32; 2]>,
}

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
