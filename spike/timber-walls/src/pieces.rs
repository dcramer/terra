//! A and B: a log or a beam made along a layout's log, from its seed.

use crate::Relief;
use glam::Vec3;
use std::collections::BTreeMap;
use std::f32::consts::{PI, TAU};
use terra_build::Surface;
use terra_build::logs::{LogKit, Piece, Split};

/// A's bark, and how far it stands out.
pub const BARK: &str = "pine_bark";
const BARK_METRES: [f32; 2] = [1.0, 1.0];
const BARK_RELIEF: f32 = 0.022;
/// B's hewn faces.
const HEWN: &str = "rough_wood";
const HEWN_METRES: [f32; 2] = [0.6, 1.4];
/// C's log wall: six logs to a copy, so 1.5 m puts one in each 0.25 m course; shifted so
/// its grooves fall where the courses meet.
pub const LOG_WALL: &str = "beam_wall_01";
pub const LOG_WALL_METRES: [f32; 2] = [1.5, 1.5];
pub const LOG_WALL_SHIFT: f32 = 0.0;
pub const LOG_WALL_BEVEL: f32 = 0.06;
pub const LOG_WALL_RELIEF: f32 = 0.07;
const END: &str = "log_end";
const DAUB: &str = "clay_plaster";
const DAUB_TINT: [f32; 3] = [0.9, 0.86, 0.82];
pub const LOG_WALL_TINT: [f32; 3] = [0.88, 0.79, 0.68];

/// A number from a seed, the same on every machine.
struct Random(u64);

impl Random {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut x = self.0;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^= x >> 31;
        (x >> 40) as f32 / (1u64 << 24) as f32
    }
    fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

/// A grid of points, `rows` along and `columns` round, closed round, as triangles with
/// normals smooth across it; `uv` gives each point's place on the texture, the seam's
/// last column its own.
fn tube(
    out: &mut Surface,
    grid: &[Vec<Vec3>],
    uv: &dyn Fn(usize, usize) -> [f32; 2],
    colour: [f32; 3],
    centre: &[Vec3],
) {
    let rows = grid.len();
    let columns = grid[0].len();
    // Normals: the sum of the faces round each point.
    let mut normals = vec![vec![Vec3::ZERO; columns]; rows];
    for i in 0..rows - 1 {
        for j in 0..columns {
            let k = (j + 1) % columns;
            let (a, b, c, d) = (grid[i][j], grid[i + 1][j], grid[i + 1][k], grid[i][k]);
            let n1 = (b - a).cross(c - a);
            let n2 = (c - a).cross(d - a);
            for (r, col, n) in [
                (i, j, n1 + n2),
                (i + 1, j, n1),
                (i + 1, k, n1 + n2),
                (i, k, n2),
            ] {
                normals[r][col] += n;
            }
        }
    }
    // Facing out, from the line down the middle.
    let outward = {
        let (a, b, c) = (grid[1][0], grid[2][0], grid[2][1]);
        let n = (b - a).cross(c - a);
        n.dot(a - centre[1]) > 0.0
    };
    for i in 0..rows - 1 {
        for j in 0..columns {
            let k = (j + 1) % columns;
            let quad = [(i, j), (i + 1, j), (i + 1, k), (i, k)];
            let mut tri = |t: [usize; 3]| {
                let t = if outward { t } else { [t[0], t[2], t[1]] };
                for index in t {
                    let (r, c) = quad[index];
                    out.positions.push(grid[r][c]);
                    let n = normals[r][c];
                    out.normals
                        .push(if outward { n } else { -n }.normalize_or_zero());
                    // The seam's last column takes its own place on the texture.
                    let column = if index >= 2 && k == 0 { columns } else { c };
                    out.uvs.push(uv(r, column));
                    out.colours.push(colour);
                }
            };
            tri([0, 1, 2]);
            tri([0, 2, 3]);
        }
    }
}

