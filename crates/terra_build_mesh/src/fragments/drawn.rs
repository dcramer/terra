//! How a fragment is drawn (D305): the material a kind's fragments are made of and the
//! bevelled box each shape is, its faces in its material, timber's grain along it.

use super::Shape;
use crate::{Material, Surfaces};
use glam::{Quat, Vec3};
use terra_build::CELL_M;
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
/// Whether a face of a whole block, by its normal along the block's own axes, is where it
/// split from the cubes round it rather than the wall's face: its beds, top and bottom, its
/// ends along the wall (its first axis, which `grain` turns along the wall it fell from) and
/// its broken arrises; its two sides across the wall are the wall's own faces. A tower's
/// blocks lit from above in their face's colour read as a grid of flat peach slabs (castle
/// film round 4); blocks whose every side was the wall's face, as blank boxes (rounds 8-11).
pub(super) fn split(shape: Shape, normal: Vec3) -> bool {
    shape == Shape::Whole
        && (normal.y.abs() > 0.7 || normal.x.abs() > 0.7 || normal.abs().max_element() < 0.95)
}

/// Where a split face's texture is taken from, shifted off the wall's face: the broken
/// stone behind it, its core's stones, not the face it split from seen again.
const SPLIT_SHIFT: [f32; 2] = [0.37, 0.53];

/// Where a whole block stood in what it fell from (D305): its middle in the base's space and
/// how it was turned then ([`turned`]), so its faces wear the texture they wore in the wall,
/// courses and stones where they were, and a block's pieces go on wearing it. None for a
/// broken piece, all of it broken.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Laid {
    pub at: Vec3,
    pub turn: Quat,
}

impl Laid {
    /// A block of `size` along its own axes, turned `turn`, laid from `foot`, its low corner:
    /// its middle there from its size along the base's axes, as its `at` is.
    pub fn over(foot: Vec3, turn: Quat, size: Vec3) -> Laid {
        Laid {
            at: foot + (turn * size).abs() * 0.5,
            turn,
        }
    }

    /// Where a part of it, `offset` from its middle along its own axes, stood.
    pub fn part(self, offset: Vec3) -> Laid {
        Laid {
            at: self.at + self.turn * offset,
            ..self
        }
    }
}

/// The three ways a whole block is turned as it is laid (`grain`): as built, its first axis
/// up, its first axis along z.
pub(super) fn turned(index: u8) -> Quat {
    match index {
        1 => Quat::from_rotation_z(core::f32::consts::FRAC_PI_2),
        2 => Quat::from_rotation_y(core::f32::consts::FRAC_PI_2),
        _ => Quat::IDENTITY,
    }
}

/// Which of the three ([`turned`]) a turn is.
fn turn_index(turn: Quat) -> u8 {
    let first = turn * Vec3::X;
    if first.y.abs() > 0.5 {
        1
    } else if first.z.abs() > 0.5 {
        2
    } else {
        0
    }
}

/// What a fragment's mesh is made for, which the game keeps one mesh for each of
/// ([`Fragment::mesh`]): its shape; how many cubes it is along its own axes, so its texture
/// is worn at its size along each, never stretched (a slab's thin sides squeezed a texture
/// to a blur, castle film rounds 8-11: "blank cubes"); its sawn ends; and how it was laid and
/// where its low corner stood, in cubes within its texture's repeat ([`repeat`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cut {
    pub shape: Shape,
    /// Zero for a broken piece, drawn as one [`PIECE_M`] across.
    pub cubes: [u8; 3],
    pub sawn: [bool; 2],
    pub laid: Option<(u8, [u8; 3])>,
}

/// The most cubes along an axis a mesh is made for; a bigger section wears it stretched.
const MOST_CUBES: f32 = 16.0;

impl Cut {
    /// A broken piece of a shape.
    pub fn piece(shape: Shape) -> Cut {
        Cut {
            shape,
            cubes: [0; 3],
            sawn: [true; 2],
            laid: None,
        }
    }

    /// A whole block of `size` metres along its own axes, laid as `laid` says, in a texture
    /// repeating every `repeat` cubes along the base's axes.
    pub fn whole(size: Vec3, sawn: [bool; 2], laid: Option<Laid>, repeat: [u8; 3]) -> Cut {
        let cubes = (size / CELL_M)
            .round()
            .clamp(Vec3::ONE, Vec3::splat(MOST_CUBES));
        let laid = laid.map(|laid| {
            let reach = (laid.turn * cubes * CELL_M).abs();
            let low = ((laid.at - reach * 0.5) / CELL_M).round().as_ivec3();
            let at =
                [0, 1, 2].map(|axis| low[axis].rem_euclid(i32::from(repeat[axis].max(1))) as u8);
            (turn_index(laid.turn), at)
        });
        Cut {
            shape: Shape::Whole,
            cubes: cubes.to_array().map(|c| c as u8),
            sawn,
            laid,
        }
    }

