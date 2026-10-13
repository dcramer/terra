//! Long members made from their pieces (D200, docs/building-kits.md): a profile swept along
//! each log the layout gives (`layout::runs`), shaped by the kit's numbers and the piece's
//! seed, its face displaced by its material's height and textured by it, its cut ends
//! showing their grain, and daub in the groove under it. Pure data, so it runs anywhere.
//!
//! Two profiles from one sweep, after the timber spike ([research/timber-walls.md]):
//!
//! - **Round**, a log in its bark: tapered from its butt, bowed, its section a little uneven
//!   and swelling here and there, flattened a little where it sits on the one below, its
//!   bark standing out by the bark's own height map; its sawn ends chamfered by the axe.
//! - **Hewn**, a beam squared by the axe: flat faces gently waved and scalloped by the adze,
//!   its edges rounded and wandering where the axe met the tree's round; its ends sawn
//!   square, a band of daub between it and the course below.
//!
//! Each log differs by its seed in thickness, taper, bend, the turn of its section, where
//! its texture falls, its shade and its warmth, by as much as the kit's variation says, so a
//! wall is never machine-regular (D14: lightly stylised, not a turned dowel).
//!
//! [research/timber-walls.md]: ../../../research/timber-walls.md

use crate::draw::cuboid;
use crate::kit::{Kit, Notch, Profile};
use crate::layout::{self, Piece, Random, Split};
use crate::mesh::{Material, Surface, Surfaces};
use crate::relief::Relief;
use glam::Vec3;
use std::f32::consts::{PI, TAU};

/// Faces that meet at more than about 50 degrees are shaded apart, as a crease; less, smooth
/// across (docs/art-style.md).
const CREASE: f32 = 0.64;
/// How far round a hewn beam's edges are rounded, and a sawn plank's.
const HEWN_EDGE_M: f32 = 0.03;
const PLANK_EDGE_M: f32 = 0.006;
/// How far apart a plank's rings are: its face waves only gently, and a floor has many.
const PLANK_STEP_M: f32 = 0.4;

/// Draws a log, a beam or a post, and the daub under it, into the kit's materials: its face
/// in `Log`, its cut ends in `LogEnd`, its daub in `Daub`. `face` is the face material's
/// height map, which a round log's bark stands out by.
pub fn draw(piece: &Piece, kit: &Kit, face: Option<&Relief>, surfaces: &mut Surfaces) {
    let Piece::Log {
        from,
        to,
        split,
        seed,
        chinking,
        lean,
        role,
    } = *piece
    else {
        return;
    };
    let chinking = chinking.map(|(near, far)| (layout::metres(near), layout::metres(far)));
    // A member laid for a role, as the kit dresses it: its own section, size and face.
    if let Some(crate::kit::Dress::Member {
        profile,
        scale,
        material,
    }) = role.and_then(|role| kit.dress.roles.get(&role)).cloned()
    {
        let mut dressed = kit.clone();
        if let Some(texture) = kit.textures.get(&material) {
            dressed.textures.insert(Material::Log, texture.clone());
        }
        dressed.timber.profile = profile;
        dressed.diameter *= scale;
        if profile == Profile::Hewn {
            dressed.timber.relief = 0.0;
            dressed.timber.taper = dressed.timber.taper.min(0.02);
        }
        let mut own = Surfaces(Default::default(), surfaces.1);
        member(
            from.metres(),
            to.metres(),
            split,
            seed,
            chinking,
            lean.map(|rise| rise as f32 / layout::TICKS as f32),
            &dressed,
            if profile == Profile::Round {
                face
            } else {
                None
            },
            &mut own,
        );
        for (each, surface) in own.0 {
            let into = if each == Material::Log {
                material
            } else {
                each
            };
            surfaces.of(into).append(surface);
        }
        return;
    }
    member(
        from.metres(),
        to.metres(),
        split,
        seed,
        chinking,
        lean.map(|rise| rise as f32 / layout::TICKS as f32),
        kit,
        face,
        surfaces,
    );
}

