//! How a fragment is drawn (D305): the material a kind's fragments are made of and the
//! bevelled box each shape is, its faces in its material, timber's grain along it.

use super::Shape;
use crate::{Material, Surfaces};
use glam::Vec3;
use terra_build::kit::{Kit, Pattern};

/// The material a kind's fragments are made of: its walls' own; for timber, the wood its
/// members split to, squared, rather than the bark that wraps them.
pub fn material(kit: &Kit) -> Material {
    match kit.pattern {
        Pattern::Logs if kit.textures.contains_key(&Material::Hewn) => Material::Hewn,
        Pattern::Logs => Material::Log,
        Pattern::Masonry => Material::Masonry,
        Pattern::Roof if kit.textures.contains_key(&Material::Covering) => Material::Covering,
        Pattern::Roof => Material::Board,
    }
}

/// How big a piece usually is, in metres: the size its mesh wears its texture at.
pub(super) const PIECE_M: f32 = 0.25;
/// How much darker a fragment is drawn than its material's face: broken stone in the dirt
/// and shadow of its fall, not the dressed face of a wall.
/// Whole blocks are the wall's own face moving, and keep its shade: darkened, a wall read as
/// another stone the moment it broke (the castle film's review, 2026-10-21).
pub(super) const FRAGMENT_SHADE: f32 = 0.72;
/// A whole block's beds, top and bottom, and its broken arrises are where it split from the
/// courses round it: broken stone, as a standing wall's break is drawn
/// (`sections::laying::worn_within`), its sides the wall's face. A tower's blocks lit from
/// above in their face's colour read as a grid of flat peach slabs (castle film round 4).
fn split_shade(shape: Shape, normal: Vec3, colour: [f32; 3]) -> [f32; 3] {
    let split =
        shape == Shape::Whole && (normal.y.abs() > 0.7 || normal.abs().max_element() < 0.95);
    if split {
        colour.map(|c| c * BED_SHADE)
    } else {
        colour
    }
}

/// How dark a whole block's beds and broken edges are drawn beside its face: as a standing
/// cube beside a break, worn two quarters (`mesh::wear`'s darkening at half lost).
const BED_SHADE: f32 = 0.72;
/// How much smaller than its cubes a whole block is drawn, so it never fights the cubes drawn
/// where they were for the frames before their sections are drawn again; no more, as gaps of
/// 0.97 read as a lattice of dark seams while the blocks fell together.
pub(super) const SHRINK: f32 = 0.99;

