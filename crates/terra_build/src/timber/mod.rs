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

/// A hewn member's half width, as a fraction of its kit's log.
const HEWN_WIDE: f32 = 0.45;
/// How far a wall's log reaches past where it meets the next course, in metres, so the two
/// sit into each other along their scribed faces and no line of light shows between.
const SCRIBE_M: f32 = 0.005;

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
        // What rests on a beam is set a hair into its flat bed, not on it, so the two faces
        // are not drawn in one plane.
        Profile::Hewn => (diameter * HEWN_WIDE * 2.0, hewn_high(kit) * 2.0),
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
    // Lighter and darker by its seed about the kit's tint, not only darker, which left a
    // wall's logs a fifth of the light under their tint on average (D373).
    let shade = 1.0 + t.variation.shade * (random.unit() - 0.5);
    let warmth = t.variation.warmth * random.signed();
    let colour = [
        (t.tint[0] * shade * (1.0 + warmth)).clamp(0.0, 1.0),
        (t.tint[1] * shade).clamp(0.0, 1.0),
        (t.tint[2] * shade * (1.0 - warmth)).clamp(0.0, 1.0),
    ];
    let age = weathered(&mut made, kit, &frame, outside.unwrap_or(WEATHER), seed);
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
            chinked(kit, seed, age),
        );
    }
}

mod cuts;
mod harm;
mod plank;
mod shapes;
mod sweep;

use cuts::{LEAN_RING_M, densify, level_cut, plumb_cut, rings_at};
use harm::{break_up, chunk_depth, harmed_along};
use shapes::{Squared, hewn, hewn_high, round, squared};
use sweep::{cap, end_colour, smoothstep, sweep};

pub use plank::plank;
pub(crate) use sweep::end_cell;

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

/// A member weathered its own way (D262 as amended, the developers: "id like them more
/// varied and we want to focus on a degree of realism per our art style"): its seed's age,
/// how far from fresh brown toward grey silver its colour has gone and how much more of its
/// bark is worn to bare wood; dark stains in patches along it, sap run and mild rot; and
/// lichen flecking its face to the weather and its top. All by the kit's `variation`, none
/// where it gives none. Gives its age, 0 to 1.
fn weathered(made: &mut Member, kit: &Kit, frame: &Frame, weather: Vec3, seed: u32) -> f32 {
    let v = &kit.timber.variation;
    let mut random = Random(terra_gen::hash::of(&[seed, 0x77e1]));
    // Each log its own age, from fresh to as grey as the kit lets it go.
    let age = (v.age * random.unit()).clamp(0.0, 1.0);
    if v.age <= 0.0 && v.stain <= 0.0 && v.lichen <= 0.0 {
        return age;
    }
    let phases = [
        random.unit() * TAU,
        random.unit() * TAU,
        random.unit() * TAU,
    ];
    let weather_flat = weather.with_y(0.0).normalize_or_zero();
    // Cooler and paler as it greys, toward silver: most of the kit's warm tint undone and a
    // little lighter, so an old log's wood is grey where a young one's is brown, never blue.
    let tint = kit.timber.tint;
    let mean = (tint[0] + tint[1] + tint[2]) / 3.0;
    let silver: [f32; 3] = std::array::from_fn(|c| {
        1.0 + (libm::powf(mean / tint[c].max(0.05), 0.75) * 1.06 - 1.0) * age
    });
    let stain_tint = v.stain_tint;
    let lichen_tint = v.lichen_tint;
    let mut hue = Vec::with_capacity(made.grid.len());
    for ring in &made.grid {
        let centre = ring.iter().copied().sum::<Vec3>() / ring.len().max(1) as f32;
        let metres = (centre - frame.from).dot(frame.axis);
        // Stains in patches a few tens of centimetres long, darkest at their middles.
        let s = 0.55 * libm::sinf(metres * 2.3 + phases[0])
            + 0.45 * libm::sinf(metres * 5.9 + phases[1]);
        let stained = v.stain * smoothstep(0.35, 0.95, s);
        let mut out = Vec::with_capacity(ring.len());
        for (j, p) in ring.iter().enumerate() {
            let outward = (*p - centre).normalize_or_zero();
            let to = outward.dot(weather_flat).max(0.0);
            let up = outward.y.max(0.0);
            // Lichen in flecks a hand across, on the face the weather beats on and the top.
            let fleck = libm::sinf(metres * 11.0 + j as f32 * 1.7 + phases[2])
                * libm::sinf(metres * 7.3 - j as f32 * 0.9 + phases[0]);
            let lichen = v.lichen * (0.7 * to + 0.5 * up).min(1.0) * smoothstep(0.2, 0.7, fleck);
            out.push([0, 1, 2].map(|c| {
                let k = silver[c] * (1.0 + (stain_tint[c] - 1.0) * stained);
                k * (1.0 + (lichen_tint[c] - 1.0) * lichen)
            }));
        }
        hue.push(out);
    }
    made.hue = hue;
    // An older log has lost more of its bark where the weather gets at it, none more
    // underneath, where nothing does.
    for ring in &mut made.wear {
        for w in ring {
            *w = (*w * (1.0 + age * *w)).min(1.0);
        }
    }
    age
}

/// The daub's colour under a member: the kit's chinking, a little lighter or darker by the
/// member's seed, as each course was daubed in its own batch, and dried paler where the
/// member above has weathered most.
fn chinked(kit: &Kit, seed: u32, age: f32) -> [f32; 3] {
    let mut random = Random(terra_gen::hash::of(&[seed, 0xd0b]));
    let shade = 1.0 + kit.timber.variation.shade * 0.7 * random.signed() + 0.08 * age;
    kit.colours.chinking.map(|v| (v * shade).clamp(0.0, 1.0))
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
    /// What each point's colour is multiplied by for its weathering (`weathered`), ring by
    /// ring. Empty for none.
    hue: Vec<Vec<[f32; 3]>>,
    /// Which points are on a break (`break_up`), shaded by their own flat faces with hard
    /// edges rather than smoothed round, ring by ring. Empty for none.
    cut: Vec<Vec<bool>>,
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

#[cfg(test)]
mod tests;
