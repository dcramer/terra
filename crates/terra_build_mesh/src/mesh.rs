//! Turning a base's cubes into triangles.
//!
//! Cubes of a kind with a kit are left to the kit, which lays its own pieces (`layout`,
//! `timber`, `roofing`); everything else is drawn here: walls of the plain kind as their
//! cubes, a floor or flat roof as its cubes with a floor's colour on top and a ceiling's
//! under it. Openings, doorways and windows, are holes, not drawn.

use glam::IVec3;
use glam::Vec3;
use std::collections::BTreeMap;
use terra_build::{Base, CELL_M, Fill, Kind};

// Colours are as they should appear on screen (sRGB).
const HULL: [f32; 3] = [0.93, 0.9, 0.82];
const INNER_WALL: [f32; 3] = [0.88, 0.82, 0.72];
const TOP: [f32; 3] = [0.84, 0.81, 0.74];
const ROOF: [f32; 3] = [0.74, 0.73, 0.7];
const UNDERSIDE: [f32; 3] = [0.42, 0.43, 0.47];
const FLOOR: [f32; 3] = [0.6, 0.45, 0.33];
const CEILING: [f32; 3] = [0.93, 0.91, 0.86];

pub use terra_build::pieces::Material;

/// Surfaces by what they are drawn in; the second, whether what is drawn into them may
/// share its corners ([`Surface::sharing`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surfaces(pub BTreeMap<Material, Surface>, pub bool);

impl Surfaces {
    /// Surfaces whose long members share their corners, as a GPU draws best.
    pub fn sharing() -> Surfaces {
        Surfaces(BTreeMap::new(), true)
    }

    pub fn of(&mut self, material: Material) -> &mut Surface {
        let sharing = self.1;
        self.0.entry(material).or_insert_with(|| Surface {
            sharing,
            ..Surface::default()
        })
    }
}

/// Where a point on a flat face falls on a texture so many metres across, mapped from
/// the face's plane, upright on walls.
pub(crate) fn planar(point: Vec3, normal: Vec3, metres: [f32; 2]) -> [f32; 2] {
    let n = normal.abs();
    let (a, b) = if n.y >= n.x && n.y >= n.z {
        (point.x, point.z)
    } else if n.x >= n.z {
        (point.z, -point.y)
    } else {
        (point.x, -point.y)
    };
    [a / metres[0], b / metres[1]]
}

/// Triangles with flat faces: three entries per triangle, no shared corners; and, where
/// it is `sharing`, the long members swept into it with their corners shared (`shared`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surface {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub colours: Vec<[f32; 3]>,
    /// Where each corner falls on its texture, if it has one.
    pub uvs: Vec<[f32; 2]>,
    /// How far each corner's face is worn to what its texture names under it (`worn`: a
    /// log's bark to its bare wood), 0 to 1, by corner; corners past its end are unworn.
    pub wear: Vec<f32>,
    /// Triangles whose corners are shared, drawn with the others.
    pub shared: Indexed,
    /// Whether a long member swept into it shares its corners (`shared`), rather than
    /// giving each triangle its own: a sixth as many to send to the GPU.
    pub sharing: bool,
}

impl Surface {
    pub fn triangle_count(&self) -> usize {
        self.positions.len() / 3 + self.shared.triangle_count()
    }

    /// All of it as shared corners: the loose triangles each their own three, after the
    /// shared ones.
    pub fn indexed(&self) -> Indexed {
        let mut out = self.shared.clone();
        let first = out.positions.len() as u32;
        out.positions.extend(&self.positions);
        out.normals.extend(&self.normals);
        out.colours.extend(&self.colours);
        if !self.wear.is_empty() {
            worn(&mut out.wear, first as usize);
            out.wear.extend(&self.wear);
        }
        if self.uvs.len() == self.positions.len() {
            out.uvs.extend(&self.uvs);
        } else {
            out.uvs
                .extend(std::iter::repeat_n([0.0; 2], self.positions.len()));
        }
        out.indices
            .extend((0..self.positions.len() as u32).map(|i| first + i));
        out
    }

    /// Another surface added to this one.
    pub fn append(&mut self, other: Surface) {
        if self.positions.is_empty() && self.shared.positions.is_empty() {
            let sharing = self.sharing;
            *self = other;
            self.sharing = sharing;
            return;
        }
        if !other.wear.is_empty() {
            worn(&mut self.wear, self.positions.len());
            self.wear.extend(other.wear);
        }
        self.positions.extend(other.positions);
        self.normals.extend(other.normals);
        self.colours.extend(other.colours);
        self.uvs.extend(other.uvs);
        self.shared.append(other.shared);
    }

