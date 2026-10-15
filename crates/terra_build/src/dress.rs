//! A kit's dress drawn (D215, research/role-pieces.md): the pieces a layout lays by each
//! exposed edge's role (`layout::roles`) made into surfaces, and the masks that ground and
//! weather what a kit draws.
//!
//! **Profiles** (`Piece::Trim`): a section drawn in Blender (`tools/kits`) swept along one
//! cube's edge, a stone a cube long: mitred where its run turns round a corner, out or
//! back, with a chamfered joint where it runs on into the next cube's, capped where it
//! stops, run on past a stopped end as far as the kit says (a lintel bearing on the wall
//! either side), raised and run down into the ground where a foot steps with it.
//!
//! **Corner stones** (`Piece::Corner`): a stone modelled in Blender for each course up an
//! upright edge, turned and mirrored onto its two faces: quoins long and short by turns,
//! jamb stones long and short along the wall.
//!
//! Both are textured from the world's planes, as the cubes' surface is, each stone a part of
//! the texture of its own and a little lighter or darker, in the kit's `Dressed` material.

use crate::kit::{Dress, Kit, Masks};
use crate::layout::{self, Dir, End, Piece, TICKS, hash};
use crate::mesh::{Indexed, Material, Surface, Surfaces, planar};
use crate::{Base, CELL_M, Fill};
use glam::{IVec3, Mat3, Vec3};
use std::collections::BTreeMap;

/// A kit's pieces, as `tools/kits/pieces.py` writes them from their Blender source.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Pieces {
    /// Sections by name, as points (out, up) in metres from the edge they are swept along:
    /// out from its face, up from its line.
    pub profiles: BTreeMap<String, Vec<[f32; 2]>>,
    /// Corner stones by set.
    pub corners: BTreeMap<String, Corners>,
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

/// A set of corner stones: each a variant. A set with `short` ones lays long and short by
/// turns, each with its long side on the same face; with none, its stones turn each course,
/// their long side on one face and then the other.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Corners {
    pub long: Vec<Stone>,
    pub short: Vec<Stone>,
}

/// A corner stone's triangles: its corner at the origin, its faces looking +X and +Z with
/// the wall behind in -X and -Z, from y = 0 up a course; its longer side along its +Z face.
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

/// How tall a course of dressed stone is: two to a cube.
pub const COURSE_M: f32 = CELL_M * 0.5;

/// How far along a band a point one metre over its line moves where it is mitred to a
/// ramp's raking band: the tangent of half the 45 degrees between them.
const BEND_MITRE: f32 = 0.414_213_56;

/// How wide and deep the chamfered joint between two stones of a band is.
const JOINT_M: f32 = 0.012;
const JOINT_DEPTH_M: f32 = 0.009;
/// How far back a square end's part set into the wall is cut, for each metre it is in.
const SQUARE_RECESS: f32 = 0.2;
/// How far past a lintel's or a sill's back its reveal is sunk.
const REVEAL_PAST_M: f32 = 0.06;
/// How far a corner stone may run along a face it shares with the cube's other edge's
/// stones: to half the cube, short of a joint, so the two meet and never pass.
const SHARED_REACH_M: f32 = CELL_M * 0.5 - JOINT_M * 0.5;

/// Faces meeting at more than about 50 degrees are shaded apart (docs/art-style.md).
const CREASE: f32 = 0.64;

/// Draws a dressed piece into the kit's `Dressed` material; nothing if the kit has no
/// pieces for its role.
pub fn draw(piece: &Piece, kit: &Kit, surfaces: &mut Surfaces) {
    let Some(pieces) = kit.dress.made.as_deref() else {
        return;
    };
    // A profile in a material of its own, as an earth wall's timber lintel; else dressed stone.
    let material = match *piece {
        Piece::Trim { role, .. } | Piece::Band { role, .. } => match kit.dress.roles.get(&role) {
            Some(Dress::Profile {
                material: Some(material),
                ..
            }) => *material,
            _ => Material::Dressed,
        },
        _ => Material::Dressed,
    };
    let grain = matches!(material, Material::Hewn | Material::Log);
    let metres = kit
        .textures
        .get(&material)
        .map_or([1.2, 1.2], |texture| texture.metres);
    let mut made = Indexed::default();
    match *piece {
        Piece::Trim {
            cube,
            role,
            side,
            other,
            ends,
            lift,
            sink,
            seed,
        } => {
            let Some(Dress::Profile {
                name,
                extend,
                joints,
                ..
            }) = kit.dress.roles.get(&role)
            else {
                return;
            };
            let Some(profile) = pieces.profiles.get(name) else {
                return;
            };
            let edge = layout::roles::Edge { role, side, other };
            let (from, to) = layout::roles::line(cube, edge);
            let at = |p: [i32; 3]| IVec3::from_array(p).as_vec3() * (CELL_M / TICKS as f32);
            let sweep = Sweep {
                from: at(from),
                to: at(to),
                side: unit(side),
                up: Vec3::Y,
                ends,
                extend: *extend,
                joints: *joints,
                lift: layout::metres(lift),
                sink: layout::metres(sink),
                tint: kit.dress.tint,
                grain,
            };
            sweep.draw(profile, metres, seed, &mut made);
        }
        Piece::Band {
            role,
            from,
            to,
            side,
            ends,
            seed,
            ..
        } => {
            let Some(Dress::Profile {
                name,
                extend,
                joints,
                ..
            }) = kit.dress.roles.get(&role)
            else {
                return;
            };
            let Some(profile) = pieces.profiles.get(name) else {
                return;
            };
            let (from, to) = (from.metres(), to.metres());
            let along = (to - from).normalize_or(Vec3::X);
            let side = unit(side);
            let sweep = Sweep {
                from,
                to,
                side,
                up: along.cross(side).normalize_or(Vec3::Y) * along.cross(side).y.signum(),
                ends,
                extend: *extend,
                joints: *joints,
                lift: 0.0,
                sink: 0.0,
                tint: kit.dress.tint,
                grain,
            };
            sweep.draw(profile, metres, seed, &mut made);
        }
        Piece::Corner {
            cube,
            role,
            faces,
            courses,
            shared,
            seed,
        } => {
            let Some(Dress::Corners { set }) = kit.dress.roles.get(&role) else {
                return;
            };
            let Some(set) = pieces.corners.get(set) else {
                return;
            };
            corner(
                cube,
                (faces, shared),
                courses,
                set,
                (metres, kit.dress.tint),
                seed,
                &mut made,
            );
        }
        _ => return,
    }
    let out = surfaces.of(material);
    if out.sharing {
        out.shared.append(made);
    } else {
        for &i in &made.indices {
            let i = i as usize;
            out.positions.push(made.positions[i]);
            out.normals.push(made.normals[i]);
            out.colours.push(made.colours[i]);
            out.uvs.push(made.uvs[i]);
        }
    }
}

