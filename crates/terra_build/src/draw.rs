//! Pieces made into surfaces by material, ready to draw: long members and a deck's planks by
//! [`timber`], the boards round openings as boxes their texture is mapped onto, a kit's
//! dress by [`dress`](crate::dress). Doors and windows are props,
//! and block kinds are drawn as their cubes' surface (`surface.rs`), so neither is here.
//!
//! [`timber`]: crate::timber

use crate::kit::Kit;
use crate::layout::Piece;
use crate::mesh::{Material, Surface, Surfaces};
use crate::relief::Relief;
use glam::Vec3;

/// The size in metres of a kit's texture for a material, or a metre square if it has none.
fn metres(kit: &Kit, material: Material) -> [f32; 2] {
    kit.textures
        .get(&material)
        .map_or([1.0, 1.0], |texture| texture.metres)
}

/// Draws a piece, each part in its material. `face` is the height map of the kit's `Log`
/// material, which its logs' bark stands out by, once it has been read.
pub fn draw(piece: &Piece, kit: &Kit, face: Option<&Relief>, surfaces: &mut Surfaces) {
    draw_as(piece, kit, face, false, surfaces);
}

/// Draws a piece as [`draw`] does, or, `plain`, with less of its detail, as a ghost is drawn:
/// a roof's covering as a sheet without its courses.
pub fn draw_as(
    piece: &Piece,
    kit: &Kit,
    face: Option<&Relief>,
    plain: bool,
    surfaces: &mut Surfaces,
) {
    match *piece {
        Piece::Log { .. } => crate::timber::draw(piece, kit, face, surfaces),
        Piece::Board { min, max } => cuboid(
            surfaces.of(Material::Board),
            min.metres(),
            max.metres(),
            metres(kit, Material::Board),
            [1.0; 3],
        ),
        Piece::Plank { .. } => crate::timber::plank(piece, kit, surfaces),
        Piece::Beam { .. } => crate::timber::beam(piece, kit, surfaces),
        Piece::Trim { .. } | Piece::Band { .. } | Piece::Corner { .. } => {
            crate::dress::draw(piece, kit, surfaces)
        }
        Piece::Covering { .. } | Piece::Roof { .. } => {
            crate::roofing::draw_as(piece, kit, plain, surfaces)
        }
        Piece::Block { .. } | Piece::Gable { .. } | Piece::Wattle { .. } => {}
    }
}

/// A box from one corner to the other, each face's texture mapped from its plane.
pub(crate) fn cuboid(
    surface: &mut Surface,
    min: Vec3,
    max: Vec3,
    metres: [f32; 2],
    colour: [f32; 3],
) {
    let corner = |x: bool, y: bool, z: bool| {
        Vec3::new(
            if x { max.x } else { min.x },
            if y { max.y } else { min.y },
            if z { max.z } else { min.z },
        )
    };
    let faces = [
        (
            Vec3::X,
            [
                (true, false, true),
                (true, false, false),
                (true, true, false),
                (true, true, true),
            ],
        ),
        (
            Vec3::NEG_X,
            [
                (false, false, false),
                (false, false, true),
                (false, true, true),
                (false, true, false),
            ],
        ),
        (
            Vec3::Y,
            [
                (false, true, true),
                (true, true, true),
                (true, true, false),
                (false, true, false),
            ],
        ),
        (
            Vec3::NEG_Y,
            [
                (false, false, false),
                (true, false, false),
                (true, false, true),
                (false, false, true),
            ],
        ),
        (
            Vec3::Z,
            [
                (false, false, true),
                (true, false, true),
                (true, true, true),
                (false, true, true),
            ],
        ),
        (
            Vec3::NEG_Z,
            [
                (true, false, false),
                (false, false, false),
                (false, true, false),
                (true, true, false),
            ],
        ),
    ];
    for (normal, corners) in faces {
        let corners = corners.map(|(x, y, z)| corner(x, y, z));
        surface.planar_quad(corners, normal, metres, colour);
    }
}

