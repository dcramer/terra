//! A base's cubes as a hull, for far off (docs/building-plan.md, drawing): what Unreal's
//! hierarchical levels of detail call an approximated mesh, a building's outer shape at a
//! few hundred triangles with its materials' colours baked in, in place of its pieces past
//! where they can be told apart. Every cube in that is not an opening shows each face open
//! to the air or to a cube that does not fill its side; the faces of whole cubes lying in
//! one plane in one colour are merged into rectangles (greedy meshing, as voxel games mesh
//! their chunks); a cut cube shows its top at its shape's height over each corner
//! (`Shape::top`), its sides cut to it, so roofs keep their pitch. Its colours are each
//! kit's materials' mean, darkened a little for the shade a surface's relief casts.
//!
//! Pure data, so it runs on any thread: the game makes it on its workers.

use crate::kit::{Kits, Pattern};
use crate::mesh::{Material, Surface};
use crate::relief::Maps;
use crate::{Base, CELL_M, Cube, Fill, Kind, Shape};
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, HashSet};

/// How much darker than its material's mean colour a face is drawn: the shade its relief
/// and its seams cast, which the hull has none of.
const SHADE: f32 = 0.85;

/// The hull of a base's cubes from `low` to `high` inclusive, in the base's own space, its
/// colours sRGB as a surface's are.
pub fn hull(base: &Base, kits: &Kits, maps: &Maps, low: IVec3, high: IVec3) -> Surface {
    let store = base.store();
    let solid = |at: IVec3| store.get(at).is_some_and(|cube| fills(&cube));
    let mut colours = Colours::new(kits, maps);
    // Whole faces by their direction, the plane they lie in and their colour: the cells
    // of each plane, merged into rectangles once all are known.
    let mut planes: BTreeMap<(usize, i32, usize), HashSet<[i32; 2]>> = BTreeMap::new();
    let mut out = Surface::default();
    for (at, cube) in store.within(low, high) {
        if !cube.is_in() {
            continue;
        }
        let tops = corners(cube.shape);
        if tops == [1.0; 4] {
            for (dir, normal) in DIRECTIONS.iter().enumerate() {
                if solid(at + *normal) {
                    continue;
                }
                let colour = colours.of(cube.kind, dir == UP);
                let (plane, cell) = plane_of(dir, at);
                planes.entry((dir, plane, colour)).or_default().insert(cell);
            }
        } else {
            cut(&mut out, at, &cube, tops, &solid, &mut colours);
        }
    }
    for ((dir, plane, colour), cells) in planes {
        let colour = colours.list[colour];
        for [u0, v0, u1, v1] in rectangles(cells) {
            let corners = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]].map(|uv| point(dir, plane, uv));
            quad(&mut out, corners, DIRECTIONS[dir].as_vec3(), colour);
        }
    }
    out
}

/// Whether a cube fills its sides, so its neighbours' faces against it are hidden.
fn fills(cube: &Cube) -> bool {
    cube.is_in() && cube.fill == Fill::Solid && corners(cube.shape) == [1.0; 4]
}

/// A shape's height over its corners, as fractions of a cube, west-north, east-north,
/// east-south and west-south: whole for any that fills its top, near enough far off.
fn corners(shape: Shape) -> [f32; 4] {
    match shape {
        Shape::Half | Shape::Slope(_) | Shape::OuterCorner(_) | Shape::InnerCorner(_) => {
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]].map(|[u, v]| shape.top(u, v))
        }
        _ => [1.0; 4],
    }
}

/// The six directions a face looks, and which of them is up.
const DIRECTIONS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];
const UP: usize = 2;

/// The plane a cube's face in a direction lies in, counted in cubes along it, and where on
/// that plane the face is.
fn plane_of(dir: usize, at: IVec3) -> (i32, [i32; 2]) {
    match dir {
        0 => (at.x + 1, [at.z, at.y]),
        1 => (at.x, [at.z, at.y]),
        2 => (at.y + 1, [at.x, at.z]),
        3 => (at.y, [at.x, at.z]),
        4 => (at.z + 1, [at.x, at.y]),
        _ => (at.z, [at.x, at.y]),
    }
}

/// A point of a plane, in metres, from where on it it is.
fn point(dir: usize, plane: i32, [u, v]: [i32; 2]) -> Vec3 {
    let (p, u, v) = (plane as f32, u as f32, v as f32);
    let cubes = match dir {
        0 | 1 => Vec3::new(p, v, u),
        2 | 3 => Vec3::new(u, p, v),
        _ => Vec3::new(u, v, p),
    };
    cubes * CELL_M
}

