//! Block kinds drawn as their cubes' own surface (D197, research/block-modules.md): stone,
//! brick, plaster and earth are not laid as models but as the outer faces of their cubes,
//! each a fine grid, rounded along every outside edge and displaced by their material's
//! height, so a wall of any shape, carved anywhere, is one surface with no seams.
//!
//! **Rounding.** Each cube is shrunk by the bevel on every side where nothing hides it,
//! and a point of the surface is moved onto the bevel's radius from the nearest point of
//! the nearest such shrunk box, over all the cubes round it: flat in the middle of a face,
//! a quarter-round along an outside edge, a rounded corner where three meet. Sides against
//! a neighbour are not shrunk, so faces of cubes side by side run on as one, and inside
//! corners stay sharp, as real ones are. Taking the nearest over every cube round a point,
//! not only the face's own, makes it the same for every face that shares the point.
//!
//! **Relief.** The material's height, read where the texture is drawn (the same planar
//! mapping as its colour), pushes each point out along its rounded normal. Along an
//! outside edge the three axes' heights are blended by that normal, so the faces either
//! side of it meet. Near an inside edge, one with three of the four cubes round it full,
//! the relief fades out, as the faces there push different ways; how near is measured to
//! the edge itself, so every face at a point agrees.

use crate::mesh::planar;
use crate::{CELL_M, Surface};
use glam::{IVec3, Vec3};
use std::collections::{BTreeSet, HashMap};

/// How a block kind's surface is made: from its kit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// The radius its outside edges are rounded to, in metres.
    pub bevel: f32,
    /// How far its relief stands out, from the deepest joint to the proudest stone.
    pub relief: f32,
    /// How finely each cube's face is divided.
    pub steps: u32,
    /// How many metres across and down one copy of its texture covers.
    pub metres: [f32; 2],
}

/// How far an inside corner's relief fades out from it, in metres.
const INSIDE_FADE_M: f32 = 0.06;

/// The faces, as an axis and a side, with the two axes across each.
const FACES: [(usize, f32); 6] = [
    (0, 1.0),
    (0, -1.0),
    (1, 1.0),
    (1, -1.0),
    (2, 1.0),
    (2, -1.0),
];

fn unit(axis: usize, side: f32) -> Vec3 {
    let mut v = Vec3::ZERO;
    v[axis] = side;
    v
}

fn step(axis: usize, side: i32) -> IVec3 {
    let mut v = IVec3::ZERO;
    v[axis] = side;
    v
}

