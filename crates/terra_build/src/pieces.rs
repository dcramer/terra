//! What a building's pieces are, as its rules and its kits name them and its layout and
//! drawing read them (D407): the role of a cube's edge, a long piece of roof and the stage of
//! its roof cube it goes up in, the material a piece is drawn in, how a prop hangs in its hole
//! and how a kind breaks; how big a member laid for a role is, from its kit's numbers; and a
//! kit's modelled pieces as read from their files, made fit to lay as an engine's import
//! cleans a mesh. Plain data and the rules' own arithmetic, so the rules name none of what lays
//! or draws them.

use crate::Cube;
use crate::kit::{Kit, Profile};
use glam::Vec3;
use std::collections::BTreeMap;

/// What an edge of a cube is, by where it is (the module's comment).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Role {
    Arris,
    Jamb,
    Inside,
    Crest,
    Nose,
    Sill,
    Head,
    Soffit,
    Foot,
    Border,
    Meet,
    /// Not an edge: a post's shaft, up a column of cubes cut as posts (D193), as the kit
    /// dresses it: a member of its timber, or a pillar of its stone.
    Post,
}

impl Role {
    pub const ALL: [Role; 12] = [
        Role::Arris,
        Role::Jamb,
        Role::Inside,
        Role::Crest,
        Role::Nose,
        Role::Sill,
        Role::Head,
        Role::Soffit,
        Role::Foot,
        Role::Border,
        Role::Meet,
        Role::Post,
    ];

    /// Whether it runs up and down, rather than level.
    pub fn upright(self) -> bool {
        matches!(self, Role::Arris | Role::Jamb | Role::Inside)
    }
}

/// How many stages a roof cube goes up in: its frame, its battens, its covering and its
/// trims, a step of its fill each (`kit.cost.steps`).
pub const ROOF_STAGES: u8 = 4;

/// How thick a pitched roof is under its covering: its rafters and battens. Walls and their
/// gables stop under it, and a walker stands on it where it is drawn (`walk`).
pub const ROOF_THICK_M: f32 = 0.15;

/// How far a roof cube is up, by its fill: nothing, then its frame, battens, covering and
/// trims, a quarter of the cube each.
pub fn stage_of(cube: &Cube) -> u8 {
    let level = u16::from(cube.level());
    (level * u16::from(ROOF_STAGES)).div_ceil(16) as u8
}

/// What a long piece of roof is.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum RoofPart {
    /// The frame: a rafter up a slope, or the beam under a ridge, hip or valley.
    Rafter,
    /// A batten across the rafters under the foot of a course.
    Batten,
    /// Along an eave: a fascia, or thatch's cut butt.
    Eave,
    /// The board along a verge, a gable's sloping edge: the bargeboard.
    Verge,
    /// The cap over a ridge.
    Ridge,
    /// The cap over a hip.
    Hip,
    /// The lining of a valley.
    Valley,
    /// Where the roof rises to something standing on or beside it along a level line: an
    /// apron, as under a chimney or along a lean-to's head.
    Flashing,
    /// Where the roof meets something standing on or beside it along a sloping line: step
    /// flashing over soakers, up a chimney's or a dormer cheek's sides.
    Step,
    /// Where the roof falls to something standing on or beside it along a level line: a
    /// back gutter, above a chimney.
    Gutter,
}

impl RoofPart {
    /// The stage of its roof cube it goes up in.
    pub fn stage(self) -> u8 {
        match self {
            RoofPart::Rafter => 1,
            RoofPart::Batten => 2,
            _ => 4,
        }
    }
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
    /// A block kind's surface: stone, plaster, earth (D197).
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

/// How a prop hangs in its hole: as a door, or as shutters.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Hang {
    #[default]
    Door,
    Shutters,
}

/// How a kind breaks (D305): data on its kit, by its pattern unless its material says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Breaks {
    /// Chunky irregular blocks: stone, brick.
    #[default]
    Chunks,
    /// Lengths and splinters: logs, timber.
    Lengths,
    /// Small flat bits: wattle and plaster, a roof's covering.
    Bits,
    /// Crumbling lumps: packed earth.
    Crumbs,
}

/// A hewn member's half width, as a fraction of its kit's log.
pub const HEWN_WIDE: f32 = 0.45;

