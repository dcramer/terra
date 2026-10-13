//! Candidate 3, profiles and corner pieces: candidate 2's surface and masks, and over it
//! the kit's 2D profiles (drawn in Blender, `blender/pieces.py`) swept along the role
//! edges the role function finds, mitred where two runs meet at a corner and capped where
//! one stops: a coping along every crest, a plinth along every foot, a sill along a hole's
//! bottom, a lintel along its head running on into the wall either side, a fascia along
//! an overhang's edge. Up every outside corner Blender-modelled quoins, long and short by
//! turns, fitted between the plinth and the coping; up every hole's sides jamb stones.
//! Each piece stands out an eighth of a cube at most (D192 as amended), or, for reference,
//! stays inside the cubes with the rubble set back behind it.
//!
//! Timber has its own pieces by role, from the same generator as its logs (D200): a hewn
//! plate along every crest, a hewn sill on whatever it stands on, a post up every free end,
//! and log ends running past the corners.

use crate::layers::{self, Blocks, COURSE_M, Features, Mode, masks, noise, stone_id};
use crate::masonry::planar;
use crate::model::Model;
use crate::roles::{Beyond, Edge, Neighbours, Run, mitres, runs};
use crate::shapes::Case;
use crate::today::{self, Laid};
use crate::{ASSETS, Drawn, Reliefs, add, add_surfaces, folder};
use glam::{IVec3, Mat3, Quat, Vec3};
use std::collections::BTreeMap;
use terra_build::kit::{Kit, Kits, Pattern, Profile};
use terra_build::layout::{Piece, Place, Split};
use terra_build::{CELL_M, Material, Surface, Surfaces};

/// The pieces authored in Blender.
pub struct Pieces {
    /// Profiles by name, overhanging and flush.
    pub over: BTreeMap<String, Vec<[f32; 2]>>,
    pub flush: BTreeMap<String, Vec<[f32; 2]>>,
    /// Corner stones by name: `quoin_over_0`, `jamb_long_flush_2`…
    pub stones: BTreeMap<String, Model>,
}

impl Pieces {
    pub fn load() -> Pieces {
        let text = std::fs::read_to_string("out/profiles.json")
            .expect("out/profiles.json: run blender/pieces.py first (README.md)");
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        let read = |which: &str| -> BTreeMap<String, Vec<[f32; 2]>> {
            json[which]
                .as_object()
                .unwrap()
                .iter()
                .map(|(name, points)| {
                    let points = points
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| [p[0].as_f64().unwrap() as f32, p[1].as_f64().unwrap() as f32])
                        .collect();
                    (name.clone(), points)
                })
                .collect()
        };
        let stones = Model::load_nodes("out/pieces.glb", &format!("{ASSETS}/models/spike"), Some(()))
            .into_iter()
            .collect();
        Pieces {
            over: read("over"),
            flush: read("flush"),
            stones,
        }
    }

    fn profile(&self, name: &str, overhang: bool) -> &[[f32; 2]] {
        let set = if overhang { &self.over } else { &self.flush };
        &set[name]
    }
}

/// How far the rubble is set back behind flush pieces, so they read.
const FLUSH_INSET_M: f32 = 0.03;
/// How far a lintel runs into the wall either side of its opening, and a sill's horns.
const BEARING_M: f32 = 0.2;
const HORN_M: f32 = 0.06;
/// A joint between two stones of a band: how wide its chamfer is and how deep.
const JOINT_M: f32 = 0.012;
const JOINT_DEPTH_M: f32 = 0.008;

/// Faces meeting at more than about 50 degrees are shaded apart (docs/art-style.md).
const CREASE: f32 = 0.64;

/// Turns and mirrors a piece so its +X face looks along `a` and its +Z face along `b`.
pub fn orient(a: Vec3, b: Vec3) -> (Quat, Vec3) {
    let c = a.cross(Vec3::Y);
    if c.dot(b) > 0.5 {
        (Quat::from_mat3(&Mat3::from_cols(a, Vec3::Y, b)), Vec3::ONE)
    } else {
        (
            Quat::from_mat3(&Mat3::from_cols(a, Vec3::Y, c)),
            Vec3::new(1.0, 1.0, -1.0),
        )
    }
}

