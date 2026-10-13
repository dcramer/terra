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

use crate::mesh::{QuickMap, planar};
use crate::{CELL_M, Surface};
use glam::{IVec3, Vec3};
use std::collections::BTreeSet;

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
    /// Its tops, if they are drawn in a material of their own, as a stone floor's
    /// flagstones are: how many metres one copy of that covers, and how far its relief
    /// stands out.
    pub top: Option<([f32; 2], f32)>,
}

/// What a surface is displaced by: its material's height where its texture is at a point,
/// from 0 to 1, and its tops' material's, if they have one.
pub struct Heights<'a> {
    pub sides: &'a dyn Fn([f32; 2]) -> f32,
    pub top: &'a dyn Fn([f32; 2]) -> f32,
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

/// Draws the cubes as one surface into `out`, in the base's own space, and their tops into
/// `tops` if the look gives them a material of their own. `hides` says which other cubes
/// hide a face against them; the cubes drawn always hide one another.
pub fn draw(
    cubes: &BTreeSet<[i32; 3]>,
    hides: &dyn Fn(IVec3) -> bool,
    look: &Look,
    heights: &Heights,
    colour: [f32; 3],
    out: &mut Surface,
    tops: &mut Surface,
) {
    draw_cut(
        cubes,
        hides,
        look,
        heights,
        &|_, _| f32::MAX,
        colour,
        out,
        tops,
    );
}

/// As [`draw`], with each cube's surface cut off at a height over each point of it, as a
/// gable's is at its roof's underside: `cut` gives it for a cube and a point of its face
/// before it is rounded, nothing of the face going higher.
#[allow(clippy::too_many_arguments)]
pub fn draw_cut(
    cubes: &BTreeSet<[i32; 3]>,
    hides: &dyn Fn(IVec3) -> bool,
    look: &Look,
    heights: &Heights,
    cut: &dyn Fn(IVec3, Vec3) -> f32,
    colour: [f32; 3],
    out: &mut Surface,
    tops: &mut Surface,
) {
    draw_part(
        cubes,
        &|_| true,
        hides,
        look,
        heights,
        cut,
        colour,
        out,
        tops,
    );
}

