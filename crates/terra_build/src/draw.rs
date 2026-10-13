//! Pieces made into surfaces by material, ready to draw: long members by [`timber`], boards,
//! footings and floors as boxes their texture is mapped onto. Doors and windows are props,
//! and block kinds are drawn as their cubes' surface (`surface.rs`), so neither is here.
//!
//! [`timber`]: crate::timber

use crate::kit::Kit;
use crate::layout::Piece;
use crate::mesh::{Material, Surface, Surfaces};
use crate::relief::Relief;
use glam::Vec3;

/// How tall and deep a footing of stone is, its top at the wall's foot.
const FOOTING_HIGH_M: f32 = 0.5;
const FOOTING_DEEP_M: f32 = 0.6;

/// The size in metres of a kit's texture for a material, or a metre square if it has none.
fn metres(kit: &Kit, material: Material) -> [f32; 2] {
    kit.textures
        .get(&material)
        .map_or([1.0, 1.0], |texture| texture.metres)
}

/// Draws a piece, each part in its material. `face` is the height map of the kit's `Log`
/// material, which its logs' bark stands out by, once it has been read.
pub fn draw(piece: &Piece, kit: &Kit, face: Option<&Relief>, surfaces: &mut Surfaces) {
    match *piece {
        Piece::Log { .. } => crate::timber::draw(piece, kit, face, surfaces),
        Piece::Board { min, max } => cuboid(
            surfaces.of(Material::Board),
            min,
            max,
            metres(kit, Material::Board),
            [1.0; 3],
        ),
        Piece::Footing { from, to, .. } => {
            // A wall of stone under the wall's line, its top a little over the foot.
            let along = (to - from).normalize_or_zero();
            let across = Vec3::Y.cross(along) * FOOTING_DEEP_M * 0.5;
            let low = from - across - Vec3::Y * (FOOTING_HIGH_M - 0.02);
            let high = to + across + Vec3::Y * 0.02;
            cuboid(
                surfaces.of(Material::Stone),
                low.min(high),
                low.max(high),
                metres(kit, Material::Stone),
                [1.0; 3],
            );
        }
        Piece::Floor { min, max } => cuboid(
            surfaces.of(Material::Floor),
            min - Vec3::Y * 0.03,
            max + Vec3::Y * 0.005,
            metres(kit, Material::Floor),
            [1.0; 3],
        ),
        // A prop, or part of its kind's surface.
        Piece::Opening { .. } | Piece::Block { .. } => {}
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
            "Footing Stone",
            "Floor Floor",
        ] {
            assert!(
                drawn.get(each).is_some_and(|&n| n > 0),
                "no {each}: {drawn:?}"
            );
        }
    }
}