/// How wide and how high a member laid for a role is as this code draws it, before its seed
/// varies it a little, in metres: what a layout needs to set what rests on it, or what it
/// rests on, against its faces (the asset contract: a piece's real size, from its kit's data,
/// with no offsets kept for a kit in code). A role the kit does not dress as a member is
/// drawn as a log of the kit's own.
pub fn section(kit: &Kit, role: Option<Role>) -> (f32, f32) {
    let (profile, scale) = role
        .and_then(|role| kit.dress.member(role))
        .map_or((kit.timber.profile, 1.0), |(profile, scale, _)| {
            (profile, scale)
        });
    let diameter = kit.diameter * scale;
    match profile {
        Profile::Round => (diameter * 1.03, diameter * 1.03),
        // What rests on a beam is set a hair into its flat bed, not on it, so the two faces
        // are not drawn in one plane.
        Profile::Hewn => (diameter * HEWN_WIDE * 2.0, hewn_high(kit) * 2.0),
    }
}

/// A hewn member's half height: half a course less its joint, so the daub between courses
/// shows as a line `joint` tall.
pub fn hewn_high(kit: &Kit) -> f32 {
    ((kit.course - kit.timber.joint.max(0.0)) * 0.5).max(0.01)
}

/// How many times its length each copy of a module is drawn far off (a ridge's caps, an
/// eave's butts, a hip's): a ridge tile 30 cm long is a few pixels at 60 m and more, so
/// copies twice as long read the same and a line has half as many.
pub const FAR_REPEAT: f32 = 2.0;

/// A kit's pieces, as `tools/kits/pieces.py` writes them from their Blender source.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Pieces {
    /// Sections by name, as points (out, up) in metres from the edge they are swept along:
    /// out from its face, up from its line.
    pub profiles: BTreeMap<String, Vec<[f32; 2]>>,
    /// Modules by name (D258, `modules`): lengths of mesh repeated along a line.
    pub modules: BTreeMap<String, Module>,
}

/// A module (D258): a length of mesh modelled along +X from 0 to `length`, Y up from what
/// it sits on and Z across, laid along a line in whole copies (`modules::lay`); several
/// variants, picked a copy at a time by its seed, each also at a far budget; and a cap,
/// modelled past +X's end, closing an end where its line stops.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Module {
    pub length: f32,
    /// Its contract (D290, D303): its real size, along (X), up (Y) and across (Z), in
    /// metres, from its origin, its anchor, where the layout seats its line (a roof's deck or
    /// covering, a wall's face); the model it was made from, if modelled, which is this size
    /// wherever it is used; and its named sockets in its own frame (`start` and `end`).
    pub dims: [f32; 3],
    pub model: Option<String>,
    pub sockets: BTreeMap<String, [f32; 3]>,
    /// Whether it is its pattern's, shared by its materials and drawn in the pattern's atlas
    /// (`Material::Trim`), rather than a material's own (`Material::Piece`); set as read.
    #[serde(skip)]
    pub shared: bool,
    /// Over a crease, how far either side its up turns from one plane's to the other's
    /// (`modules::Line::blend`).
    pub blend: f32,
    pub near: Vec<Stone>,
    pub far: Vec<Stone>,
    pub cap: Option<Stone>,
}

impl Module {
    /// How big its section is, up (Y) and across (Z), as modelled: what a member scaled to
    /// its frame's size is scaled from.
    pub fn size(&self) -> [f32; 2] {
        let mut low = [f32::MAX; 2];
        let mut high = [f32::MIN; 2];
        for p in self.near.iter().flat_map(|m| &m.positions) {
            for (k, axis) in [1, 2].into_iter().enumerate() {
                low[k] = low[k].min(p[axis]);
                high[k] = high[k].max(p[axis]);
            }
        }
        [0, 1].map(|k| (high[k] - low[k]).max(0.0))
    }

    /// The most triangles one copy has near, its cap's apart.
    pub fn triangles(&self) -> usize {
        self.near
            .iter()
            .map(|m| m.triangles.len())
            .max()
            .unwrap_or(0)
    }
}

/// A mesh of a module's: triangles over shared corners.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Stone {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    /// A modelled piece's places in its kit's atlas (`Material::Piece`), a corner each, V
    /// down from the top; none for a piece made from numbers, mapped by where it lies.
    pub uvs: Vec<[f32; 2]>,
}