/// As [`draw_cut`], keeping only the faces of the cubes `keep` says: the rest are worked
/// out, so the edges and shading where they meet the kept ones come out as they would
/// with all of them drawn, and left out. A section drawn with the cubes a step beyond its
/// edge meets its neighbour's surface with no seam (docs/building-plan.md, drawing).
#[allow(clippy::too_many_arguments)]
pub fn draw_part(
    cubes: &BTreeSet<[i32; 3]>,
    keep: &dyn Fn(IVec3) -> bool,
    hides: &dyn Fn(IVec3) -> bool,
    look: &Look,
    heights: &Heights,
    cut: &dyn Fn(IVec3, Vec3) -> f32,
    colour: [f32; 3],
    out: &mut Surface,
    tops: &mut Surface,
) {
    // Whether a cube is full, asked of each once: `hides` reads the base.
    let known: std::cell::RefCell<QuickMap<IVec3, bool>> = Default::default();
    let full = |at: IVec3| {
        if let Some(&full) = known.borrow().get(&at) {
            return full;
        }
        let full = cubes.contains(&at.to_array()) || hides(at);
        known.borrow_mut().insert(at, full);
        full
    };
    let n = look.steps.max(1) as usize;
    // Each cube shrunk by the bevel on every side nothing hides.
    let shrunk: QuickMap<IVec3, (Vec3, Vec3)> = cubes
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
    let mut inside: QuickMap<IVec3, Vec<(Vec3, Vec3)>> = QuickMap::default();
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
    // The inside edges near a cube, which a point of its faces may be near: those further
    // off are further than the fade from it.
    let near_inside = |cube: IVec3| -> Vec<(Vec3, Vec3)> {
        let mut found = Vec::new();
        for x in -1..=2 {
            for y in -1..=2 {
                for z in -1..=2 {
                    if let Some(edges) = inside.get(&(cube + IVec3::new(x, y, z))) {
                        found.extend(edges);
                    }
                }
            }
        }
        found
    };
    let fade = |p: Vec3, edges: &[(Vec3, Vec3)]| {
        edges
            .iter()
            .map(|&(from, to)| {
                let along = (to - from).normalize();
                let t = (p - from).dot(along).clamp(0.0, CELL_M);
                (p.distance(from + along * t) / INSIDE_FADE_M).min(1.0)
            })
            .fold(1.0_f32, f32::min)
    };
    // Each face drawn: a grid of points, placed once each, and its triangles' corners in it.
    let mut faces: Vec<Face> = Vec::new();
    for &cube in cubes {
        let cube = IVec3::from_array(cube);
        let keeping = keep(cube);
        // A cube left out matters only where it meets one kept, a step away.
        if !keeping
            && !(-1..=1)
                .any(|x| (-1..=1).any(|y| (-1..=1).any(|z| keep(cube + IVec3::new(x, y, z)))))
        {
            continue;
        }
        let low = cube.as_vec3() * CELL_M;
        let edges = near_inside(cube);
        // The shrunk boxes of the cubes round this one, which any point of its faces may
        // be nearest.
        let round: Vec<(Vec3, Vec3)> = (-1..=1)
            .flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| IVec3::new(x, y, z))))
            .filter_map(|offset| shrunk.get(&(cube + offset)).copied())
            .collect();
        for (axis, side) in FACES {
            let out_step = step(axis, side as i32);
            let beyond = cube + out_step;
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
            // A side against another of the cubes drawn shows where that one is cut lower
            // than this one, as a slope's side shows over a half beside it: from the other's
            // cut up to this one's. Against anything else full, nothing shows.
            let against_cut = axis != 1
                && cubes.contains(&beyond.to_array())
                && (0..=n).any(|i| {
                    (0..=n).any(|j| {
                        let p = point(i, j);
                        cut(beyond, p) < cut(cube, p).min(p.y) - 1e-4
                    })
                });
            if full(beyond) && !against_cut {
                continue;
            }
            // Only the boxes that may be nearest a point of this face: none further from
            // the face than its own cube's box can be, at most the bevel along every axis.
            let (face_low, face_high) = {
                let (mut l, mut h) = (low, low + Vec3::splat(CELL_M));
                l[axis] = plane;
                h[axis] = plane;
                (l, h)
            };
            let reach = look.bevel * look.bevel * 3.0 + 1e-6;
            let round: Vec<(Vec3, Vec3)> = round
                .iter()
                .copied()
                .filter(|&(l, h)| {
                    let gap = (l - face_high).max(face_low - h).max(Vec3::ZERO);
                    gap.length_squared() <= reach
                })
                .collect();
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
                rounded + outward * (displaced(p, outward, look, heights) * fade(p, &edges))
            };
            // Wound so each triangle faces out along the face's normal.
            let flip = side < 0.0;
            let top = axis == 1 && side > 0.0 && look.top.is_some();
            let metres = match look.top {
                Some((metres, _)) if top => metres,
                _ => look.metres,
            };
            let mut face = Face {
                points: Vec::with_capacity((n + 1) * (n + 1)),
                uvs: Vec::with_capacity((n + 1) * (n + 1)),
                triangles: Vec::with_capacity(n * n * 2),
                top,
                keeping,
            };
            for i in 0..=n {
                for j in 0..=n {
                    let p = point(i, j);
                    let mut q = place(p);
                    q.y = q.y.min(cut(cube, p));
                    if against_cut {
                        q.y = q.y.max(cut(beyond, p).min(cut(cube, p)));
                    }
                    face.points.push(q);
                    face.uvs.push(planar(p, normal, metres));
                }
            }
            let at = |i: usize, j: usize| (i * (n + 1) + j) as u32;
            for i in 0..n {
                for j in 0..n {
                    let corners = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
                    for [x, y, z] in [[0, 1, 2], [0, 2, 3]] {
                        let (y, z) = if flip { (z, y) } else { (y, z) };
                        let t = [corners[x], corners[y], corners[z]];
                        let [pa, pb, pc] = t.map(|c| face.points[c as usize]);
                        // Cut away to nothing.
                        if (pb - pa).cross(pc - pa).length_squared() < 1e-12 {
                            continue;
                        }
                        face.triangles.push(t);
                    }
                }
            }
            faces.push(face);
        }
    }
    // Smooth across every edge the faces share: the normal at a point is the sum of its
    // triangles' around it. Inside a face that is its own triangles; on a face's edge,
    // where other faces meet it, every triangle with a corner there, found by place.
    let edge = |index: u32| {
        let (i, j) = (index as usize / (n + 1), index as usize % (n + 1));
        i == 0 || j == 0 || i == n || j == n
    };
    let key = |p: Vec3| ((p * 2048.0).round()).as_ivec3();
    let mut sums: QuickMap<IVec3, Vec3> = QuickMap::default();
    let mut own: Vec<Vec<Vec3>> = Vec::with_capacity(faces.len());
    for face in &faces {
        let mut local = vec![Vec3::ZERO; face.points.len()];
        for t in &face.triangles {
            let [pa, pb, pc] = t.map(|c| face.points[c as usize]);
            let normal = (pb - pa).cross(pc - pa);
            for &c in t {
                if edge(c) {
                    *sums.entry(key(face.points[c as usize])).or_default() += normal;
                } else {
                    local[c as usize] += normal;
                }
            }
        }
        own.push(local);
    }
    for (face, local) in faces.iter().zip(own) {
        if !face.keeping {
            continue;
        }
        let into = if face.top { &mut *tops } else { &mut *out };
        let normal_at = |c: u32, fallback: Vec3| {
            let sum = if edge(c) {
                sums[&key(face.points[c as usize])]
            } else {
                local[c as usize]
            };
            sum.normalize_or(fallback)
        };
        if into.sharing {
            // Each point of the grid once, with the triangles' corners shared.
            let mut index: Vec<u32> = vec![u32::MAX; face.points.len()];
            for t in &face.triangles {
                let [pa, pb, pc] = t.map(|c| face.points[c as usize]);
                let fallback = (pb - pa).cross(pc - pa).normalize_or_zero();
                for &c in t {
                    let slot = &mut index[c as usize];
                    if *slot == u32::MAX {
                        *slot = into.shared.corner(
                            face.points[c as usize],
                            normal_at(c, fallback),
                            face.uvs[c as usize],
                            colour,
                        );
                    }
                    into.shared.indices.push(*slot);
                }
            }
            continue;
        }
        for t in &face.triangles {
            let corners = t.map(|c| face.points[c as usize]);
            let fallback = (corners[1] - corners[0])
                .cross(corners[2] - corners[0])
                .normalize_or_zero();
            let normals = t.map(|c| normal_at(c, fallback));
            into.textured(corners, normals, t.map(|c| face.uvs[c as usize]), colour);
        }
    }
}