/// A way's unit vector.
fn unit(dir: Dir) -> Vec3 {
    dir.step().as_vec3()
}

/// Where a piece set into a wall is bedded: a lintel or a sill, swept along an opening's
/// head or sill and on past it into the wall either side. The wall's face is sunk behind
/// it and built up a little round its edges (`surface::Heights::bed`), so the piece sits
/// all but flush, its ends and its edge away from the opening lost in the wall, as cob or
/// rubble is built up round a timber, rather than standing on the face like a board.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bed {
    /// Along the edge, from one end of the piece to the other, on the wall's face.
    pub from: Vec3,
    pub to: Vec3,
    /// Out of the face, and up the face square to the edge.
    pub out: Vec3,
    pub up: Vec3,
    /// How far the piece reaches up the face from the edge, lowest and highest, in metres.
    pub low: f32,
    pub high: f32,
    /// How far its face stands out of the wall's.
    pub face: f32,
    /// Whether each end, at `from` and at `to`, is the piece's end, which the wall laps
    /// onto, rather than running on into the next cube's.
    pub ends: [bool; 2],
    /// Whether the opening is under it (a lintel) rather than over it (a sill): the edge
    /// along the opening is the one the wall does not lap onto.
    pub over_opening: bool,
    /// How far it reaches into the wall behind the face, in metres.
    pub back: f32,
}

impl Bed {
    /// How far a point of the wall's face is inside the piece's outline on the face, in
    /// metres, less than nothing outside it; none if the point is not on this face.
    pub fn inside(&self, p: Vec3) -> Option<f32> {
        let rel = p - self.from;
        // Only the face itself, not the opening's reveal behind it.
        if rel.dot(self.out).abs() > 1e-3 {
            return None;
        }
        let length = self.from.distance(self.to);
        let along = rel.dot((self.to - self.from) / length.max(1e-6));
        let up = rel.dot(self.up);
        // Past an end running on into the next cube's, that one's bed takes over; along
        // the opening's side, the piece's own edge shows.
        let edge = |lapped: bool, inset: f32| {
            if lapped || inset < 0.0 {
                inset
            } else {
                f32::MAX
            }
        };
        let inside = edge(self.ends[0], along)
            .min(edge(self.ends[1], length - along))
            .min(edge(!self.over_opening, up - self.low))
            .min(edge(self.over_opening, self.high - up));
        Some(inside)
    }

    /// As [`Bed::inside`], for a point of the opening's reveal the piece's underside (a
    /// lintel's) or top (a sill's) lies in: how far inside the piece's depth into the wall
    /// it is, with how far the piece stands out of it, nothing. The reveal is sunk behind
    /// the piece as the face is, else the two are drawn in one plane and fight.
    pub fn reveal(&self, p: Vec3) -> Option<f32> {
        let rel = p - self.from;
        let flush = if self.over_opening {
            self.low
        } else {
            self.high
        };
        if flush.abs() > 1e-3 || rel.dot(self.up).abs() > 1e-3 {
            return None;
        }
        let deep = -rel.dot(self.out);
        if deep < -1e-3 {
            return None;
        }
        let length = self.from.distance(self.to);
        let along = rel.dot((self.to - self.from) / length.max(1e-6));
        // Sunk on a little past its back, where the lintel on the wall's other face meets
        // it, so the head is behind both all the way across.
        // Along it only its own ends bound it, not where it runs on into the next cube's,
        // whose bed takes over past it.
        if (!self.ends[0] && along < 0.0) || (!self.ends[1] && along > length) {
            return None;
        }
        let end = |own: bool, inset: f32| if own { inset } else { f32::MAX };
        Some(
            (self.back + REVEAL_PAST_M - deep)
                .min(end(self.ends[0], along))
                .min(end(self.ends[1], length - along)),
        )
    }

    /// Where a point of a wall's surface is against the piece, on its face or its reveal:
    /// how far inside the piece's outline, how far the piece stands out there, and whether
    /// the wall laps over its edges there: on its face, not on a reveal, where its edge
    /// away from the opening is inside the wall.
    pub fn at(&self, p: Vec3) -> Option<(f32, f32, bool)> {
        self.inside(p)
            .map(|inside| (inside, self.face, true))
            .or_else(|| self.reveal(p).map(|inside| (inside, 0.0, false)))
    }
}

/// The bed of a piece set into a wall, if it is one: a lintel or a sill swept along an
/// opening's head or sill (`Piece::Trim`, `Role::Head` or `Role::Sill`).
pub fn bed(piece: &Piece, kit: &Kit) -> Option<Bed> {
    let Piece::Trim {
        cube,
        role,
        side,
        other,
        ends,
        ..
    } = *piece
    else {
        return None;
    };
    if !matches!(role, layout::Role::Head | layout::Role::Sill) {
        return None;
    }
    let Some(Dress::Profile { name, extend, .. }) = kit.dress.roles.get(&role) else {
        return None;
    };
    let profile = kit.dress.made.as_deref()?.profiles.get(name)?;
    let low = profile.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    let high = profile.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    let face = profile.iter().map(|p| p[0]).fold(0.0f32, f32::max);
    let edge = layout::roles::Edge { role, side, other };
    let (from, to) = layout::roles::line(cube, edge);
    let at = |p: [i32; 3]| IVec3::from_array(p).as_vec3() * (CELL_M / TICKS as f32);
    let (from, to) = (at(from), at(to));
    let along = (to - from).normalize_or(Vec3::X);
    let reach = ends.map(|end| if end == End::Square { *extend } else { 0.0 });
    Some(Bed {
        from: from - along * reach[0],
        to: to + along * reach[1],
        out: unit(side),
        up: Vec3::Y,
        low,
        high,
        face,
        ends: ends.map(|end| !matches!(end, End::On | End::Step)),
        over_opening: role == layout::Role::Head,
        back: -profile.iter().map(|p| p[0]).fold(0.0f32, f32::min),
    })
}

