//! Turning a base's cubes into triangles.
//!
//! Walls and pitched roofs of a kind with a kit are left to the kit, which lays its own
//! pieces (`layout`, `timber`, `roofing`); everything else is drawn here: walls of the plain
//! kind as their cubes, a floor or flat roof as its cubes with a floor's colour on top and a
//! ceiling's under it, glass as a pane up the middle of its wall, the gable over a plain
//! wall up to a pitched roof, and a plain roof itself.

use crate::{Base, CELL_M, Fill, IVec3, Kind, STOREY_CUBES, STOREY_M, storey_of, wall_layer};
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
const GLASS: [f32; 3] = [0.55, 0.8, 0.9];

/// How thick a pitched roof is under its covering: its rafters and battens. Walls and their
/// gables stop under it.
pub(crate) const ROOF_THICK_M: f32 = 0.15;
/// How far a pitched roof of no kit overhangs its walls, at its eaves and its gable ends
/// (D175).
const EAVES_M: f32 = 0.5;
/// How finely a pitched roof is sampled, in metres: a quarter of a cell, on which every
/// ridge, hip and valley of a roof over walls in four directions falls, or on its
/// diagonals (research/procedural-buildings.md).
const ROOF_STEP_M: f32 = CELL_M * 0.5;

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
        self.positions.extend(other.positions);
        self.normals.extend(other.normals);
        self.colours.extend(other.colours);
        self.uvs.extend(other.uvs);
        self.shared.append(other.shared);
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

    /// A triangle seen from the side its normal points to, wound counter-clockwise from
    /// there.
    pub(crate) fn triangle(&mut self, corners: [Vec3; 3], normal: Vec3, colour: [f32; 3]) {
        self.positions.extend(corners);
        self.normals.extend([normal; 3]);
        self.colours.extend([colour; 3]);
        self.uvs.extend([[0.0; 2]; 3]);
    }

    /// A triangle with a normal and a place on its texture at each corner.
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

    /// A flat, convex shape seen from the side its normal points to, its corners in order
    /// counter-clockwise from there. Corners that meet, as a gable's do at its foot, are
    /// left out.
    fn polygon(&mut self, corners: &[Vec3], normal: Vec3, colour: [f32; 3]) {
        let first = corners[0];
        for pair in corners[1..].windows(2) {
            let area = (pair[0] - first).cross(pair[1] - first).length();
            if area > 1e-6 {
                self.triangle([first, pair[0], pair[1]], normal, colour);
            }
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
    pub indices: Vec<u32>,
}

impl Indexed {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Another's triangles added to these.
    pub fn append(&mut self, other: Indexed) {
        if self.positions.is_empty() {
            *self = other;
            return;
        }
        let first = self.positions.len() as u32;
        self.positions.extend(other.positions);
        self.normals.extend(other.normals);
        self.colours.extend(other.colours);
        self.uvs.extend(other.uvs);
        self.indices
            .extend(other.indices.into_iter().map(|i| first + i));
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
}

impl Surface {
    /// The same triangles with every corner that is the same in every way, place, normal,
    /// colour and place on the texture, shared.
    pub fn weld(&self) -> Indexed {
        let mut out = Indexed::default();
        let mut seen: std::collections::HashMap<
            [u32; 11],
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
            ];
            let index = *seen.entry(key).or_insert_with(|| {
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
    /// Window glass, to be drawn see-through and from both sides.
    pub glass: Surface,
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
        let tile = IVec3::new(at.x, storey_of(at.y), at.z);
        match cube.fill {
            Fill::Open => continue,
            Fill::Glass => {
                pane(&mut mesh, base, at);
                continue;
            }
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
                    _ if base.contains(tile + out) => INNER_WALL,
                    _ => HULL,
                }
            } else {
                // A floor or flat roof: its top a floor where a room is over it.
                let above = IVec3::new(at.x, storey_of(at.y + 1), at.z);
                match (out.y, &look) {
                    (1, Some(look)) if base.contains(above) => look.floor,
                    (1, Some(look)) => look.roof,
                    (1, None) if base.contains(above) => FLOOR,
                    (1, None) => ROOF,
                    (-1, _) if base.contains(above - IVec3::Y) => CEILING,
                    (_, Some(look)) => look.side,
                    _ => UNDERSIDE,
                }
            };
            let corners = corners.map(|c| low + Vec3::from_array(c) * CELL_M);
            mesh.structure.quad(corners, out.as_vec3(), colour);
        }
    }
    for (tile, _) in base.walls() {
        // A kit lays its own gables (`layout::runs`, `layout::roofs`).
        if base.kind(tile).and_then(&kit).is_none() {
            gable(&mut mesh, base, tile);
        }
    }
    pitched_roofs(&mut mesh, base, &kit);
    mesh
}

/// A pane of glass up the middle of a cube of a wall, across the way the wall runs.
fn pane(mesh: &mut BaseMesh, base: &Base, at: IVec3) {
    let tile = IVec3::new(at.x, storey_of(at.y), at.z);
    let [low_left, low_right, ..] = crate::corners(tile, base.along(tile));
    let bottom = Vec3::Y * ((at.y - wall_layer(tile.y)) as f32 * CELL_M);
    let up = Vec3::Y * CELL_M;
    let normal = (low_right - low_left).cross(Vec3::Y).normalize();
    mesh.glass.quad(
        [
            low_left + bottom,
            low_right + bottom,
            low_right + bottom + up,
            low_left + bottom + up,
        ],
        normal,
        GLASS,
    );
}

/// The wall carried up to a pitched roof over it, as far as the roof's underside, so a
/// gable end is closed and a wall inside meets the slope, as the wall's own faces. In two
/// pieces along the tile, so a ridge over its middle is followed.
fn gable(mesh: &mut BaseMesh, base: &Base, tile: IVec3) {
    let top_layer = wall_layer(tile.y) + STOREY_CUBES - 1;
    if base.cube(IVec3::new(tile.x, top_layer, tile.z)).is_none() {
        return;
    }
    let [origin, right_end, ..] = crate::corners(tile, base.along(tile));
    let right = right_end - origin;
    let normal = right.normalize().cross(Vec3::Y);
    // The roof's underside over a line along the wall so far out from its middle: each
    // face meets the slope where it stands, not where the wall's middle does.
    let rise = |across: f32, out: f32| {
        let at = origin + right * across + normal * out;
        base.roof_rise(tile.y, glam::Vec2::new(at.x, at.z))
            .map(|rise| (rise - ROOF_THICK_M).max(0.0))
    };
    if rise(0.5, 0.0).is_none() {
        return;
    }
    let side = |out: Vec3| {
        if base.contains(tile + out.round().as_ivec3()) {
            INNER_WALL
        } else {
            HULL
        }
    };
    let (depth, front, back) = (CELL_M * 0.5, side(normal), side(-normal));
    let height = origin.y + STOREY_M;
    let at = |across: f32, up: f32, out: f32| (origin + right * across).with_y(up) + normal * out;
    for (out, colour) in [(depth, front), (-depth, back)] {
        let (Some(start), Some(middle), Some(end)) =
            (rise(0.0, out), rise(0.5, out), rise(1.0, out))
        else {
            continue;
        };
        for (left, right, rise_left, rise_right) in
            [(0.0, 0.5, start, middle), (0.5, 1.0, middle, end)]
        {
            if rise_left.max(rise_right) < 0.01 {
                continue;
            }
            let (top_left, top_right) = (height + rise_left, height + rise_right);
            let corners = [
                at(left, height, out),
                at(right, height, out),
                at(right, top_right, out),
                at(left, top_left, out),
            ];
            // Wound counter-clockwise as seen from the side it faces.
            if out > 0.0 {
                mesh.structure.polygon(&corners, normal, colour);
            } else {
                let [a, b, c, d] = corners;
                mesh.structure.polygon(&[b, a, d, c], -normal, colour);
            }
        }
    }
}

/// Every pitched roof of no kit's kind, as its height above the walls sampled on a fine
/// lattice: each small square in four triangles from its middle, so the faces are as flat
/// as the roof. It overhangs its walls by its eaves, carried on down at its pitch, and its
/// edge shows its thickness. A kit lays its own roofs (`layout::roofs`).
fn pitched_roofs(mesh: &mut BaseMesh, base: &Base, kit: &impl Fn(Kind) -> Option<KitLook>) {
    let per = (CELL_M / ROOF_STEP_M).round() as i32;
    let out = (EAVES_M / ROOF_STEP_M).round() as i32;
    let mut squares: BTreeMap<[i32; 3], ()> = BTreeMap::new();
    for (_, roof) in base.roofs() {
        let section = &roof.section;
        let kind = base.roof_kind(roof);
        if kind.and_then(kit).is_some() {
            continue;
        }
        for x in section.min[0] * per - out..(section.max[0] + 1) * per + out {
            for z in section.min[2] * per - out..(section.max[2] + 1) * per + out {
                squares.insert([x, section.storey(), z], ());
            }
        }
    }
    let height = |storey: i32, at: glam::Vec2| {
        base.sections()
            .iter()
            .filter(|section| section.storey() == storey)
            .filter_map(|section| section.rise_beyond(at, EAVES_M))
            .max_by(|a, b| a.total_cmp(b))
    };
    let down = Vec3::Y * ROOF_THICK_M;
    for &[x, storey, z] in squares.keys() {
        let eaves = (storey + 1) as f32 * STOREY_M;
        let at = |dx: f32, dz: f32| {
            let along =
                glam::Vec2::new((x as f32 + dx) * ROOF_STEP_M, (z as f32 + dz) * ROOF_STEP_M);
            let rise = height(storey, along).unwrap_or(0.0);
            Vec3::new(along.x, eaves + rise, along.y)
        };
        let middle = at(0.5, 0.5);
        let corners = [at(0.0, 0.0), at(1.0, 0.0), at(1.0, 1.0), at(0.0, 1.0)];
        for i in 0..4 {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            let normal = (a - middle).cross(b - middle).normalize_or_zero();
            // Wound so that its top faces up.
            let (a, b, normal) = if normal.y < 0.0 {
                (b, a, -normal)
            } else {
                (a, b, normal)
            };
            mesh.structure.triangle([middle, a, b], normal, ROOF);
            mesh.structure
                .triangle([middle - down, b - down, a - down], -normal, CEILING);
        }
        // Its edge, where no square of roof carries on.
        for (step, (a, b)) in [
            ([0, -1], (corners[1], corners[0])),
            ([1, 0], (corners[2], corners[1])),
            ([0, 1], (corners[3], corners[2])),
            ([-1, 0], (corners[0], corners[3])),
        ] {
            if squares.contains_key(&[x + step[0], storey, z + step[1]]) {
                continue;
            }
            let normal = Vec3::new(step[0] as f32, 0.0, step[1] as f32);
            mesh.structure
                .quad([a - down, b - down, b, a], normal, UNDERSIDE);
        }
    }
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
    use crate::{BuildOp, Panel, RoofSection, RoofStyle};

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
        base.apply(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [5, 0, 4],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        base
    }

    /// The top of `room`'s roof at a distance south, worked out by hand: it is 3 m by
    /// 2.5, its gable rising from the north and south.
    fn room_roof(z: f32) -> f32 {
        STOREY_M + z.min(2.5 - z) * 0.7
    }

    /// Checks the walls drawn in `colours` are carried up to the roof's underside over
    /// them and no further, at the gable ends and along the eaves, each face where it
    /// stands; returns their corners and normals.
    fn up_to_the_roof(mesh: &BaseMesh, colours: &[[f32; 3]]) -> Vec<(Vec3, Vec3)> {
        let walls: Vec<(Vec3, Vec3)> = mesh
            .structure
            .positions
            .iter()
            .zip(&mesh.structure.normals)
            .zip(&mesh.structure.colours)
            .filter(|((_, n), colour)| colours.contains(colour) && n.y == 0.0)
            .map(|((p, n), _)| (*p, *n))
            .collect();
        for (p, n) in &walls {
            let under = room_roof(p.z) - ROOF_THICK_M;
            assert!(
                p.y <= STOREY_M + 1e-4 || (p.y - under).abs() < 1e-4,
                "{p} {n} against {under}"
            );
        }
        // Above the walls' tops at the gable ends, and along the eaves.
        for end in [true, false] {
            let above = walls
                .iter()
                .filter(|(p, n)| (n.x.abs() > 0.5) == end && p.y > STOREY_M + 1e-4)
                .count();
            assert!(above > 0, "nothing above the walls, end {end}");
        }
        walls
    }

    #[test]
    fn every_triangle_faces_the_way_its_normal_says() {
        let mesh = room().mesh(|_| None);
        assert!(mesh.structure.triangle_count() > 0 && mesh.glass.triangle_count() > 0);
        for surface in [&mesh.structure, &mesh.glass] {
            for (corners, normal) in surface.positions.chunks(3).zip(surface.normals.chunks(3)) {
                let own = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
                assert!(own.dot(normal[0]) > 0.0, "{corners:?} {normal:?}");
            }
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
    fn plain_walls_are_carried_up_to_the_roof_and_no_further() {
        up_to_the_roof(&room().mesh(|_| None), &[HULL, INNER_WALL]);
    }

    #[test]
    fn walls_and_pitched_roofs_with_a_kit_are_left_to_it() {
        let look = KitLook {
            floor: [0.0; 3],
            roof: [1.0, 0.0, 0.0],
            side: [0.0; 3],
        };
        let plain = room().mesh(|_| None);
        let kitted = room().mesh(|_| Some(look.clone()));
        assert!(kitted.structure.triangle_count() < plain.structure.triangle_count());
        // Neither its roof nor its gables: nothing over the walls' tops.
        assert!(
            plain
                .structure
                .positions
                .iter()
                .any(|p| p.y > STOREY_M + 0.1)
        );
        assert!(
            kitted
                .structure
                .positions
                .iter()
                .all(|p| p.y <= STOREY_M + 1e-4)
        );
        assert!(kitted.textured.0.is_empty());
    }
}
