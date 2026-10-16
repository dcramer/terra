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
//! **Corner stones** (`Piece::Corner`): a dressed block for each course up an upright edge,
//! made from the material's own wall (D215 as amended): a bevelled block of the size its
//! pattern's numbers give, standing a little proud of both faces, cut from its own texture
//! and height map, the inside of one of its largest stones (`Relief::stones`) wrapped round
//! the block's faces; quoins long and short by turns, jamb stones long and short along the
//! wall. Nothing modelled is shared, so every block material's corners match its wall.
//!
//! Profiles are textured from the world's planes, as the cubes' surface is, each stone a
//! part of the texture of its own and a little lighter or darker, in the kit's `Dressed`
//! material.

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

/// How tall a course of dressed stone is: two to a cube.
pub const COURSE_M: f32 = CELL_M * 0.5;

/// How far along a band a point one metre over its line moves where it is mitred to a
/// ramp's raking band: the tangent of half the 45 degrees between them.
const BEND_MITRE: f32 = 0.414_213_56;

/// How wide and deep the chamfered joint between two stones of a band is.
const JOINT_M: f32 = 0.012;
const JOINT_DEPTH_M: f32 = 0.009;
/// How far a soffit hangs under the underside it runs along.
const SOFFIT_DROP_M: f32 = 0.004;
/// How far back a square end's part set into the wall is cut, for each metre it is in.
const SQUARE_RECESS: f32 = 0.2;
/// And a piece's whose part set in is all out of sight: a little past the diagonal.
const BURIED_RECESS: f32 = 1.05;
/// How far short of the cube's depth an opening's head or sill is sunk behind its piece.
const REVEAL_SHORT_M: f32 = 0.002;
/// How far past its half-cube step into what rises behind it a cut shape's band reaches.
const BAND_PAST_M: f32 = 0.004;
/// How far short of its square end a border stops.
const BORDER_SHORT_M: f32 = 0.003;
/// How far a corner stone may run along a face it shares with the cube's other edge's
/// stones: to half the cube, short of a joint, so the two meet and never pass.
const SHARED_REACH_M: f32 = CELL_M * 0.5 - JOINT_M * 0.5;

/// Faces meeting at more than about 50 degrees are shaded apart (docs/art-style.md).
const CREASE: f32 = 0.64;

/// Draws a dressed piece into the kit's `Dressed` material, a corner stone into its wall's
/// own (`Masonry`), cut from `wall`, its height map, where it is given; nothing if the kit
/// has no pieces for its role.
pub fn draw(
    piece: &Piece,
    kit: &Kit,
    wall: Option<&crate::relief::Relief>,
    surfaces: &mut Surfaces,
) {
    if let Piece::Corner {
        cube,
        role,
        faces,
        courses,
        shared,
        seed,
    } = *piece
    {
        let Some(Dress::Corners {
            long,
            short,
            proud,
            bevel,
            joint,
        }) = kit.dress.roles.get(&role)
        else {
            return;
        };
        let material = if kit.textures.contains_key(&Material::Masonry) {
            Material::Masonry
        } else {
            Material::Dressed
        };
        let metres = kit
            .textures
            .get(&material)
            .map_or([1.2, 1.2], |texture| texture.metres);
        let share = match kit.damage.get(&Material::Masonry) {
            Some(crate::kit::Breaks::Joints { share, .. }) => *share,
            _ => STONE_SHARE,
        };
        let stones = wall.map(|wall| wall.stones(share));
        let mut made = Indexed::default();
        corner(
            cube,
            (faces, shared),
            courses,
            &CornerStones {
                long,
                short,
                proud: *proud,
                bevel: *bevel,
                joint: *joint,
            },
            (
                stones
                    .as_deref()
                    .map(Vec::as_slice)
                    .filter(|s| !s.is_empty()),
                metres,
                kit.tint,
            ),
            seed,
            &mut made,
        );
        surfaces.of(material).add(made);
        return;
    }
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
            let (mut from, mut to) = (at(from), at(to));
            // A border sunk into the floor it lies on, ended square where that floor ends
            // in a face, stops a hair short of it, its end not in that face's plane.
            if role == layout::Role::Border {
                let along = (to - from).normalize_or_zero();
                if matches!(ends[0], End::Square | End::Free) {
                    from += along * BORDER_SHORT_M;
                }
                if matches!(ends[1], End::Square | End::Free) {
                    to -= along * BORDER_SHORT_M;
                }
            }
            let sweep = Sweep {
                from,
                to,
                side: unit(side),
                up: Vec3::Y,
                ends,
                extend: *extend,
                joints: *joints,
                // A soffit hangs a little under the underside it runs along, so its own
                // underside is not in that face's plane.
                lift: layout::metres(lift)
                    - if role == layout::Role::Soffit {
                        SOFFIT_DROP_M
                    } else {
                        0.0
                    },
                sink: layout::metres(sink),
                tint: kit.dress.tint,
                grain,
                buried: role == layout::Role::Foot,
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
            // On a shape cut in half cubes, a step a half cube deep: a nose modelled for a
            // step a cube deep reaches back no further than its own and a hair into what
            // rises behind it, not on into the stone of the stair over it, through whose
            // face its back would show, nor ending in the plane of the face behind it.
            let back = -CELL_M * 0.5 - BAND_PAST_M;
            let profile: Vec<[f32; 2]> = profile.iter().map(|&[x, y]| [x.max(back), y]).collect();
            let profile = &profile;
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
                buried: false,
            };
            sweep.draw(profile, metres, seed, &mut made);
        }
        _ => return,
    }
    surfaces.of(material).add(made);
}