/// A stone's shade and where its texture falls, from its seed: each a part of the texture
/// of its own, a little lighter or darker and warmer or cooler, as stones from one bed are.
fn stone_look(seed: u32, tint: [f32; 3]) -> ([f32; 3], [f32; 2]) {
    let shade = 0.86 + 0.16 * layout::unit(hash(&[seed, 3]));
    let warm = 0.06 * (layout::unit(hash(&[seed, 4])) - 0.5);
    (
        [
            tint[0] * shade * (1.0 + warm),
            tint[1] * shade,
            tint[2] * shade * (1.0 - warm),
        ],
        [
            layout::unit(hash(&[seed, 1])),
            layout::unit(hash(&[seed, 2])),
        ],
    )
}

/// A profile swept along one cube's edge.
struct Sweep {
    from: Vec3,
    to: Vec3,
    /// Out from the face it looks out of, and up from the line, square to both it and the
    /// way out: straight up along a level edge, square to a slope along a raking one.
    side: Vec3,
    up: Vec3,
    ends: [End; 2],
    extend: f32,
    joints: bool,
    lift: f32,
    sink: f32,
    tint: [f32; 3],
    /// Whether its texture runs along it, as timber's grain does, rather than from the
    /// world's planes, as stone's is.
    grain: bool,
}

impl Sweep {
    fn draw(&self, profile: &[[f32; 2]], metres: [f32; 2], seed: u32, out: &mut Indexed) {
        if profile.len() < 2 {
            return;
        }
        // Raised to step with the ground, its lowest points run on down to reach it.
        let low = profile.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        let points: Vec<[f32; 2]> = profile
            .iter()
            .map(|&[o, u]| {
                let u = u + self.lift;
                [
                    o,
                    if u - self.lift <= low + 1e-4 {
                        u - self.sink
                    } else {
                        u
                    },
                ]
            })
            .collect();
        let normals = segment_normals(&points);
        let n = points.len();
        let length = self.from.distance(self.to);
        let along = (self.to - self.from) / length;
        // How each end is cut: run on, mitred out or back, or square.
        let mitre = self.ends.map(|end| match end {
            End::Out => 1.0,
            End::Back => -1.0,
            _ => 0.0,
        });
        // Mitred in its own plane where a ramp's raking coping meets a level one: on the
        // line halving the 45 degrees between them, over or under the bend.
        let bend = self.ends.map(|end| match end {
            End::Over => BEND_MITRE,
            End::Under => -BEND_MITRE,
            _ => 0.0,
        });
        let extend = self.ends.map(|end| match end {
            End::Square => self.extend,
            _ => 0.0,
        });
        // A square end's part set into the wall cut back from its line, so it is not in the
        // plane of the wall's end or an opening's reveal there, nor of the next piece's;
        // and a joint's or a step's, at a cube's side, so it is not in the plane of a step
        // in the footing or the wall under it there.
        let recess = self.ends.map(|end| match end {
            End::Square if self.extend == 0.0 => SQUARE_RECESS,
            End::On | End::Step => SQUARE_RECESS,
            _ => 0.0,
        });
        let joint = self
            .ends
            .map(|end| self.joints && matches!(end, End::On | End::Step));
        let start = |o: f32, u: f32| {
            -extend[0] - o * mitre[0] + (-o).max(0.0) * recess[0] - (u - self.lift) * bend[0]
        };
        let end = |o: f32, u: f32| {
            length + extend[1] + o * mitre[1] - (-o).max(0.0) * recess[1]
                + (u - self.lift) * bend[1]
        };
        // Rings along it: at each end, and in from a joint by its chamfer.
        let mut rings: Vec<(f32, bool)> = vec![(f32::MIN, joint[0])];
        if joint[0] {
            rings.push((JOINT_M, false));
        }
        if joint[1] {
            rings.push((length - JOINT_M, false));
        }
        rings.push((f32::MAX, joint[1]));
        let world = |o: f32, u: f32, a: f32| self.from + along * a + self.side * o + self.up * u;
        let (colour, shift) = stone_look(seed, self.tint);
        let shift = if self.joints { shift } else { [0.0; 2] };
        let uv = |p: Vec3, normal: Vec3| {
            let t = planar(p, normal, metres);
            [t[0] + shift[0], t[1] + shift[1]]
        };
        // How far round the section each point is, for a texture that runs along it.
        let mut round = vec![0.0f32; n];
        for i in 1..n {
            let (a, b) = (points[i - 1], points[i]);
            round[i] = round[i - 1] + ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        }
        let along_uv = |ring: usize, i: usize| {
            let (low, high) = (
                start(points[i][0], points[i][1]),
                end(points[i][0], points[i][1]),
            );
            let at = if low <= high {
                rings[ring].0.clamp(low, high)
            } else {
                (low + high) * 0.5
            };
            [round[i] / metres[0] + shift[0], at / metres[1] + shift[1]]
        };
        // A point of the section, at a ring, pulled in where the ring is a joint.
        let point = |ring: usize, i: usize, seg: usize| -> Vec3 {
            let (at, inset) = rings[ring];
            let [mut o, mut u] = points[i];
            if inset {
                let nn = vertex_normal(&normals, seg, if i == seg { 0 } else { 1 });
                o -= nn[0] * JOINT_DEPTH_M;
                u -= nn[1] * JOINT_DEPTH_M;
            }
            // Mitred at both ends of a short line, a point far in from the face would
            // pass itself: it meets its mirror in the middle.
            let (low, high) = (
                start(points[i][0], points[i][1]),
                end(points[i][0], points[i][1]),
            );
            let a = if low <= high {
                at.clamp(low, high)
            } else {
                (low + high) * 0.5
            };
            world(o, u, a)
        };
        for ring in 0..rings.len() - 1 {
            for seg in 0..n - 1 {
                let quad = [
                    (point(ring, seg, seg), vertex_normal(&normals, seg, 0)),
                    (point(ring + 1, seg, seg), vertex_normal(&normals, seg, 0)),
                    (
                        point(ring + 1, seg + 1, seg),
                        vertex_normal(&normals, seg, 1),
                    ),
                    (point(ring, seg + 1, seg), vertex_normal(&normals, seg, 1)),
                ];
                let grained = [
                    along_uv(ring, seg),
                    along_uv(ring + 1, seg),
                    along_uv(ring + 1, seg + 1),
                    along_uv(ring, seg + 1),
                ];
                let face = (quad[2].0 - quad[0].0).cross(quad[3].0 - quad[1].0);
                if face.length_squared() < 1e-12 {
                    continue;
                }
                let world_normal =
                    |nn: [f32; 2]| (self.side * nn[0] + self.up * nn[1]).normalize_or(self.up);
                let flat =
                    (self.side * normals[seg][0] + self.up * normals[seg][1]).normalize_or(self.up);
                let mut k = 0;
                let corners = quad.map(|(p, nn)| {
                    let at = if self.grain { grained[k] } else { uv(p, flat) };
                    k += 1;
                    out.corner(p, world_normal(nn), at, colour)
                });
                // Wound to face out.
                let outward = face.dot(flat) >= 0.0;
                for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
                    if outward {
                        out.indices.extend([corners[a], corners[b], corners[c]]);
                    } else {
                        out.indices.extend([corners[a], corners[c], corners[b]]);
                    }
                }
            }
        }
        // Each end closed, so a piece whose neighbour is not yet in, or never will be, is
        // never seen hollow: across its joint's chamfer, or its whole section.
        for (which, ring, outward) in [(0, 0, -along), (1, rings.len() - 1, along)] {
            if mitre[which] != 0.0 || bend[which] != 0.0 {
                continue;
            }
            let ring_points: Vec<Vec3> = (0..n).map(|i| point(ring, i, i.min(n - 2))).collect();
            let flat: Vec<[f32; 2]> = ring_points
                .iter()
                .map(|p| {
                    let rel = *p - self.from;
                    [rel.dot(self.side), rel.dot(self.up)]
                })
                .collect();
            for tri in ears(&flat) {
                let c = tri.map(|i| ring_points[i]);
                let face = (c[1] - c[0]).cross(c[2] - c[0]);
                let tri = if face.dot(outward) >= 0.0 {
                    tri
                } else {
                    [tri[0], tri[2], tri[1]]
                };
                for i in tri {
                    let p = ring_points[i];
                    let index = out.corner(p, outward, uv(p, outward), colour);
                    out.indices.push(index);
                }
            }
        }
    }
}