/// Draws a long member from one point to another in metres, with the daub under it so far
/// along it from `from`, and each end cut to a roof's slope rising so many metres a metre
/// in from it (none, sawn square): as [`draw`] draws a log, and as a roof's rafters are.
#[allow(clippy::too_many_arguments)]
pub(crate) fn member(
    from: Vec3,
    to: Vec3,
    split: Split,
    seed: u32,
    chinking: Option<(f32, f32)>,
    lean: [f32; 2],
    kit: &Kit,
    face: Option<&Relief>,
    surfaces: &mut Surfaces,
) {
    let length = from.distance(to);
    if length < 0.05 {
        return;
    }
    let frame = Frame::new(from, to);
    let mut random = Random(seed);
    let mut made = match kit.timber.profile {
        Profile::Round => round(&frame, kit, split, face, &mut random),
        Profile::Hewn => hewn(&frame, kit, split, &mut random),
    };
    // A gable's log cut to the roof over it: nothing of it above the slope rising from its
    // foot at each end, as a builder saws it.
    if lean.iter().any(|&rise| rise > 0.0) {
        let foot = kit.radius();
        for p in made.grid.iter_mut().flatten() {
            let along = (*p - from).dot(frame.axis);
            let mut cut = f32::MAX;
            if lean[0] > 0.0 {
                cut = cut.min(from.y - foot + lean[0] * along);
            }
            if lean[1] > 0.0 {
                cut = cut.min(to.y - foot + lean[1] * (length - along));
            }
            p.y = p.y.min(cut);
        }
    }
    let t = &kit.timber;
    let shade = 1.0 - t.variation.shade * random.unit();
    let warmth = t.variation.warmth * random.signed();
    let colour = [
        (t.tint[0] * shade * (1.0 + warmth)).clamp(0.0, 1.0),
        (t.tint[1] * shade).clamp(0.0, 1.0),
        (t.tint[2] * shade * (1.0 - warmth)).clamp(0.0, 1.0),
    ];
    sweep(surfaces.of(Material::Log), &made, colour);
    // Its ends, the cut face's picture across each, turned by the seed.
    let spin = random.unit() * TAU;
    let last = made.grid.len() - 1;
    let ends = surfaces.of(Material::LogEnd);
    for (ring, outward) in [(&made.grid[0], -frame.axis), (&made.grid[last], frame.axis)] {
        cap(ends, ring, outward, &frame, spin, made.grain, t.end_tint);
    }
    if let Some((near, far)) = chinking {
        let (depth, high) = made.daub;
        let along = frame.axis;
        let across = frame.side * depth;
        let mid = Vec3::Y * (-kit.course * 0.5);
        let (start, end) = (from + along * near + mid, from + along * far + mid);
        let (low, top) = (
            start - across - Vec3::Y * (high * 0.5),
            end + across + Vec3::Y * (high * 0.5),
        );
        let metres = kit
            .textures
            .get(&Material::Daub)
            .map_or([1.0, 1.0], |texture| texture.metres);
        cuboid(
            surfaces.of(Material::Daub),
            low.min(top),
            low.max(top),
            metres,
            kit.colours.chinking,
        );
    }
}

/// A member's frame: along it, and across it level and up; for an upright post, across it
/// east and north.
struct Frame {
    from: Vec3,
    length: f32,
    axis: Vec3,
    side: Vec3,
    up: Vec3,
}

impl Frame {
    fn new(from: Vec3, to: Vec3) -> Frame {
        let length = from.distance(to);
        let axis = (to - from) / length;
        let level = if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        let side = axis.cross(level).normalize();
        let up = side.cross(axis);
        Frame {
            from,
            length,
            axis,
            side,
            up,
        }
    }
}

/// A member made: its rings of points along it, each with where it falls on the face's
/// texture, how far across the end grain's picture its ends reach, and how deep and tall
/// the daub under it is.
struct Member {
    grid: Vec<Vec<Vec3>>,
    uvs: Vec<Vec<[f32; 2]>>,
    /// How much of the end grain's picture a cut end shows, from its middle: all of it
    /// for a round log with its bark ring, the heart for a squared beam.
    grain: [f32; 2],
    daub: (f32, f32),
}

/// Rings this far apart, or further, are a member drawn far off, with no more than its ends.
const COARSE_STEP_M: f32 = 4.0;