/// Draws the cubes as one surface into `out`, in the base's own space. `hides` says which
/// other cubes hide a face against them; the cubes drawn always hide one another.
/// `height` gives the material's height, from 0 to 1, where its texture is at a point.
pub fn draw(
    cubes: &BTreeSet<[i32; 3]>,
    hides: &dyn Fn(IVec3) -> bool,
    look: &Look,
    height: &dyn Fn([f32; 2]) -> f32,
    colour: [f32; 3],
    out: &mut Surface,
) {
    let full = |at: IVec3| cubes.contains(&at.to_array()) || hides(at);
    let n = look.steps.max(1) as usize;
    // Each cube shrunk by the bevel on every side nothing hides.
    let shrunk: HashMap<IVec3, (Vec3, Vec3)> = cubes
        .iter()
        .map(|&cube| {
            let cube = IVec3::from_array(cube);
            let mut low = cube.as_vec3() * CELL_M;
            let mut high = low + Vec3::splat(CELL_M);
            for axis in 0..3 {
                if !full(cube - step(axis, 1)) {
                    low[axis] += look.bevel;
                }
                if !full(cube + step(axis, 1)) {
                    high[axis] -= look.bevel;
                }
            }
            (cube, (low, high))
        })
        .collect();
    // Every inside edge: a cube's edge with three of the four cubes round it full, kept by
    // the cell its middle is in, so a point looks only at those near it.
    let mut inside: HashMap<IVec3, Vec<(Vec3, Vec3)>> = HashMap::new();
    for &cube in cubes {
        let cube = IVec3::from_array(cube);
        for axis in 0..3 {
            let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
            for (da, db) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                // The edge along `axis` at this corner of the cube, and the four cubes round
                // it.
                let corner = cube + step(a, da) + step(b, db);
                let round = [(0, 0), (-1, 0), (0, -1), (-1, -1)]
                    .map(|(ea, eb)| full(corner + step(a, ea) + step(b, eb)));
                if round.iter().filter(|&&f| f).count() == 3 {
                    let from = corner.as_vec3() * CELL_M;
                    let to = from + unit(axis, CELL_M);
                    let cell = ((from + to) * 0.5 / CELL_M).floor().as_ivec3();
                    let edges = inside.entry(cell).or_default();
                    if !edges.contains(&(from, to)) {
                        edges.push((from, to));
                    }
                }
            }
        }
    }
    let fade = |p: Vec3| {
        let cell = (p / CELL_M).floor().as_ivec3();
        (-1..=1)
            .flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| IVec3::new(x, y, z))))
            .filter_map(|offset| inside.get(&(cell + offset)))
            .flatten()
            .map(|&(from, to)| {
                let along = (to - from).normalize();
                let t = (p - from).dot(along).clamp(0.0, CELL_M);
                (p.distance(from + along * t) / INSIDE_FADE_M).min(1.0)
            })
            .fold(1.0_f32, f32::min)
    };
    let mut triangles: Vec<[Vec3; 3]> = Vec::new();
    let mut uvs: Vec<[[f32; 2]; 3]> = Vec::new();
    for &cube in cubes {
        let cube = IVec3::from_array(cube);
        let low = cube.as_vec3() * CELL_M;
        // The shrunk boxes of the cubes round this one, which any point of its faces may
        // be nearest.
        let round: Vec<(Vec3, Vec3)> = (-1..=1)
            .flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| IVec3::new(x, y, z))))
            .filter_map(|offset| shrunk.get(&(cube + offset)).copied())
            .collect();
        for (axis, side) in FACES {
            let out_step = step(axis, side as i32);
            if full(cube + out_step) {
                continue;
            }
            let normal = unit(axis, side);
            let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
            let plane = if side > 0.0 {
                low[axis] + CELL_M
            } else {
                low[axis]
            };
            let point = |i: usize, j: usize| {
                let mut p = Vec3::ZERO;
                p[axis] = plane;
                p[a] = low[a] + CELL_M * i as f32 / n as f32;
                p[b] = low[b] + CELL_M * j as f32 / n as f32;
                p
            };
            let place = |p: Vec3| -> Vec3 {
                let nearest = round
                    .iter()
                    .map(|&(low, high)| p.clamp(low, high))
                    .min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
                    .unwrap_or(p);
                let away = p - nearest;
                let length = away.length();
                let (rounded, outward) = if length > 1e-6 {
                    let outward = away / length;
                    (nearest + outward * look.bevel, outward)
                } else {
                    (p, normal)
                };
                let relief = (blended(p, outward, look, height) - 0.5) * look.relief * fade(p);
                rounded + outward * relief
            };
            // Wound so each triangle faces out along the face's normal.
            let flip = side < 0.0;
            for i in 0..n {
                for j in 0..n {
                    let corners = [
                        point(i, j),
                        point(i + 1, j),
                        point(i + 1, j + 1),
                        point(i, j + 1),
                    ];
                    let placed = corners.map(place);
                    let mapped = corners.map(|p| planar(p, normal, look.metres));
                    for [x, y, z] in [[0, 1, 2], [0, 2, 3]] {
                        let (y, z) = if flip { (z, y) } else { (y, z) };
                        triangles.push([placed[x], placed[y], placed[z]]);
                        uvs.push([mapped[x], mapped[y], mapped[z]]);
                    }
                }
            }
        }
    }
    // Smooth across every edge the faces share: the normal at a point is the sum of its
    // triangles' around it.
    let key = |p: Vec3| ((p * 2048.0).round()).as_ivec3();
    let mut sums: HashMap<IVec3, Vec3> = HashMap::new();
    for t in &triangles {
        let face = (t[1] - t[0]).cross(t[2] - t[0]);
        for p in t {
            *sums.entry(key(*p)).or_default() += face;
        }
    }
    for (t, uv) in triangles.iter().zip(&uvs) {
        let face = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
        let normals = t.map(|p| sums[&key(p)].normalize_or(face));
        out.textured(*t, normals, *uv, colour);
    }
}

