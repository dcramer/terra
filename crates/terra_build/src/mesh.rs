//! Turning a base's cubes into triangles.
//!
//! Cubes of a kind with a kit are left to the kit, which lays its own pieces (`layout`,
//! `timber`, `roofing`); everything else is drawn here: walls of the plain kind as their
//! cubes, a floor or flat roof as its cubes with a floor's colour on top and a ceiling's
//! under it. Openings, doorways and windows, are holes, not drawn.

use crate::{Base, CELL_M, Fill, IVec3, Kind};
use glam::Vec3;
use std::collections::BTreeMap;

// Colours are as they should appear on screen (sRGB).
const HULL: [f32; 3] = [0.93, 0.9, 0.82];
const INNER_WALL: [f32; 3] = [0.88, 0.82, 0.72];
const TOP: [f32; 3] = [0.84, 0.81, 0.74];
const ROOF: [f32; 3] = [0.74, 0.73, 0.7];
const UNDERSIDE: [f32; 3] = [0.42, 0.43, 0.47];
const FLOOR: [f32; 3] = [0.6, 0.45, 0.33];
const CEILING: [f32; 3] = [0.93, 0.91, 0.86];

/// How thick a pitched roof is under its covering: its rafters and battens. Walls and their
/// gables stop under it.
pub(crate) const ROOF_THICK_M: f32 = 0.15;

/// The colours a kit gives what it does not lay itself: its floors and flat roofs.
#[derive(Clone, Debug, PartialEq)]
pub struct KitLook {
    pub floor: [f32; 3],
    pub roof: [f32; 3],
    /// The sides and undersides of its floors and flat roofs.
    pub side: [f32; 3],
}