/// Cells of a plane merged into rectangles, each as its lowest and highest corner, the
/// highest one past its last cell: greedily, along the first axis and then the second.
fn rectangles(mut cells: HashSet<[i32; 2]>) -> Vec<[i32; 4]> {
    let mut order: Vec<[i32; 2]> = cells.iter().copied().collect();
    order.sort_by_key(|&[u, v]| (v, u));
    let mut out = Vec::new();
    for [u0, v0] in order {
        if !cells.remove(&[u0, v0]) {
            continue;
        }
        let mut u1 = u0 + 1;
        while cells.remove(&[u1, v0]) {
            u1 += 1;
        }
        let mut v1 = v0 + 1;
        while (u0..u1).all(|u| cells.contains(&[u, v1])) {
            for u in u0..u1 {
                cells.remove(&[u, v1]);
            }
            v1 += 1;
        }
        out.push([u0, v0, u1, v1]);
    }
    out
}

/// A cut cube: its top over its corners, its sides up to them, its underside if open.
fn cut(
    out: &mut Surface,
    at: IVec3,
    cube: &Cube,
    tops: [f32; 4],
    solid: &dyn Fn(IVec3) -> bool,
    colours: &mut Colours,
) {
    let base = at.as_vec3();
    let corner = |i: usize, up: f32| {
        let [u, v] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]][i];
        (base + Vec3::new(u, up, v)) * CELL_M
    };
    let (top, side) = (colours.of(cube.kind, true), colours.of(cube.kind, false));
    let (top, side) = (colours.list[top], colours.list[side]);
    // The top, split along the diagonal its shape creases on: the one whose middle is at
    // the shape's height over the cube's middle.
    let middle = cube.shape.top(0.5, 0.5);
    let corners = [0, 1, 2, 3].map(|i| corner(i, tops[i]));
    let across =
        ((tops[0] + tops[2]) * 0.5 - middle).abs() > ((tops[1] + tops[3]) * 0.5 - middle).abs();
    let triangles = if across {
        [[1, 2, 3], [1, 3, 0]]
    } else {
        [[0, 1, 2], [0, 2, 3]]
    };
    for [a, b, c] in triangles {
        triangle(out, [corners[a], corners[b], corners[c]], Vec3::Y, top);
    }
    // Each side, from the foot to the top's height at its two corners, where nothing fills
    // it: west-north to east-north is the north side, and so on round.
    for (i, normal) in [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X]
        .iter()
        .enumerate()
    {
        let (a, b) = (i, (i + 1) % 4);
        if solid(at + *normal) || (tops[a] <= 0.0 && tops[b] <= 0.0) {
            continue;
        }
        let side_corners = [
            corner(a, 0.0),
            corner(b, 0.0),
            corner(b, tops[b]),
            corner(a, tops[a]),
        ];
        quad(out, side_corners, normal.as_vec3(), side);
    }
    if !solid(at - IVec3::Y) {
        let under = [0, 1, 2, 3].map(|i| corner(i, 0.0));
        quad(out, under, Vec3::NEG_Y, side);
    }
}

/// A quad, its corners in order round it, wound to face the way its normal looks.
fn quad(out: &mut Surface, corners: [Vec3; 4], normal: Vec3, colour: [f32; 3]) {
    let [a, b, c, d] = corners;
    triangle(out, [a, b, c], normal, colour);
    triangle(out, [a, c, d], normal, colour);
}

/// A triangle facing the side `toward` is on, its normal its own, dropped if it has no
/// area.
fn triangle(out: &mut Surface, corners: [Vec3; 3], toward: Vec3, colour: [f32; 3]) {
    let [a, b, c] = corners;
    let cross = (b - a).cross(c - a);
    if cross.length_squared() < 1e-10 {
        return;
    }
    let (b, c, cross) = if cross.dot(toward) < 0.0 {
        (c, b, -cross)
    } else {
        (b, c, cross)
    };
    let normal = cross.normalize();
    let shared = &mut out.shared;
    let first = shared.corner(a, normal, [0.0; 2], colour);
    shared.corner(b, normal, [0.0; 2], colour);
    shared.corner(c, normal, [0.0; 2], colour);
    shared.indices.extend([first, first + 1, first + 2]);
}