/// A section's outward normal at each of its segments, in (out, up): the side away from
/// the cube, the solid being on the side of the cube's inside.
fn segment_normals(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    // Which side is solid: by the winding of the section closed on itself.
    let area: f32 = (0..points.len())
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    points
        .windows(2)
        .map(|w| {
            let d = [w[1][0] - w[0][0], w[1][1] - w[0][1]];
            let l = libm::sqrtf(d[0] * d[0] + d[1] * d[1]).max(1e-9);
            if area > 0.0 {
                [d[1] / l, -d[0] / l]
            } else {
                [-d[1] / l, d[0] / l]
            }
        })
        .collect()
}

/// A section's point's normal at either end of a segment: shared with the next segment's
/// where they meet at less than a crease, its own where they meet at more.
fn vertex_normal(normals: &[[f32; 2]], seg: usize, end: usize) -> [f32; 2] {
    let own = normals[seg];
    let other = if end == 0 {
        seg.checked_sub(1).map(|s| normals[s])
    } else {
        normals.get(seg + 1).copied()
    };
    match other {
        Some(o) if own[0] * o[0] + own[1] * o[1] > CREASE => {
            let s = [own[0] + o[0], own[1] + o[1]];
            let l = libm::sqrtf(s[0] * s[0] + s[1] * s[1]).max(1e-9);
            [s[0] / l, s[1] / l]
        }
        _ => own,
    }
}