/// What a surface is drawn in: plain colour from each triangle, or one of a kit's
/// materials, which the game gives textures (D14).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Material {
    Plain,
    /// A log's side, and a log's sawn end.
    Log,
    LogEnd,
    /// The daub between logs.
    Daub,
    /// Boards: round openings, under and along a roof's edge.
    Board,
    /// A roof's covering: shakes, slates or thatch.
    #[serde(alias = "Shakes")]
    Covering,
    /// A footing's stone.
    Stone,
    /// A floor's planks.
    Floor,
    /// A block kind's surface: stone, brick, plaster, earth (D197).
    Masonry,
    /// Mortar where a roof meets a wall standing above it.
    Flashing,
    /// Dressed stone: the pieces a block kind's edges are dressed with by role (D215).
    Dressed,
    /// Squared timber: the plates, sills and posts a timber kind's edges are dressed with
    /// by role (D215), and a framed kind's frame (D236).
    Hewn,
    /// Woven wattle: a framed kind's infill before it is daubed, or where it is cut into
    /// (D236).
    Wattle,
    /// A kit's modelled pieces (D251, D258): their own textures baked into one atlas, read
    /// by each piece's own UVs. A material's own pieces (its pack's).
    Piece,
    /// A pattern's modelled pieces, shared by its materials (D290): their own textures baked
    /// into the pattern's atlas, as `Piece`.
    Trim,
}

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

    /// Each corner's tangent for its normal map: the way its texture's U runs across its
    /// triangle, square to its normal, and in the fourth the side V runs to, as glTF and
    /// Bevy take them. Worked out a triangle at a time from its corners and their places
    /// on the texture (Lengyel's method), which, as no two triangles share a corner, is as
    /// good as MikkTSpace here and a hundred times quicker on a log wall's 100,000.
    pub fn tangents(&self) -> Vec<[f32; 4]> {
        let mut out = Vec::with_capacity(self.positions.len());
        for (corners, (normals, uvs)) in self
            .positions
            .chunks_exact(3)
            .zip(self.normals.chunks_exact(3).zip(self.uvs.chunks_exact(3)))
        {
            let (e1, e2) = (corners[1] - corners[0], corners[2] - corners[0]);
            let (du1, dv1) = (uvs[1][0] - uvs[0][0], uvs[1][1] - uvs[0][1]);
            let (du2, dv2) = (uvs[2][0] - uvs[0][0], uvs[2][1] - uvs[0][1]);
            let det = du1 * dv2 - du2 * dv1;
            let (u, v) = if det.abs() > 1e-12 {
                let r = 1.0 / det;
                ((e1 * dv2 - e2 * dv1) * r, (e2 * du1 - e1 * du2) * r)
            } else {
                (e1, e1.cross(normals[0]))
            };
            for n in normals {
                let t = (u - *n * n.dot(u)).normalize_or(n.any_orthonormal_vector());
                let w = if n.cross(t).dot(v) < 0.0 { -1.0 } else { 1.0 };
                out.push([t.x, t.y, t.z, w]);
            }
        }
        out
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

    /// Its corners that are the same in every way (place, normal, colour, texture place)
    /// made one, as a mesh optimiser welds them: a surface drawn a triangle at a time
    /// shares nothing, three corners a triangle, and its neighbours in one plane and shade
    /// share most of theirs. What is drawn is the same.
    pub fn welded(self) -> Indexed {
        let mut out = Indexed::default();
        let mut seen: std::collections::HashMap<[u32; 11], u32> =
            std::collections::HashMap::with_capacity(self.positions.len());
        let mut remap = Vec::with_capacity(self.positions.len());
        for i in 0..self.positions.len() {
            let (p, n, c) = (self.positions[i], self.normals[i], self.colours[i]);
            let uv = self.uvs.get(i).copied().unwrap_or([0.0; 2]);
            let key =
                [p.x, p.y, p.z, n.x, n.y, n.z, c[0], c[1], c[2], uv[0], uv[1]].map(f32::to_bits);
            let at = *seen.entry(key).or_insert_with(|| {
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
    /// triangles meeting at a corner each giving theirs and the corner taking their sum.
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

/// FNV-1a over a corner's numbers' bits: quick, and it needs no crate.
#[derive(Default)]
pub(crate) struct Corners(u64);

/// A hash map keyed quickly ([`Corners`]): for places and cubes, never for anything a
/// stranger chooses.
pub(crate) type QuickMap<K, V> =
    std::collections::HashMap<K, V, std::hash::BuildHasherDefault<Corners>>;

impl std::hash::Hasher for Corners {
    fn finish(&self) -> u64 {
        // The low bits of FNV are weak; folded so a table's buckets spread.
        self.0 ^ (self.0 >> 29)
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn write_u32(&mut self, n: u32) {
        self.0 = (self.0 ^ u64::from(n)).wrapping_mul(0x0100_0000_01b3);
    }
    fn write_i32(&mut self, n: i32) {
        self.write_u32(n as u32);
    }
    // A number a step, not a byte a step: a piece is hashed by many of them.
    fn write_u8(&mut self, n: u8) {
        self.write_u32(u32::from(n));
    }
    fn write_u16(&mut self, n: u16) {
        self.write_u32(u32::from(n));
    }
    fn write_u64(&mut self, n: u64) {
        self.write_u32(n as u32);
        self.write_u32((n >> 32) as u32);
    }
    fn write_i64(&mut self, n: i64) {
        self.write_u64(n as u64);
    }
    fn write_usize(&mut self, n: usize) {
        self.write_u64(n as u64);
    }
    fn write_isize(&mut self, n: isize) {
        self.write_u64(n as u64);
    }
}

impl Surface {
    /// The same triangles with every corner that is the same in every way, place, normal,
    /// colour, place on the texture and wear, shared.
    pub fn weld(&self) -> Indexed {
        let mut out = Indexed::default();
        let mut seen: std::collections::HashMap<
            [u32; 12],
            u32,
            std::hash::BuildHasherDefault<Corners>,
        > = std::collections::HashMap::with_capacity_and_hasher(
            self.positions.len() / 3,
            Default::default(),
        );
        out.indices.reserve(self.positions.len());
        let textured = self.uvs.len() == self.positions.len();
        for i in 0..self.positions.len() {
            let (p, n, c) = (self.positions[i], self.normals[i], self.colours[i]);
            let uv = if textured { self.uvs[i] } else { [0.0; 2] };
            let wear = self.wear.get(i).copied().unwrap_or(0.0);
            let key = [
                p.x.to_bits(),
                p.y.to_bits(),
                p.z.to_bits(),
                n.x.to_bits(),
                n.y.to_bits(),
                n.z.to_bits(),
                c[0].to_bits(),
                c[1].to_bits(),
                c[2].to_bits(),
                uv[0].to_bits(),
                uv[1].to_bits(),
                wear.to_bits(),
            ];
            let index = *seen.entry(key).or_insert_with(|| {
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
            out.indices.push(index);
        }
        out
    }
}

/// A base ready to draw, in the base's own space, in metres.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BaseMesh {
    /// Walls, floors, roofs and gables, seen from outside and from inside.
    pub structure: Surface,
    /// What a kit's materials draw: its roofs' covering, edges and undersides.
    pub textured: Surfaces,
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

pub(crate) fn build(base: &Base, kit: impl Fn(Kind) -> Option<KitLook>) -> BaseMesh {
    let mut mesh = BaseMesh::default();
    // What hides a face: a cube drawn as a box beside it.
    let boxed = |at: IVec3| {
        base.cube(at).is_some_and(|cube| {
            cube.fill == Fill::Solid && (!base.in_wall(at) || kit(cube.kind).is_none())
        })
    };
    for (at, cube) in base.cubes() {
        let wall = base.in_wall(at);
        let look = kit(cube.kind);
        match cube.fill {
            Fill::Open => continue,
            // A kit draws its kind's cubes itself: a block kind as its surface, a timber
            // kind's walls as its members and its floors as decks.
            Fill::Solid if look.is_some() => continue,
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
                match (out.y, &look) {
                    (1, Some(look)) if above => look.floor,
                    (1, Some(look)) => look.roof,
                    (1, None) if above => FLOOR,
                    (1, None) => ROOF,
                    (-1, _) if below => CEILING,
                    (_, Some(look)) => look.side,
                    _ => UNDERSIDE,
                }
            };
            let corners = corners.map(|c| low + Vec3::from_array(c) * CELL_M);
            mesh.structure.quad(corners, out.as_vec3(), colour);
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

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
            for t in surface.tangents() {
                assert!(
                    (Vec3::new(t[0], t[1], t[2]) - Vec3::X).length() < 1e-5,
                    "{t:?}"
                );
                let bitangent = Vec3::Y.cross(Vec3::new(t[0], t[1], t[2])) * t[3];
                assert!((bitangent - Vec3::Z * flip).length() < 1e-5, "{t:?}");
            }
        }
    }
    use crate::{BuildOp, Panel};

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
        let mesh = room().mesh(|_| None);
        let surface = &mesh.structure;
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
            cube: Some(crate::Cube::new(Kind::PLAIN, Fill::Solid)),
        });
        // Two cubes side by side: ten faces, not twelve.
        assert_eq!(base.mesh(|_| None).structure.triangle_count(), 20);
    }

    #[test]
    fn walls_with_a_kit_are_left_to_it() {
        let look = KitLook {
            floor: [0.0; 3],
            roof: [1.0, 0.0, 0.0],
            side: [0.0; 3],
        };
        let plain = room().mesh(|_| None);
        let kitted = room().mesh(|_| Some(look.clone()));
        assert!(kitted.structure.triangle_count() < plain.structure.triangle_count());
        assert!(kitted.textured.0.is_empty());
    }
}
