//! Long members made from their pieces (D200, docs/building-kits.md): a profile swept along
//! each log the layout gives (`layout::runs`), shaped by the kit's numbers and the piece's
//! seed, its face textured by its material, its bark's relief that material's normal map and
//! parallax (D284), its cut ends
//! showing their grain, and daub in the groove under it. Pure data, so it runs anywhere.
//!
//! Two profiles from one sweep, after the timber spike ([research/timber-walls.md]):
//!
//! - **Round**, a log in its bark: tapered from its butt, bowed, its section a little uneven
//!   and swelling here and there, flattened a little where it sits on the one below, its
//!   bark's relief its material's maps; its sawn ends chamfered by the axe.
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
use glam::Vec3;
use std::f32::consts::{PI, TAU};

/// Faces that meet at more than about 50 degrees are shaded apart, as a crease; less, smooth
/// across (docs/art-style.md).
const CREASE: f32 = 0.64;
/// How far round a hewn beam's edges are rounded, and a sawn plank's.
const HEWN_EDGE_M: f32 = 0.03;
const PLANK_EDGE_M: f32 = 0.006;
/// How far apart a round log's rings are at most where its shape bends: its bow, taper and
/// swells, each over metres, change little in a quarter of one.
const SHAPE_RING_M: f32 = 0.25;
/// How far apart a plank's rings are: its face waves only gently, and a floor has many.
const PLANK_STEP_M: f32 = 0.4;
/// How far in from where it is laid to the daub under a log stops.
const DAUB_IN_M: f32 = 0.002;

/// A hewn member's half width, as a fraction of its kit's log, and half height, of a course.
const HEWN_WIDE: f32 = 0.45;
const HEWN_HIGH: f32 = 0.39;

/// How wide and how high a member laid for a role is as this code draws it, before its seed
/// varies it a little, in metres: what a layout needs to set what rests on it, or what it
/// rests on, against its faces (the asset contract: a piece's real size, from its kit's data,
/// with no offsets kept for a kit in code). A role the kit does not dress as a member is
/// drawn as a log of the kit's own.
pub fn section(kit: &Kit, role: Option<crate::layout::Role>) -> (f32, f32) {
    let (profile, scale) = role
        .and_then(|role| kit.dress.member(role))
        .map_or((kit.timber.profile, 1.0), |(profile, scale, _)| {
            (profile, scale)
        });
    let diameter = kit.diameter * scale;
    match profile {
        Profile::Round => (diameter * 1.03, diameter * 1.03),
        Profile::Hewn => (diameter * HEWN_WIDE * 2.0, kit.course * HEWN_HIGH * 2.0),
    }
}

/// How much of its health a point of a member has lost, from 0 to 1, by where it is and
/// which way its face looks out there (`loss::struck`, D305).
pub type Harm<'a> = &'a dyn Fn(Vec3, Vec3) -> f32;

/// Draws a log, a beam or a post, and the daub under it, into the kit's materials: its face
/// in `Log`, its cut ends in `LogEnd`, its daub in `Daub`.
pub fn draw(piece: &Piece, kit: &Kit, surfaces: &mut Surfaces) {
    draw_harmed(piece, kit, None, surfaces);
}

/// As [`draw`], broken where it has lost so much of its health that chunks of it have gone
/// (`harm`, [`break_up`]).
pub fn draw_harmed(piece: &Piece, kit: &Kit, harm: Option<Harm>, surfaces: &mut Surfaces) {
    let Piece::Log {
        from,
        to,
        split,
        seed,
        chinking,
        lean,
        lift,
        role,
        outside,
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
            lift.map(layout::metres),
            outside.map(|dir| dir.step().as_vec3()),
            harm,
            &dressed,
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
        lift.map(layout::metres),
        outside.map(|dir| dir.step().as_vec3()),
        harm,
        kit,
        surfaces,
    );
}