/// Triangles filling a simple polygon, by clipping ears.
fn ears(points: &[[f32; 2]]) -> Vec<[usize; 3]> {
    let mut idx: Vec<usize> = (0..points.len()).collect();
    let area: f32 = (0..points.len())
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    let ccw = area > 0.0;
    let cross = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    let mut out = Vec::new();
    while idx.len() > 3 {
        let n = idx.len();
        let mut clipped = false;
        for i in 0..n {
            let (ia, ib, ic) = (idx[(i + n - 1) % n], idx[i], idx[(i + 1) % n]);
            let (a, b, c) = (points[ia], points[ib], points[ic]);
            if (cross(a, b, c) > 1e-12) != ccw {
                continue;
            }
            let inside = idx.iter().any(|&j| {
                if j == ia || j == ib || j == ic {
                    return false;
                }
                let p = points[j];
                let (d1, d2, d3) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
                if ccw {
                    d1 > 0.0 && d2 > 0.0 && d3 > 0.0
                } else {
                    d1 < 0.0 && d2 < 0.0 && d3 < 0.0
                }
            });
            if inside {
                continue;
            }
            out.push([ia, ib, ic]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

/// Corner stones up an upright edge of a cube, one a course in the courses it shows.
fn corner(
    cube: IVec3,
    (faces, shared): ([Dir; 2], [bool; 2]),
    courses: [bool; 2],
    set: &Corners,
    (metres, tint): ([f32; 2], [f32; 3]),
    seed: u32,
    out: &mut Indexed,
) {
    if set.long.is_empty() {
        return;
    }
    // The edge's foot: the cube's corner on both faces.
    let mut base = cube.as_vec3() * CELL_M;
    for face in faces {
        if face.sign() > 0 {
            base[face.axis()] += CELL_M;
        }
    }
    for (course, shows) in courses.into_iter().enumerate() {
        if !shows {
            continue;
        }
        let turn = (cube.y * 2 + course as i32).rem_euclid(2) == 1;
        let stone_seed = hash(&[seed, course as u32]);
        fn pick(stones: &[Stone], seed: u32) -> Option<&Stone> {
            (!stones.is_empty()).then(|| &stones[hash(&[seed, 9]) as usize % stones.len()])
        }
        let (stone, [a, b]) = if set.short.is_empty() {
            (
                pick(&set.long, stone_seed),
                if turn { [faces[1], faces[0]] } else { faces },
            )
        } else {
            (
                pick(if turn { &set.short } else { &set.long }, stone_seed),
                faces,
            )
        };
        let Some(stone) = stone else {
            continue;
        };
        // Its +Z face onto the first face, its +X onto the second.
        let turned = Mat3::from_cols(unit(b), Vec3::Y, unit(a));
        let mirrored = turned.determinant() < 0.0;
        let at = base + Vec3::Y * (course as f32 * COURSE_M);
        let (colour, shift) = stone_look(stone_seed, tint);
        // On a face it shares with the cube's other edge's stones, squeezed to stop a joint
        // short of the face's middle, where the other's stop. It runs along the first face
        // as far back as its -X goes, along the second as its -Z.
        let shares = |face: Dir| faces.iter().zip(shared).any(|(f, s)| *f == face && s);
        let squeeze = |shares: bool, reach: f32| {
            if shares && reach > SHARED_REACH_M {
                SHARED_REACH_M / reach
            } else {
                1.0
            }
        };
        let back = |axis: usize| stone.positions.iter().map(|p| -p[axis]).fold(0.0, f32::max);
        let (sx, sz) = (squeeze(shares(a), back(0)), squeeze(shares(b), back(2)));
        let first = out.positions.len() as u32;
        for (p, n) in stone.positions.iter().zip(&stone.normals) {
            let [mut x, y, mut z] = *p;
            let [mut nx, ny, mut nz] = *n;
            if x < 0.0 {
                x *= sx;
                nx /= sx;
            }
            if z < 0.0 {
                z *= sz;
                nz /= sz;
            }
            let p = at + turned * Vec3::new(x, y, z);
            let n = (turned * Vec3::new(nx, ny, nz)).normalize_or(Vec3::Y);
            let t = planar(p, n, metres);
            out.corner(p, n, [t[0] + shift[0], t[1] + shift[1]], colour);
        }
        for t in &stone.triangles {
            let [x, y, z] = t.map(|i| first + i);
            if mirrored {
                out.indices.extend([x, z, y]);
            } else {
                out.indices.extend([x, y, z]);
            }
        }
    }
}

/// The ground's height under a point of a base, in metres in its own space, as the roles
/// read it (`layout::roles::Roles::ground`).
pub fn ground_at(base: &Base, point: Vec3) -> f32 {
    let column = [
        libm::floorf(point.x / CELL_M) as i32,
        libm::floorf(point.z / CELL_M) as i32,
    ];
    layout::metres(layout::roles::ground_ticks(base, column))
}

/// Smooth value noise over lattices so many metres across, from 0 to 1, read at many
/// points in turn: the corners of the cells last read are kept, a few to hand for each
/// lattice by where they are, as a surface's corners come face by face, so most fall in a
/// cell read just before, and a cell's corners are eight hashes.
#[derive(Default)]
struct Noise {
    /// Each lattice by its size and seed, and the cells of it kept.
    lattices: Vec<(f32, u32, Box<Cells>)>,
}

/// A lattice's cells kept to hand, each its corner and its eight corners' values.
type Cells = [Option<(IVec3, [f32; 8])>; CELLS_KEPT];

/// How many cells of each lattice are kept to hand.
const CELLS_KEPT: usize = 32;

impl Noise {
    fn at(&mut self, p: Vec3, size: f32, seed: u32) -> f32 {
        let q = p / size;
        let base = q.floor();
        let f = q - base;
        let f = f * f * (Vec3::splat(3.0) - 2.0 * f);
        let b = base.as_ivec3();
        let lattice = match self
            .lattices
            .iter()
            .position(|(s, k, _)| *s == size && *k == seed)
        {
            Some(at) => at,
            None => {
                self.lattices
                    .push((size, seed, Box::new([None; CELLS_KEPT])));
                self.lattices.len() - 1
            }
        };
        let cells = &mut self.lattices[lattice].2;
        let slot = (b.x.wrapping_mul(73_856_093)
            ^ b.y.wrapping_mul(19_349_663)
            ^ b.z.wrapping_mul(83_492_791)) as u32 as usize
            % CELLS_KEPT;
        let v = match cells[slot] {
            Some((cell, v)) if cell == b => v,
            _ => {
                let at = |dx: i32, dy: i32, dz: i32| {
                    layout::unit(hash(&[
                        (b.x + dx) as u32,
                        (b.y + dy) as u32,
                        (b.z + dz) as u32,
                        seed,
                    ]))
                };
                let v = [
                    at(0, 0, 0),
                    at(1, 0, 0),
                    at(0, 1, 0),
                    at(1, 1, 0),
                    at(0, 0, 1),
                    at(1, 0, 1),
                    at(0, 1, 1),
                    at(1, 1, 1),
                ];
                cells[slot] = Some((b, v));
                v
            }
        };
        let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
        let x00 = lerp(v[0], v[1], f.x);
        let x10 = lerp(v[2], v[3], f.x);
        let x01 = lerp(v[4], v[5], f.x);
        let x11 = lerp(v[6], v[7], f.x);
        lerp(lerp(x00, x10, f.y), lerp(x01, x11, f.y), f.z)
    }
}

/// `x` to the power one and a half, as `libm::powf` gives it to within a rounding, a
/// tenth of the time: masks raise a fraction so at most corners.
fn three_halves(x: f32) -> f32 {
    x * x.sqrt()
}

/// Where a point is among a block kind's lifts so tall: how many lifts up, the seams
/// wandering a few centimetres up and down along the wall. The surface's ridges
/// (`surface::Packed`) follow it, and the masks' strata the same through their kept
/// `Noise`, which reads the same lattice, so they agree.
pub(crate) fn lift_at(p: Vec3, lift: f32) -> f32 {
    p.y / lift + 0.35 * (smooth(p, 1.1, 11) - 0.5)
}

/// Smooth value noise over a lattice so many metres across, from 0 to 1.
pub(crate) fn smooth(p: Vec3, size: f32, seed: u32) -> f32 {
    let q = p / size;
    let base = q.floor();
    let f = q - base;
    let f = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let b = base.as_ivec3();
    let at = |dx: i32, dy: i32, dz: i32| {
        layout::unit(hash(&[
            (b.x + dx) as u32,
            (b.y + dy) as u32,
            (b.z + dz) as u32,
            seed,
        ]))
    };
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(at(0, 0, 0), at(1, 0, 0), f.x);
    let x10 = lerp(at(0, 1, 0), at(1, 1, 0), f.x);
    let x01 = lerp(at(0, 0, 1), at(1, 0, 1), f.x);
    let x11 = lerp(at(0, 1, 1), at(1, 1, 1), f.x);
    lerp(lerp(x00, x10, f.y), lerp(x01, x11, f.y), f.z)
}

/// A corner's shade by the kit's masks: dirt and damp rising unevenly from the ground,
/// outside edges worn lighter by their normal's turn, a block kind's lifts and its storey's
/// top paler, dry under the eaves, and, for timber, grey higher up.
fn masked(
    masks: &Masks,
    (timber, layered): (bool, bool),
    p: Vec3,
    normal: Vec3,
    ground: f32,
    noise: &mut Noise,
) -> [f32; 3] {
    let mut c = Vec3::ONE;
    // A block kind's lifts, each a little lighter or darker, wandering along the wall.
    if layered && masks.strata > 0.0 {
        // Each lift its own shade, a darker seam where one was rammed on the last, the
        // seams wandering a few centimetres up and down along the wall.
        let lift = masks.lift.max(0.05);
        let y = p.y / lift + 0.35 * (noise.at(p, 1.1, 11) - 0.5);
        let layer = libm::floorf(y);
        let within = y - layer;
        let own = layout::unit(hash(&[layer as i32 as u32, 17])) - 0.5;
        let seam = (1.0 - (within.min(1.0 - within) / 0.06).min(1.0)) * 0.7;
        // Within a lift, finer layers where each basketful was rammed, fainter.
        let fine = libm::sinf(y * std::f32::consts::TAU * 3.0 + 2.0 * noise.at(p, 0.7, 15)) * 0.2;
        // Grit in coarse, sparse patches, not an even speckle.
        let speckle = noise.at(p, 0.55, 13) - 0.5;
        c *= 1.0 + masks.strata * (own * 1.4 + fine + speckle * 0.35 - seam);
    }
    let height = p.y - ground;
    // Paler from a storey's height above the ground up, dry under the eaves and out of
    // reach of the splash, the line wandering along the wall.
    if layered && masks.dry > 0.0 {
        let reach = masks.dry_reach.max(0.1);
        let wander = reach * 0.35 * (noise.at(p, 0.9, 29) - 0.5);
        let dry = (1.0 - (crate::STOREY_M - height + wander) / reach).clamp(0.0, 1.0);
        if dry > 0.0 {
            let pale = masks.dry * dry * dry * (0.7 + 0.6 * noise.at(p, 0.3, 31));
            let luma = (c.x + c.y + c.z) / 3.0;
            c = c.lerp(Vec3::splat(luma), pale * 0.5) * (1.0 + pale);
        }
    }
    // Dirt rises at most 1.4 times its height, so above that it is not worked out.
    if masks.dirt > 0.0 && height < masks.dirt * 1.4 {
        let rise = masks.dirt * (0.6 + 0.8 * noise.at(p, 0.45, 3));
        let dirt = three_halves((1.0 - height / rise).clamp(0.0, 1.0));
        c = c.lerp(Vec3::from_array(masks.dirt_tint), dirt);
    }
    if masks.wear > 0.0 {
        let a = normal.abs();
        let round = ((1.0 - a.x.max(a.y).max(a.z)) / 0.29).clamp(0.0, 1.0);
        c *= 1.0 + masks.wear * round;
    }
    if timber && masks.weather > 0.0 {
        let grey = masks.weather * (height / 3.0).clamp(0.0, 1.0) * (0.5 + noise.at(p, 0.8, 5));
        let luma = (c.x + c.y + c.z) / 3.0;
        c = c.lerp(Vec3::splat(luma * 1.05), grey.clamp(0.0, 1.0));
    }
    (c * underside(normal)).to_array()
}

/// How much light a face looking down keeps: none of the sky's, and of the ground's only
/// what the wall it stands out from, or the opening it spans, lets it see. The sky light
/// lights an underside by the sunlit ground's whole bounce (`terra_world::skylight`), which
/// left a lintel's soffit and every quoin's underside lit brighter and greener than the
/// wall's shaded face beside it; a modelled piece bakes this occlusion into its texture, as
/// the logs' grooves are shaded (D262).
fn underside(normal: Vec3) -> f32 {
    let t = ((-normal.y - UNDER_FROM) / (1.0 - UNDER_FROM)).clamp(0.0, 1.0);
    1.0 - UNDERSIDE * t * t * (3.0 - 2.0 * t)
}

/// Faces within about 55 degrees of straight down are shaded, the most `UNDERSIDE` darker.
const UNDER_FROM: f32 = 0.55;
const UNDERSIDE: f32 = 0.65;

/// Where the rain marks a block kind's wall (D215), from its cubes: the damp splashed up
/// over its own foot, and the streaks drawn down under a sill or an open top; each column's
/// foot and each cell's source looked up once.
struct Rained<'a> {
    base: &'a Base,
    kind: crate::Kind,
    feet: BTreeMap<[i32; 3], f32>,
    sources: BTreeMap<[i32; 3], Option<f32>>,
}

impl Rained<'_> {
    fn ours(&self, at: IVec3) -> bool {
        self.base
            .cube(at)
            .is_some_and(|cube| cube.kind == self.kind && layout::drawn_fill(&cube) == Fill::Solid)
    }

    /// The cell of the wall a point of its face is on: a little in from the face.
    fn cell(p: Vec3, n: Vec3) -> IVec3 {
        let level = Vec3::new(n.x, 0.0, n.z).normalize_or_zero();
        ((p - level * (CELL_M * 0.5)) / CELL_M).floor().as_ivec3()
    }

    /// The bottom of the wall under a cell, in metres: as far down as the kind goes.
    fn foot(&mut self, cell: IVec3) -> f32 {
        if let Some(&foot) = self.feet.get(&cell.to_array()) {
            return foot;
        }
        let mut at = cell;
        for _ in 0..crate::STOREY_CUBES * 2 {
            if !self.ours(at - IVec3::Y) {
                break;
            }
            at -= IVec3::Y;
        }
        let foot = at.y as f32 * CELL_M;
        self.feet.insert(cell.to_array(), foot);
        foot
    }

    /// The height a streak runs down from over a cell: the foot of an opening or the open
    /// top of the wall over it, within reach; none under a roof or another kind.
    fn source(&mut self, cell: IVec3, reach: f32) -> Option<f32> {
        if let Some(&found) = self.sources.get(&cell.to_array()) {
            return found;
        }
        let mut found = None;
        let cells = libm::ceilf(reach / CELL_M) as i32;
        for up in 1..=cells.max(1) {
            let at = cell + IVec3::Y * up;
            if self.ours(at) {
                continue;
            }
            // An opening's foot, or a top open to the sky, not under the eaves of a roof.
            let open = match self.base.cube(at) {
                None => (1..=crate::STOREY_CUBES)
                    .all(|over| self.base.cube(at + IVec3::Y * over).is_none()),
                Some(cube) => cube.kind == self.kind,
            };
            if open {
                found = Some(at.y as f32 * CELL_M);
            }
            break;
        }
        self.sources.insert(cell.to_array(), found);
        found
    }

    /// What a point of the wall's face is shaded by: darker over its foot, and in streaks
    /// under a sill or an open top.
    fn shade(&mut self, masks: &Masks, p: Vec3, n: Vec3) -> Vec3 {
        let mut k = Vec3::ONE;
        // Only the sides: a top is not splashed nor streaked.
        if n.y.abs() > 0.7 {
            return k;
        }
        let cell = Self::cell(p, n);
        let tint = Vec3::from_array(masks.dirt_tint);
        if masks.splash > 0.0 {
            let up = p.y - self.foot(cell);
            let reach = masks.splash * (0.7 + 0.6 * smooth(p, 0.35, 51));
            let wet = libm::powf((1.0 - up / reach).clamp(0.0, 1.0), 0.8);
            k = k.lerp(tint, wet);
        }
        if masks.streaks > 0.0
            && let Some(from) = self.source(cell, masks.streak_reach)
        {
            let down = from - p.y;
            if down > 0.0 && down < masks.streak_reach {
                // Long and narrow, as water runs: stretched up the wall, fine across it.
                let across = Vec3::new(p.x * 9.0, p.y * 0.7, p.z * 9.0);
                let streak = ((smooth(across, 1.0, 53) - 0.42) * 3.0).clamp(0.0, 1.0);
                let fade = 1.0 - down / masks.streak_reach;
                k = k.lerp(k * tint, masks.streaks * streak * fade);
            }
        }
        k
    }
}

/// A kit's masks laid over a surface it drew in a material, in its base's own space.
pub fn mask(surface: &mut Surface, kit: &Kit, material: Material, base: &Base) {
    let masks = &kit.dress.masks;
    // Nothing drawn in it, so no columns to span: the table below would be empty bounds.
    if surface.positions.is_empty() && surface.shared.positions.is_empty() {
        return;
    }
    let timber = matches!(
        material,
        Material::Log | Material::LogEnd | Material::Board | Material::Daub
    );
    let layered = material == Material::Masonry;
    // The ground's height by column, looked up once each, in a table over the columns the
    // surface stands on.
    let column = |p: Vec3| [(p.x / CELL_M).floor() as i32, (p.z / CELL_M).floor() as i32];
    let (low, high) = surface
        .positions
        .iter()
        .chain(&surface.shared.positions)
        .map(|&p| column(p))
        .fold(([i32::MAX; 2], [i32::MIN; 2]), |(l, h), c| {
            (
                [l[0].min(c[0]), l[1].min(c[1])],
                [h[0].max(c[0]), h[1].max(c[1])],
            )
        });
    let wide = (high[0] - low[0] + 1).max(0) as usize;
    let deep = (high[1] - low[1] + 1).max(0) as usize;
    let mut known: Vec<f32> = vec![f32::NAN; wide * deep];
    let mut ground = |p: Vec3| {
        let at = column(p);
        let slot = (at[0] - low[0]) as usize + (at[1] - low[1]) as usize * wide;
        if known[slot].is_nan() {
            known[slot] = layout::metres(layout::roles::ground_ticks(base, at));
        }
        known[slot]
    };
    let mut noise = Noise::default();
    let mut rained = Rained {
        base,
        kind: kit.kind(),
        feet: BTreeMap::new(),
        sources: BTreeMap::new(),
    };
    let rains = layered && (masks.splash > 0.0 || masks.streaks > 0.0);
    let mut shade = |p: Vec3, n: Vec3, c: &mut [f32; 3]| {
        let mut k = Vec3::from_array(masked(
            masks,
            (timber, layered),
            p,
            n,
            ground(p),
            &mut noise,
        ));
        if rains {
            k *= rained.shade(masks, p, n);
        }
        *c = [c[0] * k.x, c[1] * k.y, c[2] * k.z];
    };
    for ((p, n), c) in surface
        .positions
        .iter()
        .zip(&surface.normals)
        .zip(surface.colours.iter_mut())
    {
        shade(*p, *n, c);
    }
    let shared = &mut surface.shared;
    for ((p, n), c) in shared
        .positions
        .iter()
        .zip(&shared.normals)
        .zip(shared.colours.iter_mut())
    {
        shade(*p, *n, c);
    }
}

/// A kit's masks laid over a roof's covering and its mortar, in its base's own space, as
/// the role pieces' are (D215): toward its eaves, where the rain runs off and lingers, the
/// covering greened by moss and lichen in blotches; up toward its ridge and hips greyed by
/// the sun and weather, as timber is; along its eaves and verges, its edges worn lighter.
/// Where each point is on its roof is `roofs`'s to say.
pub fn roof_mask(surface: &mut Surface, kit: &Kit, roofs: &crate::layout::roofs::Roofs) {
    let masks = &kit.dress.masks;
    if masks.moss <= 0.0 && masks.weather <= 0.0 && masks.wear <= 0.0 {
        return;
    }
    let mut noise = Noise::default();
    let mut shade = |p: Vec3, c: &mut [f32; 3]| {
        let Some((up, inside)) = roofs.place_of(p) else {
            return;
        };
        let up: f32 = up.clamp(-0.5, 1.0);
        let mut colour = Vec3::from_array(*c);
        if masks.moss > 0.0 {
            let blotch = noise.at(p, 0.6, 17) * 0.7 + noise.at(p, 2.1, 19) * 0.6;
            let low = three_halves((1.0 - up).clamp(0.0, 1.0));
            let moss = (masks.moss * low * blotch * 1.8).clamp(0.0, 0.85);
            colour = colour.lerp(colour * Vec3::from_array(masks.moss_tint), moss);
        }
        if masks.weather > 0.0 {
            let grey = masks.weather * up.max(0.0) * (0.4 + noise.at(p, 1.3, 23));
            let luma = (colour.x + colour.y + colour.z) / 3.0;
            colour = colour.lerp(Vec3::splat(luma * 1.08), grey.clamp(0.0, 1.0));
        }
        // Lighter along its open edges, which take the weather first.
        let worn = masks.wear.max(masks.weather * 0.5);
        let edge = (1.0 - inside / 0.5).clamp(0.0, 1.0);
        colour *= 1.0 + worn * edge * (0.5 + noise.at(p, 0.4, 29));
        *c = colour.to_array();
    };
    for (p, c) in surface.positions.iter().zip(surface.colours.iter_mut()) {
        shade(*p, c);
    }
    let shared = &mut surface.shared;
    for (p, c) in shared.positions.iter().zip(shared.colours.iter_mut()) {
        shade(*p, c);
    }
}

/// How many triangles a profile swept along a cube's edge has at most: along it in three
/// lengths, between its joints' chamfers, and its two ends closed.
pub fn trim_triangles(profile: &[[f32; 2]]) -> usize {
    let segments = profile.len().saturating_sub(1);
    segments * 3 * 2 + 2 * profile.len().saturating_sub(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rain_streaks_a_wall_under_its_sill_and_open_top_not_under_a_roof_and_wets_its_foot() {
        use crate::{BuildOp, Cube, Kind};
        let kind = Kind(4);
        let mut base = Base::default();
        // A wall eight cubes long and six high, a window two cubes up in it.
        base.apply(BuildOp::Fill {
            min: [0, 0, 0],
            max: [7, 5, 0],
            cube: Some(Cube::new(kind, Fill::Solid)),
        });
        base.apply(BuildOp::Fill {
            min: [2, 2, 0],
            max: [3, 3, 0],
            cube: Some(Cube::new(kind, Fill::Open)),
        });
        // A roof's cube over its east end.
        base.apply(BuildOp::Fill {
            min: [6, 6, 0],
            max: [7, 6, 0],
            cube: Some(Cube::new(Kind::FIRST_ROOF, Fill::Solid)),
        });
        let masks = Masks {
            dirt_tint: [0.5; 3],
            splash: 0.5,
            streaks: 1.0,
            streak_reach: 1.5,
            ..Masks::default()
        };
        let mut rained = Rained {
            base: &base,
            kind,
            feet: BTreeMap::new(),
            sources: BTreeMap::new(),
        };
        let out = Vec3::Z;
        // Somewhere along the face under the sill a streak darkens it; nowhere beside it.
        let darkest = |rained: &mut Rained, x0: f32, x1: f32, y: f32| {
            (0..40)
                .map(|i| {
                    let x = x0 + (x1 - x0) * i as f32 / 40.0;
                    rained.shade(&masks, Vec3::new(x, y, CELL_M), out).x
                })
                .fold(1.0f32, f32::min)
        };
        assert!(darkest(&mut rained, 1.0, 2.0, 0.8) < 0.9);
        assert!((darkest(&mut rained, 2.6, 3.4, 0.8) - 1.0).abs() < 1e-4);
        // Under the open top, streaked; under the roof, dry.
        assert!(darkest(&mut rained, 0.0, 1.0, 2.6) < 0.9);
        assert!((darkest(&mut rained, 3.0, 4.0, 2.6) - 1.0).abs() < 1e-4);
        // Wet over its foot, dry a metre up.
        let foot = rained.shade(&masks, Vec3::new(0.25, 0.05, CELL_M), out).x;
        assert!(foot < 0.65, "{foot}");
        let up = rained.shade(&masks, Vec3::new(0.25, 1.4, CELL_M), out).x;
        assert!((up - 1.0).abs() < 1e-4, "{up}");
    }

    #[test]
    fn a_bands_ends_at_a_joint_or_a_step_keep_its_part_in_the_wall_off_the_cubes_side() {
        // A band along a cube's face, half of it set into the wall, run on at one end and
        // stepping at the other: the ends of the part set in are cut back off the cube's
        // sides, where a step in the footing under it may have a face of its own; the part
        // standing out still meets the next band's at the line.
        let sweep = Sweep {
            from: Vec3::ZERO,
            to: Vec3::new(0.5, 0.0, 0.0),
            side: Vec3::Z,
            up: Vec3::Y,
            ends: [End::On, End::Step],
            extend: 0.0,
            joints: false,
            lift: 0.0,
            sink: 0.0,
            tint: [1.0; 3],
            grain: false,
        };
        let mut made = Indexed::default();
        let profile = [[-0.05, 0.0], [0.05, 0.0], [0.05, 0.1], [-0.05, 0.1]];
        sweep.draw(&profile, [1.0, 1.0], 1, &mut made);
        let at_end = |p: &Vec3| p.x < 1e-4 || p.x > 0.5 - 1e-4;
        assert!(made.positions.iter().any(|p| p.z > 0.0 && at_end(p)));
        for p in &made.positions {
            if p.z < -1e-3 {
                assert!(!at_end(p), "{p} set in at the cube's side");
            }
        }
    }

    #[test]
    fn a_face_looking_down_is_shaded_as_its_occlusion_would_be_and_a_side_is_not() {
        let masks = Masks::default();
        let mut noise = Noise::default();
        let mut at = |normal: Vec3| {
            masked(
                &masks,
                (false, false),
                Vec3::new(0.0, 2.0, 0.0),
                normal,
                0.0,
                &mut noise,
            )[1]
        };
        let (down, side, up) = (at(-Vec3::Y), at(Vec3::X), at(Vec3::Y));
        assert!(
            (side - 1.0).abs() < 1e-5 && (up - 1.0).abs() < 1e-5,
            "{side} {up}"
        );
        assert!(down < 0.6, "a soffit as lit as a wall: {down}");
        // A log's lower flank, half way down, by a fifth at most: it sees the sky too.
        let flank = at(Vec3::new(1.0, -1.0, 0.0).normalize());
        assert!(flank > 0.8, "{flank}");
    }

    #[test]
    fn an_empty_surface_takes_the_masks_without_a_fault() {
        // A section can draw nothing in a material; its masks then have no columns.
        let kit = crate::layout::tests::kit();
        let mut surface = Surface::default();
        mask(&mut surface, &kit, Material::Log, &Base::default());
        assert_eq!(surface.triangle_count(), 0);
    }

    #[test]
    fn a_simple_polygon_is_filled_by_its_ears() {
        let square = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        assert_eq!(ears(&square).len(), 2);
        // An L, six corners: four triangles.
        let ell = [
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [0.0, 2.0],
        ];
        assert_eq!(ears(&ell).len(), 4);
    }
}