/// A long member far off, as a plain box from one end to the other along its middle, so
/// wide across and so high, its texture mapped from each face's plane: what a log or a
/// deck's plank comes to a few pixels across, where its round or chamfered section and its
/// rings cannot be seen.
pub(crate) fn member_box(
    surface: &mut Surface,
    from: Vec3,
    to: Vec3,
    half: [f32; 2],
    metres: [f32; 2],
    colour: [f32; 3],
) {
    let along = (to - from).normalize_or(Vec3::X);
    let across = along.cross(Vec3::Y).normalize_or(Vec3::Z) * half[0];
    let up = across
        .normalize_or(Vec3::Z)
        .cross(along)
        .normalize_or(Vec3::Y)
        * half[1];
    let at = |end: Vec3, a: f32, b: f32| end + across * a + up * b;
    let side = |a: f32, b: f32, c: f32, d: f32| {
        [at(from, a, b), at(to, a, b), at(to, c, d), at(from, c, d)]
    };
    let faces = [
        (up, side(1.0, 1.0, -1.0, 1.0)),
        (-up, side(-1.0, -1.0, 1.0, -1.0)),
        (across, side(1.0, -1.0, 1.0, 1.0)),
        (-across, side(-1.0, 1.0, -1.0, -1.0)),
        (
            -along,
            [
                at(from, 1.0, -1.0),
                at(from, 1.0, 1.0),
                at(from, -1.0, 1.0),
                at(from, -1.0, -1.0),
            ],
        ),
        (
            along,
            [
                at(to, -1.0, -1.0),
                at(to, -1.0, 1.0),
                at(to, 1.0, 1.0),
                at(to, 1.0, -1.0),
            ],
        ),
    ];
    for (normal, corners) in faces {
        let normal = normal.normalize_or(Vec3::Y);
        // Wound to face out, whichever way the frame turned.
        let wound = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .dot(normal)
            > 0.0;
        let corners = if wound {
            corners
        } else {
            [corners[0], corners[3], corners[2], corners[1]]
        };
        surface.planar_quad(corners, normal, metres, colour);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::Panel;
    use crate::layout::lay;
    use crate::layout::tests::{cabin, textured, wall};
    use std::collections::BTreeMap;

    /// Each triangle of a shape with no hollows in it faces away from its middle, and its
    /// corners wind the way its normals point; how many there were.
    pub(crate) fn facing_out(surface: &Surface, what: &str) -> usize {
        let middle = surface.positions.iter().sum::<Vec3>() / surface.positions.len() as f32;
        for (corners, normals) in surface.positions.chunks(3).zip(surface.normals.chunks(3)) {
            let own = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
            let centre = (corners[0] + corners[1] + corners[2]) / 3.0;
            assert!(
                own.dot(centre - middle) > 0.0,
                "{what}: {corners:?} faces in"
            );
            assert!(
                normals.iter().all(|n| own.dot(*n) > 0.0),
                "{what}: {corners:?} is wound against its normals {normals:?}"
            );
        }
        surface.triangle_count()
    }

    #[test]
    fn every_piece_is_drawn_facing_out() {
        let kit = textured();
        let mut base = cabin();
        wall(&mut base, [[0, 0, 2]], Panel::WINDOW);
        let layout = lay(&base, &kit);
        let mut drawn: BTreeMap<String, usize> = BTreeMap::new();
        for piece in &layout.pieces {
            let mut surfaces = Surfaces::default();
            draw(piece, &kit, None, &mut surfaces);
            let what = format!("{piece:?}");
            let kind = what.split([' ', '{']).next().unwrap_or("").to_string();
            for (material, surface) in &surfaces.0 {
                let count = facing_out(surface, &what);
                *drawn.entry(format!("{kind} {material:?}")).or_default() += count;
            }
        }
        for each in [
            "Log Log",
            "Log LogEnd",
            "Log Daub",
            "Board Board",
            "Plank Floor",
            "Plank LogEnd",
        ] {
            assert!(
                drawn.get(each).is_some_and(|&n| n > 0),
                "no {each}: {drawn:?}"
            );
        }
    }
}
