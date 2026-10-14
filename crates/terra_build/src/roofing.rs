//! Roofs drawn from their pieces (`layout::roofs`): rafters in the kit's timber, battens
//! and boards, the covering in courses that step out at each butt, caps over ridges and
//! hips, valley boards and mortar fillets against walls. Pure data, so it runs anywhere.
//!
//! **Courses.** A square's covering is cut into the bands between its courses' lower
//! edges, level lines a course's rise apart from the eaves up, so they run on round hips and
//! across every square. Each band leans out from the roof a little more at its foot than at
//! its head, as a course of shakes or slates lies on the one below, so each butt stands out
//! as a step, by an amount that wanders along the course; the covering's texture is laid
//! so its rows sit on the steps (the kit's `rows` and `phase`), each course shifted along so
//! no two rows repeat. Thatch is the same with a thick coat and gentle steps.

use crate::kit::{Kit, Notch, Ridge};
use crate::layout::roofs::{RoofPart, planar_normal};
use crate::layout::{Piece, Place, Split, hash, metres, unit};
use crate::mesh::{Material, Surface, Surfaces, planar};
use glam::Vec3;
use std::f32::consts::PI;

/// The size in metres of a kit's texture for a material, or a metre square if it has none.
fn size(kit: &Kit, material: Material) -> [f32; 2] {
    kit.textures
        .get(&material)
        .map_or([1.0, 1.0], |texture| texture.metres)
}

/// Draws a piece of roof, each part in its material.
pub fn draw(piece: &Piece, kit: &Kit, surfaces: &mut Surfaces) {
    draw_as(piece, kit, false, surfaces);
}

/// Draws a piece of roof as [`draw`] does, or, `plain`, its covering as a flat sheet over
/// the roof without its courses, as a ghost is drawn.
pub fn draw_as(piece: &Piece, kit: &Kit, plain: bool, surfaces: &mut Surfaces) {
    match *piece {
        Piece::Covering {
            corners,
            middle,
            courses,
            open,
            ..
        } => {
            let square = Square {
                corners: corners.map(Place::metres),
                middle: middle.metres(),
                base: metres(courses.0),
                rise: metres(courses.1).max(0.01),
                open,
            };
            if plain {
                square.sheet(kit, surfaces.of(Material::Covering));
            } else {
                square.cover(kit, surfaces);
            }
        }
        Piece::Roof {
            part,
            from,
            to,
            sides,
            seed,
            ..
        } => {
            let (from, to) = (from.metres(), to.metres());
            let sides = sides.map(Place::metres);
            match part {
                RoofPart::Rafter => rafter(from, to, seed, kit, surfaces),
                RoofPart::Batten => batten(from, to, sides[0], kit, surfaces),
                RoofPart::Fascia | RoofPart::Verge => board(from, to, sides[0], kit, surfaces),
                RoofPart::Ridge | RoofPart::Hip => cap(from, to, sides, seed, kit, surfaces),
                RoofPart::Valley => valley(from, to, sides, kit, surfaces),
                RoofPart::Flashing => fillet(from, to, sides[0], kit, surfaces),
            }
        }
        _ => {}
    }
}

/// A square of roof: its corners and middle on the roof's top, and its courses.
struct Square {
    corners: [Vec3; 4],
    middle: Vec3,
    /// The first course's lower edge, and each course's rise, in metres.
    base: f32,
    rise: f32,
    open: [bool; 4],
}

/// A flat part of a square, wound counter-clockwise from above, and the square's sides it
/// lies along: the whole square where it is flat, else each of its four triangles.
type Facet = (Vec<Vec3>, Vec<usize>);

impl Square {
    fn facets(&self) -> Vec<Facet> {
        let triangles: Vec<Facet> = (0..4)
            .map(|side| {
                let (a, b) = (self.corners[side], self.corners[(side + 1) % 4]);
                (vec![self.middle, b, a], vec![side])
            })
            .collect();
        let n = up(&triangles[0].0);
        if triangles.iter().all(|(t, _)| up(t).dot(n) > 1.0 - 1e-5) {
            let mut quad = self.corners.to_vec();
            if (quad[1] - quad[0]).cross(quad[2] - quad[0]).y < 0.0 {
                quad.reverse();
            }
            return vec![(quad, vec![0, 1, 2, 3])];
        }
        triangles
    }

