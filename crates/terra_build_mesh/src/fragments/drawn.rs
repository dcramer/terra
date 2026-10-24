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
/// An end with nothing that fell beyond it (`sawn`, its course's end: a tower's corner, a
/// window's jamb) stood open in the wall and is the wall's face there: drawn as broken stone,
/// set back and darker, every corner block of a falling tower read as a hole in its corner
/// the moment it came down (castle film round 28, 14.23 s).
/// An arris is broken where either face it joins is.
pub(super) fn split(shape: Shape, normal: Vec3, sawn: [bool; 2]) -> bool {
    let broken_end = normal.x.abs() > 0.3 && !sawn[usize::from(normal.x > 0.0)];
    shape == Shape::Whole && (normal.y.abs() > 0.3 || broken_end)
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
    /// A whole block still where it stood, not yet moved: drawn as the wall drew it, its
    /// faces flat and whole, no arrises and no broken stone ([`Cut::still_standing`]).
    pub intact: bool,
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
            intact: false,
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
            intact: false,
        }
    }

    /// The same block while it still stands where it stood, the moment its wall came down
    /// and before it moves: drawn as the wall was, so that what fell reads as the wall until
    /// each block breaks away, the break spreading from where it gave way as they start to
    /// move, rather than the whole wall turning into a lattice of bevelled blocks with
    /// broken edges in one frame (castle film round 28, 14.23 s).
    pub fn still_standing(self) -> Cut {
        Cut {
            intact: self.laid.is_some(),
            ..self
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

/// How dark a whole block's broken faces are drawn beside its face, at their rim and at their
/// deepest: below a standing cube beside a break worn two quarters (`mesh::wear`'s 0.72), as
/// beds facing the sky in the face's shade read as the lightest planes in a fall (castle
/// film round 13).
pub(super) const FRACTURE_SHADE: [f32; 2] = [0.55, 0.36];
/// How far in a broken face is set back at most, and how far apart the points of its relief
/// are, in metres; the most points along a side.
pub(super) const FRACTURE_DEPTH_M: f32 = 0.07;
const FRACTURE_STEP_M: f32 = 0.14;
const FRACTURE_POINTS: usize = 12;

/// A face a whole block split along, as broken stone (D305): not the flat plane it was cut
/// along but a rough surface set back into the block, as shipped destruction draws a
/// fracture's inside (Unreal's Chaos fracture: its cut planes broken by noise, an interior
/// material on them; docs only). A grid of points a [`FRACTURE_STEP_M`] apart over it, its
/// rim where it was so it meets the block's edges, each inner point pushed in by a bowl and
/// by noise where it stood, up to [`FRACTURE_DEPTH_M`]; each facet lit its own way, darker
/// the deeper it lies ([`FRACTURE_SHADE`]), its texture the stone behind the face.
/// `corners` are the face's on the unit box, `facing` its normal, `size` the block's in
/// metres, so the relief is in metres however the game stretches the box.
fn fracture(
    surface: &mut crate::Surface,
    corners: [Vec3; 4],
    facing: Vec3,
    size: Vec3,
    colour: [f32; 3],
    stand: &impl Fn(Vec3, Vec3) -> (Vec3, Vec3),
    metres: [f32; 2],
) {
    let axis = facing.abs().max_position();
    let mut inward = Vec3::ZERO;
    inward[axis] = -facing[axis].signum() / size[axis];
    let (along, across) = (corners[1] - corners[0], corners[3] - corners[0]);
    let points = |side: Vec3| {
        ((side * size).length() / FRACTURE_STEP_M)
            .round()
            .clamp(2.0, FRACTURE_POINTS as f32) as usize
    };
    let (ns, nt) = (points(along), points(across));
    let depth = |i: usize, j: usize| -> f32 {
        if i == 0 || j == 0 || i == ns || j == nt {
            return 0.0;
        }
        let (s, t) = (i as f32 / ns as f32, j as f32 / nt as f32);
        let p = corners[0] + along * s + across * t;
        let q = (stand(p, facing).0 * 100.0).round().as_ivec3();
        let rough = terra_gen::hash::unit3(0xf7ac, q.x as u32, q.y as u32, q.z as u32);
        let bowl = libm::sinf(core::f32::consts::PI * s) * libm::sinf(core::f32::consts::PI * t);
        FRACTURE_DEPTH_M * (0.4 * bowl + 0.6 * rough)
    };
    let mut grid = vec![(Vec3::ZERO, 0.0f32); (ns + 1) * (nt + 1)];
    for j in 0..=nt {
        for i in 0..=ns {
            let d = depth(i, j);
            let p = corners[0] + along * (i as f32 / ns as f32) + across * (j as f32 / nt as f32);
            grid[j * (ns + 1) + i] = (p + inward * d, d);
        }
    }
    let shift = SPLIT_SHIFT;
    let mut facet = |mut a: (Vec3, f32), b: (Vec3, f32), mut c: (Vec3, f32)| {
        let mut normal = (b.0 - a.0).cross(c.0 - a.0);
        if normal.dot(facing) < 0.0 {
            std::mem::swap(&mut a, &mut c);
            normal = -normal;
        }
        let normal = normal.normalize_or(facing);
        let deep = (a.1 + b.1 + c.1) / (3.0 * FRACTURE_DEPTH_M);
        let shade = FRACTURE_SHADE[0] + (FRACTURE_SHADE[1] - FRACTURE_SHADE[0]) * deep;
        let triangle = [a.0, b.0, c.0];
        let uvs = triangle.map(|p| {
            let (at, looks) = stand(p, facing);
            let [u, v] = crate::mesh::planar(at, looks, metres);
            [u + shift[0], v + shift[1]]
        });
        surface.textured(triangle, [normal; 3], uvs, colour.map(|c| c * shade));
    };
    for j in 0..nt {
        for i in 0..ns {
            let at = |i: usize, j: usize| grid[j * (ns + 1) + i];
            let (p00, p10, p11, p01) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
            // Each cell cut along either diagonal, so its facets don't line up in rows.
            if (i + j * 3) % 2 == 0 {
                facet(p00, p10, p11);
                facet(p00, p11, p01);
            } else {
                facet(p00, p10, p01);
                facet(p10, p11, p01);
            }
        }
    }
}
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
    let intact = cut.intact && !timber;
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
        // A face it split along, broken stone: set back in a rough relief ([`fracture`]).
        let facing = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .normalize_or(Vec3::Y);
        let facing = if facing.dot(middle) < 0.0 {
            -facing
        } else {
            facing
        };
        if !timber
            && corners.len() == 4
            && facing.abs().max_element() > 0.95
            && !intact
            && split(shape, facing, sawn)
        {
            let corners = [corners[0], corners[1], corners[2], corners[3]];
            fracture(
                surfaces.of(material),
                corners,
                facing,
                size,
                colour,
                &stand,
                metres,
            );
            return;
        }
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
                let split = !intact && split(shape, normal, sawn);
                let shift = if split { SPLIT_SHIFT } else { [0.0; 2] };
                let uvs = triangle.map(|p| {
                    let (at, facing) = stand(p, normal);
                    let [u, v] = crate::mesh::planar(at, facing, metres);
                    [u + shift[0], v + shift[1]]
                });
                let colour = if split {
                    colour.map(|c| c * FRACTURE_SHADE[0])
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
    // As it stood, a plain box: its arrises and corners pieces of no size, left out.
    let bevel = if intact { 0.0 } else { shape.bevel() };
    for corners in bevelled(bevel)
        .iter()
        .filter(|corners| !intact || corners.len() == 4)
    {
        if intact
            && (corners[1] - corners[0])
                .cross(corners[3] - corners[0])
                .length()
                < 1e-6
        {
            continue;
        }
        add(corners);
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