/// A kit's modules made fit to lay, as an engine's import cleans a mesh before building it
/// (Unreal's mesh build drops degenerate and duplicate triangles): every mesh of each
/// ([`clean`]). The kit does it once, as it reads its pieces.
pub fn clean_all(pieces: &mut Pieces) {
    for module in pieces.modules.values_mut() {
        for mesh in module
            .near
            .iter_mut()
            .chain(module.far.iter_mut())
            .chain(module.cap.iter_mut())
        {
            clean(mesh);
        }
    }
}

/// How nearly in one plane a sliver folded back over its neighbour lies: within 45 degrees
/// of it, as no surface of a model folds back on itself with its normals smooth across.
const FOLD: f32 = 1.0;

/// A modelled mesh made fit to lay: without triangles of no area, a triangle drawn twice (a
/// face made double-sided, both copies of which a layer turns to look one way), or a sliver
/// folded back over its neighbour across their shared edge, looking the same way by its
/// corners' normals, which laid lies in its neighbour's plane over it and flickers. The
/// smaller of a fold goes; its neighbour covers where it was.
pub fn clean(mesh: &mut Stone) {
    let key = |p: [f32; 3]| p.map(|v| libm::roundf(v * 1e5) as i64);
    let point = |i: u32| Vec3::from_array(mesh.positions[i as usize]);
    let normal = |t: &[u32; 3]| -> Vec3 {
        t.iter()
            .filter_map(|&i| mesh.normals.get(i as usize))
            .map(|n| Vec3::from_array(*n))
            .sum()
    };
    let area = |t: &[u32; 3]| {
        let [a, b, c] = t.map(point);
        (b - a).cross(c - a).length() * 0.5
    };
    let in_range = |t: &[u32; 3]| t.iter().all(|&i| (i as usize) < mesh.positions.len());
    // Each triangle once, by its corners' places, and with some area.
    let mut seen = std::collections::HashSet::new();
    let mut keep: Vec<bool> = mesh
        .triangles
        .iter()
        .map(|t| {
            if !in_range(t) || area(t) < 1e-10 {
                return false;
            }
            let mut corners = t.map(|i| key(mesh.positions[i as usize]));
            corners.sort();
            seen.insert(corners)
        })
        .collect();
    // Folds: two triangles across an edge, the one's far corner on the same side of the edge
    // as the other's and nearly in its plane.
    let mut edges: std::collections::HashMap<[[i64; 3]; 2], Vec<usize>> = Default::default();
    for (k, t) in mesh.triangles.iter().enumerate() {
        if !keep[k] {
            continue;
        }
        for e in 0..3 {
            let mut edge = [t[e], t[(e + 1) % 3]].map(|i| key(mesh.positions[i as usize]));
            edge.sort();
            edges.entry(edge).or_default().push(k);
        }
    }
    let far_corner = |t: &[u32; 3], edge: &[[i64; 3]; 2]| {
        t.iter()
            .copied()
            .find(|&i| !edge.contains(&key(mesh.positions[i as usize])))
    };
    let mut edges: Vec<_> = edges.into_iter().collect();
    edges.sort_by_key(|edge| edge.0);
    for (edge, on) in edges {
        for x in 0..on.len() {
            for y in x + 1..on.len() {
                let (i, j) = (on[x], on[y]);
                if !keep[i] || !keep[j] {
                    continue;
                }
                let (ti, tj) = (mesh.triangles[i], mesh.triangles[j]);
                // The larger as the plane, the smaller the one that may fold over it.
                let (big, small, gone) = if area(&ti) >= area(&tj) {
                    (ti, tj, j)
                } else {
                    (tj, ti, i)
                };
                let (Some(a), Some(b)) = (far_corner(&big, &edge), far_corner(&small, &edge))
                else {
                    continue;
                };
                let ends: Vec<Vec3> = big.iter().filter(|&&k| k != a).map(|&k| point(k)).collect();
                let [e0, e1] = [ends[0], ends[1]];
                let n = (e1 - e0).cross(point(a) - e0).normalize_or_zero();
                let mut side = n.cross(e1 - e0).normalize_or_zero();
                if side.dot(point(a) - e0) < 0.0 {
                    side = -side;
                }
                let (h, z) = (side.dot(point(b) - e0), n.dot(point(b) - e0));
                if h > 0.0 && z.abs() < h * FOLD && normal(&big).dot(normal(&small)) > 0.0 {
                    keep[gone] = false;
                }
            }
        }
    }
    let mut k = 0;
    mesh.triangles.retain(|_| {
        k += 1;
        keep[k - 1]
    });
    unlap(mesh);
}