/// Where the rings are along a member, as fractions of its length: close at its ends for
/// the axe's chamfer, then every `step`.
fn stations(length: f32, step: f32, chamfer: f32) -> Vec<f32> {
    let c = (chamfer / length).min(0.2);
    // Rings further apart than a log is long, as one is drawn far off: its ends only.
    if step >= COARSE_STEP_M {
        return vec![0.0, c, 1.0 - c, 1.0];
    }
    let steps = ((length / step).ceil() as usize).max(2);
    let mut at = vec![0.0, c * 0.4, c];
    for s in 1..steps {
        at.push(c + (1.0 - 2.0 * c) * s as f32 / steps as f32);
    }
    at.extend([1.0 - c, 1.0 - c * 0.4, 1.0]);
    at
}

/// Whether a point of a split log's section is on or past its cut, along its middle: its
/// top half sits flat on what is below, its bottom half has its flat side up. Such a point
/// is laid on the cut, where the section would cross it without its bumps, so the cut face
/// is flat and its edges go one way.
fn on_cut(height: f32, split: Split) -> bool {
    match split {
        Split::Top => height <= 0.0,
        Split::Bottom => height >= 0.0,
        Split::Whole => false,
    }
}

/// A: a round log, tapered and bowed as a tree grows, its section a little uneven and
/// swelling here and there, its bark displaced by the bark's height.
fn round(
    frame: &Frame,
    kit: &Kit,
    split: Split,
    face: Option<&Relief>,
    random: &mut Random,
) -> Member {
    let t = &kit.timber;
    let metres = kit
        .textures
        .get(&Material::Log)
        .map_or([1.0, 1.0], |texture| texture.metres);
    let radius = kit.radius() * 1.03 * (1.0 + t.variation.thickness * random.signed());
    let thick_first = random.unit() < 0.5;
    let taper = t.taper * (0.6 + 0.8 * random.unit());
    let bow = random.signed() * t.bend;
    let sag = random.signed() * t.bend * 0.5;
    let phases = [0; 4].map(|_| random.unit() * TAU);
    let shift = [random.unit() * 3.0, random.unit() * 3.0];
    let chamfer = match t.notch {
        Notch::Saddle => 0.03,
        Notch::Flush => 0.012,
    };
    let at = stations(frame.length, t.step, chamfer);
    let last = at.len() - 1;
    let columns = t.sides as usize;
    let around = TAU * radius;
    let uv = |t_along: f32, j: usize| {
        [
            shift[0] + j as f32 / columns as f32 * around / metres[0],
            shift[1] + t_along * frame.length / metres[1],
        ]
    };
    // What each point round the section needs that does not change along the log, worked
    // out once: its angle's sine and cosine, and its unevenness's.
    let round: Vec<[f32; 5]> = (0..columns)
        .map(|j| {
            // From underneath, where the texture's seam is out of sight.
            let angle = -PI * 0.5 + TAU * j as f32 / columns as f32;
            [
                libm::sinf(angle),
                libm::cosf(angle),
                libm::sinf(2.0 * angle + phases[1]),
                libm::cosf(2.0 * angle + phases[1]),
                libm::sinf(3.0 * angle + phases[2]),
            ]
        })
        .collect();
    let mut grid = Vec::with_capacity(at.len());
    let mut uvs = Vec::with_capacity(at.len());
    for (i, &along) in at.iter().enumerate() {
        let (shift_sin, shift_cos) = (libm::sinf(along * 1.3), libm::cosf(along * 1.3));
        let arc = libm::sinf(PI * along);
        let centre = frame.from
            + frame.axis * (frame.length * along)
            + frame.side * (bow * arc)
            + frame.up * (sag * arc);
        let butt = if thick_first {
            1.0 - 2.0 * along
        } else {
            2.0 * along - 1.0
        };
        let swell = 1.0 + t.swell * libm::sinf(along * frame.length * 2.1 + phases[0]);
        // The chamfer round a sawn end, the bark's relief fading into it.
        let (end, fade) = match (t.notch, i.min(last - i)) {
            (Notch::Saddle, 0) => (0.86, 0.0),
            (Notch::Saddle, 1) => (0.95, 0.4),
            (Notch::Flush, 0) => (0.95, 0.0),
            (Notch::Flush, 1) => (0.99, 0.5),
            _ => (1.0, 1.0),
        };
        let mut ring = Vec::with_capacity(columns);
        let mut ring_uv = Vec::with_capacity(columns + 1);
        for (j, &[sin, cos, sin2, cos2, sin3]) in round.iter().enumerate() {
            ring_uv.push(uv(along, j));
            // sin(2a + phase + along * 1.3), by the sum of angles.
            let wave = sin2 * shift_cos + cos2 * shift_sin;
            let uneven = 1.0 + t.uneven * wave + t.uneven * 0.6 * sin3;
            let relief = face.map_or(0.0, |face| (face.at(uv(along, j)) - 0.5) * t.relief * fade);
            let r = radius * (1.0 + taper * butt) * swell * uneven * end + relief;
            // Scribed a little flatter top and bottom, to sit on the log below.
            let height = sin * r * 0.94;
            let (across, height) = if on_cut(height, split) {
                let plain = radius * (1.0 + taper * butt) * end;
                (cos * plain, 0.0)
            } else {
                (cos * r, height)
            };
            ring.push(centre + frame.side * across + frame.up * height);
        }
        // The seam's last column, where the texture comes round to its start.
        ring_uv.push(uv(along, columns));
        grid.push(ring);
        uvs.push(ring_uv);
    }
    Member {
        grid,
        uvs,
        grain: [0.47, 0.47],
        daub: (kit.radius() * t.daub.0, t.daub.1),
    }
}

