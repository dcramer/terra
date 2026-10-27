//! Where the clouds are (docs/sky.md stage 1, D52, D315, D518, D540): the shapes' drift on the
//! wind at their height and the weather map they are drawn from, both worked out from the
//! world's clock and the climate alone, never a machine's run time, so every machine at one
//! clock draws one sky.
//!
//! - **The shapes** are `sky.wesl`'s noise in the start's frame (`Start`, metres east and south
//!   of it), the air's own frame (D212, D334) rather than the planet's: a direction meets a
//!   layer [`LAYER_M`] over the eye, and the noise there is shifted by the wind aloft times the
//!   world's seconds. The shift is worked out here in `f64`, an offset for each octave of each
//!   field, wrapped where the shader's noise lattice repeats ([`PERIOD`]), so it stays exact on
//!   a world years old and the shader adds only small numbers.
//! - **The weather map** ([`build_map`]) is the climate's cloud round the eye out past the
//!   horizon the layer reaches, a texture as Horizon's Nubis has (Schneider 2015, 2017): cover,
//!   what falls, the cloud's kind, and the share of it that is snow. It is built for whole steps
//!   of the clock ([`MAP_STEP_DAYS`]) and the sky blends from one step's to the next's, so it
//!   changes as the climate does, a few times a minute of play, never every frame (D127).

use crate::climate::ClimateFields;
use crate::start::Start;
use bevy::math::{DVec2, Vec2, Vec3, Vec4};
use terra_climate::{Climate, WorldTime};

/// The 2D layer's height over the eye: a cumulus base (sky.md's table, 0.5 to 2 km).
pub const LAYER_M: f32 = 1500.0;
/// How many metres of the layer a unit of the shapes' noise spans: the look's 0.55 units to a
/// layer's height, as it was drawn before the layer had a height.
const SHAPE_M: f64 = LAYER_M as f64 / 0.55;
/// The clouds' fields, each `sky.wesl`'s billow at so many metres a unit: the shapes, the deck's
/// broad bellies (0.28 of the shapes' frequency) and its fine scud (1.3 of it).
pub const FIELD_M: [f64; 3] = [SHAPE_M, SHAPE_M / 0.28, SHAPE_M / 1.3];
/// Which wind carries each field: the layer's, the layer's, the scud's under the deck.
const FIELD_WIND: [Carried; 3] = [Carried::Layer, Carried::Layer, Carried::Scud];
/// The octaves of `sky.wesl`'s billow: how much finer each is, and the shift it is taken at.
const OCTAVES: [(f64, f64); 4] = [(1.0, 0.0), (2.1, 7.0), (4.3, 19.0), (8.9, 5.0)];
/// The shader's noise lattice repeats every so many cells (`sky.wesl`'s `PERIOD`), so an offset
/// wraps there with no seam. At the shapes' 2.7 km a unit it repeats every 700 km.
pub const PERIOD: f64 = 256.0;
/// The scud under a deck: low cloud a few hundred metres up.
const SCUD_M: f64 = 300.0;
/// The height the climate's wind is given at, as a station's anemometer.
const WIND_GIVEN_M: f64 = 10.0;
/// Hellmann's exponent over open land: the wind at a height grows as its seventh root, so
/// 1.5 km up it is about twice the ground's (research/2026-10-26-volumetric-skies.md §3).
const HELLMANN: f64 = 1.0 / 7.0;
/// How far the wind over the friction layer is turned from the ground's: to the right in the
/// northern hemisphere, to the left in the southern (the Ekman spiral; over land the surface
/// wind is backed some 20 to 30 degrees from the wind aloft, Stull, *An Introduction to
/// Boundary Layer Meteorology*, 1988, §8.5).
const VEER_DEG: f64 = 20.0;