/// The share of a wall's height map its joints are where its kit does not say (`Breaks`).
const STONE_SHARE: f32 = 0.22;

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
        // Sunk through the cube it is set into, so the head is behind the lintel on each
        // face of a wall a cube thick, and in a thicker one behind each lintel all the way
        // to the next cube's, never in the plane of either; not on the wall's other face.
        // Along it only its own ends bound it, not where it runs on into the next cube's,
        // whose bed takes over past it.
        if (!self.ends[0] && along < 0.0) || (!self.ends[1] && along > length) {
            return None;
        }
        let end = |own: bool, inset: f32| if own { inset } else { f32::MAX };
        Some(
            (CELL_M - REVEAL_SHORT_M - deep)
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
    /// Whether its part set into the wall is all out of sight, as a plinth's is: at a
    /// square end it is cut back square to the corner's diagonal, so two plinths meeting
    /// at an outside corner from its two faces, each ended square where the ground steps,
    /// keep each to its own side of it rather than lying in one another in the wall.
    buried: bool,
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
        // Shared with a cube corner to corner: out to the turn in the wall, back to the
        // plane halfway between the two out of it.
        let shared = self.ends.map(|end| end == End::Shared);
        let mitre_at = |which: usize, o: f32| {
            if shared[which] {
                if o > 0.0 { -1.0 } else { 1.0 }
            } else {
                mitre[which]
            }
        };
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
            End::Square | End::Free if (self.extend == 0.0 || end == End::Free) && self.buried => {
                BURIED_RECESS
            }
            End::Square | End::Free if self.extend == 0.0 || end == End::Free => SQUARE_RECESS,
            End::On | End::Step => SQUARE_RECESS,
            _ => 0.0,
        });
        let joint = self
            .ends
            .map(|end| self.joints && matches!(end, End::On | End::Step));
        // A raking band's square end cut plumb, as a raking coping's is, so nothing of its top
        // leans out past the cube's face it ends at: along it as far, for each metre up
        // square to it, as its up leans back over its own line.
        let level = Vec3::new(along.x, 0.0, along.z);
        let raking = along.y.abs() > 1e-3 && level.length() > 1e-3;
        let lean = if raking {
            let h = level.normalize();
            -self.up.dot(h) / along.dot(h)
        } else {
            0.0
        };
        let plumb = self.ends.map(|end| {
            if matches!(end, End::Square | End::Free) {
                lean
            } else {
                0.0
            }
        });
        let start = |o: f32, u: f32| {
            -extend[0] - o * mitre_at(0, o) + (-o).max(0.0) * recess[0] - (u - self.lift) * bend[0]
                + (u - self.lift) * plumb[0]
        };
        let end = |o: f32, u: f32| {
            length + extend[1] + o * mitre_at(1, o) - (-o).max(0.0) * recess[1]
                + (u - self.lift) * bend[1]
                + (u - self.lift) * plumb[1]
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
        // A plumb end looks level along it.
        let ahead = if raking { level.normalize() } else { along };
        for (which, ring, outward) in [(0, 0, -along), (1, rings.len() - 1, along)] {
            if mitre[which] != 0.0 || bend[which] != 0.0 || shared[which] {
                continue;
            }
            let outward = if plumb[which] != 0.0 {
                ahead * outward.dot(along).signum()
            } else {
                outward
            };
            let ring_points: Vec<Vec3> = (0..n).map(|i| point(ring, i, i.min(n - 2))).collect();
            let flat: Vec<[f32; 2]> = ring_points
                .iter()
                .map(|p| {
                    let rel = *p - self.from;
                    [rel.dot(self.side), rel.dot(self.up)]
                })
                .collect();
            let points: Vec<glam::Vec2> = flat.iter().map(|&p| p.into()).collect();
            for tri in crate::mesh::ear_clip(&points) {
                // None across points in a line in the section, a sliver with no area there:
                // where its end is cut back off the line, as a square end's part set into
                // the wall is, it would lie in a face of the piece's own, its underside.
                let [p, q, r] = tri.map(|i| flat[i]);
                if ((q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])).abs() < 1e-7 {
                    continue;
                }
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

/// A kind's corner stones, as its pattern gives their numbers (`Dress::Corners`).
struct CornerStones<'a> {
    long: &'a [[f32; 2]],
    short: &'a [[f32; 2]],
    proud: f32,
    bevel: f32,
    joint: f32,
}

/// A corner stone for each course of the cube's upright edge that shows one: a dressed
/// block of the wall's own stone, its foot at the course's, its corner the edge's, standing
/// `proud` of both faces, its long side along the first face and its short along the second,
/// turned each course where it has no `short` stones (quoins) or long and short by turns
/// (jamb stones). Where it shares a face with the cube's other edge's stones it reaches
/// along it no further than the face's middle, short of a joint. Each of its faces wears the
/// inside of one of the wall's own largest stones (`stones`, middles and sizes on its
/// texture), else the wall's texture at its own size; each a little lighter or darker.
fn corner(
    cube: IVec3,
    (faces, shared): ([Dir; 2], [bool; 2]),
    courses: [bool; 2],
    set: &CornerStones,
    (stones, metres, tint): (Option<&[crate::cells::Stone]>, [f32; 2], [f32; 3]),
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
        let pick = |sizes: &[[f32; 2]]| -> Option<[f32; 2]> {
            (!sizes.is_empty()).then(|| sizes[hash(&[stone_seed, 9]) as usize % sizes.len()])
        };
        let (size, [a, b]) = if set.short.is_empty() {
            (
                pick(set.long),
                if turn { [faces[1], faces[0]] } else { faces },
            )
        } else {
            (pick(if turn { set.short } else { set.long }), faces)
        };
        let Some([along_a, along_b]) = size else {
            continue;
        };
        let shares = |face: Dir| faces.iter().zip(shared).any(|(f, s)| *f == face && s);
        let reach = |shares: bool, length: f32| {
            if shares {
                length.min(SHARED_REACH_M + set.joint)
            } else {
                length
            }
        };
        // In its own frame: its +Z face on the first face (a), running back along -X; its +X
        // face on the second (b), running back along -Z; a course high less its joints.
        let (along_a, along_b) = (reach(shares(a), along_a), reach(shares(b), along_b));
        let low = Vec3::new(-along_a + set.joint, set.joint, -along_b + set.joint);
        let high = Vec3::new(set.proud, COURSE_M - set.joint, set.proud);
        let turned = Mat3::from_cols(unit(b), Vec3::Y, unit(a));
        let mirrored = turned.determinant() < 0.0;
        let at = base + Vec3::Y * (course as f32 * COURSE_M);
        let (colour, shift) = stone_look(stone_seed, tint);
        // Each of its faces cut from a stone of its own, across the face and down it (up
        // it, its top and bottom), its middle at the stone's, as large as the stone allows
        // without a joint showing; else the wall's texture at its own size.
        let face_of = |n: Vec3| -> usize {
            let n = n.abs();
            if n.y >= n.x && n.y >= n.z {
                2
            } else if n.x > n.z {
                1
            } else {
                0
            }
        };
        let flat = |face: usize, p: Vec3| -> [f32; 2] {
            match face {
                0 => [p.x, -p.y],
                1 => [-p.z, -p.y],
                _ => [p.x, p.z],
            }
        };
        // Each face at the wall texture's own scale, never larger (D362): the inside of
        // one of the wall's larger stones, laid again mirrored beside itself as far as the
        // face reaches, so a face larger than the stone is the same rock carried on, not
        // one stone blown up to it (two and a half times, before).
        let centre_of = (low + high) * 0.5;
        let scale = 1.0 / metres[0].max(0.01);
        let cut = |face: usize| -> Tiling {
            let middle = flat(face, centre_of);
            let (a, b) = (flat(face, low), flat(face, high));
            let origin = [a[0].min(b[0]), a[1].min(b[1])];
            match stones {
                Some(stones) => {
                    let (centre, radius) =
                        pick_stone(stones, hash(&[stone_seed, 11 + face as u32]));
                    let half = (radius * STONE_INSIDE).max(MIN_TILE_M * 0.5 * scale);
                    Tiling {
                        centre,
                        middle,
                        origin,
                        scale,
                        tile: 2.0 * half / scale,
                    }
                }
                None => Tiling {
                    centre: shift,
                    middle,
                    origin,
                    scale,
                    tile: f32::INFINITY,
                },
            }
        };
        let cuts = [cut(0), cut(1), cut(2)];
        let first = out.positions.len() as u32;
        let triangles = bevelled_box(low, high, set.bevel);
        for corners in triangles.chunks_exact(3) {
            let face = face_of(corners[0].1);
            let tiling = &cuts[face];
            for (p, n, uv) in tiling.lay(corners, |p| flat(face, p)) {
                out.corner(
                    at + turned * p,
                    (turned * n).normalize_or(Vec3::Y),
                    uv,
                    colour,
                );
            }
        }
        let count = out.positions.len() as u32 - first;
        for t in (0..count).step_by(3) {
            let [x, y, z] = [first + t, first + t + 1, first + t + 2];
            if mirrored {
                out.indices.extend([x, z, y]);
            } else {
                out.indices.extend([x, y, z]);
            }
        }
    }
}