    /// For each facet, the valleys running through it from the square's middle to a
    /// corner, where the next triangle round rises above its plane: each as its two ends and
    /// the way into the facet, square to it in its plane.
    fn valleys(&self, facets: &[Facet]) -> Vec<Vec<(Vec3, Vec3, Vec3)>> {
        if facets.len() != 4 {
            return vec![Vec::new(); facets.len()];
        }
        (0..4)
            .map(|side| {
                let (t, _) = &facets[side];
                let n = up(t);
                let mut found = Vec::new();
                // Its edges to the corners after and before it, shared with the triangles
                // either side.
                for (corner, next) in [((side + 1) % 4, (side + 1) % 4), (side, (side + 3) % 4)] {
                    let (p, q) = (self.middle, self.corners[corner]);
                    let (other, _) = &facets[next];
                    let apex = other
                        .iter()
                        .copied()
                        .max_by(|a, b| {
                            let off = |x: Vec3| x.distance(p).min(x.distance(q));
                            off(*a).total_cmp(&off(*b))
                        })
                        .unwrap_or(p);
                    // A crease, not the few millimetres corners are rounded to on the grid.
                    if n.dot(apex - p) <= 0.015 {
                        continue;
                    }
                    let third = t
                        .iter()
                        .copied()
                        .max_by(|a, b| {
                            let off = |x: Vec3| x.distance(p).min(x.distance(q));
                            off(*a).total_cmp(&off(*b))
                        })
                        .unwrap_or(p);
                    let mut inward = n.cross(q - p).normalize_or_zero();
                    if inward.dot(third - p) < 0.0 {
                        inward = -inward;
                    }
                    found.push((p, q, inward));
                }
                found
            })
            .collect()
    }

    /// The course a height is in, and how far up it, from 0 at its foot to 1 at its head.
    fn course(&self, y: f32) -> (i32, f32) {
        let at = (y - self.base) / self.rise;
        let course = at.floor();
        (course as i32, at - course)
    }

    /// The covering as a flat sheet over the roof, for a ghost.
    fn sheet(&self, kit: &Kit, out: &mut Surface) {
        let r = &kit.roof;
        let texture = size(kit, Material::Covering);
        for (polygon, _) in self.facets() {
            let n = up(&polygon);
            let lifted: Vec<Vec3> = polygon
                .iter()
                .map(|p| *p + n * (r.thick + r.butt))
                .collect();
            for i in 1..lifted.len() - 1 {
                let corners = [lifted[0], lifted[i], lifted[i + 1]];
                out.textured(
                    corners,
                    [n; 3],
                    corners.map(|p| planar(p, n, texture)),
                    r.tint,
                );
            }
        }
    }

