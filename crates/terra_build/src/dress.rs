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
use crate::{Base, CELL_M};
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
}

/// How tall a course of dressed stone is: two to a cube.
pub const COURSE_M: f32 = CELL_M * 0.5;

/// How wide and deep the chamfered joint between two stones of a band is.
const JOINT_M: f32 = 0.012;
const JOINT_DEPTH_M: f32 = 0.009;

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
                faces,
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
        let extend = self.ends.map(|end| match end {
            End::Square => self.extend,
            _ => 0.0,
        });
        let joint = self
            .ends
            .map(|end| self.joints && matches!(end, End::On | End::Step));
        let start = |o: f32| -extend[0] - o * mitre[0];
        let end = |o: f32| length + extend[1] + o * mitre[1];
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
            let (low, high) = (start(points[i][0]), end(points[i][0]));
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
            let (low, high) = (start(points[i][0]), end(points[i][0]));
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
            if mitre[which] != 0.0 {
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
    faces: [Dir; 2],
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
        let first = out.positions.len() as u32;
        for (p, n) in stone.positions.iter().zip(&stone.normals) {
            let p = at + turned * Vec3::from_array(*p);
            let n = (turned * Vec3::from_array(*n)).normalize_or(Vec3::Y);
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

/// Smooth value noise over a lattice so many metres across, from 0 to 1.
fn smooth(p: Vec3, size: f32, seed: u32) -> f32 {
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
/// outside edges worn lighter by their normal's turn, and, for timber, grey higher up.
fn masked(
    masks: &Masks,
    (timber, layered): (bool, bool),
    p: Vec3,
    normal: Vec3,
    ground: f32,
) -> [f32; 3] {
    let mut c = Vec3::ONE;
    // A block kind's lifts, each a little lighter or darker, wandering along the wall.
    if layered && masks.strata > 0.0 {
        // Each lift its own shade, a darker seam where one was rammed on the last, the
        // seams wandering a few centimetres up and down along the wall.
        let lift = masks.lift.max(0.05);
        let y = p.y / lift + 0.35 * (smooth(p, 1.1, 11) - 0.5);
        let layer = libm::floorf(y);
        let within = y - layer;
        let own = layout::unit(hash(&[layer as i32 as u32, 17])) - 0.5;
        let seam = (1.0 - (within.min(1.0 - within) / 0.1).min(1.0)) * 0.45;
        let speckle = smooth(p, 0.25, 13) - 0.5;
        c *= 1.0 + masks.strata * (own * 1.4 + speckle * 0.5 - seam);
    }
    let height = p.y - ground;
    if masks.dirt > 0.0 {
        let rise = masks.dirt * (0.6 + 0.8 * smooth(p, 0.45, 3));
        let dirt = libm::powf((1.0 - height / rise).clamp(0.0, 1.0), 1.5);
        c = c.lerp(Vec3::from_array(masks.dirt_tint), dirt);
    }
    if masks.wear > 0.0 {
        let a = normal.abs();
        let round = ((1.0 - a.x.max(a.y).max(a.z)) / 0.29).clamp(0.0, 1.0);
        c *= 1.0 + masks.wear * round;
    }
    if timber && masks.weather > 0.0 {
        let grey = masks.weather * (height / 3.0).clamp(0.0, 1.0) * (0.5 + smooth(p, 0.8, 5));
        let luma = (c.x + c.y + c.z) / 3.0;
        c = c.lerp(Vec3::splat(luma * 1.05), grey.clamp(0.0, 1.0));
    }
    c.to_array()
}

/// A kit's masks laid over a surface it drew in a material, in its base's own space.
pub fn mask(surface: &mut Surface, kit: &Kit, material: Material, base: &Base) {
    let masks = &kit.dress.masks;
    if masks.dirt <= 0.0 && masks.wear <= 0.0 && masks.weather <= 0.0 && masks.strata <= 0.0 {
        return;
    }
    let timber = matches!(
        material,
        Material::Log | Material::LogEnd | Material::Board | Material::Daub
    );
    let layered = material == Material::Masonry;
    // The ground's height by column, looked up once each.
    let mut known: BTreeMap<[i32; 2], f32> = BTreeMap::new();
    let mut ground = |p: Vec3| {
        let column = [
            libm::floorf(p.x / CELL_M) as i32,
            libm::floorf(p.z / CELL_M) as i32,
        ];
        *known
            .entry(column)
            .or_insert_with(|| layout::metres(layout::roles::ground_ticks(base, column)))
    };
    let mut shade = |p: Vec3, n: Vec3, c: &mut [f32; 3]| {
        let k = masked(masks, (timber, layered), p, n, ground(p));
        *c = [c[0] * k[0], c[1] * k[1], c[2] * k[2]];
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
pub fn roof_mask(surface: &mut Surface, kit: &Kit, base: &Base) {
    let masks = &kit.dress.masks;
    if masks.moss <= 0.0 && masks.weather <= 0.0 && masks.wear <= 0.0 {
        return;
    }
    // Each roof: its sections, the walls' top under it and its highest rise.
    let sections: Vec<(crate::RoofSection, f32, f32)> = base
        .sections()
        .iter()
        .map(|section| {
            let (low, high) = section.bounds();
            let span = high - low;
            let run = match (section.style, section.along_x) {
                (crate::RoofStyle::Hip, _) => span.x.min(span.y) * 0.5,
                (crate::RoofStyle::Gable, true) => span.y * 0.5,
                (crate::RoofStyle::Gable, false) => span.x * 0.5,
                (crate::RoofStyle::LeanTo, true) => span.y,
                (crate::RoofStyle::LeanTo, false) => span.x,
            };
            let eaves = (section.storey() + 1) as f32 * crate::STOREY_M;
            (*section, eaves, (run * section.pitch).max(0.1))
        })
        .collect();
    let reach = kit.eaves + 0.3;
    let shade = |p: Vec3, c: &mut [f32; 3]| {
        let flat = glam::Vec2::new(p.x, p.z);
        // The section whose covering is nearest the point's height.
        let found = sections
            .iter()
            .filter_map(|(section, eaves, highest)| {
                let rise = section.rise_beyond(flat, reach)?;
                let (low, high) = section.bounds();
                let inside = (flat - low).min(high - flat).min_element();
                Some(((eaves + rise - p.y).abs(), rise / highest, inside))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, up, inside)) = found else {
            return;
        };
        let up = up.clamp(-0.5, 1.0);
        let mut colour = Vec3::from_array(*c);
        if masks.moss > 0.0 {
            let blotch = smooth(p, 0.6, 17) * 0.7 + smooth(p, 2.1, 19) * 0.6;
            let low = libm::powf((1.0 - up).clamp(0.0, 1.0), 1.5);
            let moss = (masks.moss * low * blotch * 1.8).clamp(0.0, 0.85);
            colour = colour.lerp(colour * Vec3::from_array(masks.moss_tint), moss);
        }
        if masks.weather > 0.0 {
            let grey = masks.weather * up.max(0.0) * (0.4 + smooth(p, 1.3, 23));
            let luma = (colour.x + colour.y + colour.z) / 3.0;
            colour = colour.lerp(Vec3::splat(luma * 1.08), grey.clamp(0.0, 1.0));
        }
        // Lighter along its open edges, which take the weather first.
        let worn = masks.wear.max(masks.weather * 0.5);
        let edge = (1.0 - (inside + kit.eaves) / 0.5).clamp(0.0, 1.0);
        colour *= 1.0 + worn * edge * (0.5 + smooth(p, 0.4, 29));
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