/// How much of a stone's inside, from its middle to its nearest joint, half a corner stone's
/// face's copy of it spans: a square whose corners stay within the stone (under 1/sqrt 2),
/// so no joint shows in the face.
const STONE_INSIDE: f32 = 0.68;
/// The least a face's copy of its stone covers, in metres: a smaller stone is drawn larger
/// than its wall's, so a face is not cut into slivers.
const MIN_TILE_M: f32 = 0.12;

/// One of a wall's larger stones for a seed: among those at least half the largest's size.
fn pick_stone(stones: &[crate::cells::Stone], seed: u32) -> crate::cells::Stone {
    let largest = stones.iter().map(|s| s.1).fold(0.0, f32::max);
    let large: Vec<_> = stones.iter().filter(|s| s.1 >= largest * 0.5).collect();
    *large[seed as usize % large.len().max(1)]
}

/// How a corner stone's face wears its stone: at the wall's own scale (`scale`, texture a
/// metre), the stone's inside, `tile` metres across, laid from the face's corner and again
/// mirrored beside itself, so the texture runs on unbroken and nothing is drawn larger than
/// the wall draws it.
struct Tiling {
    centre: [f32; 2],
    middle: [f32; 2],
    /// Where the face's copies start, its corner: as few as cover it.
    origin: [f32; 2],
    scale: f32,
    tile: f32,
}