    /// The covering, course by course, its edges closed where no covering carries on, and
    /// its underside boarded.
    fn cover(&self, kit: &Kit, surfaces: &mut Surfaces) {
        let r = &kit.roof;
        let texture = size(kit, Material::Covering);
        let rows = r.rows.max(0.1);
        let facets = self.facets();
        let valleys = self.valleys(&facets);
        for ((polygon, sides), cuts) in facets.into_iter().zip(valleys) {
            let n = up(&polygon);
            // Cut back from a valley through it, so the valley is an open channel, its
            // lining showing between the courses' cut ends (`valley`).
            let mut polygon = polygon;
            for &(from, _, inward) in &cuts {
                polygon = keep_beyond(&polygon, from + inward * VALLEY_M, inward);
            }
            if polygon.len() < 3 {
                continue;
            }
            let level = Vec3::Y.cross(n).normalize_or(Vec3::X);
            let upslope = n.cross(level);
            // How far out from the roof a point stands, in its course: the course's butt
            // at its foot, wandering along it, down to the covering's thickness at its
            // head.
            let butt = |course: i32, p: Vec3| {
                let along = p.dot(level) / 0.37;
                let cell = along.floor();
                let t = along - cell;
                let t = t * t * (3.0 - 2.0 * t);
                let at = |c: f32| unit(hash(&[course as u32, c as i32 as u32, 41]));
                let wander = at(cell) * (1.0 - t) + at(cell + 1.0) * t;
                r.butt * (1.0 + r.uneven * (wander * 2.0 - 1.0))
            };
            let out = |course: i32, f: f32, p: Vec3| {
                p + n * (r.thick + butt(course, p) * (1.0 - f).clamp(0.0, 1.0))
            };
            let uv = |course: i32, f: f32, p: Vec3| {
                let shift = unit(hash(&[course as u32, 77])) * 7.0;
                [
                    p.dot(level) / texture[0] + shift,
                    r.phase - (course as f32 + f) / rows,
                ]
            };
            let covering = surfaces.of(Material::Covering);
            let colour = r.tint;
            let middle = polygon.iter().sum::<Vec3>() / polygon.len() as f32;
            // The square's sides no covering carries on past, and which way is out of each.
            let edges: Vec<(Vec3, Vec3, Vec3)> = sides
                .iter()
                .filter(|&&side| self.open[side])
                .map(|&side| {
                    let (a, b) = (self.corners[side], self.corners[(side + 1) % 4]);
                    let outward = (a - middle)
                        .reject_from(b - a)
                        .with_y(0.0)
                        .normalize_or(Vec3::X);
                    (a, b, outward)
                })
                .collect();
            // And where it is cut back from a valley, its cut edge, facing the channel.
            let edges: Vec<(Vec3, Vec3, Vec3)> = edges
                .into_iter()
                .chain(cuts.iter().map(|&(from, to, inward)| {
                    (from + inward * VALLEY_M, to + inward * VALLEY_M, -inward)
                }))
                .collect();
            let (bands, steps) = bands(&polygon, self);
            // Each band leans out a little more at its foot than its head, shaded as one.
            let exposure = self.rise / upslope.y.max(1e-3);
            let tilted = (n + upslope * (r.butt / exposure)).normalize_or(n);
            for (course, band) in &bands {
                let placed: Vec<(Vec3, [f32; 2])> = band
                    .iter()
                    .map(|&p| {
                        let (_, f) = self.course(p.y);
                        // A point on the band's top edge is at the head of its course.
                        let head = self.base + (*course + 1) as f32 * self.rise;
                        let foot = self.base + *course as f32 * self.rise;
                        let f = if (p.y - head).abs() < 1e-5 {
                            1.0
                        } else if (p.y - foot).abs() < 1e-5 {
                            0.0
                        } else {
                            f
                        };
                        (out(*course, f, p), uv(*course, f, p))
                    })
                    .collect();
                for i in 1..placed.len().saturating_sub(1) {
                    let corners = [placed[0].0, placed[i].0, placed[i + 1].0];
                    let face = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
                    if face.length() < 1e-9 {
                        continue;
                    }
                    covering.textured(
                        corners,
                        [tilted; 3],
                        [placed[0].1, placed[i].1, placed[i + 1].1],
                        colour,
                    );
                }
                // Where no covering carries on past a side, its edge, down to the roof.
                for &(a, b, outward) in &edges {
                    let on = |x: Vec3| (x - a).cross(b - a).length() < 1e-5 * (b - a).length();
                    for (i, &p) in band.iter().enumerate() {
                        let q = band[(i + 1) % band.len()];
                        if !on(p) || !on(q) || p.distance(q) < 1e-5 {
                            continue;
                        }
                        let (tp, tq) = (placed[i].0, placed[(i + 1) % placed.len()].0);
                        edge(covering, [p, q, tq, tp], outward, texture, colour);
                    }
                }
            }
            // The butts: a step out at the foot of each course, facing down the slope.
            for (course, (a, b)) in steps {
                let (low_a, low_b) = (out(course - 1, 1.0, a), out(course - 1, 1.0, b));
                let (high_a, high_b) = (out(course, 0.0, a), out(course, 0.0, b));
                let quad = facing([low_a, low_b, high_b, high_a], -upslope);
                let v = r.phase - course as f32 / rows;
                let uvs = quad.map(|p| {
                    let out = (p - a).dot(n) - r.thick;
                    [p.dot(level) / texture[0], v - out / texture[1]]
                });
                let normal = -upslope;
                for [i, j, k] in [[0, 1, 2], [0, 2, 3]] {
                    covering.textured(
                        [quad[i], quad[j], quad[k]],
                        [normal; 3],
                        [uvs[i], uvs[j], uvs[k]],
                        colour,
                    );
                }
            }
            // Its underside, boarded, seen from inside.
            let board = size(kit, Material::Board);
            let under = surfaces.of(Material::Board);
            for i in 1..polygon.len() - 1 {
                let corners = [polygon[0], polygon[i + 1], polygon[i]];
                under.textured(
                    corners,
                    [-n; 3],
                    corners.map(|p| planar(p, -n, board)),
                    [0.8; 3],
                );
            }
        }
    }
}