/// A shape of fragment, a metre across, in a kind's material: a box with its twelve edges
/// bevelled and its eight corners cut, which the game scales to each piece; its texture as it
/// would be on one `drawn_m` across ([`Fragment::mesh`]).
///
/// Timber's is a piece of the log it came from, its grain along its own first axis (the
/// game turns it along the wall it fell from, `Fragment::turn`): its ends across the grain
/// the log's sawn end, rings and all (`LogEnd`); its two sides the face of the log, bark
/// and weathered wood; its top and bottom, and its bevels, where it was split from the rest,
/// fresh pale wood (the face's worn set, `Texture::worn`), its grain running along it.
pub fn surfaces(kit: &Kit, shape: Shape, drawn_m: f32, sawn: [bool; 2]) -> Surfaces {
    let material = material(kit);
    let metres = kit
        .textures
        .get(&material)
        .map_or([1.0, 1.0], |texture| texture.metres)
        .map(|m| m / drawn_m.max(0.05));
    let timber = kit.pattern == Pattern::Logs && kit.textures.contains_key(&Material::LogEnd);
    let colour = if material == Material::Masonry {
        kit.tint
    } else {
        [1.0; 3]
    }
    .map(|channel| {
        channel
            * if timber {
                TIMBER_SHADE
            } else if shape == Shape::Whole {
                1.0
            } else {
                FRAGMENT_SHADE
            }
    });
    let end = crate::timber::end_cell(kit, 0x51ce);
    let mut surfaces = Surfaces::sharing();
    let mut wood = Surfaces::sharing();
    let half = 0.5;
    let b = half * shape.bevel();
    let inner = half - b;
    // Every corner of a bevelled box is a point of the cube's corner pulled in along two
    // axes; each face, edge and corner a flat piece through such points.
    let point = |sign: Vec3, out: [bool; 3]| -> Vec3 {
        Vec3::new(
            sign.x * if out[0] { half } else { inner },
            sign.y * if out[1] { half } else { inner },
            sign.z * if out[2] { half } else { inner },
        )
    };
    let mut add = |corners: &[Vec3]| {
        let middle = corners.iter().copied().sum::<Vec3>() / corners.len() as f32;
        for i in 1..corners.len() - 1 {
            let (mut a, mut c) = (corners[0], corners[i + 1]);
            let bb = corners[i];
            let mut normal = (bb - a)
                .cross(c - a)
                .normalize_or(middle.normalize_or(Vec3::Y));
            if normal.dot(middle) < 0.0 {
                std::mem::swap(&mut a, &mut c);
                normal = -normal;
            }
            let triangle = [a, bb, c];
            if !timber {
                let uvs = triangle.map(|p| crate::mesh::planar(p, normal, metres));
                let colour = split_shade(shape, normal, colour);
                surfaces
                    .of(material)
                    .textured(triangle, [normal; 3], uvs, colour);
                continue;
            }
            if normal.x.abs() > 0.9 && sawn[usize::from(normal.x > 0.0)] {
                // Its sawn end: the end's picture over it, one of the kit's.
                let (corner, size) = end;
                let uvs = triangle.map(|p| {
                    [
                        corner[0] + size * (0.5 + p.z * 0.94),
                        corner[1] + size * (0.5 - p.y * 0.94),
                    ]
                });
                surfaces
                    .of(Material::LogEnd)
                    .textured(triangle, [normal; 3], uvs, colour);
                continue;
            }
            let uvs = triangle.map(|p| wood_uv(p, normal, metres));
            let side = normal.z.abs() > 0.9;
            let (wear, shade) = if side { (0.25, 1.0) } else { (1.0, FRESH_WOOD) };
            let surface = wood.of(material);
            surface.textured(
                triangle,
                [normal; 3],
                uvs,
                colour.map(|c| (c * shade).min(1.0)),
            );
            crate::mesh::worn(&mut surface.wear, surface.positions.len() - 3);
            surface.wear.extend([wear; 3]);
        }
    };
    let signs = [-1.0f32, 1.0];
    // Faces.
    for axis in 0..3 {
        for s in signs {
            let mut quad = Vec::new();
            for (a, b2) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let mut sign = Vec3::ZERO;
                sign[axis] = s;
                sign[(axis + 1) % 3] = a;
                sign[(axis + 2) % 3] = b2;
                let mut out = [false; 3];
                out[axis] = true;
                quad.push(point(sign, out));
            }
            add(&quad);
        }
    }
    // Edges: along each axis, at each of its four corners round it.
    for axis in 0..3 {
        let (p, q) = ((axis + 1) % 3, (axis + 2) % 3);
        for sp in signs {
            for sq in signs {
                let mut quad = Vec::new();
                for (along, outer) in [(-1.0, p), (1.0, p), (1.0, q), (-1.0, q)] {
                    let mut sign = Vec3::ZERO;
                    sign[axis] = along;
                    sign[p] = sp;
                    sign[q] = sq;
                    let mut out = [false; 3];
                    out[outer] = true;
                    quad.push(point(sign, out));
                }
                add(&quad);
            }
        }
    }
    // Corners.
    for sx in signs {
        for sy in signs {
            for sz in signs {
                let sign = Vec3::new(sx, sy, sz);
                add(&[0, 1, 2].map(|axis| {
                    let mut out = [false; 3];
                    out[axis] = true;
                    point(sign, out)
                }));
            }
        }
    }
    for (material, surface) in wood.0 {
        surfaces.of(material).append(surface);
    }
    surfaces
}

/// How much darker timber's fragments are drawn than its face, less than stone's: split wood
/// is pale, and its fresh faces paler again than its weathered ones.
pub(super) const TIMBER_SHADE: f32 = 0.9;
const FRESH_WOOD: f32 = 1.2;

/// Where on its wood's texture a point of a timber fragment is: along the grain, across it
/// by whichever way the face looks least and along it by its first axis; on a snapped end,
/// across the grain where the length broke from the rest of its member, split wood, its
/// fibres running up it.
fn wood_uv(p: Vec3, normal: Vec3, metres: [f32; 2]) -> [f32; 2] {
    if normal.x.abs() > 0.9 {
        return [p.z / metres[0], p.y / metres[0]];
    }
    let across = if normal.y.abs() > normal.z.abs() {
        2
    } else {
        1
    };
    [p[across] / metres[0], p.x / metres[1]]
}