    /// How big it is drawn, in metres along its own axes.
    fn metres(&self) -> Vec3 {
        if self.cubes[0] == 0 {
            Vec3::splat(PIECE_M)
        } else {
            Vec3::from_array(self.cubes.map(f32::from)) * CELL_M
        }
    }
}

/// How many cubes a kind's fragment texture repeats over along the base's x, y and z
/// ([`Cut::laid`]): its copy's size in cubes, one for timber, whose lengths wear their grain
/// along themselves wherever they lay.
pub fn repeat(kit: &Kit) -> [u8; 3] {
    let metres = kit
        .textures
        .get(&material(kit))
        .map_or([CELL_M; 2], |texture| texture.metres);
    if kit.pattern == Pattern::Logs {
        return [1; 3];
    }
    let cubes = |m: f32| (m / CELL_M).round().clamp(1.0, 64.0) as u8;
    [cubes(metres[0]), cubes(metres[1]), cubes(metres[0])]
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
/// would be on one the cut's size ([`Cut`]).
///
/// A whole block of stone, plaster or roofing laid in a wall wears on its two faces across
/// the wall the texture where they stood in it, as the standing wall draws it (`planar` in
/// the base's space), and on the faces it split along ([`split`]) the broken stone behind
/// them, darker; a broken piece, its texture all over, darker again.
///
/// Timber's is a piece of the log it came from, its grain along its own first axis (the
/// game turns it along the wall it fell from, `Fragment::turn`): its ends across the grain
/// the log's sawn end, rings and all (`LogEnd`); its two sides the face of the log, bark
/// and weathered wood; its top and bottom, and its bevels, where it was split from the rest,
/// fresh pale wood (the face's worn set, `Texture::worn`), its grain running along it.
pub fn surfaces(kit: &Kit, cut: &Cut) -> Surfaces {
    let (shape, sawn) = (cut.shape, cut.sawn);
    let material = material(kit);
    let metres = kit
        .textures
        .get(&material)
        .map_or([1.0, 1.0], |texture| texture.metres);
    let size = cut.metres();
    // Where a point of the unit box stood, and which way a face of it looked, in the base's
    // space, for a laid block; on its own axes at its size for anything else.
    let (stood, laid_turn) = match cut.laid {
        Some((turn, at)) => {
            let turn = turned(turn);
            let low = Vec3::from_array(at.map(f32::from)) * CELL_M;
            (low + (turn * size).abs() * 0.5, turn)
        }
        None => (Vec3::ZERO, Quat::IDENTITY),
    };
    let stand = |p: Vec3, normal: Vec3| (stood + laid_turn * (p * size), laid_turn * normal);
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
    // Every corner of a bevelled box is a point of the cube's corner pulled in along two
    // axes; each face, edge and corner a flat piece through such points ([`bevelled`]).
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
                let split = split(shape, normal);
                let shift = if split { SPLIT_SHIFT } else { [0.0; 2] };
                let uvs = triangle.map(|p| {
                    let (at, facing) = stand(p, normal);
                    let [u, v] = crate::mesh::planar(at, facing, metres);
                    [u + shift[0], v + shift[1]]
                });
                let colour = if split {
                    colour.map(|c| c * BED_SHADE)
                } else {
                    colour
                };
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
            let uvs = triangle.map(|p| wood_uv(p * size, normal, metres));
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
    for corners in bevelled(shape.bevel()) {
        add(&corners);
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

/// The flat pieces of a box a metre across with its edges bevelled by `bevel` of its half
/// width, each by its corners in order round it: its six faces, its twelve edges and its
/// eight cut corners, every corner a point of the cube's corner pulled in along two axes.
fn bevelled(bevel: f32) -> Vec<Vec<Vec3>> {
    let half = 0.5;
    let inner = half - half * bevel;
    let point = |sign: Vec3, out: [bool; 3]| -> Vec3 {
        Vec3::new(
            sign.x * if out[0] { half } else { inner },
            sign.y * if out[1] { half } else { inner },
            sign.z * if out[2] { half } else { inner },
        )
    };
    let signs = [-1.0f32, 1.0];
    let mut pieces = Vec::new();
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
            pieces.push(quad);
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
                pieces.push(quad);
            }
        }
    }
    // Corners.
    for sx in signs {
        for sy in signs {
            for sz in signs {
                let sign = Vec3::new(sx, sy, sz);
                pieces.push(
                    [0, 1, 2]
                        .map(|axis| {
                            let mut out = [false; 3];
                            out[axis] = true;
                            point(sign, out)
                        })
                        .to_vec(),
                );
            }
        }
    }
    pieces
}