/// A flat cap over a ring, facing `outward`, the cut face's picture across it as `uv`
/// says from a point's place across the ring, from -1 to 1.
fn cap(
    out: &mut Surface,
    ring: &[Vec3],
    outward: Vec3,
    side: Vec3,
    up: Vec3,
    uv: &dyn Fn(f32, f32) -> [f32; 2],
    colour: [f32; 3],
) {
    let centre = ring.iter().copied().sum::<Vec3>() / ring.len() as f32;
    let reach = ring
        .iter()
        .map(|p| (p - centre).dot(side).abs().max((p - centre).dot(up).abs()))
        .fold(1e-3, f32::max);
    let place = |p: Vec3| uv((p - centre).dot(side) / reach, (p - centre).dot(up) / reach);
    for j in 0..ring.len() {
        let k = (j + 1) % ring.len();
        let mut t = [centre, ring[j], ring[k]];
        if (t[1] - t[0]).cross(t[2] - t[0]).dot(outward) < 0.0 {
            t.swap(1, 2);
        }
        for p in t {
            out.positions.push(p);
            out.normals.push(outward);
            out.uvs.push(place(p));
            out.colours.push(colour);
        }
    }
}

/// The log's frame: along it, and across it level and up.
fn frame(from: Vec3, to: Vec3) -> (f32, Vec3, Vec3, Vec3) {
    let length = from.distance(to);
    let axis = (to - from) / length;
    let side = axis.cross(Vec3::Y).normalize();
    let up = side.cross(axis);
    (length, axis, side, up)
}

/// A: a round log, tapered and bowed as a tree grows, its section a little uneven and
/// swelling here and there, its bark displaced by the bark's height; its sawn ends showing
/// their grain; the daub in the groove under it.
pub fn round(piece: &Piece, kit: &LogKit, bark: &Relief, out: &mut BTreeMap<String, Surface>) {
    const END_TINT: [f32; 3] = [0.86, 0.82, 0.78];
    let Piece::Log {
        from,
        to,
        split,
        seed,
        chinking,
    } = *piece
    else {
        return;
    };
    let (length, axis, side, up) = frame(from, to);
    if length < 0.05 {
        return;
    }
    let mut random = Random(seed as u64 * 7919 + 1);
    let radius = kit.diameter * 0.5 * 1.03;
    let thick_first = random.unit() < 0.5;
    let bow = random.signed() * kit.bend;
    let sag = random.signed() * kit.bend * 0.5;
    let phases = [0; 5].map(|_| random.unit() * TAU);
    let shift = [random.unit() * 3.0, random.unit() * 3.0];
    let shade = 0.84 + random.unit() * 0.16;
    // Stations along it: close at the ends, for the axe's chamfer, and every 6 cm between.
    let steps = ((length / 0.08).ceil() as usize).max(2);
    let chamfer = (0.03 / length).min(0.2);
    let mut at = vec![0.0, chamfer * 0.4, chamfer];
    for s in 1..steps {
        at.push(chamfer + (1.0 - 2.0 * chamfer) * s as f32 / steps as f32);
    }
    at.extend([1.0 - chamfer, 1.0 - chamfer * 0.4, 1.0]);
    let last = at.len() - 1;
    let columns = 18;
    let around = TAU * radius;
    let mut centres = Vec::new();
    let grid: Vec<Vec<Vec3>> = at
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            let arc = libm::sinf(PI * t);
            let centre = from + axis * (length * t) + side * (bow * arc) + up * (sag * arc);
            centres.push(centre);
            let taper = 1.0
                + kit.taper
                    * if thick_first {
                        1.0 - 2.0 * t
                    } else {
                        2.0 * t - 1.0
                    };
            let swell = 1.0 + 0.02 * libm::sinf(t * length * 2.1 + phases[0]);
            // The chamfer round a sawn end, and the bark's relief fading into it.
            let (end_scale, fade) = if i == 0 || i == last {
                (0.86, 0.0)
            } else if i == 1 || i == last - 1 {
                (0.95, 0.4)
            } else {
                (1.0, 1.0)
            };
            (0..columns)
                .map(|j| {
                    // The seam, where the texture meets itself, underneath.
                    let angle = -PI * 0.5 + TAU * j as f32 / columns as f32;
                    let uneven = 1.0
                        + 0.035 * libm::sinf(2.0 * angle + phases[1] + t * 1.3)
                        + 0.02 * libm::sinf(3.0 * angle + phases[2]);
                    let uv = [
                        shift[0] + j as f32 / columns as f32 * around / BARK_METRES[0],
                        shift[1] + t * length / BARK_METRES[1],
                    ];
                    let relief = (bark.at(uv) - 0.5) * BARK_RELIEF * fade;
                    let r = radius * taper * swell * uneven * end_scale + relief;
                    let across = libm::cosf(angle) * r;
                    let mut height = libm::sinf(angle) * r * 0.94;
                    match split {
                        Split::Top => height = height.max(0.0),
                        Split::Bottom => height = height.min(0.0),
                        Split::Whole => {}
                    }
                    centre + side * across + up * height
                })
                .collect()
        })
        .collect();
    let uv = |i: usize, j: usize| {
        [
            shift[0] + j as f32 / columns as f32 * around / BARK_METRES[0],
            shift[1] + at[i] * length / BARK_METRES[1],
        ]
    };
    tube(
        out.entry(BARK.into()).or_default(),
        &grid,
        &uv,
        [shade, shade * 0.97, shade * 0.94],
        &centres,
    );
    let spin = random.unit() * TAU;
    let (s, c) = (libm::sinf(spin), libm::cosf(spin));
    let end_uv = |x: f32, y: f32| {
        let (x, y) = (x * c - y * s, x * s + y * c);
        [0.5 + 0.47 * x, 0.5 - 0.47 * y]
    };
    let ends = out.entry(END.into()).or_default();
    cap(ends, &grid[0], -axis, side, up, &end_uv, END_TINT);
    cap(ends, &grid[last], axis, side, up, &end_uv, END_TINT);
    // The daub: a flat strip in the groove under it, recessed behind the logs' faces.
    if let Some((near, far)) = chinking {
        let depth = radius * 0.8;
        let mid = -kit.course * 0.5;
        daub(
            out,
            from + axis * near,
            from + axis * far,
            side,
            depth,
            mid,
            0.08,
        );
    }
}