    /// Another's triangles added to its own, copied from where they are.
    pub fn extend_from(&mut self, other: &Surface) {
        if !other.wear.is_empty() {
            worn(&mut self.wear, self.positions.len());
            self.wear.extend_from_slice(&other.wear);
        }
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.colours.extend_from_slice(&other.colours);
        self.uvs.extend_from_slice(&other.uvs);
        self.shared.extend_from(&other.shared);
    }

    /// Triangles with shared corners added: kept shared where it shares its corners, else
    /// as loose triangles ([`Indexed::loose`]), their wear with them.
    pub(crate) fn add(&mut self, made: Indexed) {
        if self.sharing {
            self.shared.append(made);
        } else {
            self.append(made.loose());
        }
    }

    /// A triangle with a normal and a place on its texture at each corner.
    /// Room for so many more corners of loose triangles.
    pub(crate) fn reserve(&mut self, corners: usize) {
        self.positions.reserve(corners);
        self.normals.reserve(corners);
        self.colours.reserve(corners);
        self.uvs.reserve(corners);
    }

    pub(crate) fn textured(
        &mut self,
        corners: [Vec3; 3],
        normals: [Vec3; 3],
        uvs: [[f32; 2]; 3],
        colour: [f32; 3],
    ) {
        self.positions.extend(corners);
        self.normals.extend(normals);
        self.colours.extend([colour; 3]);
        self.uvs.extend(uvs);
    }

    /// A flat rectangle, its texture mapped from its plane at so many metres across.
    pub(crate) fn planar_quad(
        &mut self,
        corners: [Vec3; 4],
        normal: Vec3,
        metres: [f32; 2],
        colour: [f32; 3],
    ) {
        let [a, b, c, d] = corners;
        for triangle in [[a, b, c], [a, c, d]] {
            let uvs = triangle.map(|point| planar(point, normal, metres));
            self.textured(triangle, [normal; 3], uvs, colour);
        }
    }

    /// A rectangle seen from the side its normal points to. Corners go low-left,
    /// low-right, high-right, high-left as seen from that side.
    pub(crate) fn quad(&mut self, corners: [Vec3; 4], normal: Vec3, colour: [f32; 3]) {
        let [a, b, c, d] = corners;
        self.positions.extend([a, b, c, a, c, d]);
        self.normals.extend([normal; 6]);
        self.colours.extend([colour; 6]);
        self.uvs.extend([[0.0; 2]; 6]);
    }
}

/// A surface with its corners shared between the triangles that meet at them, as a GPU
/// draws it best: a sixth as many corners as a log's triangles have, and the indices of
/// each triangle's three.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Indexed {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub colours: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// As [`Surface::wear`].
    pub wear: Vec<f32>,
    pub indices: Vec<u32>,
}

/// Wear given to every corner up to `corners`, those not given it unworn: so wear can be
/// added after corners that had none, and corners added after it that have none.
pub(crate) fn worn(wear: &mut Vec<f32>, corners: usize) {
    if wear.len() < corners {
        wear.resize(corners, 0.0);
    }
}

impl Indexed {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Its triangles each with corners of their own, wear and all: the other way from
    /// [`Surface::indexed`].
    pub fn loose(&self) -> Surface {
        let mut out = Surface::default();
        out.reserve(self.indices.len());
        let worn = !self.wear.is_empty();
        for &i in &self.indices {
            let i = i as usize;
            out.positions.push(self.positions[i]);
            out.normals.push(self.normals[i]);
            out.colours.push(self.colours[i]);
            out.uvs.push(self.uvs.get(i).copied().unwrap_or([0.0; 2]));
            if worn {
                out.wear.push(self.wear.get(i).copied().unwrap_or(0.0));
            }
        }
        out
    }

