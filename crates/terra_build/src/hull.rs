//! A base's cubes as a hull, for far off (docs/building-plan.md, drawing): what Unreal's
//! hierarchical levels of detail call an approximated mesh, a building's outer shape at a
//! few hundred triangles with its materials' colours baked in, in place of its pieces past
//! where they can be told apart. Every cube in it shows each face open to the air or to a
//! cube that does not fill its side, an opening (a window, a doorway) as a dark pane flush
//! with its wall, as its glass and door read from afar; the faces of whole cubes lying in
//! one plane in one colour are merged into rectangles (greedy meshing, as voxel games mesh
//! their chunks); a cut cube shows its top at its shape's height over each corner
//! (`Shape::top`), its sides cut to it, so roofs keep their pitch. Its colours are each
//! kit's materials' mean, darkened a little for the shade a surface's relief casts.
//!
//! Pure data, so it runs on any thread: the game makes it on its workers.

use crate::kit::{Kits, Pattern};
use crate::mesh::{Material, Surface};
use crate::relief::Maps;
use crate::{Base, CELL_M, Cube, Cubes, Facing, Fill, Kind, Shape};
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, HashSet};

/// How much darker than its material's mean colour a face is drawn: the shade its relief
/// and its seams cast, which the hull has none of.
pub(crate) const SHADE: f32 = 0.85;

/// An opening's colour far off (sRGB): its glass, frame and door in the shade of its
/// reveal, as the far detail's windows read. Drawn in its wall's colour, every window of a
/// house far off was gone and its walls one blank box.
const OPENING: [f32; 3] = [0.26, 0.27, 0.29];

/// The hull of a base's cubes from `low` to `high` inclusive, in the base's own space, its
/// colours sRGB as a surface's are.
pub fn hull(base: &Base, kits: &Kits, maps: &Maps, low: IVec3, high: IVec3) -> Surface {
    let store = base.store();
    // A roof cube something stands on is drawn whole, in what stands on it: the wall
    // carried down into it to the roof's deck (D343).
    let carried = |at: IVec3, cube: Cube| -> Cube {
        match crate::layout::roofs::standing_on(base, at) {
            Some(over) if cube.kind.is_roof() && cube.is_in() => Cube {
                kind: over.kind,
                shape: Shape::Whole,
                ..cube
            },
            _ => cube,
        }
    };
    let solid = |at: IVec3| store.get(at).is_some_and(|cube| fills(&carried(at, cube)));
    let mut colours = Colours::new(kits, maps);
    // Whole faces by their direction, the plane they lie in and their colour: the cells
    // of each plane, merged into rectangles once all are known.
    let mut planes: BTreeMap<(usize, i32, usize), HashSet<[i32; 2]>> = BTreeMap::new();
    // Slopes' tops by the way they rise, their layer, the line they run along and their
    // colour: where along it each is, merged into runs, as a roof's course is one plane.
    let mut slopes: BTreeMap<(u8, i32, i32, usize), Vec<i32>> = BTreeMap::new();
    let mut out = Surface::default();
    for (at, cube) in store.within(low, high) {
        if !drawn(&cube) {
            continue;
        }
        let cube = carried(at, cube);
        let tops = corners(cube.shape);
        if tops == [1.0; 4] {
            for (dir, normal) in DIRECTIONS.iter().enumerate() {
                if solid(at + *normal) {
                    continue;
                }
                let colour = if cube.fill == Fill::Open {
                    colours.opening()
                } else {
                    colours.of(cube.kind, dir == UP)
                };
                let (plane, cell) = plane_of(dir, at);
                planes.entry((dir, plane, colour)).or_default().insert(cell);
            }
            continue;
        }
        let top_drawn = if let Shape::Slope(facing) = cube.shape {
            let colour = colours.of(cube.kind, true);
            let (line, along) = if along_x(facing) {
                (at.z, at.x)
            } else {
                (at.x, at.z)
            };
            slopes
                .entry((facing as u8, at.y, line, colour))
                .or_default()
                .push(along);
            true
        } else {
            false
        };
        cut(&mut out, at, &cube, tops, !top_drawn, store, &mut colours);
    }
    for ((dir, plane, colour), cells) in planes {
        let colour = colours.list[colour];
        for [u0, v0, u1, v1] in rectangles(cells) {
            let corners = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]].map(|uv| point(dir, plane, uv));
            quad(&mut out, corners, DIRECTIONS[dir].as_vec3(), colour);
        }
    }
    for ((facing, y, line, colour), mut along) in slopes {
        let facing = [Facing::North, Facing::East, Facing::South, Facing::West][facing as usize];
        let tops = corners(Shape::Slope(facing));
        let colour = colours.list[colour];
        along.sort_unstable();
        let mut runs: Vec<(i32, i32)> = Vec::new();
        for a in along {
            match runs.last_mut() {
                Some(run) if run.1 + 1 == a => run.1 = a,
                _ => runs.push((a, a)),
            }
        }
        for (first, last) in runs {
            // The run's corners, west-north round to west-south, at the slope's heights.
            let (x0, x1, z0, z1) = if along_x(facing) {
                (first, last + 1, line, line + 1)
            } else {
                (line, line + 1, first, last + 1)
            };
            let corners = [[x0, z0], [x1, z0], [x1, z1], [x0, z1]]
                .iter()
                .zip(tops)
                .map(|(&[x, z], up)| Vec3::new(x as f32, y as f32 + up, z as f32) * CELL_M)
                .collect::<Vec<_>>();
            quad(
                &mut out,
                [corners[0], corners[1], corners[2], corners[3]],
                Vec3::Y,
                colour,
            );
        }
    }
    out
}

