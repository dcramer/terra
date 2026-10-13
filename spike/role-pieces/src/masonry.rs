//! A block kind's surface as `terra_build::surface` draws it (D197), with one thing added:
//! each point of it asks the roles what it is (`Zones`), so a part of the same surface can
//! be drawn in another material, stand out or sink back, and take a shade by where it is.
//! Copied from `surface::draw_cut` and changed only there; what is the same is not
//! explained again here.

use glam::{IVec3, Vec3};
use std::collections::{BTreeSet, HashMap};
use terra_build::relief::Relief;
use terra_build::{CELL_M, Surface};

/// What a point of the surface is part of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// The kind's face fill, its rubble.
    Field,
    /// A top's own material, its flags.
    Top,
    /// Dressed stone: quoins, a plinth, a coping, drawn in the surface itself.
    Dressed,
}

/// A point's part, from the roles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Here {
    pub layer: Layer,
    /// How far it is set back from where the kind's surface would be, in metres; less than
    /// nothing stands it out.
    pub inset: f32,
    /// Which stone it is part of, for its texture's place, if it is dressed.
    pub stone: u32,
    /// How far into a joint between dressed stones it is, from 0 to 1.
    pub joint: f32,
}

pub trait Zones {
    /// What a point of a face is, the face facing `normal`, at a place before rounding.
    fn at(&self, p: Vec3, normal: Vec3) -> Here;
    /// Its colour, at a placed point, facing `outward`, so far round an outside edge
    /// (0 on a face, 1 at the middle of a rounded edge).
    fn tint(&self, p: Vec3, at: Vec3, normal: Vec3, outward: Vec3, round: f32) -> [f32; 3];
}