    /// Its corners that are the same in every way (place, normal, colour, texture place, wear)
    /// made one, as a mesh optimiser welds them: a surface drawn a triangle at a time
    /// shares nothing, three corners a triangle, and its neighbours in one plane and shade
    /// share most of theirs. What is drawn is the same.
    pub fn welded(self) -> Indexed {
        let mut out = Indexed::default();
        let mut seen: std::collections::HashMap<
            [u32; 12],
            u32,
            std::hash::BuildHasherDefault<Welding>,
        > = std::collections::HashMap::with_capacity_and_hasher(
            self.positions.len(),
            Default::default(),
        );
        let mut remap = Vec::with_capacity(self.positions.len());
        for i in 0..self.positions.len() {
            let (p, n, c) = (self.positions[i], self.normals[i], self.colours[i]);
            let uv = self.uvs.get(i).copied().unwrap_or([0.0; 2]);
            let wear = self.wear.get(i).copied().unwrap_or(0.0);
            let key = [
                p.x, p.y, p.z, n.x, n.y, n.z, c[0], c[1], c[2], uv[0], uv[1], wear,
            ]
            .map(f32::to_bits);
            let at = *seen.entry(key).or_insert_with(|| {
                if wear > 0.0 {
                    worn(&mut out.wear, out.positions.len());
                    out.wear.push(wear);
                }
                out.positions.push(p);
                out.normals.push(n);
                out.colours.push(c);
                out.uvs.push(uv);
                (out.positions.len() - 1) as u32
            });
            remap.push(at);
        }
        out.indices = self.indices.iter().map(|&i| remap[i as usize]).collect();
        out
    }

    /// Another's triangles added to these.
    pub fn append(&mut self, other: Indexed) {
        if self.positions.is_empty() {
            *self = other;
            return;
        }
        let first = self.positions.len() as u32;
        if !other.wear.is_empty() {
            worn(&mut self.wear, self.positions.len());
            self.wear.extend(other.wear);
        }
        self.positions.extend(other.positions);
        self.normals.extend(other.normals);
        self.colours.extend(other.colours);
        self.uvs.extend(other.uvs);
        self.indices
            .extend(other.indices.into_iter().map(|i| first + i));
    }

    /// Another's corners and triangles added to its own, copied from where they are.
    pub fn extend_from(&mut self, other: &Indexed) {
        let first = self.positions.len() as u32;
        if !other.wear.is_empty() {
            worn(&mut self.wear, self.positions.len());
            self.wear.extend_from_slice(&other.wear);
        }
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.colours.extend_from_slice(&other.colours);
        self.uvs.extend_from_slice(&other.uvs);
        self.indices.extend(other.indices.iter().map(|i| first + i));
    }

    /// Room for so many more corners, and indices of them.
    pub(crate) fn reserve(&mut self, corners: usize, indices: usize) {
        self.positions.reserve(corners);
        self.normals.reserve(corners);
        self.uvs.reserve(corners);
        self.colours.reserve(corners);
        self.indices.reserve(indices);
    }

    /// A corner worn so far (`wear`), and its index.
    pub(crate) fn worn_corner(
        &mut self,
        p: Vec3,
        n: Vec3,
        uv: [f32; 2],
        colour: [f32; 3],
        wear: f32,
    ) -> u32 {
        if wear > 0.0 {
            worn(&mut self.wear, self.positions.len());
            self.wear.push(wear);
        }
        self.corner(p, n, uv, colour)
    }

    /// A corner, and its index.
    pub(crate) fn corner(&mut self, p: Vec3, n: Vec3, uv: [f32; 2], colour: [f32; 3]) -> u32 {
        self.positions.push(p);
        self.normals.push(n);
        self.uvs.push(uv);
        self.colours.push(colour);
        (self.positions.len() - 1) as u32
    }