impl Tiling {
    /// A face's triangle, cut where its copies of the stone meet, each piece's corners
    /// with their texture places: positions, normals and places, three a triangle.
    fn lay(
        &self,
        corners: &[(Vec3, Vec3)],
        flat: impl Fn(Vec3) -> [f32; 2],
    ) -> Vec<(Vec3, Vec3, [f32; 2])> {
        let n = corners[0].1;
        // Each corner's place in tiles, from the face's corner.
        let at = |p: Vec3| -> [f32; 2] {
            let t = flat(p);
            [0, 1].map(|i| (t[i] - self.origin[i]) / self.tile)
        };
        let mut out = Vec::new();
        if !self.tile.is_finite() {
            // No stones: the wall's texture at its own size, as it runs.
            for &(p, _) in corners {
                let t = flat(p);
                let uv = [0, 1].map(|i| self.centre[i] + (t[i] - self.middle[i]) * self.scale);
                out.push((p, n, uv));
            }
            return out;
        }
        let points: Vec<(Vec3, [f32; 2])> = corners.iter().map(|&(p, _)| (p, at(p))).collect();
        let lo = [0, 1].map(|i| {
            points
                .iter()
                .map(|q| q.1[i])
                .fold(f32::MAX, f32::min)
                .floor() as i32
        });
        let hi = [0, 1].map(|i| {
            points
                .iter()
                .map(|q| q.1[i])
                .fold(f32::MIN, f32::max)
                .ceil() as i32
        });
        for i in lo[0]..hi[0] {
            for j in lo[1]..hi[1] {
                let mut poly = points.clone();
                for (axis, edge, keep_above) in [
                    (0, i, true),
                    (0, i + 1, false),
                    (1, j, true),
                    (1, j + 1, false),
                ] {
                    poly = clip(&poly, axis, edge as f32, keep_above);
                    if poly.len() < 3 {
                        break;
                    }
                }
                if poly.len() < 3 {
                    continue;
                }
                for k in 1..poly.len() - 1 {
                    for &(p, t) in [&poly[0], &poly[k], &poly[k + 1]] {
                        out.push((p, n, self.place(t, [i, j])));
                    }
                }
            }
        }
        out
    }