/// The weather map: so many texels a side, each so many metres, round its middle.
pub const MAP_TEXELS: u32 = 128;
pub const MAP_TEXEL_M: f32 = 500.0;
/// The map's side: 64 km, past the 30 km to which the layer reaches at its lowest drawn lift.
pub const MAP_SIDE_M: f32 = MAP_TEXELS as f32 * MAP_TEXEL_M;
/// The map's middle moves on a grid of so many metres, so a place is mapped the same each time
/// and by every machine whose eye is near it.
const MIDDLE_GRID_M: f32 = 4000.0;
/// The map is built for whole steps of the clock, an hour of the world's (75 s of play), and
/// drawn blended between the step's and the next: the fronts move some 40 m in a step, a
/// fraction of a texel.
pub const MAP_STEP_DAYS: f64 = 1.0 / 24.0;

#[derive(Clone, Copy)]
enum Carried {
    Layer,
    Scud,
}

/// The winds that carry the clouds, metres a second east and south in the start's frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CloudWinds {
    /// At the layer's height: the climate's prevailing wind grown by Hellmann's law and veered.
    pub layer: DVec2,
    /// The scud under a deck, low down: grown less and not turned.
    pub scud: DVec2,
}

impl CloudWinds {
    /// The climate's winds aloft, its prevailing wind at its mean speed in a frame whose north
    /// is `north` (east and south). The wind aloft is the steady large-scale flow the ground's
    /// gusts and turnings are stirred down from (`terra_climate::Day::at`), so the clouds ride
    /// the climate's mean, as Nubis and Flight Simulator move theirs by one wind per weather.
    pub fn of(climate: &Climate, north: Vec2, northern: bool) -> Self {
        let w = &climate.wind;
        let north = north.as_dvec2().normalize_or(DVec2::new(0.0, -1.0));
        // East is a quarter turn clockwise from north, seen from above: in a frame of east and
        // south, (x, z), that is north turned by +90 degrees about up.
        let east = DVec2::new(-north.y, north.x);
        let toward = |bearing_deg: f64, height_m: f64| {
            let speed = f64::from(w.mean_m_s) * libm::pow(height_m / WIND_GIVEN_M, HELLMANN);
            let b = bearing_deg.to_radians();
            (east * libm::sin(b) + north * libm::cos(b)) * speed
        };
        // It blows from its bearing, so toward the opposite one.
        let bearing = f64::from(w.prevailing_from_deg) + 180.0;
        let veer = if northern { VEER_DEG } else { -VEER_DEG };
        Self {
            layer: toward(bearing + veer, f64::from(LAYER_M)),
            scud: toward(bearing, SCUD_M),
        }
    }

    /// The winds of a world's climate in the start's frame.
    pub fn of_world(fields: &ClimateFields, start: &Start) -> Self {
        let at = start.translation;
        let north = start.rotation.inverse() * fields.frame.north(at);
        let northern = fields.frame.latitude(at) >= 0.0;
        Self::of(
            fields.fields.climate(),
            Vec2::new(north.x, north.z),
            northern,
        )
    }
}

/// The world's seconds of play at a moment of its clock: what the clouds drift by.
pub fn seconds(time: WorldTime) -> f64 {
    time.0 * f64::from(crate::daylight::DAY_SECONDS)
}

/// The shader's offset for each octave of each field (`sky.wesl`'s `drifting`): the noise at
/// `p` metres east and south of `anchor` in the start's frame is the noise at
/// `p / FIELD_M[f] * frequency + offset`, the shapes carried by their wind for so many seconds.
/// Wrapped where the lattice repeats, so the sum the shader makes is the unwrapped one exactly.
pub fn offsets(winds: &CloudWinds, seconds: f64, anchor: Vec2) -> [Vec4; 12] {
    let mut out = [Vec4::ZERO; 12];
    for (f, (&metres, carried)) in FIELD_M.iter().zip(FIELD_WIND).enumerate() {
        let wind = match carried {
            Carried::Layer => winds.layer,
            Carried::Scud => winds.scud,
        };
        // Where the anchor is in the moving air: the shapes over it came from upwind.
        let at = (anchor.as_dvec2() - wind * seconds) / metres;
        for (o, &(frequency, shift)) in OCTAVES.iter().enumerate() {
            let wrap = |v: f64| (v * frequency + shift).rem_euclid(PERIOD) as f32;
            out[f * 4 + o] =
                Vec4::new(wrap(at.x), shift.rem_euclid(PERIOD) as f32, wrap(at.y), 0.0);
        }
    }
    out
}