    /// Each corner's tangent for its normal map, as [`Surface::tangents`] gives them, the
    /// triangles meeting at a corner each giving theirs and the corner taking their sum, each
    /// triangle's ways along its texture as directions weighted by its area, as MikkTSpace
    /// weighs them: summed as Lengyel's unnormalised, a sliver whose texture barely changes
    /// across it, as on a rounded edge mapped from the face beside it, outweighed every
    /// other and turned the frame relief mapping steps through (streaks down a footing's
    /// rounded edge, research/kit-review-2026-10-14.md).
    pub fn tangents(&self) -> Vec<[f32; 4]> {
        let mut u = vec![Vec3::ZERO; self.positions.len()];
        let mut v = vec![Vec3::ZERO; self.positions.len()];
        for t in self.indices.chunks_exact(3) {
            let [a, b, c] = [t[0] as usize, t[1] as usize, t[2] as usize];
            let (e1, e2) = (
                self.positions[b] - self.positions[a],
                self.positions[c] - self.positions[a],
            );
            let (du1, dv1) = (
                self.uvs[b][0] - self.uvs[a][0],
                self.uvs[b][1] - self.uvs[a][1],
            );
            let (du2, dv2) = (
                self.uvs[c][0] - self.uvs[a][0],
                self.uvs[c][1] - self.uvs[a][1],
            );
            let det = du1 * dv2 - du2 * dv1;
            if det.abs() <= 1e-12 {
                continue;
            }
            let r = 1.0 / det;
            let (tu, tv) = ((e1 * dv2 - e2 * dv1) * r, (e2 * du1 - e1 * du2) * r);
            let area = e1.cross(e2).length();
            let (tu, tv) = (tu.normalize_or_zero() * area, tv.normalize_or_zero() * area);
            for i in [a, b, c] {
                u[i] += tu;
                v[i] += tv;
            }
        }
        self.normals
            .iter()
            .zip(u.iter().zip(&v))
            .map(|(n, (u, v))| {
                let t = (*u - *n * n.dot(*u)).normalize_or(n.any_orthonormal_vector());
                let w = if n.cross(t).dot(*v) < 0.0 { -1.0 } else { 1.0 };
                [t.x, t.y, t.z, w]
            })
            .collect()
    }
}

/// A simple polygon's triangles by ear clipping, as indices into its points; what is left
/// when no ear is found, as of a polygon that crosses itself, a fan from its first point.
pub(crate) fn ear_clip(points: &[glam::Vec2]) -> Vec<[usize; 3]> {
    let twice_area: f32 = (0..points.len())
        .map(|i| points[i].perp_dot(points[(i + 1) % points.len()]))
        .sum();
    let mut left: Vec<usize> = (0..points.len()).collect();
    if twice_area < 0.0 {
        left.reverse();
    }
    let cross =
        |a: usize, b: usize, c: usize| (points[b] - points[a]).perp_dot(points[c] - points[a]);
    let inside = |p: glam::Vec2, [a, b, c]: [usize; 3]| {
        let (pa, pb, pc) = (points[a], points[b], points[c]);
        (pb - pa).perp_dot(p - pa) >= 0.0
            && (pc - pb).perp_dot(p - pb) >= 0.0
            && (pa - pc).perp_dot(p - pc) >= 0.0
    };
    let mut out = Vec::new();
    while left.len() > 3 {
        let n = left.len();
        let ear = (0..n).find(|&i| {
            let t = [left[(i + n - 1) % n], left[i], left[(i + 1) % n]];
            cross(t[0], t[1], t[2]) > 1e-12
                && left.iter().filter(|&&k| !t.contains(&k)).all(|&k| {
                    !inside(points[k], t) || points[k] == points[t[0]] || points[k] == points[t[2]]
                })
        });
        let Some(i) = ear else {
            // Collinear points left, or one crossing itself: a flat corner first, else a fan.
            if let Some(i) = (0..n)
                .find(|&i| cross(left[(i + n - 1) % n], left[i], left[(i + 1) % n]).abs() <= 1e-12)
            {
                left.remove(i);
                continue;
            }
            for k in 1..n - 1 {
                out.push([left[0], left[k], left[k + 1]]);
            }
            return out;
        };
        out.push([left[(i + n - 1) % n], left[i], left[(i + 1) % n]]);
        left.remove(i);
    }
    if left.len() == 3 && cross(left[0], left[1], left[2]).abs() > 1e-12 {
        out.push([left[0], left[1], left[2]]);
    }
    out
}

/// A corner's twelve numbers hashed eight bytes at a step, multiplied and folded, for welding
/// ([`Indexed::welded`]), which hashes every corner of every mesh made: the standard
/// library's hash, made to withstand keys chosen against it, cost a fifth more there.
#[derive(Default)]
pub(crate) struct Welding(u64);

impl std::hash::Hasher for Welding {
    fn finish(&self) -> u64 {
        let h = self.0;
        (h ^ (h >> 32)).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (h >> 29)
    }
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            let word = u64::from_le_bytes(chunk.try_into().expect("eight bytes"));
            self.0 = (self.0.rotate_left(23) ^ word).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        }
        for &byte in chunks.remainder() {
            self.0 = (self.0.rotate_left(23) ^ u64::from(byte)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        }
    }
}