/// Draws a long member from one point to another in metres, with the daub under it so far
/// along it from `from`, and each end cut to a roof's slope rising so many metres a metre
/// in from it (none, sawn square), from so far over its foot there (`lift`), its weathered
/// side `outside` where it walls a room (else the way the weather comes): as [`draw`]
/// draws a log, and as a roof's rafters are.
#[allow(clippy::too_many_arguments)]
pub(crate) fn member(
    from: Vec3,
    to: Vec3,
    split: Split,
    seed: u32,
    chinking: Option<(f32, f32)>,
    lean: [f32; 2],
    lift: [f32; 2],
    outside: Option<Vec3>,
    harm: Option<Harm>,
    kit: &Kit,
    surfaces: &mut Surfaces,
) {
    let length = from.distance(to);
    if length < 0.05 {
        return;
    }
    let frame = Frame::new(from, to);
    let mut random = Random(seed);
    // Broken only where it has lost enough somewhere along it for chunks to have gone, and
    // only if its kit's timber breaks so; then drawn finer round, so a chunk has an edge.
    let chunks = chunk_depth(kit);
    let harm = harm.filter(|harm| chunks > 0.0 && harmed_along(&frame, kit.radius(), harm));
    let mut made = match kit.timber.profile {
        Profile::Round => round(
            &frame,
            kit,
            split,
            chinking.is_some(),
            outside.unwrap_or(WEATHER),
            if harm.is_some() { 2 } else { 1 },
            &mut random,
        ),
        Profile::Hewn => hewn(&frame, kit, split, &mut random),
    };
    // A gable's log cut to the roof over it: nothing of it above the slope rising from its
    // foot at each end, as a builder saws it.
    let upright = frame.axis.y.abs() > 0.99;
    if upright && lean != [0.0, 0.0] {
        // An upright's top cut to a roof's slope rising so much a metre east and south,
        // its highest at its top.
        let half = section(kit, None).0 * 0.5;
        let top = from.y.max(to.y);
        let middle = top - (lean[0].abs() + lean[1].abs()) * half;
        for p in made.grid.iter_mut().flatten() {
            let cut = middle + lean[0] * (p.x - from.x) + lean[1] * (p.z - from.z);
            p.y = p.y.min(cut);
        }
    } else if lean.iter().any(|&rise| rise > 0.0) {
        // Rings close enough to follow the slope: one drawn far off has its ends alone,
        // and its top would fall from its cut end the whole length of it.
        densify(&mut made, LEAN_RING_M);
        let foot = kit.radius();
        // And a ring where each cut meets the member's top, so its top is level up to there
        // and on the slope after, not cut across between rings far apart.
        let top = made
            .grid
            .iter()
            .flatten()
            .map(|p| p.y - (from.y + (*p - from).dot(frame.axis) * frame.axis.y))
            .fold(f32::MIN, f32::max);
        let mut at = Vec::new();
        if lean[0] > 0.0 {
            at.push((top + foot - lift[0]) / lean[0]);
        }
        if lean[1] > 0.0 {
            at.push(length - (top + foot - lift[1]) / lean[1]);
        }
        rings_at(&mut made, from, frame.axis, &at);
        for p in made.grid.iter_mut().flatten() {
            let along = (*p - from).dot(frame.axis);
            let mut cut = f32::MAX;
            if lean[0] > 0.0 {
                cut = cut.min(from.y - foot + lift[0] + lean[0] * along);
            }
            if lean[1] > 0.0 {
                cut = cut.min(to.y - foot + lift[1] + lean[1] * (length - along));
            }
            p.y = p.y.min(cut);
        }
    }
    if let Some(harm) = harm {
        break_up(&mut made, &frame, harm, chunks, seed);
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
    // Its ends, the cut face's picture across each, turned by the seed, each end a picture
    // of its own where the kit has several.
    let spin = random.unit() * TAU;
    let last = made.grid.len() - 1;
    let ends = surfaces.of(Material::LogEnd);
    for (n, (ring, outward)) in [(&made.grid[0], -frame.axis), (&made.grid[last], frame.axis)]
        .into_iter()
        .enumerate()
    {
        let cell = end_cell(kit, seed.wrapping_add(n as u32));
        let colour = end_colour(t, seed.wrapping_add(n as u32), shade, warmth);
        cap(ends, ring, outward, &frame, spin, made.grain, colour, cell);
    }
    if let Some((near, far)) = chinking {
        // A hair in from where it is laid to, the log's ends or the wall's faces, out of the
        // plane of sawn ends there.
        let (depth, high) = made.daub;
        // And under an end cut to a roof's slope, in from it as far as the slope rises
        // from the log's foot to the daub's top, so none of it stands over the roof.
        let over_foot = (kit.radius() - kit.course * 0.5 + high * 0.5).max(0.0) + DAUB_IN_M;
        let under_slope = lean.map(|rise| if rise > 0.0 { over_foot / rise } else { 0.0 });
        let (near, far) = (
            near.max(under_slope[0]) + DAUB_IN_M,
            // Past its end only at a joint, under the next log (`layout::runs`).
            if under_slope[1] > 0.0 {
                far.min(length - under_slope[1])
            } else {
                far
            } - DAUB_IN_M,
        );
        if far <= near {
            return;
        }
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
    /// What each point's colour is multiplied by, ring by ring, where it differs from one:
    /// the shade of the grooves and the mottling along it. Empty for none.
    tone: Vec<Vec<f32>>,
    /// How worn each point's face is to what its texture wears down to (`Texture::worn`),
    /// ring by ring. Empty for none.
    wear: Vec<Vec<f32>>,
    /// Which points are on a break (`break_up`), shaded by their own flat faces with hard
    /// edges rather than smoothed round, ring by ring. Empty for none.
    cut: Vec<Vec<bool>>,
}

/// How far apart a member's rings are at most where its end is cut to a roof's slope.
const LEAN_RING_M: f32 = 0.25;

/// Rings put between a member's rings where they are further apart than `most`, on the
/// lines between them, so its shape is as it was but its points close enough to be cut.
/// Rings added where a member's rings, along it from `from`, pass these places, each a
/// blend of the two either side.
fn rings_at(made: &mut Member, from: Vec3, axis: Vec3, places: &[f32]) {
    for &place in places {
        let along = |ring: &[Vec3]| {
            let middle = ring.iter().copied().sum::<Vec3>() / ring.len().max(1) as f32;
            (middle - from).dot(axis)
        };
        let Some(i) = (1..made.grid.len()).find(|&i| {
            let (a, b) = (along(&made.grid[i - 1]), along(&made.grid[i]));
            a + 1e-3 < place && place < b - 1e-3
        }) else {
            continue;
        };
        let (a, b) = (along(&made.grid[i - 1]), along(&made.grid[i]));
        let t = (place - a) / (b - a);
        let ring: Vec<Vec3> = made.grid[i - 1]
            .iter()
            .zip(&made.grid[i])
            .map(|(p, q)| p.lerp(*q, t))
            .collect();
        let uv: Vec<[f32; 2]> = made.uvs[i - 1]
            .iter()
            .zip(&made.uvs[i])
            .map(|(p, q)| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t])
            .collect();
        made.grid.insert(i, ring);
        made.uvs.insert(i, uv);
        if !made.tone.is_empty() {
            let tone: Vec<f32> = made.tone[i - 1]
                .iter()
                .zip(&made.tone[i])
                .map(|(p, q)| p + (q - p) * t)
                .collect();
            made.tone.insert(i, tone);
        }
        if !made.wear.is_empty() {
            let wear: Vec<f32> = made.wear[i - 1]
                .iter()
                .zip(&made.wear[i])
                .map(|(p, q)| p + (q - p) * t)
                .collect();
            made.wear.insert(i, wear);
        }
    }
}

/// How deep a kit's timber loses chunks where it is harmed most (`Breaks::Grain`), in
/// metres: none if its kit does not say.
fn chunk_depth(kit: &Kit) -> f32 {
    [Material::Log, Material::Hewn]
        .iter()
        .find_map(|material| match kit.damage.get(material) {
            Some(crate::kit::Breaks::Grain { chunks, .. }) => Some(*chunks),
            _ => None,
        })
        .unwrap_or(0.0)
}

/// From how much lost a member starts to lose chunks, and the share of the places a chunk
/// could break from that do where it has lost everything.
const CHUNK_FROM: f32 = 0.45;
const CHUNK_MOST: f32 = 0.8;
/// How far apart along a member the places a chunk could break from are, in metres, and how
/// long a chunk is along the grain, the least and the most.
const CHUNK_EVERY_M: f32 = 0.16;
const CHUNK_LONG_M: [f32; 2] = [0.14, 0.32];
/// How far apart a member's rings are where it is broken, so a chunk's ends are its own.
const CHUNK_RING_M: f32 = 0.025;
/// How far back a sawn end splinters where it is harmed most, at its longest splinter.
const SPLINTER_M: f32 = 0.14;

/// Whether a member has lost enough anywhere along it to have lost chunks: read every tenth
/// of a metre along it, on each of its four sides.
fn harmed_along(frame: &Frame, radius: f32, harm: Harm) -> bool {
    let count = (frame.length / 0.1).ceil().max(1.0) as usize;
    (0..=count).any(|i| {
        let at = frame.from + frame.axis * (frame.length * i as f32 / count as f32);
        [frame.side, -frame.side, frame.up, -frame.up]
            .into_iter()
            .any(|out| harm(at + out * radius, out) > CHUNK_FROM)
    })
}

/// A member broken where it is harmed (D305), as wood breaks: chunks split or chopped out
/// of it, each cut by flat faces, an impact's one slanting into it along the grain or an
/// axe's two meeting in a V across it, their edges hard; the fresh wood under them bared
/// (its texture's worn set, `Texture::worn`, its grain running along the member) and pale;
/// its sawn ends splintered back into long thin spikes along the grain. Its rings are drawn
/// close where it has lost enough, so a cut has its own edges; the rest of it, between
/// chunks, keeps its round. Points move only in toward its middle or back from its end, so
/// it stays closed and faces out.
fn break_up(made: &mut Member, frame: &Frame, harm: Harm, depth: f32, seed: u32) {
    let middle_of = |ring: &[Vec3]| ring.iter().copied().sum::<Vec3>() / ring.len().max(1) as f32;
    let along = |ring: &[Vec3]| (middle_of(ring) - frame.from).dot(frame.axis);
    let out_at = |p: Vec3, middle: Vec3| {
        let d = p - middle;
        (d - frame.axis * d.dot(frame.axis)).normalize_or_zero()
    };
    let unit = |a: u32, b: i32, c: i32| {
        let mut hash = seed
            ^ a.wrapping_mul(0x9E37_79B1)
            ^ (b as u32).wrapping_mul(0x85EB_CA77)
            ^ (c as u32).wrapping_mul(0xC2B2_AE3D);
        hash ^= hash >> 15;
        hash = hash.wrapping_mul(0x2C1B_3C6D);
        hash ^= hash >> 12;
        (hash & 0xFFFF) as f32 / 65535.0
    };
    let length = frame.length;
    let radius = made.grid[0]
        .iter()
        .map(|p| out_at(*p, middle_of(&made.grid[0])).dot(*p - middle_of(&made.grid[0])))
        .fold(0.0f32, f32::max)
        .max(0.05);
    // The chunks: at places along it, each round it where its seed says, broken out as
    // more is lost there.
    struct Chunk {
        from: f32,
        to: f32,
        /// The flat faces it is cut by, each a point on it and its normal out of the wood.
        faces: [(Vec3, Vec3); 2],
    }
    let mut chunks: Vec<Chunk> = Vec::new();
    let places = (length / CHUNK_EVERY_M).floor().max(1.0) as i32;
    for k in 0..places {
        let r = |salt: u32| unit(salt, k, 7);
        let middle_along = (k as f32 + 0.2 + 0.6 * r(1)) * length / places as f32;
        let turn = r(2) * TAU;
        let out = (frame.side * libm::cosf(turn) + frame.up * libm::sinf(turn)).normalize();
        let centre = frame.from + frame.axis * middle_along;
        let share =
            ((harm(centre + out * radius, out) - CHUNK_FROM) / (1.0 - CHUNK_FROM)).clamp(0.0, 1.0);
        if r(3) >= share * CHUNK_MOST {
            continue;
        }
        let long = CHUNK_LONG_M[0] + (CHUNK_LONG_M[1] - CHUNK_LONG_M[0]) * r(4);
        let (from, to) = (
            (middle_along - long * 0.5).max(0.0),
            (middle_along + long * 0.5).min(length),
        );
        let deep = depth * (0.5 + 0.5 * r(5)) * (0.6 + 0.4 * share);
        let floor = centre + out * (radius - deep);
        // An axe's V across the grain, or an impact's one face slanting in along it from
        // one end, its other end broken off square.
        let (a, b) = if r(6) < 0.5 {
            let slope = 0.9 + 0.8 * r(7);
            (
                (floor, (out + frame.axis * slope).normalize()),
                (floor, (out - frame.axis * slope).normalize()),
            )
        } else {
            let sign = if r(7) < 0.5 { 1.0 } else { -1.0 };
            let slope = 0.25 + 0.3 * r(8);
            let tilt = (out + frame.axis * sign * slope).normalize();
            ((floor, tilt), (floor, tilt))
        };
        chunks.push(Chunk {
            from,
            to,
            faces: [a, b],
        });
    }
    // Rings close where a chunk is, and over the ends where they splinter.
    let end_harm = [frame.from, frame.from + frame.axis * length].map(|end| {
        [frame.side, -frame.side, frame.up, -frame.up]
            .into_iter()
            .map(|out| harm(end + out * radius, out))
            .fold(0.0f32, f32::max)
    });
    let splinters = end_harm.map(|h| ((h - CHUNK_FROM) / (1.0 - CHUNK_FROM)).clamp(0.0, 1.0));
    let mut spans: Vec<(f32, f32)> = chunks
        .iter()
        .map(|c| (c.from - 0.03, c.to + 0.03))
        .collect();
    if splinters[0] > 0.0 {
        spans.push((0.0, SPLINTER_M + 0.04));
    }
    if splinters[1] > 0.0 {
        spans.push((length - SPLINTER_M - 0.04, length));
    }
    let mut places = Vec::new();
    for (from, to) in spans {
        let count = ((to - from) / CHUNK_RING_M).ceil() as usize;
        places.extend((0..=count).map(|k| from + (to - from) * k as f32 / count.max(1) as f32));
    }
    places.sort_by(f32::total_cmp);
    places.dedup_by(|a, b| (*a - *b).abs() < 0.004);
    rings_at(made, frame.from, frame.axis, &places);
    let rows = made.grid.len();
    let columns = made.grid[0].len();
    if made.wear.is_empty() {
        made.wear = vec![vec![0.0; columns]; rows];
    }
    made.cut = vec![vec![false; columns]; rows];
    for i in 0..rows {
        let at = along(&made.grid[i]).clamp(0.0, length);
        for j in 0..columns {
            let mut p = made.grid[i][j];
            let mut cut = false;
            for chunk in &chunks {
                if at < chunk.from || at > chunk.to {
                    continue;
                }
                // Out of the wood past both faces: onto the nearer.
                let past = chunk.faces.map(|(on, normal)| (p - on).dot(normal));
                if past[0] > 0.0 && past[1] > 0.0 {
                    let k = if past[0] < past[1] { 0 } else { 1 };
                    p -= chunk.faces[k].1 * past[k];
                    cut = true;
                }
            }
            // A sawn end splintered back, a spike left standing here and there.
            for (end, share) in splinters.into_iter().enumerate() {
                let from_end = if end == 0 { at } else { length - at };
                if share <= 0.0 || from_end > SPLINTER_M + 0.03 {
                    continue;
                }
                let spike = unit(4 + end as u32, j as i32, 0);
                let reach = if spike < 0.3 {
                    SPLINTER_M * share * 0.08
                } else {
                    SPLINTER_M * share * (0.35 + 0.65 * unit(6 + end as u32, j as i32, 1))
                };
                let inward = if end == 0 { frame.axis } else { -frame.axis };
                let lift = reach * (1.0 - from_end / (SPLINTER_M + 0.03)).clamp(0.0, 1.0);
                if lift > 1e-4 {
                    p += inward * lift;
                    cut = true;
                }
            }
            if cut {
                made.grid[i][j] = p;
                made.cut[i][j] = true;
                made.wear[i][j] = 1.0;
                if let Some(tone) = made.tone.get_mut(i).and_then(|ring| ring.get_mut(j)) {
                    *tone *= FRESH_WOOD;
                }
            }
        }
    }
}

/// How much paler the fresh wood under a chunk is than the face round it.
const FRESH_WOOD: f32 = 1.18;

fn densify(made: &mut Member, most: f32) {
    let mut grid = Vec::with_capacity(made.grid.len());
    let mut uvs = Vec::with_capacity(made.uvs.len());
    let mut tone = Vec::with_capacity(made.tone.len());
    let mut wear = Vec::with_capacity(made.wear.len());
    for i in 0..made.grid.len() {
        if i > 0 {
            let (a, b) = (&made.grid[i - 1], &made.grid[i]);
            let gap = a[0].distance(b[0]);
            let more = (gap / most).ceil() as usize;
            for k in 1..more {
                let t = k as f32 / more as f32;
                grid.push(a.iter().zip(b).map(|(p, q)| p.lerp(*q, t)).collect());
                let (ua, ub) = (&made.uvs[i - 1], &made.uvs[i]);
                uvs.push(
                    ua.iter()
                        .zip(ub)
                        .map(|(p, q)| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t])
                        .collect(),
                );
                if !made.tone.is_empty() {
                    let (ta, tb) = (&made.tone[i - 1], &made.tone[i]);
                    tone.push(ta.iter().zip(tb).map(|(p, q)| p + (q - p) * t).collect());
                }
                if !made.wear.is_empty() {
                    let (wa, wb) = (&made.wear[i - 1], &made.wear[i]);
                    wear.push(wa.iter().zip(wb).map(|(p, q)| p + (q - p) * t).collect());
                }
            }
        }
        grid.push(made.grid[i].clone());
        uvs.push(made.uvs[i].clone());
        if !made.tone.is_empty() {
            tone.push(made.tone[i].clone());
        }
        if !made.wear.is_empty() {
            wear.push(made.wear[i].clone());
        }
    }
    (made.grid, made.uvs, made.tone, made.wear) = (grid, uvs, tone, wear);
}