/// A flat polygon's normal, facing up.
fn up(polygon: &[Vec3]) -> Vec3 {
    let n = (polygon[1] - polygon[0])
        .cross(polygon[2] - polygon[0])
        .normalize_or(Vec3::Y);
    if n.y < 0.0 { -n } else { n }
}

/// The lowest and highest of a polygon.
fn span(polygon: &[Vec3]) -> (f32, f32) {
    let low = polygon.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let high = polygon.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    (low, high)
}

/// Where a level plane crosses a convex polygon, if it does: the two ends of the line.
fn cut(polygon: &[Vec3], height: f32) -> Option<(Vec3, Vec3)> {
    let mut found: Vec<Vec3> = Vec::new();
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        if (a.y - height) * (b.y - height) < 0.0 {
            let s = (height - a.y) / (b.y - a.y);
            found.push(a + (b - a) * s);
        } else if (a.y - height).abs() < 1e-6 {
            found.push(a);
        }
    }
    found.dedup_by(|a, b| a.distance(*b) < 1e-6);
    if found.len() > 2 && found[0].distance(found[found.len() - 1]) < 1e-6 {
        found.pop();
    }
    (found.len() >= 2).then(|| (found[0], found[found.len() - 1]))
}

/// A convex polygon split by a level plane: what is under it and what is over it.
fn split(polygon: &[Vec3], height: f32) -> (Vec<Vec3>, Vec<Vec3>) {
    let (mut under, mut over) = (Vec::new(), Vec::new());
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        if a.y <= height {
            under.push(a);
        }
        if a.y >= height {
            over.push(a);
        }
        if (a.y - height) * (b.y - height) < 0.0 {
            let s = (height - a.y) / (b.y - a.y);
            let p = (a + (b - a) * s).with_y(height);
            under.push(p);
            over.push(p);
        }
    }
    (under, over)
}

/// A flat polygon cut into the bands between its courses' lower edges, each with its
/// course, and the lines it is cut along, each with the course whose foot it is.
#[allow(clippy::type_complexity)]
fn bands(polygon: &[Vec3], square: &Square) -> (Vec<(i32, Vec<Vec3>)>, Vec<(i32, (Vec3, Vec3))>) {
    let (low, high) = span(polygon);
    let (mut course, _) = square.course(low);
    let mut rest: Vec<Vec3> = polygon.to_vec();
    let (mut bands, mut steps) = (Vec::new(), Vec::new());
    loop {
        let head = square.base + (course + 1) as f32 * square.rise;
        if head >= high - 1e-5 {
            bands.push((course, rest));
            break;
        }
        let (under, over) = split(&rest, head);
        if under.len() >= 3 {
            bands.push((course, under));
        }
        if let Some(line) = cut(polygon, head) {
            steps.push((course + 1, line));
        }
        rest = over;
        course += 1;
        if rest.len() < 3 {
            break;
        }
    }
    (bands, steps)
}

/// How far the covering stops short of a valley either side, leaving it an open channel.
const VALLEY_M: f32 = 0.13;

/// The part of a flat convex polygon on the far side of a line through `at` square to
/// `inward`, in its plane: where `(p - at) . inward` is not less than nothing.
fn keep_beyond(polygon: &[Vec3], at: Vec3, inward: Vec3) -> Vec<Vec3> {
    let d = |p: Vec3| (p - at).dot(inward);
    let mut out = Vec::new();
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        let (da, db) = (d(a), d(b));
        if da >= 0.0 {
            out.push(a);
        }
        if (da >= 0.0) != (db >= 0.0) {
            out.push(a + (b - a) * (da / (da - db)));
        }
    }
    out.dedup_by(|a, b| a.distance(*b) < 1e-6);
    if out.len() > 1 && out[0].distance(out[out.len() - 1]) < 1e-6 {
        out.pop();
    }
    out
}