/// The six faces of a cube: which way each faces, and its corners as seen from there,
/// on a cube from 0 to 1.
const FACES: [([i32; 3], [[f32; 3]; 4]); 6] = [
    (
        [1, 0, 0],
        [[1., 0., 1.], [1., 0., 0.], [1., 1., 0.], [1., 1., 1.]],
    ),
    (
        [-1, 0, 0],
        [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]],
    ),
    (
        [0, 1, 0],
        [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]],
    ),
    (
        [0, -1, 0],
        [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
    ),
    (
        [0, 0, 1],
        [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]],
    ),
    (
        [0, 0, -1],
        [[1., 0., 0.], [0., 0., 0.], [0., 1., 0.], [1., 1., 0.]],
    ),
];

/// The base as triangles, ready to draw: cubes of a kind with a kit, which `kitted` says,
/// are left to the kit.
pub fn mesh(base: &Base, kitted: impl Fn(Kind) -> bool) -> Surface {
    build(base, kitted)
}

/// The base as triangles, its cubes of a kind with a kit left to the kit.
pub fn mesh_for_kits(base: &Base, kits: &terra_build::kit::Kits) -> Surface {
    build(base, |kind| kits.of(kind).is_some())
}

/// The base's cubes of kinds no kit draws (`kitted` says which a kit draws) as plain
/// coloured boxes.
pub(crate) fn build(base: &Base, kitted: impl Fn(Kind) -> bool) -> Surface {
    let mut mesh = Surface::default();
    // What hides a face: a cube drawn as a box beside it.
    let boxed = |at: IVec3| {
        base.cube(at).is_some_and(|cube| {
            cube.fill == Fill::Solid && (!base.in_wall(at) || !kitted(cube.kind))
        })
    };
    for (at, cube) in base.cubes() {
        let wall = base.in_wall(at);
        match cube.fill {
            Fill::Open => continue,
            // A kit draws its kind's cubes itself: a block kind as its surface, a timber
            // kind's walls as its members and its floors as decks.
            Fill::Solid if kitted(cube.kind) => continue,
            Fill::Solid => {}
        }
        let low = at.as_vec3() * CELL_M;
        for (out, corners) in FACES {
            let out = IVec3::from_array(out);
            if boxed(at + out) {
                continue;
            }
            let colour = if wall {
                match out.y {
                    1 => TOP,
                    -1 => HULL,
                    _ if base.contains(at + out) => INNER_WALL,
                    _ => HULL,
                }
            } else {
                // A floor or flat roof: its top a floor where a room is over it, its
                // underside a ceiling where one is under it (D261).
                let (above, below) = (base.contains(at + IVec3::Y), base.contains(at - IVec3::Y));
                match out.y {
                    1 if above => FLOOR,
                    1 => ROOF,
                    -1 if below => CEILING,
                    _ => UNDERSIDE,
                }
            };
            let corners = corners.map(|c| low + Vec3::from_array(c) * CELL_M);
            mesh.quad(corners, out.as_vec3(), colour);
        }
    }
    mesh
}

/// How much darker a cube that has lost all its health is drawn, and the most of its
/// surface broken into dark flecks then.
const WORN_DARK: f32 = 0.34;
const WORN_FLECKS: f32 = 0.5;
/// How dark a fleck is, and how far across, in metres.
const FLECK_DARK: f32 = 0.42;
const FLECK_M: f32 = 0.07;