/// A strip of daub along a log's run, `depth` either side of the wall's middle, `high`
/// tall round `mid` below the log's middle.
fn daub(
    out: &mut BTreeMap<String, Surface>,
    start: Vec3,
    end: Vec3,
    side: Vec3,
    depth: f32,
    mid: f32,
    high: f32,
) {
    let surface = out.entry(DAUB.into()).or_default();
    let along = (end - start).normalize_or_zero();
    for face in [1.0f32, -1.0] {
        let normal = side * face;
        let low = Vec3::Y * (mid - high * 0.5);
        let top = Vec3::Y * (mid + high * 0.5);
        let corners = [
            start + normal * depth + low,
            end + normal * depth + low,
            end + normal * depth + top,
            start + normal * depth + top,
        ];
        let mut tris = [[0, 1, 2], [0, 2, 3]];
        if (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .dot(normal)
            < 0.0
        {
            tris = [[0, 2, 1], [0, 3, 2]];
        }
        for t in tris {
            for i in t {
                let p = corners[i];
                surface.positions.push(p);
                surface.normals.push(normal);
                surface.uvs.push([p.dot(along) / 2.0, -p.y / 2.0]);
                surface.colours.push(DAUB_TINT);
            }
        }
    }
}

/// B: a hewn beam, squared by the axe: flat faces a little wavy, scalloped where the adze
/// bit, its edges rounded and uneven; its sawn ends showing their grain, squared; the daub
/// between it and the course under it.
pub fn hewn(piece: &Piece, kit: &LogKit, out: &mut BTreeMap<String, Surface>) {
    const END_TINT: [f32; 3] = [0.86, 0.8, 0.74];
    let Piece::Log {
        from,
        to,
        split,
        seed,
        chinking,
    } = *piece
    else {
        return;
    };
    let (length, axis, side, up) = frame(from, to);
    if length < 0.05 {
        return;
    }
    let mut random = Random(seed as u64 * 104_729 + 3);
    // As wide as a log's middle, and a little less tall than a course, so daub shows.
    let half_wide = 0.135 + random.signed() * 0.006;
    let half_high = 0.097 + random.signed() * 0.003;
    let round = 0.03;
    let phases = [0; 6].map(|_| random.unit() * TAU);
    let shift = [random.unit() * 3.0, random.unit() * 3.0];
    let shade = 0.86 + random.unit() * 0.14;
    // The section: a rounded rectangle, walked round from under its middle.
    let mut section: Vec<(Vec3, Vec3)> = Vec::new();
    let corner_steps = 3;
    for (cx, cy, start) in [
        (1.0f32, -1.0f32, -PI * 0.5),
        (1.0, 1.0, 0.0),
        (-1.0, 1.0, PI * 0.5),
        (-1.0, -1.0, PI),
    ] {
        for s in 0..=corner_steps {
            let a = start + PI * 0.5 * s as f32 / corner_steps as f32;
            let centre = Vec3::new(cx * (half_wide - round), cy * (half_high - round), 0.0);
            let normal = Vec3::new(libm::cosf(a), libm::sinf(a), 0.0);
            section.push((centre + normal * round, normal));
        }
        // A point in the middle of the face after each corner, so the face can wave.
        let a = start + PI * 0.5;
        let normal = Vec3::new(libm::cosf(a).round(), libm::sinf(a).round(), 0.0);
        let next = Vec3::new(
            if normal.x != 0.0 {
                normal.x * half_wide
            } else {
                0.0
            },
            if normal.y != 0.0 {
                normal.y * half_high
            } else {
                0.0
            },
            0.0,
        );
        section.push((next, normal));
    }
    let columns = section.len();
    let perimeter: Vec<f32> = {
        let mut sum = 0.0;
        let mut acc = vec![0.0];
        for j in 1..=columns {
            sum += section[j % columns].0.distance(section[j - 1].0);
            acc.push(sum);
        }
        acc
    };
    let steps = ((length / 0.07).ceil() as usize).max(2);
    let chamfer = (0.015 / length).min(0.2);
    let mut at = vec![0.0, chamfer];
    for s in 1..steps {
        at.push(chamfer + (1.0 - 2.0 * chamfer) * s as f32 / steps as f32);
    }
    at.extend([1.0 - chamfer, 1.0]);
    let last = at.len() - 1;
    let mut centres = Vec::new();
    let bow = random.signed() * 0.008;
    let grid: Vec<Vec<Vec3>> = at
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            let along = t * length;
            let centre = from + axis * along + side * (bow * libm::sinf(PI * t));
            centres.push(centre);
            let end = if i == 0 || i == last { 0.93 } else { 1.0 };
            section
                .iter()
                .map(|&(p, n)| {
                    // The face waves gently along the beam, and the adze's scallops cut
                    // across it, deeper on the sides than on top and below.
                    let wave = 0.004 * libm::sinf(along * 1.7 + phases[0] + n.x * 2.0)
                        + 0.003 * libm::sinf(along * 4.3 + phases[1] + n.y * 3.0);
                    let bite =
                        libm::sinf(along * TAU / 0.13 + phases[2] + 0.8 * libm::sinf(along * 2.3));
                    let scallop = -0.003 * bite.abs() * n.x.abs();
                    // The edges, where the axe met the round of the tree, wander.
                    let edge = (1.0 - n.x.abs().max(n.y.abs()))
                        * 0.01
                        * libm::sinf(along * 3.1 + phases[3] + p.x * 9.0);
                    let q = p + n * (wave + scallop - edge.abs());
                    let x = q.x * end;
                    let mut y = q.y * end;
                    match split {
                        Split::Top => y = y.max(0.0),
                        Split::Bottom => y = y.min(0.0),
                        Split::Whole => {}
                    }
                    centre + side * x + up * y
                })
                .collect()
        })
        .collect();
    let uv = |i: usize, j: usize| {
        [
            shift[0] + perimeter[j] / HEWN_METRES[0],
            shift[1] + at[i] * length / HEWN_METRES[1],
        ]
    };
    let warm = [shade * 1.0, shade * 0.86, shade * 0.7];
    tube(
        out.entry(HEWN.into()).or_default(),
        &grid,
        &uv,
        warm,
        &centres,
    );
    // Its ends: the middle of a log's cut face, the rings and checks without the bark.
    let spin = random.unit() * TAU;
    let (s, c) = (libm::sinf(spin), libm::cosf(spin));
    let end_uv = |x: f32, y: f32| {
        let (x, y) = (x * 0.3, y * 0.24);
        let (x, y) = (x * c - y * s, x * s + y * c);
        [0.5 + x, 0.5 - y]
    };
    let ends = out.entry(END.into()).or_default();
    cap(ends, &grid[0], -axis, side, up, &end_uv, END_TINT);
    cap(ends, &grid[last], axis, side, up, &end_uv, END_TINT);
    if let Some((near, far)) = chinking {
        let mid = -kit.course * 0.5;
        daub(
            out,
            from + axis * near,
            from + axis * far,
            side,
            half_wide - 0.03,
            mid,
            0.07,
        );
    }
}