/// Four corners in order, wound to face a way.
fn facing(quad: [Vec3; 4], way: Vec3) -> [Vec3; 4] {
    let face = (quad[1] - quad[0]).cross(quad[2] - quad[0]);
    if face.dot(way) >= 0.0 {
        quad
    } else {
        [quad[1], quad[0], quad[3], quad[2]]
    }
}

/// A flat edge, its texture mapped from its plane.
fn edge(out: &mut Surface, quad: [Vec3; 4], outward: Vec3, metres: [f32; 2], colour: [f32; 3]) {
    let quad = facing(quad, outward);
    let face = (quad[1] - quad[0]).cross(quad[2] - quad[0]);
    if face.length() < 1e-8 {
        return;
    }
    let normal = face.normalize();
    for [i, j, k] in [[0, 1, 2], [0, 2, 3]] {
        let corners = [quad[i], quad[j], quad[k]];
        if (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .length()
            < 1e-9
        {
            continue;
        }
        out.textured(
            corners,
            [normal; 3],
            corners.map(|p| planar(p, normal, metres)),
            colour,
        );
    }
}

/// A box from a corner along three edges, which need not be square to one another, each
/// face's texture mapped from its plane.
fn oriented(out: &mut Surface, origin: Vec3, edges: [Vec3; 3], metres: [f32; 2], colour: [f32; 3]) {
    let [a, b, c] = edges;
    let middle = origin + (a + b + c) * 0.5;
    let at = |x: f32, y: f32, z: f32| origin + a * x + b * y + c * z;
    let faces = [
        [
            at(0., 0., 0.),
            at(1., 0., 0.),
            at(1., 1., 0.),
            at(0., 1., 0.),
        ],
        [
            at(0., 0., 1.),
            at(1., 0., 1.),
            at(1., 1., 1.),
            at(0., 1., 1.),
        ],
        [
            at(0., 0., 0.),
            at(1., 0., 0.),
            at(1., 0., 1.),
            at(0., 0., 1.),
        ],
        [
            at(0., 1., 0.),
            at(1., 1., 0.),
            at(1., 1., 1.),
            at(0., 1., 1.),
        ],
        [
            at(0., 0., 0.),
            at(0., 1., 0.),
            at(0., 1., 1.),
            at(0., 0., 1.),
        ],
        [
            at(1., 0., 0.),
            at(1., 1., 0.),
            at(1., 1., 1.),
            at(1., 0., 1.),
        ],
    ];
    for quad in faces {
        let centre = (quad[0] + quad[1] + quad[2] + quad[3]) * 0.25;
        edge(out, quad, centre - middle, metres, colour);
    }
}

/// A rafter, or the beam under a ridge, hip or valley: a long member in the kit's timber,
/// of the roof's profile and thickness.
fn rafter(from: Vec3, to: Vec3, seed: u32, kit: &Kit, surfaces: &mut Surfaces) {
    let (profile, thick) = kit.roof.rafter;
    let mut timber = kit.timber.clone();
    timber.profile = profile;
    timber.notch = Notch::Flush;
    timber.bend *= 0.3;
    timber.relief *= 0.5;
    // Seen from below and far less than a wall's logs: coarser.
    timber.sides = timber.sides.min(10);
    timber.step = timber.step.max(0.25);
    let rafters = Kit {
        diameter: thick,
        course: thick / 0.78,
        chinking: false,
        timber,
        ..kit.clone()
    };
    crate::timber::member(
        from,
        to,
        Split::Whole,
        seed,
        None,
        [0.0; 2],
        &rafters,
        None,
        surfaces,
    );
}

/// A batten across the rafters under the foot of a course: along its line, as wide as the
/// kit says up the slope toward the point beside it, under the covering.
fn batten(from: Vec3, to: Vec3, upslope: Vec3, kit: &Kit, surfaces: &mut Surfaces) {
    let (wide, thick) = kit.roof.batten;
    let n = planar_normal(from, to, upslope);
    oriented(
        surfaces.of(Material::Board),
        from,
        [to - from, across(from, to, upslope) * wide, n * -thick],
        size(kit, Material::Board),
        [0.9; 3],
    );
}

/// The way across a line on the roof toward a point on one side of it, level with the
/// roof there and square to the line.
fn across(from: Vec3, to: Vec3, side: Vec3) -> Vec3 {
    let along = (to - from).normalize_or(Vec3::X);
    (side - from).reject_from(along).normalize_or(Vec3::X)
}

/// A board along an eave or a verge, its outer face on the roof's edge, from under the
/// covering down by the kit's depth.
fn board(from: Vec3, to: Vec3, inside: Vec3, kit: &Kit, surfaces: &mut Surfaces) {
    let (deep, thick) = kit.roof.board;
    let inward = across(from, to, inside).with_y(0.0).normalize_or(Vec3::X);
    oriented(
        surfaces.of(Material::Board),
        from - Vec3::Y * deep,
        [to - from, inward * thick, Vec3::Y * (deep + 0.01)],
        size(kit, Material::Board),
        kit.roof.board_tint,
    );
}

/// The cap over a ridge or a hip: two boards meeting over it, or a roll of the covering.
fn cap(from: Vec3, to: Vec3, sides: [Vec3; 2], seed: u32, kit: &Kit, surfaces: &mut Surfaces) {
    let r = &kit.roof;
    let normals = sides.map(|side| planar_normal(from, to, side));
    let lift = r.thick + r.butt;
    let along = to - from;
    match r.ridge {
        Ridge::Boards => {
            let (_, thick) = r.board;
            for (side, n) in sides.iter().zip(normals) {
                let down = across(from, to, *side);
                // From over the line, out along the slope, raised over the courses.
                let start = from + n * lift - down * thick;
                oriented(
                    surfaces.of(Material::Board),
                    start,
                    [along, down * (r.cap + thick), n * thick],
                    size(kit, Material::Board),
                    r.board_tint,
                );
            }
        }
        Ridge::Roll => {
            let texture = size(kit, Material::Covering);
            let up = (normals[0] + normals[1]).normalize_or(Vec3::Y);
            let side = along.normalize_or(Vec3::X).cross(up).normalize_or(Vec3::Z);
            let radius = r.cap;
            // Its middle low enough that its sides meet the slopes either side.
            let centre = |p: Vec3| p + up * (lift - radius * 0.35);
            let steps = 12;
            let length = along.length();
            let pieces = (length / 0.3).ceil().max(1.0) as usize;
            let covering = surfaces.of(Material::Covering);
            let point = |t: f32, i: usize| {
                let angle = -PI * 0.55 + PI * 1.1 * i as f32 / steps as f32;
                let swell = 1.0 + 0.04 * libm::sinf(t * length * 9.0 + seed as f32 * 0.1);
                centre(from + along * t)
                    + (side * libm::sinf(angle) + up * libm::cosf(angle)) * radius * swell
            };
            for k in 0..pieces {
                let (t0, t1) = (k as f32 / pieces as f32, (k + 1) as f32 / pieces as f32);
                for i in 0..steps {
                    let quad = [
                        point(t0, i),
                        point(t1, i),
                        point(t1, i + 1),
                        point(t0, i + 1),
                    ];
                    let uv = |t: f32, j: usize| {
                        [
                            t * length / texture[0],
                            j as f32 * PI * 1.1 * radius / steps as f32 / texture[1],
                        ]
                    };
                    let uvs = [uv(t0, i), uv(t1, i), uv(t1, i + 1), uv(t0, i + 1)];
                    let mid = (quad[0] + quad[2]) * 0.5;
                    let outward = mid - centre(from + along * (t0 + t1) * 0.5);
                    let ordered = facing(quad, outward);
                    let flipped = ordered[0] != quad[0];
                    let uvs = if flipped {
                        [uvs[1], uvs[0], uvs[3], uvs[2]]
                    } else {
                        uvs
                    };
                    let normals = ordered.map(|p| {
                        let t = ((p - from).dot(along) / along.length_squared()).clamp(0.0, 1.0);
                        (p - centre(from + along * t)).normalize_or(up)
                    });
                    for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
                        covering.textured(
                            [ordered[a], ordered[b], ordered[c]],
                            [normals[a], normals[b], normals[c]],
                            [uvs[a], uvs[b], uvs[c]],
                            r.tint,
                        );
                    }
                }
            }
            // Its ends, closed.
            for (t, way) in [(0.0, -along), (1.0, along)] {
                let middle = centre(from + along * t);
                for i in 0..steps {
                    let corners = [middle, point(t, i), point(t, i + 1)];
                    let face = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
                    let corners = if face.dot(way) >= 0.0 {
                        corners
                    } else {
                        [corners[0], corners[2], corners[1]]
                    };
                    let normal = way.normalize_or(Vec3::X);
                    covering.textured(
                        corners,
                        [normal; 3],
                        corners.map(|p| planar(p, normal, texture)),
                        r.tint,
                    );
                }
            }
        }
    }
}