    /// The texture's place for a point `t` tiles in, in tile `cell`: mirrored in every
    /// other tile each way, so neighbouring copies meet on the same texels.
    fn place(&self, t: [f32; 2], cell: [i32; 2]) -> [f32; 2] {
        let half = self.tile * 0.5 * self.scale;
        [0, 1].map(|i| {
            let mut f = t[i] - cell[i] as f32;
            if cell[i].rem_euclid(2) == 1 {
                f = 1.0 - f;
            }
            self.centre[i] - half + f * 2.0 * half
        })
    }
}

/// A convex polygon cut by a line of tiles, keeping the side above or below it.
fn clip(
    poly: &[(Vec3, [f32; 2])],
    axis: usize,
    edge: f32,
    keep_above: bool,
) -> Vec<(Vec3, [f32; 2])> {
    let inside = |q: &(Vec3, [f32; 2])| {
        if keep_above {
            q.1[axis] >= edge
        } else {
            q.1[axis] <= edge
        }
    };
    let mut out = Vec::new();
    for k in 0..poly.len() {
        let (a, b) = (&poly[k], &poly[(k + 1) % poly.len()]);
        if inside(a) {
            out.push(*a);
        }
        if inside(a) != inside(b) {
            let s = (edge - a.1[axis]) / (b.1[axis] - a.1[axis]);
            let mut t = [
                a.1[0] + (b.1[0] - a.1[0]) * s,
                a.1[1] + (b.1[1] - a.1[1]) * s,
            ];
            t[axis] = edge;
            out.push((a.0 + (b.0 - a.0) * s, t));
        }
    }
    out
}

/// A box from `low` to `high` with its twelve edges bevelled by `bevel` and its eight
/// corners cut, as triangles, three corners each, each with its own face's normal, wound to
/// face out: a dressed block's hard arrises.
fn bevelled_box(low: Vec3, high: Vec3, bevel: f32) -> Vec<(Vec3, Vec3)> {
    let half = (high - low) * 0.5;
    let middle = (high + low) * 0.5;
    let b = bevel.min(half.min_element() * 0.45);
    let inner = half - Vec3::splat(b);
    let point = |sign: Vec3, out: [bool; 3]| -> Vec3 {
        middle
            + Vec3::new(
                sign.x * if out[0] { half.x } else { inner.x },
                sign.y * if out[1] { half.y } else { inner.y },
                sign.z * if out[2] { half.z } else { inner.z },
            )
    };
    let mut triangles = Vec::new();
    let mut add = |corners: &[Vec3]| {
        let centre = corners.iter().copied().sum::<Vec3>() / corners.len() as f32;
        for i in 1..corners.len() - 1 {
            let (mut p, q, mut r) = (corners[0], corners[i], corners[i + 1]);
            let mut normal = (q - p).cross(r - p).normalize_or_zero();
            if normal.dot(centre - middle) < 0.0 {
                std::mem::swap(&mut p, &mut r);
                normal = -normal;
            }
            triangles.extend([(p, normal), (q, normal), (r, normal)]);
        }
    };
    let signs = [-1.0f32, 1.0];
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
            add(&quad);
        }
    }
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
                add(&quad);
            }
        }
    }
    for sx in signs {
        for sy in signs {
            for sz in signs {
                let sign = Vec3::new(sx, sy, sz);
                add(&[0, 1, 2].map(|axis| {
                    let mut out = [false; 3];
                    out[axis] = true;
                    point(sign, out)
                }));
            }
        }
    }
    triangles
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