/// B: a hewn beam, squared by the axe: flat faces a little wavy, scalloped where the adze
/// bit, its edges rounded and uneven, a little less tall than a course so daub shows.
fn hewn(frame: &Frame, kit: &Kit, split: Split, random: &mut Random) -> Member {
    let t = &kit.timber;
    let metres = kit
        .textures
        .get(&Material::Log)
        .map_or([1.0, 1.0], |texture| texture.metres);
    let thickness = 1.0 + t.variation.thickness * random.signed();
    let half_wide = kit.diameter * 0.45 * thickness;
    let half_high = kit.course * 0.39 * (1.0 + t.variation.thickness * 0.3 * random.signed());
    let chamfer = match t.notch {
        Notch::Saddle => 0.02,
        Notch::Flush => 0.008,
    };
    let look = Squared {
        half_wide,
        half_high,
        round: HEWN_EDGE_M,
        metres,
        strip: None,
        bend: t.bend * 0.25,
        adzed: true,
        chamfer,
        step: t.step,
        // Rounded edges only as finely as the kit draws a round log: fewer far off.
        corner_steps: if t.sides < 12 { 1 } else { 3 },
    };
    let mut member = squared(frame, &look, split, random);
    member.daub = (
        half_wide * t.daub.0,
        (kit.course - 2.0 * half_high + 0.02).max(t.daub.1),
    );
    member
}

/// How a squared member is made: its half width and height, how round its edges, its face's
/// texture's size, the strip of it across the member if its texture is of boards side by
/// side, how far it bows at most, whether an adze scalloped its faces, and how deep the
/// chamfer at its ends.
struct Squared {
    half_wide: f32,
    half_high: f32,
    round: f32,
    metres: [f32; 2],
    strip: Option<[f32; 2]>,
    bend: f32,
    adzed: bool,
    chamfer: f32,
    /// How far apart its rings are along it, and how many steps round each edge.
    step: f32,
    corner_steps: usize,
}