/// The way the weather comes from, in a base's own space, for a log that walls no room:
/// the face of a lying log that looks most this way loses its bark first
/// (`Timber::exposed`); one walling a room loses it on its face out of it (D261).
const WEATHER: Vec3 = Vec3::new(-0.8, 0.0, 0.6);
/// How much paler a weathered log is at its very ends.
const END_WEATHER: f32 = 0.14;
/// How far in from a sawn end a log wearing a worn texture has lost its bark, most at the
/// end (`Texture::worn`).
const END_WEAR_M: f32 = 0.7;

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
    walled: bool,
    weather: Vec3,
    finer: u32,
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
    let columns = (t.sides * finer) as usize;
    let around = TAU * radius;
    // A face weathered where a log is exposed (`textures.ts blend --exposed`) is laid once
    // round a lying log from underneath, its middle on top and its first quarter to the
    // weather, so the bark it has lost lies there; any other face, wherever its seed says.
    let exposed = t.exposed && frame.axis.y.abs() < 0.5;
    let to_weather = frame.side.dot(weather) >= 0.0;
    // Its bark worn to bare wood point by point, where its texture names what it wears to:
    // near its sawn ends, on its top and on the face the weather beats on, in patches.
    let worn = exposed
        && kit
            .textures
            .get(&Material::Log)
            .is_some_and(|texture| texture.worn.is_some());
    let weather_flat = weather.with_y(0.0).normalize_or_zero();
    let uv = |t_along: f32, j: usize| {
        let along = shift[1] + t_along * frame.length / metres[1];
        if exposed {
            let round = j as f32 / columns as f32;
            [if to_weather { round } else { 1.0 - round }, along]
        } else {
            [
                shift[0] + j as f32 / columns as f32 * around / metres[0],
                along,
            ]
        }
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
    // Its shade: drawn from a seed of its own, so its shape is as it was without it.
    let mut own = Random(random.0 ^ 0x5bd1_e995);
    let mottle_phase = [own.unit() * TAU, own.unit() * TAU, own.unit() * TAU];
    let level = frame.axis.y.abs() < 0.5;
    // Knots where branches were trimmed off: each a low swell, dark at its heart, along it
    // and round it where its seed says, none in the sawn ends.
    let count = (frame.length * t.knots + own.unit()).floor() as usize;
    let knots: Vec<[f32; 4]> = (0..count)
        .map(|_| {
            let size = 0.03 + 0.025 * own.unit();
            // Clear of the sawn ends and their chamfer by three times its size.
            let clear = (3.0 * size).min(frame.length * 0.5);
            let along = clear + (frame.length - 2.0 * clear) * own.unit();
            let angle = own.unit() * TAU;
            let high = 0.006 + 0.008 * own.unit();
            [along, angle, size, high]
        })
        .collect();
    // Its rings where its shape bends and no closer (D284): the bark's relief is its
    // material's normal map and parallax, not its points, so between its chamfered ends a
    // ring every `SHAPE_RING_M` follows its bow, taper and swells, and three across each
    // knot raise it.
    let mut at = stations(frame.length, t.step.max(SHAPE_RING_M), chamfer);
    let ends = (at[2], at[at.len() - 3]);
    for &[along, _, size, _] in &knots {
        for k in [-1.0, 0.0, 1.0] {
            let s = (along + k * size) / frame.length;
            if s > ends.0 && s < ends.1 {
                at.push(s);
            }
        }
    }
    at.sort_by(f32::total_cmp);
    at.dedup_by(|a, b| (*a - *b) * frame.length < 0.01);
    let last = at.len() - 1;
    let mut grid = Vec::with_capacity(at.len());
    let mut uvs = Vec::with_capacity(at.len());
    // What its shade round it is, the same all along it: toward the grooves, where it meets
    // the logs above and below, in shade, most under it, where the sky does not reach; its
    // top bleached by the sun a little, as its underside is not.
    let shade: Vec<[f32; 3]> = round
        .iter()
        .map(|&[sin, cos, ..]| {
            let mut k = 1.0;
            if level {
                if walled {
                    let under = smoothstep(0.3, 0.92, -sin);
                    let over = smoothstep(0.5, 0.95, sin);
                    k *= 1.0 - t.groove * (under + 0.6 * over);
                }
                k *= 1.0 + 0.5 * t.groove * sin;
            }
            [k, libm::sinf(2.0 * cos), libm::cosf(2.0 * cos)]
        })
        .collect();
    let mut tone = Vec::with_capacity(at.len());
    let mut wear = Vec::with_capacity(if worn { at.len() } else { 0 });
    for (i, &along) in at.iter().enumerate() {
        let (shift_sin, shift_cos) = (libm::sinf(along * 1.3), libm::cosf(along * 1.3));
        // Lighter and darker in long patches along it, and round it a little.
        let metres = along * frame.length;
        let patch = t.variation.mottle
            * (0.6 * libm::sinf(metres * 1.9 + mottle_phase[0])
                + 0.4 * libm::sinf(metres * 4.7 + mottle_phase[1]))
            // Weathered paler toward its ends, where the rain and sun get at it most.
            + if exposed && !worn {
                END_WEATHER * (1.0 - smoothstep(0.0, 0.45, metres.min(frame.length - metres)))
            } else {
                0.0
            };
        // sin(2 cos a + x), by the sum of angles, a little lighter or darker round it too.
        let (x_sin, x_cos) = (
            libm::sinf(metres * 3.1 + mottle_phase[2]),
            libm::cosf(metres * 3.1 + mottle_phase[2]),
        );
        let mut ring_tone: Vec<f32> = shade
            .iter()
            .map(|&[k, turn_sin, turn_cos]| {
                let round = 0.3 * t.variation.mottle * (turn_sin * x_cos + turn_cos * x_sin);
                ((1.0 + patch + round) * k).max(0.0)
            })
            .collect();
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
            let angle = -PI * 0.5 + TAU * j as f32 / columns as f32;
            let mut knot = 0.0;
            for &[at_m, at_angle, size, high] in &knots {
                let dx = metres - at_m;
                if dx.abs() > 3.0 * size {
                    continue;
                }
                let turn = (angle - at_angle + PI).rem_euclid(TAU) - PI;
                let da = turn * radius;
                let w = libm::expf(-(dx * dx + da * da) / (size * size));
                knot += high * w * fade;
                ring_tone[j] *= 1.0 - 0.5 * w * w;
            }
            let r = radius * (1.0 + taper * butt) * swell * uneven * end + knot;
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
        if worn {
            let end = 1.0 - smoothstep(0.0, END_WEAR_M, metres.min(frame.length - metres));
            wear.push(
                round
                    .iter()
                    .map(|&[sin, cos, ..]| {
                        let out = frame.side * cos + frame.up * sin;
                        // Its face to the weather most, its top on that side some, the
                        // top on its sheltered side a little, underneath not at all.
                        let to = out.dot(weather_flat);
                        let facing =
                            (0.8 * to.max(0.0) + 0.4 * sin.max(0.0) * (to + 1.0) * 0.5).min(1.0);
                        let patchy = 0.5
                            + 0.3 * libm::sinf(metres * 1.3 + mottle_phase[1] + cos * 0.9)
                            + 0.2 * libm::sinf(metres * 3.7 + mottle_phase[2]);
                        (end + facing * (0.2 + 0.45 * patchy)).clamp(0.0, 1.0)
                    })
                    .collect(),
            );
        }
        tone.push(ring_tone);
        grid.push(ring);
        uvs.push(ring_uv);
    }
    Member {
        grid,
        uvs,
        grain: [0.47, 0.47],
        daub: (kit.radius() * t.daub.0, t.daub.1),
        tone,
        wear,
        cut: Vec::new(),
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
    let half_wide = kit.diameter * HEWN_WIDE * thickness;
    let half_high = kit.course * HEWN_HIGH * (1.0 + t.variation.thickness * 0.3 * random.signed());
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
    // How far across the texture each point of the section falls. Drawn from one plank, each
    // face takes the plank across it, running on round the section from face to face (the
    // top left to right, the right side up, the underside right to left, the left side
    // down), so every edge meets its neighbour: a joist's side or a rim beam's took one
    // column of the texture smeared down it.
    let across = |j: usize| match look.strip {
        Some([from, wide]) => {
            let (p, n) = section[j % columns];
            let x = (p.x / (2.0 * half_wide) + 0.5).clamp(0.0, 1.0);
            let y = (p.y / (2.0 * half_high) + 0.5).clamp(0.0, 1.0);
            let k = match (n.y.abs() >= n.x.abs(), n.y > 0.0, n.x > 0.0) {
                (true, true, _) => x,
                (true, false, _) => 1.0 - x,
                (false, _, true) => y,
                (false, _, false) => 1.0 - y,
            };
            from + wide * k
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
        tone: Vec::new(),
        wear: Vec::new(),
        cut: Vec::new(),
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
        // A few metres off and further, its ends alone: a board's gentle wave along it is
        // nothing there, and a deck has many.
        step: if kit.dress.far {
            COARSE_STEP_M
        } else {
            PLANK_STEP_M
        },
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
    let cell = end_cell(kit, seed);
    for (n, (ring, outward)) in [
        (&member.grid[0], -frame.axis),
        (&member.grid[last], frame.axis),
    ]
    .into_iter()
    .enumerate()
    {
        let colour = end_colour(t, seed.wrapping_add(n as u32), shade, warmth);
        cap(
            ends,
            ring,
            outward,
            &frame,
            spin,
            member.grain,
            colour,
            cell,
        );
    }
}

/// A sawn end's colour: the kit's end tint, lighter or darker and warmer or cooler with its
/// member as the member's face is, and then each end its own, a little lighter or darker and
/// greyed by the weather from hardly at all to nearly half, as a wall's ends are where some
/// were cut later or face the rain: one tint at every end of a wall read as a row of the same
/// sticker (research/kit-review-2026-10-14.md).
fn end_colour(t: &crate::kit::Timber, seed: u32, shade: f32, warmth: f32) -> [f32; 3] {
    let mut random = Random(layout::hash(&[seed, 0x5a3d]));
    let own = 1.0 + END_SHADE * random.signed();
    let grey = END_GREY * random.unit() * random.unit();
    let tinted = [
        t.end_tint[0] * shade * own * (1.0 + warmth),
        t.end_tint[1] * shade * own,
        t.end_tint[2] * shade * own * (1.0 - warmth),
    ];
    let luma = (tinted[0] + tinted[1] + tinted[2]) / 3.0;
    tinted.map(|v| (v + (luma - v) * grey).clamp(0.0, 1.0))
}

/// How much lighter or darker each sawn end is than its member's, and the most it is
/// greyed toward its own lightness.
const END_SHADE: f32 = 0.1;
const END_GREY: f32 = 0.6;

/// A member's rings as a closed tube of triangles, each corner shaded smooth across its
/// neighbours but at a crease, its texture as its rings say, the seam's last column its own.
fn sweep(out: &mut Surface, member: &Member, colour: [f32; 3]) {
    let grid = &member.grid;
    let rows = grid.len();
    let columns = grid[0].len();
    let colour_at = |r: usize, c: usize| match member.tone.get(r).and_then(|ring| ring.get(c)) {
        Some(&k) => colour.map(|v| (v * k).clamp(0.0, 1.0)),
        None => colour,
    };
    let wear_at = |r: usize, c: usize| {
        member
            .wear
            .get(r)
            .and_then(|ring| ring.get(c))
            .copied()
            .unwrap_or(0.0)
    };
    // A triangle on a break is shaded flat, its own face, so a break's edges are hard.
    let cut_at = |r: usize, c: usize| {
        member
            .cut
            .get(r)
            .and_then(|ring| ring.get(c))
            .copied()
            .unwrap_or(false)
    };
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
                    let flat = tri
                        .iter()
                        .any(|&index| cut_at(quad[index].0, quad[index].1));
                    let indices = tri.map(|index| {
                        let (r, c) = quad[index];
                        let column = if index >= 2 && k == 0 { columns } else { c };
                        let normal = normals[r][c].normalize_or_zero();
                        let uv = member.uvs[r][column];
                        if flat || normal.dot(own) < CREASE {
                            return out.shared.worn_corner(
                                grid[r][c],
                                own,
                                uv,
                                colour_at(r, c),
                                wear_at(r, c),
                            );
                        }
                        let slot = &mut smooth[r * (columns + 1) + column];
                        if *slot == u32::MAX {
                            *slot = out.shared.worn_corner(
                                grid[r][c],
                                normal,
                                uv,
                                colour_at(r, c),
                                wear_at(r, c),
                            );
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
                let flat = tri
                    .iter()
                    .any(|&index| cut_at(quad[index].0, quad[index].1));
                for index in tri {
                    let (r, c) = quad[index];
                    let smooth = if flat {
                        own
                    } else {
                        normals[r][c].normalize_or_zero()
                    };
                    if wear_at(r, c) > 0.0 {
                        crate::mesh::worn(&mut out.wear, out.positions.len());
                        out.wear.push(wear_at(r, c));
                    }
                    out.positions.push(grid[r][c]);
                    out.normals.push(if smooth.dot(own) < CREASE {
                        own
                    } else {
                        smooth
                    });
                    // The seam's last column takes its own place on the texture.
                    let column = if index >= 2 && k == 0 { columns } else { c };
                    out.uvs.push(member.uvs[r][column]);
                    out.colours.push(colour_at(r, c));
                }
            }
        }
    }
}

/// Which picture of the kit's sawn ends an end draws, by a seed, where its `LogEnd` texture
/// is an atlas of several: the corner of its cell on the texture, and the cell's size.
pub(crate) fn end_cell(kit: &Kit, seed: u32) -> ([f32; 2], f32) {
    let cells = kit
        .textures
        .get(&Material::LogEnd)
        .map_or(1, |texture| texture.cells.max(1));
    if cells == 1 {
        return ([0.0, 0.0], 1.0);
    }
    let which = crate::layout::hash(&[seed, 0x0e4d]) % (cells * cells);
    let size = 1.0 / cells as f32;
    (
        [(which % cells) as f32 * size, (which / cells) as f32 * size],
        size,
    )
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A flat cap over a ring, facing `outward`, the end grain's picture across it, turned by
/// `spin`, as much of it as `grain` says, from the atlas's cell `cell` gives.
#[allow(clippy::too_many_arguments)]
fn cap(
    out: &mut Surface,
    ring: &[Vec3],
    outward: Vec3,
    frame: &Frame,
    spin: f32,
    grain: [f32; 2],
    colour: [f32; 3],
    cell: ([f32; 2], f32),
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
        let (corner, size) = cell;
        [corner[0] + size * (0.5 + x), corner[1] + size * (0.5 - y)]
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
    use crate::layout::roles::Dir;
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
            lift: [0, 0],
            role: None,
            outside: None,
        }
    }

    fn corners(piece: &Piece, kit: &Kit) -> Vec<Vec3> {
        let mut surfaces = Surfaces::default();
        draw(piece, kit, &mut surfaces);
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

    #[test]
    fn a_seed_gives_the_same_log_on_every_machine_and_another_seed_another() {
        let (round, hewn) = (pinned(Profile::Round), pinned(Profile::Hewn));
        let sums = [
            checksum(&corners(&log(7, Split::Whole), &round)),
            checksum(&corners(&log(7, Split::Top), &round)),
            checksum(&corners(&log(7, Split::Whole), &hewn)),
        ];
        assert_eq!(
            sums,
            [
                0xa1ea_b741_3e3f_3a37,
                0x0422_b06f_8ea0_fcee,
                0x7791_36f7_9c91_a57f
            ],
            "drawn differently from the machine these were taken on (D30)"
        );
        // The same log with another seed is another log, of the same kind.
        for kit in [&round, &hewn] {
            let (a, b) = (
                corners(&log(7, Split::Whole), kit),
                corners(&log(8, Split::Whole), kit),
            );
            assert_ne!(a, b);
            assert_eq!(a, corners(&log(7, Split::Whole), kit));
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
            draw(&log(seed, Split::Whole), &kit, &mut surfaces);
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
    fn a_wall_log_is_shaded_toward_its_grooves_and_its_ends_draw_their_own_pictures() {
        let mut kit = pinned(Profile::Round);
        kit.timber.groove = 0.4;
        kit.timber.knots = 0.0;
        kit.timber.variation.mottle = 0.0;
        kit.textures.insert(
            Material::LogEnd,
            crate::kit::Texture {
                path: "ends".into(),
                metres: [1.0, 1.0],
                depth: 0.0,
                reflectance: crate::kit::matte(),
                strips: None,
                cells: 2,
                worn: None,
            },
        );
        let mut surfaces = Surfaces::default();
        draw(&log(5, Split::Whole), &kit, &mut surfaces);
        let face = &surfaces.0[&Material::Log];
        // Its middle's corners, by how high on it they are: under it darkest, its side
        // lightest, its top between.
        let lightness = |from: f32, to: f32| {
            let picked: Vec<f32> = face
                .positions
                .iter()
                .zip(&face.colours)
                .filter(|(p, _)| (p.x - 1.5).abs() < 0.3 && (from..to).contains(&(p.y - 0.125)))
                .map(|(_, c)| c[1])
                .collect();
            picked.iter().sum::<f32>() / picked.len().max(1) as f32
        };
        let (under, side, top) = (
            lightness(-1.0, -0.12),
            lightness(-0.03, 0.03),
            lightness(0.12, 1.0),
        );
        assert!(under < top && top < side, "{under} {top} {side}");
        assert!(under < side * 0.75, "{under} {side}");
        // Each end within one cell of the atlas, and across seeds every cell drawn.
        let mut cells = std::collections::BTreeSet::new();
        for seed in 0..16 {
            let mut surfaces = Surfaces::default();
            draw(&log(seed, Split::Whole), &kit, &mut surfaces);
            let ends = &surfaces.0[&Material::LogEnd];
            for corner in ends.uvs.chunks(3 * 18) {
                let cell = |uv: &[f32; 2]| ((uv[0] * 2.0) as i32, (uv[1] * 2.0) as i32);
                let first = cell(&corner[0]);
                assert!(
                    corner.iter().all(|uv| cell(uv) == first),
                    "an end across cells"
                );
                cells.insert(first);
            }
        }
        assert_eq!(cells.len(), 4, "{cells:?}");
    }

    #[test]
    fn a_walls_sawn_ends_differ_from_one_another_and_some_are_greyed() {
        let kit = pinned(Profile::Round);
        let mut ends: Vec<[f32; 3]> = Vec::new();
        for seed in 0..24 {
            let mut surfaces = Surfaces::default();
            draw(&log(seed, Split::Whole), &kit, &mut surfaces);
            let drawn = &surfaces.0[&Material::LogEnd];
            // Each end one colour across it, its two ends their own.
            let half = drawn.colours.len() / 2;
            for colours in [&drawn.colours[..half], &drawn.colours[half..]] {
                assert!(colours.iter().all(|c| c == &colours[0]));
                ends.push(colours[0]);
            }
        }
        let lightness = |c: &[f32; 3]| (c[0] + c[1] + c[2]) / 3.0;
        let (dark, light) = ends
            .iter()
            .map(lightness)
            .fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(v), h.max(v)));
        assert!(
            light - dark > 0.08,
            "every end the same lightness: {dark} {light}"
        );
        // As warm as the tint at most, and greyer: red against blue spread out.
        let warmth = |c: &[f32; 3]| c[0] - c[2];
        let tint = kit.timber.end_tint;
        let (cool, warm) = ends
            .iter()
            .map(warmth)
            .fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(v), h.max(v)));
        assert!(
            cool < (tint[0] - tint[2]) * 0.7,
            "none greyed: {cool} {warm}"
        );
        assert!(warm - cool > 0.05, "{cool} {warm}");
    }

    #[test]
    fn a_plank_drawn_from_one_board_of_its_texture_is_not_smeared_down_its_sides() {
        let mut kit = pinned(Profile::Hewn);
        kit.textures.insert(
            Material::Floor,
            crate::kit::Texture {
                path: "planks".into(),
                metres: [2.0, 2.0],
                depth: 0.0,
                reflectance: crate::kit::matte(),
                strips: Some([0.06, 0.11]),
                cells: 1,
                worn: None,
            },
        );
        // A joist, 5 cm wide and 20 cm deep, three metres long.
        let ticks = |m: f32| crate::layout::ticks(m);
        let joist = Piece::Plank {
            from: Place::near(Vec3::new(0.0, 0.0, 0.0)),
            to: Place::near(Vec3::new(3.0, 0.0, 0.0)),
            wide: ticks(0.05),
            high: ticks(0.2),
            seed: 3,
        };
        let mut surfaces = Surfaces::default();
        plank(&joist, &kit, &mut surfaces);
        let all = surfaces.0[&Material::Floor].indexed();
        let mut sides = 0;
        for t in all.indices.chunks_exact(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| i as usize);
            let normal = (all.positions[b] - all.positions[a])
                .cross(all.positions[c] - all.positions[a])
                .normalize_or_zero();
            let ys = [a, b, c].map(|i| all.positions[i].y);
            let high = ys.iter().fold(f32::MIN, |m, &y| m.max(y))
                - ys.iter().fold(f32::MAX, |m, &y| m.min(y));
            if normal.z.abs() < 0.9 || high < 0.05 {
                continue;
            }
            sides += 1;
            // Up the side, its texture runs across the plank as it does across its top,
            // about as many metres of it a metre: not one column of it all the way down.
            let us = [a, b, c].map(|i| all.uvs[i][0]);
            let spread = us.iter().fold(f32::MIN, |m, &u| m.max(u))
                - us.iter().fold(f32::MAX, |m, &u| m.min(u));
            let metres_a_metre = spread * 2.0 / high;
            assert!(
                (0.5..2.0).contains(&metres_a_metre),
                "a side's triangle stretched: {us:?} over {high} m"
            );
        }
        assert!(sides > 0);
    }

    #[test]
    fn a_weathered_face_is_laid_with_its_top_up_and_its_weather_face_to_the_weather() {
        let mut kit = pinned(Profile::Round);
        kit.timber.exposed = true;
        // Both ways along the same line: the face looking +Z, toward the weather, takes the
        // texture's first quarter, the top its middle.
        for (from, to) in [(0.0, 3.0), (3.0, 0.0)] {
            let piece = Piece::Log {
                from: Place::near(Vec3::new(from, 0.125, 0.25)),
                to: Place::near(Vec3::new(to, 0.125, 0.25)),
                split: Split::Whole,
                seed: 4,
                chinking: None,
                lean: [0, 0],
                lift: [0, 0],
                role: None,
                outside: None,
            };
            let mut surfaces = Surfaces::default();
            draw(&piece, &kit, &mut surfaces);
            let face = &surfaces.0[&Material::Log];
            let all = face
                .positions
                .iter()
                .zip(&face.uvs)
                .chain(face.shared.positions.iter().zip(&face.shared.uvs));
            let middle: Vec<_> = all.filter(|(p, _)| (p.x - 1.5).abs() < 0.1).collect();
            let most = |key: &dyn Fn(&Vec3) -> f32| {
                middle
                    .iter()
                    .max_by(|a, b| key(a.0).total_cmp(&key(b.0)))
                    .map(|(_, uv)| uv[0].rem_euclid(1.0))
                    .unwrap()
            };
            let (top, weather) = (most(&|p| p.y), most(&|p| p.z));
            assert!((top - 0.5).abs() < 0.08, "{from}: top at {top}");
            assert!(
                (weather - 0.25).abs() < 0.08,
                "{from}: weather face at {weather}"
            );
        }
    }

    #[test]
    fn a_log_wears_to_bare_wood_at_its_ends_and_on_its_face_out_of_its_room() {
        let mut kit = pinned(Profile::Round);
        kit.timber.exposed = true;
        kit.textures.insert(
            Material::Log,
            crate::kit::Texture {
                path: "bark".into(),
                metres: [1.0, 2.0],
                depth: 0.0,
                reflectance: crate::kit::matte(),
                strips: None,
                cells: 1,
                worn: Some("bare".into()),
            },
        );
        // Along X; its room to the north (-Z), so its outside is south, against the way the
        // weather comes for a log walling nothing (from the north-west).
        for (outside, out_z) in [(Some(Dir::South), 1.0), (Some(Dir::North), -1.0)] {
            let piece = Piece::Log {
                from: Place::near(Vec3::new(0.0, 0.125, 0.25)),
                to: Place::near(Vec3::new(3.0, 0.125, 0.25)),
                split: Split::Whole,
                seed: 4,
                chinking: None,
                lean: [0, 0],
                lift: [0, 0],
                role: None,
                outside,
            };
            let mut surfaces = Surfaces::default();
            draw(&piece, &kit, &mut surfaces);
            let face = surfaces.0[&Material::Log].indexed();
            assert_eq!(face.wear.len(), face.positions.len());
            let mean = |keep: &dyn Fn(&Vec3) -> bool| {
                let picked: Vec<f32> = face
                    .positions
                    .iter()
                    .zip(&face.wear)
                    .filter(|(p, _)| keep(p))
                    .map(|(_, w)| *w)
                    .collect();
                picked.iter().sum::<f32>() / picked.len().max(1) as f32
            };
            let middle = |p: &Vec3| (p.x - 1.5).abs() < 0.4;
            let side = |sign: f32| move |p: &Vec3| middle(p) && (p.z - 0.25) * sign > 0.08;
            let (out, inside) = (mean(&side(out_z)), mean(&side(-out_z)));
            assert!(out > inside + 0.15, "{outside:?}: out {out}, in {inside}");
            // Its sawn ends lose their bark whichever way it faces; its middle underside keeps it.
            let ends = mean(&|p: &Vec3| p.x < 0.1 || p.x > 2.9);
            let under = mean(&|p: &Vec3| middle(p) && p.y < 0.05);
            assert!(ends > 0.7, "ends {ends}");
            assert!(under < 0.2, "underside {under}");
        }
    }

    #[test]
    fn a_log_harmed_past_half_loses_chunks_out_of_its_outline_and_splinters_at_its_end() {
        let mut kit = pinned(Profile::Round);
        kit.damage.insert(
            Material::Log,
            crate::kit::Breaks::Grain {
                splits: 5.0,
                chunks: 0.06,
            },
        );
        // Its west half all but gone, its east half whole.
        let harm = |p: Vec3, _: Vec3| if p.x < 1.5 { 0.95 } else { 0.0 };
        let piece = log(3, Split::Whole);
        let (mut whole, mut harmed) = (Surfaces::default(), Surfaces::default());
        draw(&piece, &kit, &mut whole);
        draw_harmed(&piece, &kit, Some(&harm), &mut harmed);
        // How near its middle line it comes between two places along it.
        let nearest = |surfaces: &Surfaces, from: f32, to: f32| {
            surfaces.0[&Material::Log]
                .positions
                .iter()
                .filter(|p| p.x > from && p.x < to)
                .map(|p| glam::Vec2::new(p.y - 0.125, p.z - 0.25).length())
                .fold(f32::MAX, f32::min)
        };
        let lost = nearest(&whole, 0.2, 1.3) - nearest(&harmed, 0.2, 1.3);
        assert!(lost > 0.025, "chunks out of its outline: {lost}");
        let kept = nearest(&whole, 1.7, 2.8) - nearest(&harmed, 1.7, 2.8);
        assert!(kept.abs() < 0.004, "whole where not harmed: {kept}");
        // Broken in places, round between them: not the whole of its harmed half eaten.
        let round = nearest(&whole, 0.2, 1.3) - 0.004;
        let points: Vec<f32> = harmed.0[&Material::Log]
            .positions
            .iter()
            .filter(|p| p.x > 0.2 && p.x < 1.3)
            .map(|p| glam::Vec2::new(p.y - 0.125, p.z - 0.25).length())
            .collect();
        let broken = points.iter().filter(|r| **r < round).count();
        assert!(
            broken * 50 > points.len() && broken * 2 < points.len(),
            "{broken} of {} points broken",
            points.len()
        );
        // Its west end splintered back unevenly, its east end sawn square.
        let spread = |surfaces: &Surfaces, near: f32| {
            let xs: Vec<f32> = surfaces.0[&Material::LogEnd]
                .positions
                .iter()
                .map(|p| p.x)
                .filter(|x| (x - near).abs() < 0.2)
                .collect();
            xs.iter().copied().fold(f32::MIN, f32::max)
                - xs.iter().copied().fold(f32::MAX, f32::min)
        };
        assert!(spread(&harmed, 0.0) > 0.02, "{}", spread(&harmed, 0.0));
        assert!(spread(&whole, 0.0) < 0.005 && spread(&harmed, 3.0) < 0.005);
        // And still facing out from its middle line, clear of its ends, where splinters
        // turn its faces every way.
        let log = &harmed.0[&Material::Log];
        let mut inward = 0;
        for corners in log.positions.chunks(3) {
            if corners.iter().all(|p| p.x > 0.2 && p.x < 2.8) {
                let own = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
                let centre = (corners[0] + corners[1] + corners[2]) / 3.0;
                let out = Vec3::new(0.0, centre.y - 0.125, centre.z - 0.25);
                if own.dot(out) <= 0.0 {
                    inward += 1;
                }
            }
        }
        assert_eq!(inward, 0, "triangles facing in");
        for material in [Material::Log, Material::LogEnd] {
            assert!(harmed.0[&material].positions.iter().all(|p| p.is_finite()));
        }
    }

    #[test]
    fn every_log_is_drawn_facing_out_and_whole() {
        for profile in [Profile::Round, Profile::Hewn] {
            let kit = pinned(profile);
            for split in [Split::Whole, Split::Top, Split::Bottom] {
                let mut surfaces = Surfaces::default();
                draw(&log(3, split), &kit, &mut surfaces);
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
            lift: [0, 0],
            role: None,
            outside: None,
        };
        let mut surfaces = Surfaces::default();
        draw(&post, &pinned(Profile::Round), &mut surfaces);
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
                draw(&piece, &kit, &mut surfaces);
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