/// A cube's face as it is drawn: its grid of points, placed, where each falls on its
/// texture, its triangles as three points of the grid each, whether it is a top in a
/// material of its own, and whether it is kept.
struct Face {
    points: Vec<Vec3>,
    uvs: Vec<[f32; 2]>,
    triangles: Vec<[u32; 3]>,
    top: bool,
    keeping: bool,
}

/// How far the surface is pushed out at a point: its materials' heights there, their three
/// planar mappings blended by which way the surface faces, so the faces either side of an
/// outside edge agree; the tops' own material where they face up and have one.
fn displaced(p: Vec3, outward: Vec3, look: &Look, heights: &Heights) -> f32 {
    let weights = outward.abs();
    let weights = weights * weights * weights * weights;
    let total = weights.x + weights.y + weights.z;
    (0..3)
        .filter(|&axis| weights[axis] > 1e-4)
        .map(|axis| {
            let (height, metres, relief) = match look.top {
                Some((metres, relief)) if axis == 1 && outward.y > 0.0 => {
                    (heights.top, metres, relief)
                }
                _ => (heights.sides, look.metres, look.relief),
            };
            let at = height(planar(p, unit(axis, 1.0), metres));
            weights[axis] / total * (at - 0.5) * relief
        })
        .sum()
}