/// A squared member along a frame: a rounded rectangle swept along it, its faces gently
/// waved, scalloped by the adze if it was hewn, its edges wandering where the axe or saw
/// met the tree's round.
fn squared(frame: &Frame, look: &Squared, split: Split, random: &mut Random) -> Member {
    let (half_wide, half_high, round) = (look.half_wide, look.half_high, look.round);
    let phases = [0; 4].map(|_| random.unit() * TAU);
    let shift = [random.unit() * 3.0, random.unit() * 3.0];
    let bow = random.signed() * look.bend;
    // The section: a rounded rectangle, walked round from under its middle, with a point
    // in the middle of each face so the face can wave.
    let mut section: Vec<(Vec3, Vec3)> = Vec::new();
    let corner_steps = look.corner_steps.max(1);
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
        let a = start + PI * 0.5;
        let normal = Vec3::new(libm::cosf(a).round(), libm::sinf(a).round(), 0.0);
        section.push((normal * Vec3::new(half_wide, half_high, 0.0), normal));
    }
    let columns = section.len();
    let mut perimeter = vec![0.0];
    for j in 1..=columns {
        let step = section[j % columns].0.distance(section[j - 1].0);
        perimeter.push(perimeter[j - 1] + step);
    }
    // How far across the texture each point of the section falls.
    let across = |j: usize| match look.strip {
        Some([from, wide]) => {
            let x = section[j % columns].0.x / (2.0 * half_wide) + 0.5;
            from + wide * x.clamp(0.0, 1.0)
        }
        None => shift[0] + perimeter[j] / look.metres[0],
    };
    let at = stations(frame.length, look.step, look.chamfer);
    let last = at.len() - 1;
    let (wave_m, scallop_m, edge_m) = if look.adzed {
        (1.0, 0.003, 0.01)
    } else {
        (0.4, 0.0, 0.002)
    };
    let mut grid = Vec::with_capacity(at.len());
    let mut uvs = Vec::with_capacity(at.len());
    for (i, &along_t) in at.iter().enumerate() {
        let along = along_t * frame.length;
        let centre =
            frame.from + frame.axis * along + frame.side * (bow * libm::sinf(PI * along_t));
        let end = match i.min(last - i) {
            0 => 0.94,
            1 => 0.98,
            _ => 1.0,
        };
        let ring = section
            .iter()
            .map(|&(p, n)| {
                // The face waves gently along the member, and an adze's scallops cut
                // across it, deeper on the sides than on top and below.
                let wave = wave_m
                    * (0.004 * libm::sinf(along * 1.7 + phases[0] + n.x * 2.0)
                        + 0.003 * libm::sinf(along * 4.3 + phases[1] + n.y * 3.0));
                let bite =
                    libm::sinf(along * TAU / 0.13 + phases[2] + 0.8 * libm::sinf(along * 2.3));
                let scallop = -scallop_m * bite.abs() * n.x.abs();
                // The edges, where the axe or saw met the round of the tree, wander.
                let edge = (1.0 - n.x.abs().max(n.y.abs()))
                    * edge_m
                    * libm::sinf(along * 3.1 + phases[3] + p.x * 9.0);
                let q = p + n * (wave + scallop - edge.abs());
                let (x, y) = if on_cut(p.y, split) {
                    (p.x, 0.0)
                } else {
                    (q.x, q.y)
                };
                centre + frame.side * (x * end) + frame.up * (y * end)
            })
            .collect();
        grid.push(ring);
        uvs.push(
            (0..=columns)
                .map(|j| [across(j), shift[1] + along / look.metres[1]])
                .collect(),
        );
    }
    Member {
        grid,
        uvs,
        grain: [0.3, 0.24],
        daub: (0.0, 0.0),
    }
}

/// A deck's plank (`layout::floors`): a board, a joist or a rim beam, sawn square, its face
/// in the kit's `Floor` material, drawn from one board of it where the texture is of boards
/// side by side, a little lighter or darker by its seed; its sawn ends in `LogEnd`.
pub fn plank(piece: &Piece, kit: &Kit, surfaces: &mut Surfaces) {
    let Piece::Plank {
        from,
        to,
        wide,
        high,
        seed,
    } = *piece
    else {
        return;
    };
    let (from, to) = (from.metres(), to.metres());
    let (wide, high) = (layout::metres(wide), layout::metres(high));
    if from.distance(to) < 0.02 {
        return;
    }
    let frame = Frame::new(from, to);
    let mut random = Random(seed);
    let texture = kit.textures.get(&Material::Floor);
    let strips = texture.and_then(|texture| texture.strips);
    let strip = strips.map(|[first, apart]| {
        let count = ((1.0 - first) / apart).floor().max(1.0);
        let which = (random.unit() * count).floor().min(count - 1.0);
        // Inside the board's edges, so its seams fall in the gaps.
        [first + apart * (which + 0.06), apart * 0.88]
    });
    let look = Squared {
        half_wide: wide * 0.5,
        half_high: high * 0.5,
        round: PLANK_EDGE_M.min(high * 0.3),
        metres: texture.map_or([2.0, 2.0], |texture| texture.metres),
        strip,
        bend: 0.0,
        adzed: false,
        chamfer: 0.004,
        step: PLANK_STEP_M,
        corner_steps: 1,
    };
    let member = squared(&frame, &look, Split::Whole, &mut random);
    let t = &kit.timber;
    let shade = 1.0 - t.variation.shade * 0.8 * random.unit();
    let warmth = t.variation.warmth * random.signed();
    let colour = [
        (shade * (1.0 + warmth)).clamp(0.0, 1.0),
        shade.clamp(0.0, 1.0),
        (shade * (1.0 - warmth)).clamp(0.0, 1.0),
    ];
    sweep(surfaces.of(Material::Floor), &member, colour);
    let spin = random.unit() * TAU;
    let last = member.grid.len() - 1;
    let ends = surfaces.of(Material::LogEnd);
    for (ring, outward) in [
        (&member.grid[0], -frame.axis),
        (&member.grid[last], frame.axis),
    ] {
        cap(ends, ring, outward, &frame, spin, member.grain, t.end_tint);
    }
}