/// Wears a drawn surface where its cubes are damaged, by how many quarters of their health
/// they have lost: darker, and broken up in dark flecks that spread as more is lost, as a
/// battered wall is scorched, split and chipped, in whatever material it is (D14): each
/// corner by the cube it is part of, a little in from its face.
pub fn wear(surface: &mut Surface, worn: &[([i32; 3], u8)]) {
    let worn: BTreeMap<[i32; 3], u8> = worn.iter().copied().collect();
    let shade = |at: glam::Vec3, normal: glam::Vec3| -> f32 {
        let inside = at - normal * 0.02;
        let cube = (inside / terra_build::CELL_M).floor().as_ivec3().to_array();
        let Some(&quarters) = worn.get(&cube) else {
            return 1.0;
        };
        let lost = f32::from(quarters.min(4)) / 4.0;
        let fleck = (at / FLECK_M).floor().as_ivec3();
        let fleck = if terra_gen::hash::unit3(0, fleck.x as u32, fleck.y as u32, fleck.z as u32)
            < lost * WORN_FLECKS
        {
            FLECK_DARK
        } else {
            1.0
        };
        (1.0 - WORN_DARK * lost) * fleck
    };
    let darken = |colour: &mut [f32; 3], by: f32| {
        for channel in colour.iter_mut() {
            *channel *= by;
        }
    };
    for index in 0..surface.positions.len() {
        let normal = surface.normals.get(index).copied().unwrap_or(glam::Vec3::Y);
        let by = shade(surface.positions[index], normal);
        if let Some(colour) = surface.colours.get_mut(index) {
            darken(colour, by);
        }
    }
    let shared = &mut surface.shared;
    for index in 0..shared.positions.len() {
        let normal = shared.normals.get(index).copied().unwrap_or(glam::Vec3::Y);
        let by = shade(shared.positions[index], normal);
        if let Some(colour) = shared.colours.get_mut(index) {
            darken(colour, by);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welding_keeps_each_corners_wear_and_parts_corners_that_differ_by_it_alone() {
        // Two triangles sharing an edge, one worn through and one not, as a hole's side
        // meets the face round it.
        let mut surface = Surface::default();
        let (a, b) = (Vec3::ZERO, Vec3::X);
        surface.textured([a, b, Vec3::Z], [Vec3::Y; 3], [[0.0; 2]; 3], [1.0; 3]);
        surface.textured([b, a, -Vec3::Z], [Vec3::Y; 3], [[0.0; 2]; 3], [1.0; 3]);
        worn(&mut surface.wear, 3);
        surface.wear.extend([1.0; 3]);
        let welded = surface.indexed().welded();
        let worn_at = |i: usize| welded.wear.get(i).copied().unwrap_or(0.0);
        let first: Vec<f32> = welded.indices[..3]
            .iter()
            .map(|&i| worn_at(i as usize))
            .collect();
        let second: Vec<f32> = welded.indices[3..]
            .iter()
            .map(|&i| worn_at(i as usize))
            .collect();
        assert_eq!(first, [0.0; 3]);
        assert_eq!(second, [1.0; 3]);
    }

    #[test]
    fn shared_corners_made_loose_keep_their_wear() {
        let mut made = Indexed::default();
        let a = made.corner(Vec3::ZERO, Vec3::Y, [0.0; 2], [1.0; 3]);
        let b = made.worn_corner(Vec3::X, Vec3::Y, [1.0, 0.0], [1.0; 3], 0.5);
        let c = made.worn_corner(Vec3::Z, Vec3::Y, [0.0, 1.0], [1.0; 3], 1.0);
        made.indices.extend([a, c, b]);
        let loose = made.loose();
        assert_eq!(loose.positions, [Vec3::ZERO, Vec3::Z, Vec3::X]);
        assert_eq!(loose.wear, [0.0, 1.0, 0.5]);
        // Into a surface that does not share its corners, after triangles with none worn.
        let mut surface = Surface::default();
        surface.textured([Vec3::Y; 3], [Vec3::Y; 3], [[0.0; 2]; 3], [1.0; 3]);
        surface.add(made);
        assert_eq!(surface.positions.len(), 6);
        assert_eq!(surface.wear, [0.0, 0.0, 0.0, 0.0, 1.0, 0.5]);
    }

    #[test]
    fn ear_clipping_covers_a_polygon_whatever_its_winding_or_points_in_a_line() {
        use glam::Vec2;
        let area = |p: &[Vec2]| {
            (0..p.len())
                .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
                .sum::<f32>()
                .abs()
                * 0.5
        };
        let covered = |p: &[Vec2]| {
            ear_clip(p)
                .iter()
                .map(|&[a, b, c]| (p[b] - p[a]).perp_dot(p[c] - p[a]).abs() * 0.5)
                .sum::<f32>()
        };
        let square = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]].map(Vec2::from);
        let ell = [
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [0.0, 2.0],
        ]
        .map(Vec2::from);
        // A rectangle with corners in a line along its foot, as a cut piece's section has.
        let lined = [
            [0.0, 0.0],
            [1.0, 0.0],
            [2.0, 0.0],
            [3.0, 0.0],
            [3.0, 1.0],
            [0.0, 1.0],
        ]
        .map(Vec2::from);
        assert_eq!(ear_clip(&square).len(), 2);
        assert_eq!(ear_clip(&ell).len(), 4);
        for polygon in [square.to_vec(), ell.to_vec(), lined.to_vec()] {
            let mut backwards = polygon.clone();
            backwards.reverse();
            for p in [polygon, backwards] {
                assert!((covered(&p) - area(&p)).abs() < 1e-5, "{p:?}");
            }
        }
    }

    #[test]
    fn a_tangent_runs_the_way_the_texture_does_square_to_the_normal() {
        // A triangle facing up, its texture's U along X and V along Z; and the same with V
        // flipped. The bitangent, the normal crossed with the tangent and turned by its
        // fourth, as glTF and Bevy make it, runs the way V does.
        for flip in [1.0f32, -1.0] {
            let mut surface = Surface::default();
            let corners = [
                Vec3::ZERO,
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(1.0, 0.0, 1.0),
            ];
            let uvs = corners.map(|p| [p.x, p.z * flip]);
            surface.textured(corners, [Vec3::Y; 3], uvs, [1.0; 3]);
            for t in surface.indexed().tangents() {
                assert!(
                    (Vec3::new(t[0], t[1], t[2]) - Vec3::X).length() < 1e-5,
                    "{t:?}"
                );
                let bitangent = Vec3::Y.cross(Vec3::new(t[0], t[1], t[2])) * t[3];
                assert!((bitangent - Vec3::Z * flip).length() < 1e-5, "{t:?}");
            }
        }
    }
    use terra_build::{BuildOp, Panel};

    fn room() -> Base {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [5, 0, 4],
            kind: Kind::PLAIN,
        });
        base.apply(BuildOp::SetWall {
            tile: [2, 0, 4],
            panel: Some(Panel::WINDOW),
            kind: Kind::PLAIN,
        });
        base
    }

    #[test]
    fn every_triangle_faces_the_way_its_normal_says() {
        let mesh = mesh(&room(), |_| false);
        let surface = &mesh;
        assert!(surface.triangle_count() > 0);
        for (corners, normal) in surface.positions.chunks(3).zip(surface.normals.chunks(3)) {
            let own = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
            assert!(own.dot(normal[0]) > 0.0, "{corners:?} {normal:?}");
        }
    }

    #[test]
    fn faces_between_two_cubes_are_not_drawn() {
        let mut base = Base::default();
        base.apply(BuildOp::Fill {
            min: [0, 0, 0],
            max: [1, 0, 0],
            cube: Some(terra_build::Cube::new(Kind::PLAIN, Fill::Solid)),
        });
        // Two cubes side by side: ten faces, not twelve.
        assert_eq!(mesh(&base, |_| false).triangle_count(), 20);
    }

    #[test]
    fn walls_with_a_kit_are_left_to_it() {
        let plain = mesh(&room(), |_| false);
        let kitted = mesh(&room(), |_| true);
        assert!(kitted.triangle_count() < plain.triangle_count());
    }

    #[test]
    fn a_damaged_cubes_surface_is_drawn_darker_the_more_it_has_lost_and_no_other() {
        // A face of the cube at the origin and one of the cube two along, each a fine grid of
        // triangles, as a wall's surface is.
        let face = |x0: f32| {
            let mut surface = Surface::default();
            for i in 0..10 {
                for j in 0..10 {
                    let (u, v) = (x0 + i as f32 * 0.05, j as f32 * 0.05);
                    let corners = [
                        glam::Vec3::new(u, v, 0.5),
                        glam::Vec3::new(u + 0.05, v, 0.5),
                        glam::Vec3::new(u + 0.05, v + 0.05, 0.5),
                        glam::Vec3::new(u, v + 0.05, 0.5),
                    ];
                    surface.quad(corners, glam::Vec3::Z, [0.8; 3]);
                }
            }
            surface
        };
        let mean = |surface: &Surface| {
            surface.colours.iter().map(|c| c[0]).sum::<f32>() / surface.colours.len() as f32
        };
        let worn = |quarters: u8, x0: f32| {
            let mut surface = face(x0);
            wear(&mut surface, &[([0, 0, 0], quarters)]);
            mean(&surface)
        };
        // Untouched where it is not damaged; darker by quarters lost where it is.
        assert!((worn(4, 1.0) - 0.8).abs() < 1e-5);
        assert!(worn(1, 0.0) < 0.8);
        assert!(worn(2, 0.0) < worn(1, 0.0));
        assert!(worn(4, 0.0) < worn(2, 0.0));
        assert!(worn(4, 0.0) > 0.2, "still its material, only worn");
    }
}