/// How high moss climbs a timber wall from the ground, in metres, its top wandering.
const TIMBER_MOSS_M: f32 = 0.7;

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
    // Timber's foot, where the splash keeps it damp: moss in blotches, thinning upward.
    if timber && masks.moss > 0.0 && height < TIMBER_MOSS_M * 1.4 {
        let rise = TIMBER_MOSS_M * (0.6 + 0.8 * noise.at(p, 0.5, 41));
        let low = (1.0 - height / rise).clamp(0.0, 1.0);
        let blotch = ((noise.at(p, 0.22, 43) - 0.35) / 0.4).clamp(0.0, 1.0);
        let moss = (masks.moss * low * blotch).clamp(0.0, 0.85);
        c = c.lerp(c * Vec3::from_array(masks.moss_tint), moss);
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
    crate::light_share(1.0 - UNDERSIDE * t * t * (3.0 - 2.0 * t))
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

/// What a roof over a wall hides of the sky from what faces up under it, shaded as baked
/// occlusion would (D320): a log's or a stone's upper face under an eave, a verge or a
/// ceiling sees the roof's underside, not the sky, which lit the worn pale tops of logs on a
/// gable and indoors blue-white. Up to `SHELTERED` darker for a face looking straight up
/// with a roof close over it, fading with the roof's height over it to `SHELTER_REACH_M`.
pub fn sheltered(surface: &mut Surface, roofs: &crate::layout::roofs::Roofs) {
    if roofs.is_empty() {
        return;
    }
    let shade = |p: Vec3, n: Vec3, c: &mut [f32; 3]| {
        let up = n.y.max(0.0);
        if up < 0.05 {
            return;
        }
        // A hair out from the face, so a wall's own top under its roof's eave counts.
        let at = p + Vec3::new(n.x, 0.0, n.z) * 0.05;
        let Some(top) = roofs.top_over(at.x, at.z, p.y + 0.05) else {
            return;
        };
        let near = 1.0 - ((top - p.y) / SHELTER_REACH_M).clamp(0.0, 1.0);
        let k = crate::light_share(1.0 - SHELTERED * up * (0.4 + 0.6 * near));
        *c = c.map(|v| v * k);
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

/// How much darker a face looking up under a roof is, at most, and how high over it a roof
/// still shades it.
const SHELTERED: f32 = 0.4;
const SHELTER_REACH_M: f32 = 4.0;

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
    fn a_corner_stone_is_cut_from_inside_one_of_the_walls_own_stones_at_its_size() {
        use crate::kit::Dress;
        // A wall's height map of square stones 64 texels across in joints 8 wide.
        const SIDE: usize = 512;
        let heights = (0..SIDE * SIDE)
            .map(|i| {
                if (i % SIDE) % 64 < 8 || (i / SIDE) % 64 < 8 {
                    20
                } else {
                    220
                }
            })
            .collect();
        let wall = crate::relief::Relief::new(SIDE, SIDE, heights).unwrap();
        let mut kit = Kit::default();
        kit.textures.insert(
            Material::Masonry,
            crate::kit::Texture {
                path: "wall".into(),
                metres: [2.0, 2.0],
                depth: 0.02,
                reflectance: crate::kit::matte(),
                strips: None,
                cells: 1,
                worn: None,
            },
        );
        let (proud, joint, long) = (0.018, 0.007, [0.42, 0.23]);
        kit.dress.roles.insert(
            layout::Role::Arris,
            Dress::Corners {
                long: vec![long],
                short: Vec::new(),
                proud,
                bevel: 0.016,
                joint,
            },
        );
        let piece = Piece::Corner {
            cube: IVec3::ZERO,
            role: layout::Role::Arris,
            faces: [Dir::North, Dir::West],
            courses: [true, false],
            shared: [false, false],
            seed: 7,
        };
        let mut surfaces = Surfaces::default();
        draw(&piece, &kit, Some(&wall), &mut surfaces);
        let stone = &surfaces.0[&Material::Masonry];
        assert!(!stone.positions.is_empty(), "in the wall's own material");
        assert!(
            stone.triangle_count() <= crate::checker::CORNER_STONE,
            "{}",
            stone.triangle_count()
        );
        // Its size the pattern's numbers: so long along each face less its joint, out by
        // its proud, a course high less its joints.
        let (low, high) = stone.positions.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(l, h), p| (l.min(*p), h.max(*p)),
        );
        let size = high - low;
        let along: Vec<f32> = [size.x, size.z].to_vec();
        let wanted = [long[0] - joint + proud, long[1] - joint + proud];
        assert!(
            (along.iter().any(|a| (a - wanted[0]).abs() < 1e-3))
                && along.iter().any(|a| (a - wanted[1]).abs() < 1e-3),
            "{size}"
        );
        assert!((size.y - (COURSE_M - 2.0 * joint)).abs() < 1e-3, "{size}");
        assert!(
            (low.x + proud).abs() < 1e-3 && (low.z + proud).abs() < 1e-3,
            "proud of both faces: {low}"
        );
        // Every corner of it on one stone of the wall's picture, well inside its edges: no
        // joint shows on a block.
        let cells = wall.cells(STONE_SHARE);
        for uv in &stone.uvs {
            let inside = crate::cells::standing(&cells, *uv, &|_| 0.0, 2.0, 0.0);
            assert!(inside > 1.0, "{uv:?} is {inside} texels inside a stone");
        }
        // And no face drawn larger than the wall draws its texture, two metres a copy: each
        // triangle's metres along its texture's two ways no more than two a copy.
        let all = stone.indexed();
        for t in all.indices.chunks_exact(3) {
            let [p, q, r] = [0, 1, 2].map(|k| all.positions[t[k] as usize]);
            let [a, b, c] = [0, 1, 2].map(|k| all.uvs[t[k] as usize]);
            let (d1, d2) = ([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]]);
            let det = d1[0] * d2[1] - d1[1] * d2[0];
            // Its faces; a bevel's slant is its arris's, a centimetre or two across.
            let n = (q - p).cross(r - p);
            if det.abs() < 1e-9 || n.length() < 1e-7 || n.normalize().abs().max_element() < 0.99 {
                continue;
            }
            let ju = ((q - p) * d2[1] - (r - p) * d1[1]) / det;
            let jv = ((r - p) * d1[0] - (q - p) * d2[0]) / det;
            for way in [ju, jv] {
                assert!(
                    way.length() <= 2.0 * 1.02,
                    "drawn {} metres a copy",
                    way.length()
                );
            }
        }
    }

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
    fn a_square_ends_cap_lays_nothing_over_the_pieces_own_underside() {
        // A coping's section, its underside three points in a line from its drip back into
        // the wall: at a square end the part set in is cut back off the line, so a cap
        // across those three would lie in the underside's plane, over its own faces.
        let sweep = Sweep {
            from: Vec3::ZERO,
            to: Vec3::new(0.5, 0.0, 0.0),
            side: Vec3::Z,
            up: Vec3::Y,
            ends: [End::Square, End::Square],
            extend: 0.0,
            joints: false,
            lift: 0.0,
            sink: 0.0,
            tint: [1.0; 3],
            grain: false,
            buried: false,
        };
        let mut made = Indexed::default();
        let profile = [
            [-0.05, 0.05],
            [0.06, 0.03],
            [0.06, -0.1],
            [0.004, -0.1],
            [0.0, -0.1],
            [-0.05, -0.1],
        ];
        sweep.draw(&profile, [1.0, 1.0], 1, &mut made);
        let surface = made.loose();
        let folded = crate::inspect::self_overlap(&surface);
        assert!(folded < 1e-7, "{} cm² over itself", folded * 1e4);
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
            buried: false,
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
        // In light, as drawn: the colour, as on screen, made linear.
        let mut at = |normal: Vec3| {
            let c = masked(
                &masks,
                (false, false),
                Vec3::new(0.0, 2.0, 0.0),
                normal,
                0.0,
                &mut noise,
            )[1];
            libm::powf((c + 0.055) / 1.055, 2.4)
        };
        let (down, side, up) = (at(-Vec3::Y), at(Vec3::X), at(Vec3::Y));
        assert!(
            (side - 1.0).abs() < 1e-5 && (up - 1.0).abs() < 1e-5,
            "{side} {up}"
        );
        assert!(down < 0.6, "a soffit as lit as a wall: {down}");
        // Two thirds of its light taken at most, as drawn, not nine tenths (D373).
        assert!(down > 0.3, "a soffit crushed to black: {down}");
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
}