/// A member's rings as a closed tube of triangles, each corner shaded smooth across its
/// neighbours but at a crease, its texture as its rings say, the seam's last column its own.
fn sweep(out: &mut Surface, member: &Member, colour: [f32; 3]) {
    let grid = &member.grid;
    let rows = grid.len();
    let columns = grid[0].len();
    // Each point's normal: the sum of the faces round it. The rings go round the way that
    // makes each quad's first triangle face out, as `round` and `hewn` lay them.
    let face = |a: Vec3, b: Vec3, c: Vec3| (b - a).cross(c - a);
    let mut normals = vec![vec![Vec3::ZERO; columns]; rows];
    for i in 0..rows - 1 {
        for j in 0..columns {
            let k = (j + 1) % columns;
            let (a, b, c, d) = (grid[i][j], grid[i + 1][j], grid[i + 1][k], grid[i][k]);
            let (n1, n2) = (face(a, b, c), face(a, c, d));
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
    if out.sharing {
        // The same triangles, each corner shaded smooth shared by all that meet at it.
        let mut smooth: Vec<u32> = vec![u32::MAX; rows * (columns + 1)];
        for i in 0..rows - 1 {
            for j in 0..columns {
                let k = (j + 1) % columns;
                let quad = [(i, j), (i + 1, j), (i + 1, k), (i, k)];
                for tri in [[0, 1, 2], [0, 2, 3]] {
                    let corners = tri.map(|index| grid[quad[index].0][quad[index].1]);
                    let own = face(corners[0], corners[1], corners[2]);
                    if own.length_squared() < 1e-14 {
                        continue;
                    }
                    let own = own.normalize();
                    let indices = tri.map(|index| {
                        let (r, c) = quad[index];
                        let column = if index >= 2 && k == 0 { columns } else { c };
                        let normal = normals[r][c].normalize_or_zero();
                        let uv = member.uvs[r][column];
                        if normal.dot(own) < CREASE {
                            return out.shared.corner(grid[r][c], own, uv, colour);
                        }
                        let slot = &mut smooth[r * (columns + 1) + column];
                        if *slot == u32::MAX {
                            *slot = out.shared.corner(grid[r][c], normal, uv, colour);
                        }
                        *slot
                    });
                    out.shared.indices.extend(indices);
                }
            }
        }
        return;
    }
    for i in 0..rows - 1 {
        for j in 0..columns {
            let k = (j + 1) % columns;
            let quad = [(i, j), (i + 1, j), (i + 1, k), (i, k)];
            for tri in [[0, 1, 2], [0, 2, 3]] {
                let corners = tri.map(|index| grid[quad[index].0][quad[index].1]);
                let own = face(corners[0], corners[1], corners[2]);
                if own.length_squared() < 1e-14 {
                    continue;
                }
                let own = own.normalize();
                for index in tri {
                    let (r, c) = quad[index];
                    let smooth = normals[r][c].normalize_or_zero();
                    out.positions.push(grid[r][c]);
                    out.normals.push(if smooth.dot(own) < CREASE {
                        own
                    } else {
                        smooth
                    });
                    // The seam's last column takes its own place on the texture.
                    let column = if index >= 2 && k == 0 { columns } else { c };
                    out.uvs.push(member.uvs[r][column]);
                    out.colours.push(colour);
                }
            }
        }
    }
}

/// A flat cap over a ring, facing `outward`, the end grain's picture across it, turned by
/// `spin`, as much of it as `grain` says.
fn cap(
    out: &mut Surface,
    ring: &[Vec3],
    outward: Vec3,
    frame: &Frame,
    spin: f32,
    grain: [f32; 2],
    colour: [f32; 3],
) {
    let centre = ring.iter().copied().sum::<Vec3>() / ring.len() as f32;
    let reach = ring
        .iter()
        .map(|p| {
            let d = *p - centre;
            d.dot(frame.side).abs().max(d.dot(frame.up).abs())
        })
        .fold(1e-3, f32::max);
    let (s, c) = (libm::sinf(spin), libm::cosf(spin));
    let place = |p: Vec3| {
        let d = p - centre;
        let (x, y) = (
            d.dot(frame.side) / reach * grain[0],
            d.dot(frame.up) / reach * grain[1],
        );
        let (x, y) = (x * c - y * s, x * s + y * c);
        [0.5 + x, 0.5 - y]
    };
    for j in 0..ring.len() {
        let k = (j + 1) % ring.len();
        let mut t = [centre, ring[j], ring[k]];
        let own = (t[1] - t[0]).cross(t[2] - t[0]);
        if own.length_squared() < 1e-14 {
            continue;
        }
        if own.dot(outward) < 0.0 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::tests::facing_out;
    use crate::kit::Timber;
    use crate::layout::{Place, lay, tests as layouts};
    use crate::{CELL_M, Panel};

    fn log(seed: u32, split: Split) -> Piece {
        Piece::Log {
            from: Place::near(Vec3::new(0.0, 0.125, 0.25)),
            to: Place::near(Vec3::new(3.0, 0.125, 0.25)),
            split,
            seed,
            chinking: Some((0, crate::layout::ticks(3.0))),
            lean: [0, 0],
            role: None,
        }
    }

    fn corners(piece: &Piece, kit: &Kit, face: Option<&Relief>) -> Vec<Vec3> {
        let mut surfaces = Surfaces::default();
        draw(piece, kit, face, &mut surfaces);
        surfaces
            .0
            .values()
            .flat_map(|s| s.positions.clone())
            .collect()
    }

    /// FNV-1a over the bits of every corner.
    fn checksum(points: &[Vec3]) -> u64 {
        points
            .iter()
            .flat_map(|p| p.to_array())
            .fold(0xcbf2_9ce4_8422_2325_u64, |sum, v| {
                (sum ^ u64::from(v.to_bits())).wrapping_mul(0x0100_0000_01b3)
            })
    }

    /// A log's own numbers, written here, so retuning pine does not move it.
    fn pinned(profile: Profile) -> Kit {
        Kit {
            diameter: 0.3,
            course: 0.25,
            timber: Timber {
                profile,
                ..Timber::default()
            },
            ..Kit::default()
        }
    }

    /// A bark of ridges, for a log to stand out by.
    fn bark() -> Relief {
        Relief::new(4, 4, (0..16).map(|i| (i * 17) as u8).collect()).unwrap()
    }

    #[test]
    fn a_seed_gives_the_same_log_on_every_machine_and_another_seed_another() {
        let (round, hewn) = (pinned(Profile::Round), pinned(Profile::Hewn));
        let bark = bark();
        let sums = [
            checksum(&corners(&log(7, Split::Whole), &round, Some(&bark))),
            checksum(&corners(&log(7, Split::Top), &round, Some(&bark))),
            checksum(&corners(&log(7, Split::Whole), &hewn, None)),
        ];
        assert_eq!(
            sums,
            [
                0xa939_fdd9_58fe_fd8f,
                0xcc2a_4462_31ac_4122,
                0xae55_c81f_31d4_b99f
            ],
            "drawn differently from the machine these were taken on (D30)"
        );
        // The same log with another seed is another log, of the same kind.
        for kit in [&round, &hewn] {
            let (a, b) = (
                corners(&log(7, Split::Whole), kit, Some(&bark)),
                corners(&log(8, Split::Whole), kit, Some(&bark)),
            );
            assert_ne!(a, b);
            assert_eq!(a, corners(&log(7, Split::Whole), kit, Some(&bark)));
        }
    }

    #[test]
    fn logs_vary_in_thickness_and_shade_by_seed_within_the_kits_bounds() {
        let kit = pinned(Profile::Round);
        let mut thickest: f32 = 0.0;
        let mut thinnest = f32::MAX;
        let mut shades = Vec::new();
        for seed in 0..64 {
            let mut surfaces = Surfaces::default();
            draw(&log(seed, Split::Whole), &kit, None, &mut surfaces);
            let face = &surfaces.0[&Material::Log];
            // How far its middle ring reaches from the log's line.
            let middle = face
                .positions
                .iter()
                .filter(|p| (p.x - 1.5).abs() < 0.05)
                .map(|p| ((p.y - 0.125).powi(2) + (p.z - 0.25).powi(2)).sqrt())
                .fold(0.0, f32::max);
            thickest = thickest.max(middle);
            thinnest = thinnest.min(middle);
            shades.push(face.colours[0][1]);
        }
        let v = &kit.timber.variation;
        let r = kit.radius();
        // Thicker and thinner by about the variation and the bow, never by much more.
        assert!(
            thickest - thinnest > r * v.thickness,
            "{thinnest} to {thickest}"
        );
        assert!(
            thickest < r * 1.45 && thinnest > r * 0.8,
            "{thinnest} to {thickest}"
        );
        let (light, dark) = shades
            .iter()
            .fold((0.0f32, 1.0f32), |(l, d), s| (l.max(*s), d.min(*s)));
        assert!(light - dark > v.shade * 0.5 && light - dark <= v.shade + 1e-3);
    }

    #[test]
    fn every_log_is_drawn_facing_out_and_whole() {
        for profile in [Profile::Round, Profile::Hewn] {
            let kit = pinned(profile);
            for split in [Split::Whole, Split::Top, Split::Bottom] {
                let mut surfaces = Surfaces::default();
                draw(&log(3, split), &kit, Some(&bark()), &mut surfaces);
                for material in [Material::Log, Material::LogEnd, Material::Daub] {
                    let what = format!("{profile:?} {split:?} {material:?}");
                    let surface = &surfaces.0[&material];
                    assert!(facing_out(surface, &what) > 0, "{what}");
                    assert_eq!(surface.uvs.len(), surface.positions.len(), "{what}");
                    assert!(surface.normals.iter().all(|n| n.is_finite()), "{what}");
                }
            }
        }
        // An upright post too.
        let post = Piece::Log {
            from: Place::near(Vec3::new(0.25, 0.0, 0.25)),
            to: Place::near(Vec3::new(0.25, 3.0, 0.25)),
            split: Split::Whole,
            seed: 1,
            chinking: None,
            lean: [0, 0],
            role: None,
        };
        let mut surfaces = Surfaces::default();
        draw(&post, &pinned(Profile::Round), None, &mut surfaces);
        assert!(facing_out(&surfaces.0[&Material::Log], "post") > 0);
    }

    #[test]
    fn a_runs_logs_stay_within_its_cubes_and_the_wall_across() {
        // A cabin's logs, drawn: nothing past the cubes along a run, nor out of the
        // wall's cube across it by more than a log's bark and bow.
        for profile in [Profile::Round, Profile::Hewn] {
            let kit = pinned(profile);
            let mut base = layouts::cabin();
            layouts::wall(&mut base, [[3, 0, 5], [4, 0, 5]], Panel::Door);
            for piece in lay(&base, &kit).pieces {
                let Piece::Log { from, to, .. } = piece else {
                    continue;
                };
                let (from, to) = (from.metres(), to.metres());
                let frame = Frame::new(from, to);
                let mut surfaces = Surfaces::default();
                draw(&piece, &kit, Some(&bark()), &mut surfaces);
                for p in surfaces.0.values().flat_map(|s| &s.positions) {
                    let along = (*p - from).dot(frame.axis);
                    assert!(
                        along >= -1e-3 && along <= frame.length + 1e-3,
                        "{profile:?}: {p} past the ends of {from} {to}"
                    );
                    let across = (*p - from).dot(frame.side).abs();
                    assert!(across <= CELL_M * 0.5, "{profile:?}: {p} out of the wall");
                }
            }
        }
    }
}