/// The material's height at a point, its three planar mappings blended by which way the
/// surface faces there, so the faces either side of an outside edge agree.
fn blended(p: Vec3, outward: Vec3, look: &Look, height: &dyn Fn([f32; 2]) -> f32) -> f32 {
    let weights = outward.abs().powf(4.0);
    let total = weights.x + weights.y + weights.z;
    (0..3)
        .filter(|&axis| weights[axis] > 1e-4)
        .map(|axis| weights[axis] / total * height(planar(p, unit(axis, 1.0), look.metres)))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look() -> Look {
        Look {
            bevel: 0.04,
            relief: 0.05,
            steps: 8,
            metres: [2.0, 2.0],
        }
    }

    fn flat(_: [f32; 2]) -> f32 {
        0.5
    }

    fn bumpy(uv: [f32; 2]) -> f32 {
        0.5 + 0.5 * libm::sinf(uv[0] * 40.0) * libm::cosf(uv[1] * 37.0)
    }

    fn drawn(cubes: &[[i32; 3]], height: &dyn Fn([f32; 2]) -> f32) -> Surface {
        let set: BTreeSet<[i32; 3]> = cubes.iter().copied().collect();
        let mut out = Surface::default();
        draw(&set, &|_| false, &look(), height, [1.0; 3], &mut out);
        out
    }

    /// Every edge of every triangle is shared by exactly one other triangle: no cracks.
    fn closed(surface: &Surface) -> bool {
        let key = |p: Vec3| ((p * 2048.0).round()).as_ivec3().to_array();
        let mut edges: HashMap<([i32; 3], [i32; 3]), i32> = HashMap::new();
        for t in surface.positions.chunks(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (a, b) = (key(a), key(b));
                if a == b {
                    continue;
                }
                let k = if a < b { (a, b) } else { (b, a) };
                *edges.entry(k).or_default() += 1;
            }
        }
        edges.values().all(|&count| count == 2)
    }

    #[test]
    fn a_cube_is_closed_and_rounded() {
        let s = drawn(&[[0, 0, 0]], &flat);
        assert!(closed(&s));
        // Its corner is rounded: no point reaches the box's corner.
        let corner = Vec3::splat(CELL_M);
        assert!(s.positions.iter().all(|p| p.distance(corner) > 0.01));
        // Its faces' middles are where the cube's faces are.
        assert!(s.positions.iter().any(|p| (p.x - CELL_M).abs() < 1e-5));
    }

    #[test]
    fn cubes_side_by_side_are_one_surface_with_no_face_between() {
        let s = drawn(&[[0, 0, 0], [1, 0, 0]], &bumpy);
        assert!(closed(&s));
        // Nothing drawn on the plane between them, away from the outer faces.
        assert!(!s.positions.iter().any(|p| (p.x - CELL_M).abs() < 1e-4
            && p.y > 0.1
            && p.y < 0.4
            && p.z > 0.1
            && p.z < 0.4));
    }

    #[test]
    fn an_l_and_a_carved_wall_stay_closed_with_relief() {
        let ell = [
            [0, 0, 0],
            [1, 0, 0],
            [0, 0, 1],
            [0, 1, 0],
            [1, 1, 0],
            [0, 1, 1],
        ];
        assert!(closed(&drawn(&ell, &bumpy)));
        let mut wall: Vec<[i32; 3]> = (0..4)
            .flat_map(|x| (0..3).map(move |y| [x, y, 0]))
            .collect();
        wall.retain(|c| *c != [1, 1, 0]);
        assert!(closed(&drawn(&wall, &bumpy)));
    }

    #[test]
    fn the_relief_moves_the_surface() {
        let a = drawn(&[[0, 0, 0]], &flat);
        let b = drawn(&[[0, 0, 0]], &bumpy);
        assert_eq!(a.positions.len(), b.positions.len());
        assert!(
            a.positions
                .iter()
                .zip(&b.positions)
                .any(|(x, y)| x.distance(*y) > 0.005)
        );
    }
}