/// A placed piece's triangles textured in world space from their planes, as the surface
/// is, its texture's place shifted by its number, and shaded by the masks.
pub fn place_stone(
    model: &Model,
    at: (Vec3, Quat, Vec3),
    metres: [f32; 2],
    id: u32,
    features: &Features,
    ground: fn(f32, f32) -> f32,
    out: &mut Surface,
) {
    let mut s = Surface::default();
    model.place(at, &mut s);
    texture(&mut s, metres, id, features, ground);
    crate::add_to(out, s);
}

/// World-space planar texture and masks on a surface's triangles, by each one's facing.
pub fn texture(
    s: &mut Surface,
    metres: [f32; 2],
    id: u32,
    features: &Features,
    ground: fn(f32, f32) -> f32,
) {
    let shift = [noise(&[id as i32, 1]), noise(&[id as i32, 2])];
    let shade = 0.88 + 0.16 * noise(&[id as i32, 3]);
    for t in 0..s.positions.len() / 3 {
        let p = &s.positions[t * 3..t * 3 + 3];
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or(Vec3::Y);
        for k in 0..3 {
            let uv = planar(p[k], n, metres);
            s.uvs[t * 3 + k] = [uv[0] + shift[0], uv[1] + shift[1]];
            let m = masks(features, ground, p[k], s.normals[t * 3 + k], 0.0);
            let g = layers::DRESSED_TINT;
            let own = s.colours[t * 3 + k];
            s.colours[t * 3 + k] = [
                own[0] * m[0] * shade * g[0],
                own[1] * m[1] * shade * g[1],
                own[2] * m[2] * shade * g[2],
            ];
        }
    }
}