/// A valley's lining, an open channel between the covering's cut ends either side
/// (`Square::cover` stops short of it): boards along it on each slope, laid on the roof
/// under the covering, running a little under its cut ends.
fn valley(from: Vec3, to: Vec3, sides: [Vec3; 2], kit: &Kit, surfaces: &mut Surfaces) {
    let r = &kit.roof;
    for side in sides {
        let n = planar_normal(from, to, side);
        let out = across(from, to, side);
        oriented(
            surfaces.of(Material::Board),
            from + n * 0.002,
            [to - from, out * (VALLEY_M + 0.05), n * (r.thick * 0.5)],
            size(kit, Material::Board),
            r.board_tint.map(|c| c * 0.45),
        );
    }
}

/// A fillet of mortar where the roof meets a wall standing up through it: over the
/// covering, up the wall and out along the roof by the kit's size.
fn fillet(from: Vec3, to: Vec3, inside: Vec3, kit: &Kit, surfaces: &mut Surfaces) {
    let r = &kit.roof;
    let n = planar_normal(from, to, inside);
    let out = across(from, to, inside);
    let lift = n * (r.thick + r.butt);
    let size = r.fillet;
    let texture = self::size(kit, Material::Flashing);
    let mortar = surfaces.of(Material::Flashing);
    // Its face, from up the wall down to out along the roof.
    let (top_a, top_b) = (from + lift + Vec3::Y * size, to + lift + Vec3::Y * size);
    let (toe_a, toe_b) = (from + lift + out * size, to + lift + out * size);
    let outward = (out + Vec3::Y).normalize_or(Vec3::Y);
    edge(
        mortar,
        [toe_a, toe_b, top_b, top_a],
        outward,
        texture,
        [1.0; 3],
    );
    // Its ends.
    let along = (to - from).normalize_or(Vec3::X);
    for (base, way) in [(from + lift, -along), (to + lift, along)] {
        let corners = [base - n * r.thick, base + out * size, base + Vec3::Y * size];
        let face = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
        let corners = if face.dot(way) >= 0.0 {
            corners
        } else {
            [corners[0], corners[2], corners[1]]
        };
        mortar.textured(
            corners,
            [way; 3],
            corners.map(|p| planar(p, way, texture)),
            [1.0; 3],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::{Kits, Pattern};
    use crate::layout::roofs;
    use crate::roofer::{self, Style};
    use crate::{Base, BuildOp, Cube, Fill, Kind};

    fn cabin(style: Style) -> Base {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind: Kind::PLAIN,
        });
        let footprint = (0..10).flat_map(|x| (0..8).map(move |z| [x, z])).collect();
        for (at, shape) in roofer::over(&footprint, 6, style, None).roof {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube {
                    shape,
                    ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
                }),
            });
        }
        base
    }

    #[test]
    fn a_valley_is_an_open_channel_the_covering_stops_short_of_either_side() {
        // An L hipped, a wing off the main block's south side, their valley from the inner
        // corner at (4, 4) up to (2, 2), along x = z.
        let kit = Kit {
            id: Kind::FIRST_ROOF.0,
            pattern: Pattern::Roof,
            ..Kit::default()
        };
        let mut base = Base::default();
        let mut footprint = std::collections::BTreeSet::new();
        for (min, max) in [([0, 0, 0], [11, 0, 7]), ([0, 0, 7], [7, 0, 15])] {
            base.apply(BuildOp::AddRoom {
                min,
                max,
                kind: Kind::PLAIN,
            });
            for x in min[0]..=max[0] {
                for z in min[2]..=max[2] {
                    footprint.insert([x, z]);
                }
            }
        }
        for (at, shape) in roofer::over(&footprint, 6, Style::Hip, None).roof {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube {
                    shape,
                    ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
                }),
            });
        }
        let mut pieces = Vec::new();
        roofs::lay(
            &base,
            &kit,
            &Kits(vec![kit.clone()]),
            crate::layout::Scope::ALL,
            &mut pieces,
        );
        let mut surfaces = Surfaces::default();
        for piece in &pieces {
            draw(piece, &kit, &mut surfaces);
        }
        let covering = &surfaces.0[&Material::Covering];
        // The covering's top faces near the valley, either side of it.
        let near = |side: f32| {
            covering
                .positions
                .chunks(3)
                .filter(|t| {
                    t.iter().all(|p| {
                        let off = (p.x - p.z) * side / std::f32::consts::SQRT_2;
                        (2.6..3.7).contains(&p.x) && off > 0.0 && off < VALLEY_M * 0.6
                    })
                })
                .count()
        };
        assert_eq!(
            near(1.0),
            0,
            "covering in the channel on the main block's side"
        );
        assert_eq!(near(-1.0), 0, "covering in the channel on the wing's side");
    }

    #[test]
    fn every_piece_of_a_roof_is_drawn_and_its_covering_faces_out() {
        for style in [Style::Gable, Style::Hip, Style::LeanTo] {
            let kit = Kit {
                id: Kind::FIRST_ROOF.0,
                pattern: Pattern::Roof,
                ..Kit::default()
            };
            let mut pieces = Vec::new();
            roofs::lay(
                &cabin(style),
                &kit,
                &Kits(vec![kit.clone()]),
                crate::layout::Scope::ALL,
                &mut pieces,
            );
            let mut surfaces = Surfaces::default();
            for piece in &pieces {
                draw(piece, &kit, &mut surfaces);
            }
            for material in [Material::Covering, Material::Board, Material::Log] {
                let surface = &surfaces.0[&material];
                assert!(surface.triangle_count() > 0, "{style:?} {material:?}");
                assert_eq!(surface.uvs.len(), surface.positions.len());
                for (corners, normals) in surface.positions.chunks(3).zip(surface.normals.chunks(3))
                {
                    let own = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
                    assert!(
                        normals.iter().all(|n| n.is_finite() && own.dot(*n) >= 0.0),
                        "{style:?} {material:?}: {corners:?} wound against {normals:?}"
                    );
                }
            }
            // The covering's top is over the roof, by at most its thickness and its butts.
            let covering = &surfaces.0[&Material::Covering];
            let highest = covering
                .positions
                .iter()
                .map(|p| p.y)
                .fold(f32::MIN, f32::max);
            // A lean-to's high side carried on up past its last cube by the eaves.
            let ridge = crate::STOREY_M + if style == Style::LeanTo { 4.5 } else { 2.0 };
            let r = &kit.roof;
            assert!(
                highest <= ridge + r.thick + r.butt * 2.0 + 0.2,
                "{style:?}: {highest}"
            );
        }
    }

    #[test]
    fn a_courses_butt_stands_out_from_the_course_under_it() {
        // One square on a slope rising north, its courses 10 cm of rise apart.
        let square = Square {
            corners: [
                Vec3::new(0.0, 0.25, 0.0),
                Vec3::new(0.25, 0.25, 0.0),
                Vec3::new(0.25, 0.0, 0.25),
                Vec3::new(0.0, 0.0, 0.25),
            ],
            middle: Vec3::new(0.125, 0.125, 0.125),
            base: 0.0,
            rise: 0.1,
            open: [false; 4],
        };
        let kit = Kit::default();
        let mut surfaces = Surfaces::default();
        square.cover(&kit, &mut surfaces);
        let covering = &surfaces.0[&Material::Covering];
        // Some faces look down the slope: the butts.
        let butts = covering
            .normals
            .iter()
            .filter(|n| n.z > 0.5 && n.y < 0.5)
            .count();
        assert!(butts > 0);
        // Nothing is under the roof's top, nor over it by more than the covering.
        let roof = |p: &Vec3| 0.25 - p.z;
        for p in &covering.positions {
            let over = (p.y - roof(p)) * 0.5f32.sqrt();
            assert!(
                over > -1e-4 && over < kit.roof.thick + kit.roof.butt * 2.0,
                "{p}"
            );
        }
    }
}