/// Each kind's colours, its top's and its sides', worked out once, by number.
struct Colours<'a> {
    kits: &'a Kits,
    maps: &'a Maps,
    by_kind: BTreeMap<(Kind, bool), usize>,
    list: Vec<[f32; 3]>,
}

impl<'a> Colours<'a> {
    fn new(kits: &'a Kits, maps: &'a Maps) -> Self {
        Colours {
            kits,
            maps,
            by_kind: BTreeMap::new(),
            list: Vec::new(),
        }
    }

    /// The number of a kind's colour on top or at its sides.
    fn of(&mut self, kind: Kind, top: bool) -> usize {
        if let Some(&index) = self.by_kind.get(&(kind, top)) {
            return index;
        }
        let linear = self.linear(kind, top);
        let colour = linear.map(|c| to_srgb(c * SHADE));
        let index = self.list.len();
        self.list.push(colour);
        self.by_kind.insert((kind, top), index);
        index
    }

    /// A kind's material's mean colour, linear: its covering for a roof, its stone or its
    /// logs at its sides, its floor on top.
    fn linear(&self, kind: Kind, top: bool) -> [f32; 3] {
        let Some(kit) = self.kits.of(kind) else {
            return [0.35, 0.33, 0.3];
        };
        let materials: &[Material] = match (kit.pattern, top) {
            (Pattern::Roof, _) => &[Material::Covering],
            (Pattern::Masonry, false) => &[Material::Masonry],
            (Pattern::Masonry, true) => &[Material::Floor, Material::Masonry],
            (Pattern::Logs, false) => &[Material::Log],
            (Pattern::Logs, true) => &[Material::Floor, Material::Log],
        };
        materials
            .iter()
            .find_map(|&material| self.maps.colour(kit, material))
            .map_or_else(
                || match kit.pattern {
                    Pattern::Roof => [0.16, 0.13, 0.11],
                    Pattern::Masonry => [0.25, 0.24, 0.22],
                    Pattern::Logs => [0.22, 0.15, 0.1],
                },
                |swatch| swatch.mean(),
            )
    }
}

fn to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * libm::powf(c, 1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BuildOp;

    fn stone() -> Cube {
        Cube::new(Kind(2), Fill::Solid)
    }

    #[test]
    fn a_solid_block_is_six_rectangles() {
        let base = Base::from_shortcuts(&[BuildOp::Fill {
            min: [0, 0, 0],
            max: [7, 3, 5],
            cube: Some(stone()),
        }]);
        let surface = hull(
            &base,
            &Kits(Vec::new()),
            &Maps::default(),
            IVec3::splat(-16),
            IVec3::splat(31),
        );
        assert_eq!(surface.triangle_count(), 12);
        // Its corners span the block, half a metre a cube.
        let high = surface
            .shared
            .positions
            .iter()
            .copied()
            .reduce(Vec3::max)
            .unwrap();
        assert_eq!(high, Vec3::new(4.0, 2.0, 3.0));
    }

    #[test]
    fn a_hidden_face_is_not_drawn_and_a_slope_keeps_its_pitch() {
        // Two blocks side by side meet with no face between them; a slope on top rises.
        let base = Base::from_shortcuts(&[
            BuildOp::Fill {
                min: [0, 0, 0],
                max: [1, 0, 0],
                cube: Some(stone()),
            },
            BuildOp::Fill {
                min: [0, 1, 0],
                max: [0, 1, 0],
                cube: Some(Cube {
                    shape: Shape::Slope(crate::Facing::East),
                    ..stone()
                }),
            },
        ]);
        let surface = hull(
            &base,
            &Kits(Vec::new()),
            &Maps::default(),
            IVec3::splat(-16),
            IVec3::splat(31),
        );
        // The two blocks: a box of six rectangles less the top under the slope, which is
        // drawn whole as the slope sits on it... the slope: two top triangles, a back,
        // two raking sides, no foot (it stands on stone).
        let sloped = surface
            .shared
            .normals
            .iter()
            .filter(|n| n.y > 0.1 && n.x < -0.1)
            .count();
        assert_eq!(
            sloped, 6,
            "the slope's top, two triangles, faces up and west"
        );
        let tallest = surface
            .shared
            .positions
            .iter()
            .map(|p| p.y)
            .fold(0.0, f32::max);
        assert_eq!(tallest, 1.0);
    }
}