/// How far apart two faces of a modelled mesh may be and still lie in one plane, in metres,
/// and how nearly they must look the same way.
const PLANE_M: f32 = 0.0015;
const ONE_WAY: f32 = 0.999;

/// Each triangle of a modelled mesh that lies over a larger one in its plane, looking the same
/// way by its corners' normals, cut back to what of it the larger does not cover, as two
/// faces of a model meet where they should only touch: a cap's end face triangulated across
/// itself, two layers of a model run into one. What is left of it is cut into convex pieces,
/// its new corners' normals and texture places taken from the triangle's own.
fn unlap(mesh: &mut Stone) {
    let point = |m: &Stone, i: u32| Vec3::from_array(m.positions[i as usize]);
    let facing = |m: &Stone, t: &[u32; 3]| {
        let [a, b, c] = t.map(|i| point(m, i));
        let n = (b - a).cross(c - a);
        let along: Vec3 = t
            .iter()
            .filter_map(|&i| m.normals.get(i as usize))
            .map(|n| Vec3::from_array(*n))
            .sum();
        let n = n.normalize_or_zero();
        if n.dot(along) < 0.0 { -n } else { n }
    };
    let area = |m: &Stone, t: &[u32; 3]| {
        let [a, b, c] = t.map(|i| point(m, i));
        (b - a).cross(c - a).length() * 0.5
    };
    for _ in 0..4 {
        let mut changed = false;
        let mut k = 0;
        while k < mesh.triangles.len() {
            let small = mesh.triangles[k];
            let n = facing(mesh, &small);
            let small_area = area(mesh, &small);
            let cover = (0..mesh.triangles.len()).find(|&j| {
                if j == k {
                    return false;
                }
                let big = mesh.triangles[j];
                let big_area = area(mesh, &big);
                if big_area < small_area || (big_area == small_area && j > k) {
                    return false;
                }
                let m = facing(mesh, &big);
                if m.dot(n) < ONE_WAY {
                    return false;
                }
                let d = m.dot(point(mesh, big[0]));
                if small
                    .iter()
                    .any(|&i| (m.dot(point(mesh, i)) - d).abs() > PLANE_M)
                {
                    return false;
                }
                let flat = flatten(m);
                let tri = |t: [u32; 3]| t.map(|i| flat(point(mesh, i)));
                polygon_area(&clip_inside(&tri(small), &tri(big))) > 1e-7
            });
            let Some(j) = cover else {
                k += 1;
                continue;
            };
            let big = mesh.triangles[j];
            let m = facing(mesh, &big);
            let flat = flatten(m);
            let big_flat = big.map(|i| flat(point(mesh, i)));
            let small_flat = small.map(|i| flat(point(mesh, i)));
            let pieces = outside(&small_flat, &big_flat);
            // The new corners, each from the small triangle's by where it falls in it.
            let corners = small.map(|i| i as usize);
            let weights = |p: glam::Vec2| {
                let [a, b, c] = small_flat;
                let whole = (b - a).perp_dot(c - a);
                if whole.abs() < 1e-14 {
                    return [1.0, 0.0, 0.0];
                }
                let wb = (p - a).perp_dot(c - a) / whole;
                let wc = (b - a).perp_dot(p - a) / whole;
                [1.0 - wb - wc, wb, wc]
            };
            let mut added = Vec::new();
            for piece in pieces {
                let ids: Vec<u32> = piece
                    .iter()
                    .map(|&p| {
                        let w = weights(p);
                        let mix3 = |v: &Vec<[f32; 3]>| {
                            let mut out = Vec3::ZERO;
                            for (c, w) in corners.iter().zip(w) {
                                out += Vec3::from_array(v.get(*c).copied().unwrap_or([0.0; 3])) * w;
                            }
                            out
                        };
                        let position = mix3(&mesh.positions);
                        let normal = mix3(&mesh.normals).normalize_or(n);
                        mesh.positions.push(position.to_array());
                        mesh.normals.push(normal.to_array());
                        if !mesh.uvs.is_empty() {
                            let mut uv = [0.0; 2];
                            for (c, w) in corners.iter().zip(w) {
                                let at = mesh.uvs.get(*c).copied().unwrap_or([0.0; 2]);
                                uv[0] += at[0] * w;
                                uv[1] += at[1] * w;
                            }
                            mesh.uvs.push(uv);
                        }
                        (mesh.positions.len() - 1) as u32
                    })
                    .collect();
                for i in 1..ids.len().saturating_sub(1) {
                    let t = [ids[0], ids[i], ids[i + 1]];
                    if area(mesh, &t) < 1e-10 {
                        continue;
                    }
                    // Wound as the triangle it is cut from.
                    let [a, b, c] = t.map(|i| point(mesh, i));
                    let [sa, sb, sc] = small.map(|i| point(mesh, i));
                    let same = (b - a).cross(c - a).dot((sb - sa).cross(sc - sa)) >= 0.0;
                    added.push(if same { t } else { [t[0], t[2], t[1]] });
                }
            }
            mesh.triangles.swap_remove(k);
            mesh.triangles.extend(added);
            changed = true;
        }
        if !changed {
            break;
        }
    }
}