/// A block kind's materials, each how many metres across and how far its relief stands
/// out, and the height maps.
pub struct Materials<'a> {
    pub field: (&'a Relief, [f32; 2], f32),
    pub top: (&'a Relief, [f32; 2], f32),
    pub dressed: (&'a Relief, [f32; 2], f32),
    pub bevel: f32,
    pub steps: u32,
}

/// The surfaces drawn: the field, the tops and the dressed stone.
#[derive(Default)]
pub struct Out {
    pub field: Surface,
    pub top: Surface,
    pub dressed: Surface,
}

const INSIDE_FADE_M: f32 = 0.06;
/// How far dressed stone stands out of where the rubble's middle is, and how deep a joint
/// between two is.
const PROUD_M: f32 = 0.018;
const JOINT_M: f32 = 0.02;

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

/// As `terra_build::mesh::planar`.
pub fn planar(point: Vec3, normal: Vec3, metres: [f32; 2]) -> [f32; 2] {
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

/// A number from 0 to 1 from a stone's number, for where its texture falls.
fn shift(stone: u32) -> [f32; 2] {
    let mut x = stone.wrapping_mul(0x9e37_79b9) ^ 0x85eb_ca6b;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    [(x & 0xffff) as f32 / 65536.0, (x >> 16) as f32 / 65536.0]
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    cubes: &BTreeSet<[i32; 3]>,
    hides: &dyn Fn(IVec3) -> bool,
    m: &Materials,
    zones: &dyn Zones,
    cut: &dyn Fn(IVec3, Vec3) -> f32,
    drawn: &dyn Fn(IVec3, usize, f32) -> bool,
    out: &mut Out,
) {
    let full = |at: IVec3| cubes.contains(&at.to_array()) || hides(at);
    let n = m.steps.max(1) as usize;
    let bevel = m.bevel;
    let shrunk: HashMap<IVec3, (Vec3, Vec3)> = cubes
        .iter()
        .map(|&cube| {
            let cube = IVec3::from_array(cube);
            let mut low = cube.as_vec3() * CELL_M;
            let mut high = low + Vec3::splat(CELL_M);
            for axis in 0..3 {
                if !full(cube - step(axis, 1)) {
                    low[axis] += bevel;
                }
                if !full(cube + step(axis, 1)) {
                    high[axis] -= bevel;
                }
            }
            (cube, (low, high))
        })
        .collect();
    let mut inside: HashMap<IVec3, Vec<(Vec3, Vec3)>> = HashMap::new();
    for &cube in cubes {
        let cube = IVec3::from_array(cube);
        for axis in 0..3 {
            let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
            for (da, db) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
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
    // How far a point is pushed out along its rounded normal: each axis's layer there,
    // blended by which way it faces, as `surface::displaced` blends the three mappings.
    let displaced = |p: Vec3, outward: Vec3| -> f32 {
        let weights = outward.abs().powf(4.0);
        let total = weights.x + weights.y + weights.z;
        (0..3)
            .filter(|&axis| weights[axis] > 1e-4)
            .map(|axis| {
                let normal = unit(axis, outward[axis].signum());
                let here = zones.at(p, normal);
                let (relief, metres, depth) = match here.layer {
                    Layer::Field => m.field,
                    Layer::Top => m.top,
                    Layer::Dressed => m.dressed,
                };
                let mut uv = planar(p, normal, metres);
                if here.layer == Layer::Dressed {
                    let s = shift(here.stone);
                    uv = [uv[0] + s[0], uv[1] + s[1]];
                }
                let mut d = (relief.at(uv) - 0.5) * depth - here.inset;
                if here.layer == Layer::Dressed {
                    d += PROUD_M - here.joint * JOINT_M;
                }
                weights[axis] / total * d
            })
            .sum()
    };
    let mut triangles: Vec<([Vec3; 3], [Vec3; 3], [[f32; 2]; 3], Layer, [[f32; 3]; 3])> =
        Vec::new();
    for &cube in cubes {
        let cube = IVec3::from_array(cube);
        let low = cube.as_vec3() * CELL_M;
        let round: Vec<(Vec3, Vec3)> = (-1..=1)
            .flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| IVec3::new(x, y, z))))
            .filter_map(|offset| shrunk.get(&(cube + offset)).copied())
            .collect();
        for (axis, side) in FACES {
            let out_step = step(axis, side as i32);
            if full(cube + out_step) || !drawn(cube, axis, side) {
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
            let place = |p: Vec3| -> (Vec3, Vec3, f32) {
                let nearest = round
                    .iter()
                    .map(|&(low, high)| p.clamp(low, high))
                    .min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
                    .unwrap_or(p);
                let away = p - nearest;
                let length = away.length();
                let (rounded, outward) = if length > 1e-6 {
                    let outward = away / length;
                    (nearest + outward * bevel, outward)
                } else {
                    (p, normal)
                };
                let edge = ((1.0 - outward.abs().max_element()) / 0.29).clamp(0.0, 1.0);
                (
                    rounded + outward * (displaced(p, outward) * fade(p)),
                    outward,
                    edge,
                )
            };
            let flip = side < 0.0;
            for i in 0..n {
                for j in 0..n {
                    let corners = [
                        point(i, j),
                        point(i + 1, j),
                        point(i + 1, j + 1),
                        point(i, j + 1),
                    ];
                    let mut placed = corners.map(place);
                    for (q, p) in placed.iter_mut().zip(corners) {
                        q.0.y = q.0.y.min(cut(cube, p));
                    }
                    for [x, y, z] in [[0, 1, 2], [0, 2, 3]] {
                        let (y, z) = if flip { (z, y) } else { (y, z) };
                        let (pa, pb, pc) = (placed[x].0, placed[y].0, placed[z].0);
                        if (pb - pa).cross(pc - pa).length_squared() < 1e-12 {
                            continue;
                        }
                        let centre = (corners[x] + corners[y] + corners[z]) / 3.0;
                        let here = zones.at(centre, normal);
                        let metres = match here.layer {
                            Layer::Field => m.field.1,
                            Layer::Top => m.top.1,
                            Layer::Dressed => m.dressed.1,
                        };
                        let s = if here.layer == Layer::Dressed {
                            shift(here.stone)
                        } else {
                            [0.0; 2]
                        };
                        let uv = [corners[x], corners[y], corners[z]].map(|p| {
                            let t = planar(p, normal, metres);
                            [t[0] + s[0], t[1] + s[1]]
                        });
                        let tint = [x, y, z].map(|k| zones.tint(placed[k].0, corners[k], normal, placed[k].1, placed[k].2));
                        triangles.push(([pa, pb, pc], [Vec3::ZERO; 3], uv, here.layer, tint));
                    }
                }
            }
        }
    }
    let key = |p: Vec3| ((p * 2048.0).round()).as_ivec3();
    let mut sums: HashMap<IVec3, Vec3> = HashMap::new();
    for (t, ..) in &triangles {
        let face = (t[1] - t[0]).cross(t[2] - t[0]);
        for p in t {
            *sums.entry(key(*p)).or_default() += face;
        }
    }
    for (t, _, uv, layer, tint) in triangles {
        let face = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
        let normals = t.map(|p| sums[&key(p)].normalize_or(face));
        let into = match layer {
            Layer::Field => &mut out.field,
            Layer::Top => &mut out.top,
            Layer::Dressed => &mut out.dressed,
        };
        into.positions.extend(t);
        into.normals.extend(normals);
        into.uvs.extend(uv);
        into.colours.extend(tint);
    }
}