/// How far the volumetric clouds' 3D noise (`noise3d`) repeats, metres: the shapes' tile and the
/// edges' finer one (docs/sky.md stage 2, Nubis's shape and detail textures).
pub const SHAPE_TILE_M: f64 = 6000.0;
pub const DETAIL_TILE_M: f64 = 750.0;

/// Where the volumetric clouds' 3D noise is at the clock (`cloud_volume.wesl`): the noise at `p`
/// metres east and south of `anchor` is the shapes' tile at `p / SHAPE_TILE_M + (x, z)` and the
/// detail's at `p / DETAIL_TILE_M + (y, w)`, carried by the wind at the layer as the 2D shapes
/// are (`offsets`). Worked out in `f64` and wrapped to a share of each tile, so it stays exact on
/// a world years old and the shader adds only small numbers.
pub fn volume_offsets(winds: &CloudWinds, seconds: f64, anchor: Vec2) -> Vec4 {
    let at = anchor.as_dvec2() - winds.layer * seconds;
    let share = |v: f64, tile: f64| (v / tile).rem_euclid(1.0) as f32;
    Vec4::new(
        share(at.x, SHAPE_TILE_M),
        share(at.y, SHAPE_TILE_M),
        share(at.x, DETAIL_TILE_M),
        share(at.y, DETAIL_TILE_M),
    )
}

/// The layer one kind of cloud fills (docs/sky.md's table, D544): its base and top in metres
/// over the low ground the start stands on, the base lowered and the top raised by the rain
/// falling from it (0 to 1) and the top by how much of the sky it covers (0 to 1); and the
/// highest over the sea its top grows to, since the air over 3 km is the free air's, level over
/// the land (D544: congestus to 4.2 km, nimbostratus to 3.8 km).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    pub base_m: f32,
    pub base_by_rain_m: f32,
    pub top_m: f32,
    pub top_by_cover_m: f32,
    pub top_by_rain_m: f32,
    pub highest_over_sea_m: f32,
}

/// The kinds' layers, as the weather map's kind runs from 0 to 1 (docs/sky.md's heights, D544):
/// the overcast deck (stratus, its base 500 m over the ground and lower in rain, its tops 1.1 km
/// and 1.8 km in rain, never over nimbostratus's 3.8 km over the sea), stratocumulus (900 m,
/// 500 m thick) and cumulus (base 1.1 km, fair-weather tops 1.7 km, piling to 3.1 km the more of
/// the sky it covers, never over congestus's 4.2 km over the sea). The one place the clouds'
/// heights are kept: the march reads them (`layers_over_sea`).
pub const LAYERS: [Layer; 3] = [
    Layer {
        base_m: 500.0,
        base_by_rain_m: -200.0,
        top_m: 1100.0,
        top_by_cover_m: 0.0,
        top_by_rain_m: 700.0,
        highest_over_sea_m: 3800.0,
    },
    Layer {
        base_m: 900.0,
        base_by_rain_m: 0.0,
        top_m: 1400.0,
        top_by_cover_m: 0.0,
        top_by_rain_m: 0.0,
        highest_over_sea_m: f32::INFINITY,
    },
    Layer {
        base_m: 1100.0,
        base_by_rain_m: 0.0,
        top_m: 1700.0,
        top_by_cover_m: 1400.0,
        top_by_rain_m: 0.0,
        highest_over_sea_m: 4200.0,
    },
];