/// A profile's outward normal at each of its segments, in (out, up).
fn segment_normals(profile: &[[f32; 2]]) -> Vec<[f32; 2]> {
    // Which side is solid: by the winding of the profile closed on itself.
    let area: f32 = (0..profile.len())
        .map(|i| {
            let (a, b) = (profile[i], profile[(i + 1) % profile.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    profile
        .windows(2)
        .map(|w| {
            let d = [w[1][0] - w[0][0], w[1][1] - w[0][1]];
            let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
            // Counter-clockwise: the solid is on the left, so out is to the right.
            if area > 0.0 {
                [d[1] / l, -d[0] / l]
            } else {
                [-d[1] / l, d[0] / l]
            }
        })
        .collect()
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
    let mut guard = 0;
    while idx.len() > 3 && guard < 10_000 {
        guard += 1;
        let n = idx.len();
        let mut clipped = false;
        for i in 0..n {
            let (ia, ib, ic) = (idx[(i + n - 1) % n], idx[i], idx[(i + 1) % n]);
            let (a, b, c) = (points[ia], points[ib], points[ic]);
            let turn = cross(a, b, c);
            if (turn > 1e-12) != ccw {
                continue;
            }
            let inside = idx.iter().any(|&j| {
                if j == ia || j == ib || j == ic {
                    return false;
                }
                let p = points[j];
                let s = |u: [f32; 2], v: [f32; 2]| cross(u, v, p);
                let (d1, d2, d3) = (s(a, b), s(b, c), s(c, a));
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

/// A profile swept along a run: from `from` to `to`, its (out, up) section laid out along
/// `side` and up, run on past each end by `extend` and mitred there by `mitre` (as
/// `roles::mitres`), cut into stones a band's length with chamfered joints between; ends
/// that are not mitred are capped.
#[allow(clippy::too_many_arguments)]
pub fn sweep(
    from: Vec3,
    to: Vec3,
    side: Vec3,
    profile: &[[f32; 2]],
    extend: [f32; 2],
    mitre: [f32; 2],
    stones: bool,
    out: &mut Vec<(u32, Surface)>,
    seed: i32,
) {
    let length = from.distance(to);
    let t = (to - from) / length;
    let normals = segment_normals(profile);
    let n = profile.len();
    // Each vertex's normal for each of the two segments it joins: shared if they meet at
    // less than the crease, else its own segment's.
    let vertex_normal = |seg: usize, end: usize| -> [f32; 2] {
        let own = normals[seg];
        let other = if end == 0 {
            seg.checked_sub(1).map(|s| normals[s])
        } else {
            normals.get(seg + 1).copied()
        };
        match other {
            Some(o) if own[0] * o[0] + own[1] * o[1] > CREASE => {
                let s = [own[0] + o[0], own[1] + o[1]];
                let l = (s[0] * s[0] + s[1] * s[1]).sqrt();
                [s[0] / l, s[1] / l]
            }
            _ => own,
        }
    };
    // Where each point's section starts and ends along the run.
    let start = |o: f32| -extend[0] - o * mitre[0];
    let end = |o: f32| length + extend[1] + o * mitre[1];
    // Stones: joints where the bands' joints fall, half a stone from the cubes' lines.
    let lo = -extend[0] - 0.1;
    let hi = length + extend[1] + 0.1;
    let offset = from.x + from.z + layers::BAND_STONE_M * 0.5;
    let mut cuts = vec![lo];
    if stones {
        let first = ((lo + offset) / layers::BAND_STONE_M).ceil() as i32;
        let last = ((hi + offset) / layers::BAND_STONE_M).floor() as i32;
        for k in first..=last {
            let at = k as f32 * layers::BAND_STONE_M - offset;
            if at > lo + 0.05 && at < hi - 0.05 {
                cuts.push(at);
            }
        }
    }
    cuts.push(hi);
    let world = |o: f32, u: f32, along: f32| from + t * along + side * o + Vec3::Y * u;
    let world_normal = |nn: [f32; 2]| (side * nn[0] + Vec3::Y * nn[1]).normalize();
    for (k, w) in cuts.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let id = stone_id(&[seed, k as i32, (from.x * 8.0) as i32, (from.z * 8.0) as i32]);
        let mut s = Surface::default();
        // Rings along the stone: a chamfer at each joint, then its length in steps.
        let mut rings: Vec<(f32, f32)> = vec![(a, 1.0)];
        let inner = [a + JOINT_M, b - JOINT_M];
        let steps = (((inner[1] - inner[0]) / 0.25).ceil() as usize).max(1);
        for i in 0..=steps {
            rings.push((inner[0] + (inner[1] - inner[0]) * i as f32 / steps as f32, 0.0));
        }
        rings.push((b, 1.0));
        let joint_a = k > 0;
        let joint_b = k + 2 < cuts.len();
        for r in 0..rings.len() - 1 {
            for seg in 0..n - 1 {
                let corner = |ring: usize, which: usize| -> (Vec3, Vec3) {
                    let (along, edge) = rings[ring];
                    let p = profile[seg + which];
                    let nn = vertex_normal(seg, which);
                    // In at a joint, by its chamfer.
                    let at_joint = edge > 0.0
                        && ((ring == 0 && joint_a) || (ring == rings.len() - 1 && joint_b));
                    let (o, u) = if at_joint {
                        (p[0] - nn[0] * JOINT_DEPTH_M, p[1] - nn[1] * JOINT_DEPTH_M)
                    } else {
                        (p[0], p[1])
                    };
                    let along = along.clamp(start(p[0]), end(p[0]));
                    (world(o, u, along), world_normal(nn))
                };
                let q = [corner(r, 0), corner(r + 1, 0), corner(r + 1, 1), corner(r, 1)];
                for tri in [[0, 1, 2], [0, 2, 3]] {
                    let c = tri.map(|i| q[i]);
                    let face = (c[1].0 - c[0].0).cross(c[2].0 - c[0].0);
                    if face.length_squared() < 1e-14 {
                        continue;
                    }
                    // Wound to face out.
                    let (c0, c1, c2) = if face.dot(c[0].1 + c[1].1 + c[2].1) >= 0.0 {
                        (c[0], c[1], c[2])
                    } else {
                        (c[0], c[2], c[1])
                    };
                    s.positions.extend([c0.0, c1.0, c2.0]);
                    s.normals.extend([c0.1, c1.1, c2.1]);
                    s.colours.extend([[1.0; 3]; 3]);
                    s.uvs.extend([[0.0; 2]; 3]);
                }
            }
        }
        // Caps where the band stops square.
        for (which, along, outward, mitred) in [
            (0usize, a, -t, mitre[0] != 0.0 || k > 0),
            (1, b, t, mitre[1] != 0.0 || k + 2 < cuts.len()),
        ] {
            if mitred {
                continue;
            }
            let _ = which;
            let tris = ears(profile);
            for tri in tris {
                let c = tri.map(|i| {
                    let p = profile[i];
                    world(p[0], p[1], along.clamp(start(p[0]), end(p[0])))
                });
                let face = (c[1] - c[0]).cross(c[2] - c[0]);
                let (c1, c2) = if face.dot(outward) >= 0.0 {
                    (c[1], c[2])
                } else {
                    (c[2], c[1])
                };
                s.positions.extend([c[0], c1, c2]);
                s.normals.extend([outward; 3]);
                s.colours.extend([[1.0; 3]; 3]);
                s.uvs.extend([[0.0; 2]; 3]);
            }
        }
        out.push((id, s));
    }
}

/// The profile a run's role is swept as, how far it runs on past each end, and whether
/// it is cut into stones.
fn profile_of(role: Edge) -> Option<(&'static str, f32, bool)> {
    match role {
        Edge::Crest => Some(("coping", 0.0, true)),
        Edge::Foot => Some(("plinth", 0.0, true)),
        Edge::Sill => Some(("sill", HORN_M, false)),
        Edge::Head => Some(("head", BEARING_M, false)),
        Edge::Soffit => Some(("fascia", 0.0, true)),
        _ => None,
    }
}

/// The stone kind's pieces by role: its profiles and its corner stones.
#[allow(clippy::too_many_arguments)]
pub fn stone_pieces(
    case: &Case,
    kit: &Kit,
    blocks: &Blocks,
    pieces: &Pieces,
    overhang: bool,
    bands: bool,
    stones: bool,
    out: &mut Surface,
) {
    let runs = &blocks.features.runs;
    let metres = kit
        .textures
        .get(&Material::Dressed)
        .map_or([1.2, 1.2], |t| t.metres);
    let ground = case.ground;
    let ground_fn = |x: f32, z: f32| ground(x, z);
    let n = Neighbours {
        base: &case.base,
        kind: kit.kind(),
        ground: &ground_fn,
    };
    for (i, run) in runs.iter().enumerate() {
        let Some((name, extend, cut)) = profile_of(run.role) else {
            continue;
        };
        if !bands {
            continue;
        }
        let profile = pieces.profile(name, overhang);
        let m = mitres(runs, i);
        let mut made = Vec::new();
        sweep(
            run.from,
            run.to,
            run.out[0],
            profile,
            [extend; 2],
            m,
            cut,
            &mut made,
            run.role as i32,
        );
        for (id, mut s) in made {
            texture(&mut s, metres, id, &blocks.features, ground);
            crate::add_to(out, s);
        }
    }
    if !stones {
        return;
    }
    let tag = if overhang { "over" } else { "flush" };
    for run in runs {
        if !matches!(run.role, Edge::Arris | Edge::Jamb) {
            continue;
        }
        let edge = run.from;
        let bottom = run.cubes.iter().map(|c| c.y).min().unwrap_or(0);
        let foot = run
            .cubes
            .iter()
            .find(|c| c.y == bottom)
            .is_some_and(|c| n.roles(*c).edges.contains(&Edge::Foot));
        let courses = ((run.to.y - run.from.y) / COURSE_M).round() as i32;
        for course in 0..courses {
            if foot && course == 0 {
                continue;
            }
            let y = run.from.y + course as f32 * COURSE_M;
            let id = stone_id(&[(edge.x * 8.0) as i32, (edge.z * 8.0) as i32, course, 4]);
            let v = id % 3;
            let (a, b, name) = if run.role == Edge::Arris {
                let (a, b) = if course % 2 == 0 {
                    (run.out[1], run.out[0])
                } else {
                    (run.out[0], run.out[1])
                };
                (a, b, format!("quoin_{tag}_{v}"))
            } else {
                // The reveal's face is the +X face; the wall's, +Z, alternating long and
                // short along it.
                let cube = run.cubes.first().copied().unwrap_or(IVec3::ZERO);
                let reveal = (0..2)
                    .find(|&k| n.beyond(cube + run.out[k].as_ivec3()) == Beyond::Hole)
                    .unwrap_or(0);
                let long = if course % 2 == 0 { "long" } else { "short" };
                (
                    run.out[reveal],
                    run.out[1 - reveal],
                    format!("jamb_{long}_{tag}_{v}"),
                )
            };
            let Some(model) = pieces.stones.get(&name) else {
                continue;
            };
            let (rotation, scale) = orient(a, b);
            let at = Vec3::new(edge.x, y, edge.z);
            place_stone(model, (at, rotation, scale), metres, id, &blocks.features, ground, out);
        }
    }
}

/// A block kind's look by roles with pieces over it: its surface sunk behind them, its
/// tops cut down under the copings, and the pieces.
#[allow(clippy::too_many_arguments)]
pub fn stone(
    case: &Case,
    kit: &Kit,
    pieces_laid: &[Piece],
    reliefs: &Reliefs,
    pieces: &Pieces,
    overhang: bool,
    drawn: &mut Drawn,
) -> Blocks {
    let b = layers::blocks(case, kit, pieces_laid);
    let cut = |_: IVec3, p: Vec3| under_copings(&b.features, p);
    let mode = Mode::Pieces {
        field_inset: if overhang { 0.0 } else { FLUSH_INSET_M },
    };
    let s = layers::draw_blocks(case, kit, &b, reliefs, mode, true, &cut, &|_, _, _| true);
    add_surfaces(drawn, kit, s);
    let mut trims = Surface::default();
    stone_pieces(case, kit, &b, pieces, overhang, true, true, &mut trims);
    add(drawn, folder(kit, Material::Dressed), trims);
    b
}

/// Tops under a coping or sill cut down to its underside, so it sits on them.
pub fn under_copings(features: &Features, p: Vec3) -> f32 {
    let mut limit = f32::MAX;
    for run in features.around(p) {
        let (depth, reach) = match run.role {
            Edge::Crest => (0.1, 0.25),
            Edge::Sill => (0.09, 0.25),
            _ => continue,
        };
        let rel = p - run.from;
        let inward = -rel.dot(run.out[0]);
        let length = run.from.distance(run.to);
        let along = rel.dot((run.to - run.from) / length);
        if (-1e-3..=reach + 1e-3).contains(&inward)
            && (-1e-3..=length + 1e-3).contains(&along)
            && rel.y > -depth - 0.06
        {
            limit = limit.min(run.from.y - depth + 0.005);
        }
    }
    limit
}

/// Whether a dressed piece the layout gives is a cap where a thinner kind stands on the
/// blocks (kept), not a trim round an opening (which the role pieces replace).
pub fn is_cap(case: &Case, kit: &Kit, piece: &Piece) -> bool {
    let Piece::Dressed { min, max, .. } = piece else {
        return false;
    };
    let centre = (min.metres() + max.metres()) * 0.5;
    let cell = (centre / CELL_M).floor().as_ivec3();
    case.base
        .cube(cell)
        .is_some_and(|cube| cube.kind != kit.kind())
}

/// Timber's pieces by role: a hewn plate on every crest log, a hewn sill on the first
/// course, posts up free ends, and with an overhang, log ends past the corners.
pub fn timber(
    case: &Case,
    kit: &Kit,
    kits: &Kits,
    reliefs: &Reliefs,
    overhang: bool,
    drawn: &mut Drawn,
    door: &Model,
    window: &Model,
) {
    let mut kit = kit.clone();
    if overhang {
        kit.overhang = CELL_M / 8.0;
    }
    let mut kits = kits.clone();
    for k in &mut kits.0 {
        if k.kind() == kit.kind() {
            *k = kit.clone();
        }
    }
    let pieces = terra_build::layout::lay_among(&case.base, &kit, &kits).pieces;
    let ground = case.ground;
    let ground_fn = |x: f32, z: f32| ground(x, z);
    let n = Neighbours {
        base: &case.base,
        kind: kit.kind(),
        ground: &ground_fn,
    };
    let roles = n.all();
    let all = runs(&roles);
    let crests: Vec<&Run> = all.iter().filter(|r| r.role == Edge::Crest).collect();
    let mut hewn = kit.clone();
    hewn.timber.profile = Profile::Hewn;
    hewn.timber.relief = 0.0;
    hewn.diameter = kit.diameter * 1.12;
    hewn.timber.variation.shade *= 0.5;
    if let Some(t) = kits
        .0
        .iter()
        .find(|k| k.pattern == Pattern::Masonry)
        .and_then(|k| k.textures.get(&Material::Log))
    {
        hewn.textures.insert(Material::Log, t.clone());
    }
    let bark = reliefs.of(&kit, Material::Log);
    let mut plain = Surfaces::default();
    let mut squared = Surfaces::default();
    for piece in &pieces {
        match piece {
            Piece::Opening { .. } => {
                if case.fitted {
                    today::openings(piece, door, window, drawn);
                }
            }
            Piece::Log {
                from, to, split, ..
            } => {
                let (a, b) = (from.metres(), to.metres());
                let top = a.y.max(b.y) + kit.radius();
                // On a crest: the log's top at a crest's height, over it.
                let on_crest = crests.iter().any(|r| {
                    (r.from.y - top).abs() < 0.08 && {
                        let mid = (a + b) * 0.5;
                        let rel = mid - r.from;
                        let length = r.from.distance(r.to);
                        let along = rel.dot((r.to - r.from) / length);
                        (-0.3..=length + 0.3).contains(&along)
                            && rel.dot(r.out[0]).abs() < CELL_M
                    }
                });
                let first = *split == Split::Top;
                if on_crest || first {
                    terra_build::draw::draw(piece, &hewn, None, &mut squared);
                } else {
                    terra_build::draw::draw(piece, &kit, bark, &mut plain);
                }
            }
            _ => terra_build::draw::draw(piece, &kit, bark, &mut plain),
        }
    }
    // Posts up every free end: an end cube's face, with the wall behind it one way only.
    let mut posts = Surfaces::default();
    for (&at, r) in &roles {
        let at = IVec3::from_array(at);
        for (axis, side) in [(0usize, 1i32), (0, -1), (2, 1), (2, -1)] {
            let f = crate::roles::face_index(axis, side);
            let behind = crate::roles::face_index(axis, -side);
            let across = if axis == 0 { 2 } else { 0 };
            let open_sides = [1, -1]
                .iter()
                .all(|&s| r.faces[crate::roles::face_index(across, s)].open());
            if !(r.faces[f].open() && r.faces[behind] == Beyond::Same && open_sides) {
                continue;
            }
            // Only from the wall's foot: a free end or a doorway, not a window's sides,
            // which its shutters fill.
            if case.base.cube(at - IVec3::Y).is_some_and(|c| c.kind == kit.kind()) {
                continue;
            }
            let mut top = at;
            while case.base.cube(top + IVec3::Y).is_some_and(|c| c.kind == kit.kind())
                && n.roles(top + IVec3::Y).faces[f].open()
            {
                top += IVec3::Y;
            }
            let mut out = Vec3::ZERO;
            out[axis] = side as f32;
            let centre = (at.as_vec3() + Vec3::splat(0.5)) * CELL_M;
            let base = centre + out * (CELL_M * 0.5 - kit.radius() + if overhang { 0.04 } else { 0.0 });
            let from = Vec3::new(base.x, at.y as f32 * CELL_M - 0.02, base.z);
            let to = Vec3::new(base.x, (top.y + 1) as f32 * CELL_M + if overhang { 0.05 } else { 0.0 }, base.z);
            let post = Piece::Log {
                from: Place::near(from),
                to: Place::near(to),
                split: Split::Whole,
                seed: stone_id(&[at.x, at.y, at.z, axis as i32 * 2 + side]),
                chinking: None,
                lean: [0, 0],
            };
            let mut thick = kit.clone();
            thick.diameter *= 1.1;
            terra_build::draw::draw(&post, &thick, bark, &mut posts);
        }
    }
    for material in [
        Material::Log,
        Material::LogEnd,
        Material::Daub,
        Material::Board,
        Material::Covering,
    ] {
        layers::shade_rest(case, &kit, material, &mut plain);
        layers::shade_rest(case, &kit, material, &mut posts);
        layers::shade_rest(case, &kit, material, &mut squared);
    }
    add_surfaces(drawn, &kit, plain);
    add_surfaces(drawn, &kit, posts);
    add_surfaces(drawn, &hewn, squared);
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    case: &Case,
    kits: &Kits,
    reliefs: &Reliefs,
    door: &Model,
    window: &Model,
    pieces: &Pieces,
    overhang: bool,
) -> Drawn {
    let laid: Vec<Laid> = today::lay(case, kits);
    let mut drawn = Drawn::default();
    today::plain(case, kits, &mut drawn);
    for Laid { kit, pieces: laid_pieces } in &laid {
        if kit.pattern == Pattern::Logs {
            timber(case, kit, kits, reliefs, overhang, &mut drawn, door, window);
            continue;
        }
        let bark = reliefs.of(kit, Material::Log);
        let mut s = Surfaces::default();
        for piece in laid_pieces {
            match piece {
                Piece::Opening { .. } => {
                    if case.fitted {
                        today::openings(piece, door, window, &mut drawn);
                    }
                }
                Piece::Dressed { .. } if !is_cap(case, kit, piece) => {}
                _ => terra_build::draw::draw(piece, kit, bark, &mut s),
            }
        }
        layers::shade_rest(case, kit, Material::Covering, &mut s);
        add_surfaces(&mut drawn, kit, s);
        stone(case, kit, laid_pieces, reliefs, pieces, overhang, &mut drawn);
    }
    drawn
}