/// Whether a slope rising this way is the same along X, so its runs go along X.
fn along_x(facing: Facing) -> bool {
    matches!(facing, Facing::North | Facing::South)
}

/// Whether a cube is drawn: anything of it in, or an opening cut in a wall built (a pane).
fn drawn(cube: &Cube) -> bool {
    cube.is_in() || (cube.fill == Fill::Open && cube.level() > 0)
}

/// Whether a cube fills its sides, so its neighbours' faces against it are hidden: an
/// opening too, drawn as a pane, so its reveals are not.
fn fills(cube: &Cube) -> bool {
    drawn(cube) && corners(cube.shape) == [1.0; 4]
}

/// A shape's height over its corners, as fractions of a cube, west-north, east-north,
/// east-south and west-south: whole for any that fills its top, near enough far off.
fn corners(shape: Shape) -> [f32; 4] {
    match shape {
        Shape::Half | Shape::Slope(_) | Shape::OuterCorner(_) | Shape::InnerCorner(_) => {
            CORNERS.map(|[u, v]| shape.top(u, v))
        }
        _ => [1.0; 4],
    }
}

/// A cube's corners on its footprint, as fractions east and south: west-north, east-north,
/// east-south and west-south.
const CORNERS: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

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
    with_top: bool,
    store: &Cubes,
    colours: &mut Colours,
) {
    let solid = |at: IVec3| store.get(at).is_some_and(|cube| fills(&cube));
    let base = at.as_vec3();
    let corner = |i: usize, up: f32| {
        let [u, v] = CORNERS[i];
        (base + Vec3::new(u, up, v)) * CELL_M
    };
    let (top, side) = (colours.of(cube.kind, true), colours.of(cube.kind, false));
    let (top, side) = (colours.list[top], colours.list[side]);
    // The top, split along the diagonal its shape creases on: the one whose middle is at
    // the shape's height over the cube's middle.
    let middle = cube.shape.top(0.5, 0.5);
    let points = [0, 1, 2, 3].map(|i| corner(i, tops[i]));
    let across =
        ((tops[0] + tops[2]) * 0.5 - middle).abs() > ((tops[1] + tops[3]) * 0.5 - middle).abs();
    let triangles = if across {
        [[1, 2, 3], [1, 3, 0]]
    } else {
        [[0, 1, 2], [0, 2, 3]]
    };
    if with_top {
        for [a, b, c] in triangles {
            triangle(out, [points[a], points[b], points[c]], Vec3::Y, top);
        }
    }
    // Each side, from the foot to the top's height at its two corners, where nothing fills
    // it as high: west-north to east-north is the north side, and so on round; against a
    // cut neighbour as high along the edge they share, as along a roof's course, nothing.
    for (i, normal) in [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X]
        .iter()
        .enumerate()
    {
        let (a, b) = (i, (i + 1) % 4);
        if solid(at + *normal) || (tops[a] <= 0.0 && tops[b] <= 0.0) {
            continue;
        }
        let covered = store.get(at + *normal).is_some_and(|next| {
            let theirs = corners(next.shape);
            // Our corner (u, v) is theirs at (u, v) less the step to them.
            let across = |i: usize| {
                let [u, v] = CORNERS[i];
                let (u, v) = (u - normal.x as f32, v - normal.z as f32);
                CORNERS
                    .iter()
                    .position(|&c| c == [u, v])
                    .map_or(0.0, |j| theirs[j])
            };
            next.is_in()
                && next.fill == Fill::Solid
                && across(a) >= tops[a] - 1e-4
                && across(b) >= tops[b] - 1e-4
        });
        if covered {
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

    /// The number of an opening's colour ([`OPENING`]).
    fn opening(&mut self) -> usize {
        match self.list.iter().position(|&c| c == OPENING) {
            Some(index) => index,
            None => {
                self.list.push(OPENING);
                self.list.len() - 1
            }
        }
    }

    /// The number of a kind's colour on top or at its sides.
    fn of(&mut self, kind: Kind, top: bool) -> usize {
        if let Some(&index) = self.by_kind.get(&(kind, top)) {
            return index;
        }
        let linear = self.linear(kind, top);
        let tint = self.tint(kind);
        let mut colour = linear.map(|c| to_srgb(c * SHADE));
        for (c, t) in colour.iter_mut().zip(tint) {
            *c = (*c * t).clamp(0.0, 1.0);
        }
        let index = self.list.len();
        self.list.push(colour);
        self.by_kind.insert((kind, top), index);
        index
    }

    /// What a kind's near detail multiplies its texture's colour by (sRGB), so its hull far
    /// off is the colour it is near: a roof's covering, a block kind's earth or daub, a
    /// timber kind's logs.
    fn tint(&self, kind: Kind) -> [f32; 3] {
        match self.kits.of(kind) {
            Some(kit) => match kit.pattern {
                Pattern::Roof => kit.roof.tint,
                Pattern::Masonry => kit.tint,
                Pattern::Logs => kit.timber.tint,
            },
            None => [1.0; 3],
        }
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
    fn a_course_of_slopes_is_one_plane_with_no_sides_between() {
        let slope = Cube {
            shape: Shape::Slope(Facing::North),
            ..stone()
        };
        let base = Base::from_shortcuts(&[
            BuildOp::Fill {
                min: [0, 0, 0],
                max: [5, 0, 0],
                cube: Some(stone()),
            },
            BuildOp::Fill {
                min: [0, 1, 0],
                max: [5, 1, 0],
                cube: Some(slope),
            },
        ]);
        let surface = hull(
            &base,
            &Kits(Vec::new()),
            &Maps::default(),
            IVec3::splat(-16),
            IVec3::splat(31),
        );
        let normals = &surface.shared.normals;
        // The six slopes' tops, one quad facing up and south: two triangles.
        let sloped = normals.iter().filter(|n| n.y > 0.1 && n.z > 0.1).count();
        assert_eq!(sloped, 6);
        // Their sides: only the two at the course's ends, east and west.
        let ends = normals.iter().filter(|n| n.x.abs() > 0.9).count();
        // The stone row's two ends, a rectangle each, and the slopes' two, a triangle each.
        assert_eq!(ends, 2 * 6 + 2 * 3);
    }

    #[test]
    fn a_window_is_a_dark_pane_flush_with_its_wall_and_no_reveals() {
        // A wall seven cubes along X, five high, one thick, with an opening in its middle.
        let base = Base::from_shortcuts(&[
            BuildOp::Fill {
                min: [0, 0, 0],
                max: [6, 4, 0],
                cube: Some(stone()),
            },
            BuildOp::Fill {
                min: [3, 2, 0],
                max: [3, 2, 0],
                cube: Some(Cube::new(Kind(2), Fill::Open)),
            },
        ]);
        let surface = hull(
            &base,
            &Kits(Vec::new()),
            &Maps::default(),
            IVec3::splat(-16),
            IVec3::splat(31),
        );
        let shared = &surface.shared;
        let dark = |i: &u32| shared.colours[*i as usize] == OPENING;
        let panes: Vec<_> = shared
            .indices
            .chunks(3)
            .filter(|t| t.iter().all(dark))
            .collect();
        assert_eq!(panes.len(), 4, "a pane each side, two triangles each");
        // Its faces lie in the wall's two faces, so nothing inside the opening is drawn.
        for t in &panes {
            let z = shared.positions[t[0] as usize].z;
            assert!(z == 0.0 || z == CELL_M, "a pane at z {z}");
        }
        // Sides along X only at the wall's two ends, none round the opening.
        let ends = shared
            .indices
            .chunks(3)
            .filter(|t| shared.normals[t[0] as usize].x.abs() > 0.5)
            .count();
        assert_eq!(ends, 4, "the two ends, two triangles each");
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