/// The kinds' layers as the march takes them, over the sea for low ground `ground_m` over it:
/// rows of base, base by rain, top, top by cover and top by rain, each with the deck's,
/// stratocumulus's and cumulus's in x, y and z; and the slab all of them stand in, its bottom
/// and top over the sea. A top's growth by cover and rain is cut so it grows at most to its
/// kind's highest over the sea.
pub fn layers_over_sea(ground_m: f32) -> ([Vec4; 5], Vec2) {
    let grown = |l: &Layer| {
        let growth = l.top_by_cover_m.max(0.0) + l.top_by_rain_m.max(0.0);
        let room = (l.highest_over_sea_m - ground_m - l.top_m).max(0.0);
        if growth > room { room / growth } else { 1.0 }
    };
    let row = |f: &dyn Fn(&Layer) -> f32, over: f32| {
        Vec3::new(f(&LAYERS[0]), f(&LAYERS[1]), f(&LAYERS[2])) + Vec3::splat(over)
    };
    let rows = [
        row(&|l| l.base_m, ground_m),
        row(&|l| l.base_by_rain_m, 0.0),
        row(&|l| l.top_m, ground_m),
        row(&|l| l.top_by_cover_m * grown(l), 0.0),
        row(&|l| l.top_by_rain_m * grown(l), 0.0),
    ];
    let lowest = rows[0] + rows[1].min(Vec3::ZERO);
    let highest = rows[2] + rows[3].max(Vec3::ZERO) + rows[4].max(Vec3::ZERO);
    (
        rows.map(|r| r.extend(0.0)),
        Vec2::new(lowest.min_element(), highest.max_element()),
    )
}

/// Where the weather map round an eye has its middle, metres east and south of the start: on a
/// grid, so every eye near a place maps it alike.
pub fn middle_for(eye: Vec2) -> Vec2 {
    (eye / MIDDLE_GRID_M).round() * MIDDLE_GRID_M
}

/// The step of the clock a moment is in, and how far through it.
pub fn step_of(time: WorldTime) -> (i64, f32) {
    let steps = time.0 / MAP_STEP_DAYS;
    let step = libm::floor(steps);
    (step as i64, (steps - step) as f32)
}

/// The moment a step of the map is built for.
pub fn step_time(step: i64) -> WorldTime {
    WorldTime(step as f64 * MAP_STEP_DAYS)
}

/// One texel of the weather map, as Nubis's weather texture lays it out, with the same 0 to 1
/// ranges stage 2's volumes read:
/// - **r, cover**: the climate's cloud there, 0 clear to 1 overcast;
/// - **g, what falls**: how far rain or snow has thickened and darkened the cloud, 0 to 1
///   (`weather::Under::sky`'s thickening, so the 2D deck darkens as the light under it does);
/// - **b, kind**: Nubis's cloud type, 1 heaped cumulus, a half stratocumulus, 0 a stratus deck,
///   from the cover as the deck closes (`weather::deck_cover`, sky.md's table);
/// - **a, snow**: the share of what falls that is snow, which makes cloud flatter and paler.
pub fn texel(now: &terra_climate::Conditions) -> [u8; 4] {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [
        byte(now.cloud),
        byte(crate::weather::thickening(now.precipitation_mm_per_hour)),
        byte(1.0 - crate::weather::deck_cover(now.cloud)),
        byte(now.snow_share),
    ]
}

/// One row of the weather map with this middle at a moment: texels from west to east, the row
/// `row` from the north edge southward. Each is the climate at sea level under the texel's
/// middle (the climate's fronts do not depend on height, D315).
pub fn map_row(
    fields: &ClimateFields,
    start: &Start,
    middle: Vec2,
    time: WorldTime,
    row: u32,
) -> Vec<u8> {
    let corner = middle - Vec2::splat(MAP_SIDE_M / 2.0);
    let sea = fields.sea_radius_m();
    let mut out = Vec::with_capacity(MAP_TEXELS as usize * 4);
    for column in 0..MAP_TEXELS {
        let at = corner + (Vec2::new(column as f32, row as f32) + 0.5) * MAP_TEXEL_M;
        let point = start.at(at.x, at.y).normalize_or(Vec3::Y) * sea;
        out.extend(texel(&fields.fields.at(fields.place(point), time)));
    }
    out
}

/// The whole weather map with this middle at a moment, its rows on every core, as `Rgba8Unorm`
/// texels: a function of the climate, the place and the clock alone.
pub fn build_map(fields: &ClimateFields, start: &Start, middle: Vec2, time: WorldTime) -> Vec<u8> {
    let rows: Vec<u32> = (0..MAP_TEXELS).collect();
    terra_gen::par::map(&rows, |&row| map_row(fields, start, middle, time, row)).concat()
}

#[cfg(test)]
mod tests;