/// A point in a plane square to `n`, by two axes in it.
pub fn flatten(n: Vec3) -> impl Fn(Vec3) -> glam::Vec2 {
    let u = n.any_orthonormal_vector();
    let v = n.cross(u);
    move |p| glam::Vec2::new(p.dot(u), p.dot(v))
}

pub fn polygon_area(polygon: &[glam::Vec2]) -> f32 {
    if polygon.len() < 3 {
        return 0.0;
    }
    (0..polygon.len())
        .map(|i| polygon[i].perp_dot(polygon[(i + 1) % polygon.len()]))
        .sum::<f32>()
        .abs()
        * 0.5
}

/// A convex polygon clipped to one side of a line through `a` and `b`: the left, or the right.
fn clip_side(polygon: &[glam::Vec2], a: glam::Vec2, b: glam::Vec2, left: bool) -> Vec<glam::Vec2> {
    let side = |p: glam::Vec2| {
        let s = (b - a).perp_dot(p - a);
        if left { s } else { -s }
    };
    let mut out = Vec::with_capacity(polygon.len() + 2);
    for i in 0..polygon.len() {
        let (p, q) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        let (sp, sq) = (side(p), side(q));
        if sp >= 0.0 {
            out.push(p);
        }
        if (sp >= 0.0) != (sq >= 0.0) {
            out.push(p + (q - p) * (sp / (sp - sq)));
        }
    }
    out
}

/// A triangle wound counter-clockwise.
fn counter_clockwise(t: &[glam::Vec2; 3]) -> [glam::Vec2; 3] {
    if (t[1] - t[0]).perp_dot(t[2] - t[0]) < 0.0 {
        [t[0], t[2], t[1]]
    } else {
        *t
    }
}

/// What of a convex polygon lies inside a triangle.
pub fn clip_inside(polygon: &[glam::Vec2], triangle: &[glam::Vec2; 3]) -> Vec<glam::Vec2> {
    let t = counter_clockwise(triangle);
    let mut rest = polygon.to_vec();
    for e in 0..3 {
        rest = clip_side(&rest, t[e], t[(e + 1) % 3], true);
        if rest.len() < 3 {
            return Vec::new();
        }
    }
    rest
}

/// What of a convex polygon lies outside a triangle, as convex pieces: beyond its first edge,
/// then within that and beyond its second, then within both and beyond its third.
pub fn outside(polygon: &[glam::Vec2], triangle: &[glam::Vec2; 3]) -> Vec<Vec<glam::Vec2>> {
    let t = counter_clockwise(triangle);
    let mut rest = polygon.to_vec();
    let mut out = Vec::new();
    for e in 0..3 {
        let beyond = clip_side(&rest, t[e], t[(e + 1) % 3], false);
        if polygon_area(&beyond) > 1e-9 {
            out.push(beyond);
        }
        rest = clip_side(&rest, t[e], t[(e + 1) % 3], true);
        if rest.len() < 3 {
            break;
        }
    }
    out
}