/// A box of a block kind's material from one corner to the other, as its cubes' surface is
/// drawn: rounded along every edge by the bevel and displaced by its material, each face
/// divided about as finely as a cube's; its top in its tops' material if it has one. Door
/// steps are drawn so.
pub fn slab(
    min: Vec3,
    max: Vec3,
    look: &Look,
    heights: &Heights,
    colour: [f32; 3],
    out: &mut Surface,
    tops: &mut Surface,
) {
    let size = max - min;
    let bevel = look.bevel.min(size.min_element() * 0.45);
    let (low, high) = (min + Vec3::splat(bevel), max - Vec3::splat(bevel));
    let cell = CELL_M / look.steps.max(1) as f32;
    for (axis, side) in FACES {
        let normal = unit(axis, side);
        let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
        let (na, nb) = (
            ((size[a] / cell).ceil() as usize).max(1),
            ((size[b] / cell).ceil() as usize).max(1),
        );
        let plane = if side > 0.0 { max[axis] } else { min[axis] };
        let point = |i: usize, j: usize| {
            let mut p = Vec3::ZERO;
            p[axis] = plane;
            p[a] = min[a] + size[a] * i as f32 / na as f32;
            p[b] = min[b] + size[b] * j as f32 / nb as f32;
            p
        };
        let place = |p: Vec3| -> (Vec3, Vec3) {
            let nearest = p.clamp(low, high);
            let outward = (p - nearest).normalize_or(normal);
            let rounded = nearest + outward * bevel;
            (
                rounded + outward * displaced(p, outward, look, heights),
                outward,
            )
        };
        let top = axis == 1 && side > 0.0 && look.top.is_some();
        let metres = match look.top {
            Some((metres, _)) if top => metres,
            _ => look.metres,
        };
        let into = if top { &mut *tops } else { &mut *out };
        let flip = side < 0.0;
        for i in 0..na {
            for j in 0..nb {
                let corners = [
                    point(i, j),
                    point(i + 1, j),
                    point(i + 1, j + 1),
                    point(i, j + 1),
                ];
                let placed = corners.map(place);
                let mapped = corners.map(|p| planar(p, normal, metres));
                for [x, y, z] in [[0, 1, 2], [0, 2, 3]] {
                    let (y, z) = if flip { (z, y) } else { (y, z) };
                    into.textured(
                        [placed[x].0, placed[y].0, placed[z].0],
                        [placed[x].1, placed[y].1, placed[z].1],
                        [mapped[x], mapped[y], mapped[z]],
                        colour,
                    );
                }
            }
        }
    }
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
            top: None,
        }
    }

    fn flat(_: [f32; 2]) -> f32 {
        0.5
    }

    fn bumpy(uv: [f32; 2]) -> f32 {
        0.5 + 0.5 * libm::sinf(uv[0] * 40.0) * libm::cosf(uv[1] * 37.0)
    }

    fn drawn(cubes: &[[i32; 3]], height: &dyn Fn([f32; 2]) -> f32) -> Surface {
        drawn_as(cubes, &look(), height)
    }

    fn drawn_as(cubes: &[[i32; 3]], look: &Look, height: &dyn Fn([f32; 2]) -> f32) -> Surface {
        let (out, tops) = drawn_with_tops(cubes, look, height);
        assert!(tops.positions.is_empty());
        out
    }

    fn drawn_with_tops(
        cubes: &[[i32; 3]],
        look: &Look,
        height: &dyn Fn([f32; 2]) -> f32,
    ) -> (Surface, Surface) {
        let set: BTreeSet<[i32; 3]> = cubes.iter().copied().collect();
        let (mut out, mut tops) = (Surface::default(), Surface::default());
        let heights = Heights {
            sides: height,
            top: &flat,
        };
        draw(
            &set,
            &|_| false,
            look,
            &heights,
            [1.0; 3],
            &mut out,
            &mut tops,
        );
        (out, tops)
    }

    /// Both surfaces as one, for checking it is closed.
    fn joined(a: Surface, b: &Surface) -> Surface {
        let mut a = a;
        a.positions.extend(&b.positions);
        a.normals.extend(&b.normals);
        a.colours.extend(&b.colours);
        a.uvs.extend(&b.uvs);
        a
    }

    #[test]
    fn a_floor_of_stone_has_its_tops_in_their_own_material_and_stays_closed() {
        let floor: Vec<[i32; 3]> = (0..3)
            .flat_map(|x| (0..2).map(move |z| [x, 0, z]))
            .collect();
        let flagged = Look {
            top: Some(([1.0, 1.0], 0.01)),
            ..look()
        };
        let (sides, tops) = drawn_with_tops(&floor, &flagged, &bumpy);
        assert!(!tops.positions.is_empty() && !sides.positions.is_empty());
        // The tops face up, and none of the sides do but along the rounded edges.
        assert!(tops.normals.iter().all(|n| n.y > 0.3));
        let flat_sides = sides.normals.iter().filter(|n| n.y > 0.99).count();
        assert_eq!(flat_sides, 0);
        assert!(closed(&joined(sides, &tops)));
    }

    #[test]
    fn a_slab_is_closed_rounded_and_its_own_size() {
        let (mut out, mut tops) = (Surface::default(), Surface::default());
        let heights = Heights {
            sides: &bumpy,
            top: &flat,
        };
        let (min, max) = (Vec3::new(0.0, -0.3, 0.0), Vec3::new(1.5, 0.12, 0.32));
        slab(min, max, &look(), &heights, [1.0; 3], &mut out, &mut tops);
        assert!(closed(&out));
        let half = look().relief * 0.5 + 1e-4;
        assert!(
            out.positions
                .iter()
                .all(|p| p.cmpge(min - half).all() && p.cmple(max + half).all())
        );
        // Its corner is rounded off.
        assert!(out.positions.iter().all(|p| p.distance(max) > 0.01));
    }

    /// There are triangles, and every edge of every one is shared by exactly one other
    /// triangle: no cracks.
    fn closed(surface: &Surface) -> bool {
        let key = |p: Vec3| ((p * 2048.0).round()).as_ivec3().to_array();
        let mut edges: std::collections::HashMap<([i32; 3], [i32; 3]), i32> = Default::default();
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
        !edges.is_empty() && edges.values().all(|&count| count == 2)
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
    fn a_side_shows_over_a_lower_cut_beside_it() {
        // A half, and east of it a slope rising east: the slope's west side shows from the
        // half's top up to the slope's.
        let set: BTreeSet<[i32; 3]> = [[0, 0, 0], [1, 0, 0]].into_iter().collect();
        let cut = |cube: IVec3, at: Vec3| {
            if cube.x == 0 {
                CELL_M * 0.5
            } else {
                at.x - CELL_M
            }
        };
        let (mut out, mut tops) = (Surface::default(), Surface::default());
        let heights = Heights {
            sides: &flat,
            top: &flat,
        };
        draw_cut(
            &set,
            &|_| false,
            &look(),
            &heights,
            &cut,
            [1.0; 3],
            &mut out,
            &mut tops,
        );
        let between = out.positions.chunks(3).any(|t| {
            t.iter().all(|p| (p.x - CELL_M).abs() < 0.05) && t.iter().any(|p| p.y > CELL_M * 0.55)
        });
        assert!(
            !between,
            "the slope's side is no higher than its top at its foot"
        );
        // Against a half, a whole cube's own side shows from the half's top up.
        let whole = |cube: IVec3, _: Vec3| if cube.x == 0 { CELL_M * 0.5 } else { f32::MAX };
        let (mut out, mut tops) = (Surface::default(), Surface::default());
        draw_cut(
            &set,
            &|_| false,
            &look(),
            &heights,
            &whole,
            [1.0; 3],
            &mut out,
            &mut tops,
        );
        let side = out.positions.chunks(3).any(|t| {
            t.iter().all(|p| (p.x - CELL_M).abs() < 0.05) && t.iter().all(|p| p.y > CELL_M * 0.49)
        });
        assert!(side, "no side over the half");
        assert!(
            !out.positions.chunks(3).any(|t| {
                t.iter().all(|p| (p.x - CELL_M).abs() < 0.05)
                    && t.iter().all(|p| p.y < CELL_M * 0.45)
            }),
            "a side inside the half"
        );
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
    fn the_relief_moves_the_surface_by_half_its_depth_at_most_and_not_at_an_inside_corner() {
        // An L two cubes high, its inside corner the upright line at x and z of one cube.
        let ell = [
            [0, 0, 0],
            [1, 0, 0],
            [0, 0, 1],
            [0, 1, 0],
            [1, 1, 0],
            [0, 1, 1],
        ];
        let smooth = Look {
            relief: 0.0,
            ..look()
        };
        let (a, b) = (drawn_as(&ell, &smooth, &bumpy), drawn(&ell, &bumpy));
        assert_eq!(a.positions.len(), b.positions.len());
        let moved: Vec<(Vec3, f32)> = a
            .positions
            .iter()
            .zip(&b.positions)
            .map(|(x, y)| (*x, x.distance(*y)))
            .collect();
        // A height from 0 to 1 moves it out or in by half the relief, no more.
        let half = look().relief * 0.5;
        assert!(moved.iter().all(|&(_, by)| by <= half + 1e-5));
        assert!(moved.iter().any(|&(_, by)| by > half * 0.8));
        // Along the inside corner it does not move at all.
        let corner: Vec<f32> = moved
            .iter()
            .filter(|(at, _)| (at.x - CELL_M).abs() < 1e-5 && (at.z - CELL_M).abs() < 1e-5)
            .map(|&(_, by)| by)
            .collect();
        assert!(!corner.is_empty());
        assert!(corner.iter().all(|&by| by < 1e-5), "{corner:?}");
    }
}
