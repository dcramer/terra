//! A world's mountains (docs/demos.md, D322): ranges raised where a broad noise says, their
//! ridges ridged fractal noise damped by its own slope, and gullies cut down their flanks,
//! with what lies on them (bare rock, scree under cliffs, gravel in the gullies' beds) by
//! the rules of slope. All set by a theme's numbers, so a range is rounded domes or sharp
//! arêtes, young or worn, by data, not code.
//!
//! - **Ridges**: ridged fractal noise, each octave's `1 - |n|` sharpened, its amplitude damped
//!   by the slope the octaves before it built (Quilez, "More noise", 2008:
//!   https://iquilezles.org/articles/morenoise), so steep flanks are smooth and fine
//!   relief gathers on the gentler ground and the crests.
//! - **Gullies**: stripes laid along the downhill direction, cell by cell, the way
//!   Runevision's erosion filter makes them ("Fast and gorgeous erosion filter", 2026:
//!   https://blog.runevision.com/2026/03/fast-and-gorgeous-erosion-filter.html). Worked from
//!   the write-up only; its code is MPL-2.0, which an MIT project may not take (D196). Each
//!   finer set follows the slope the coarser ones left, so they branch.
//! - **What lies on them** the ground works out from these (`field`): bare rock past 40
//!   degrees, scree under cliffs, gravel in the gullies' beds ([`Rise::bed`]).

use glam::Vec3;
use terra_gen::{Noise, Seed};

/// A world's mountains, as its theme's numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mountains {
    /// How high the land a range stands on is raised, in metres: its valleys' floors.
    pub base_m: f32,
    /// How much higher its crests stand over its valleys, at most, in metres.
    pub relief_m: f32,
    /// How far apart its main ridges are, about, in metres.
    pub ridges_m: f32,
    /// How far across a range is, about, in metres, and the share of the land in ranges.
    pub range_m: f32,
    pub share: f32,
    /// How sharp its crests are: 1 rounded domes, 2 ridges, 3 knife-edged arêtes.
    pub sharpness: f32,
    /// How a slope bends from its valley to its crest: 1 straight, 2 a valley's gentle foot
    /// rising ever steeper to the crest, as glaciers and rivers shape them.
    pub concavity: f32,
    /// How much its slopes smooth the finer relief on them: 0 none, as young rock; 4 well worn.
    pub erosion: f32,
    /// How deep its widest gullies are cut, in metres, and how far apart they are.
    pub gullies_m: f32,
    pub gully_spacing_m: f32,
    /// How deep the floors of its valleys are hollowed, at most, in metres, as glaciers
    /// overdeepen them: the basins lakes fill (D322).
    pub basins_m: f32,
}

impl Mountains {
    /// The starting realm's: granite ranges as the Tatras are, valleys at about 950 m over
    /// the sea and crests a kilometre and more above them (docs/demos.md, open question 1).
    pub const START: Mountains = Mountains {
        base_m: 950.0,
        relief_m: 1800.0,
        ridges_m: 10500.0,
        concavity: 1.5,
        range_m: 15_000.0,
        share: 0.3,
        sharpness: 1.3,
        erosion: 1.2,
        gullies_m: 10.0,
        gully_spacing_m: 140.0,
        basins_m: 60.0,
    };

    /// The most the mountains raise the land, in metres.
    pub fn highest_m(&self) -> f32 {
        self.base_m + self.relief_m + self.gullies_m
    }
}

/// What the mountains make of a point on the ground.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rise {
    /// How much they raise the ground there, in metres.
    pub height_m: f32,
    /// How far down into a gully's bed it lies, 0 on a rib between gullies to 1 at the bottom
    /// of every set of them.
    pub bed: f32,
}

/// A world's mountains, ready to ask.
#[derive(Clone, Debug)]
pub struct Range {
    pub mountains: Mountains,
    /// Where the ranges are, their ridges and their gullies.
    ranges: Noise,
    ridges: Noise,
    gullies: Noise,
}

/// Octaves of ridges, from the main ridges to relief about 80 m across.
const OCTAVES: usize = 6;
/// Each octave is this much fainter than the last.
const GAIN: f32 = 0.5;
/// Sets of gullies, each finer by this much, and their depths' shares.
const GULLY_SETS: [(f32, f32); 3] = [(1.0, 1.0), (0.42, 0.45), (0.18, 0.2)];
/// The spread of `fbm` of three octaves, measured
/// (`the_range_noise_spreads_as_the_share_assumes`): a share of the land is ranges above
/// the threshold that many spreads up.
const RANGE_SD: f32 = 0.12;
/// How far into a range, as a share of `inside`, its floor has risen to its height.
const FLOOR_RISES: f32 = 0.45;
/// How far the ridges' space is warped, in its own units, and how strongly each octave's
/// ridges carry the next's (Musgrave's gain).
const WARP: f32 = 0.35;
const BRANCHING: f32 = 2.0;
/// The ridged sum's valleys are below the first, its crests at the second.
const VALLEY: f32 = 0.40;
const CREST: f32 = 0.68;
/// How far under the valleys' share of the ridged sum, as a share of the valleys' to the
/// crests', the floors are hollowed deepest.
const HOLLOWS: f32 = 0.45;
/// How much the stripes lean toward a rib, as if a cell's point there weighed so much.
const FADE_TO_RIB: f32 = 0.02;
/// How far across the ground the ridges' slope is measured, for the gullies to follow it: the
/// fall over tens of metres, not the crease at a crest, so the gullies turn smoothly over one.
const SLOPE_STEP_M: f32 = 20.0;

impl Range {
    pub fn new(mountains: Mountains, seed: Seed) -> Self {
        let seed = seed.derive("mountains");
        Self {
            mountains,
            ranges: Noise::new(seed.derive("ranges")),
            ridges: Noise::new(seed.derive("ridges")),
            gullies: Noise::new(seed.derive("gullies")),
        }
    }

    /// How much of a range a point is in, 0 out of it to 1 in its heart.
    pub fn inside(&self, point: Vec3) -> f32 {
        let m = &self.mountains;
        let n = self.ranges.fbm(point / m.range_m, 3, 2.0, 0.5);
        let threshold = RANGE_SD * upper_quantile(m.share);
        smoothstep(threshold - 0.16, threshold + 0.16, n)
    }

    /// How much the ridges raise a point, before gullies, in metres.
    fn ridged(&self, point: Vec3) -> f32 {
        let m = &self.mountains;
        let inside = self.inside(point);
        if inside <= 0.0 {
            return 0.0;
        }
        let ridged = self.ridges(point / m.ridges_m);
        // The range's floor rises gently with how far into it a point is; its ridges stand on
        // it, the lowest share of the sum its valleys, the rest rising faster toward the crests.
        let along = (ridged - VALLEY) / (CREST - VALLEY);
        let crest = along.clamp(0.0, 1.0);
        // The range's floor rises over its outer part and is level within, where its valleys
        // hold their lakes.
        let floor = m.base_m * smoothstep(0.0, FLOOR_RISES, inside);
        // The floors hollowed where the ridged sum falls lowest, furthest from every ridge, as
        // glaciers overdeepen a cirque's floor and a lake fills it.
        let hollow = m.basins_m * smoothstep(0.0, -HOLLOWS, along) * inside;
        floor + m.relief_m * libm::powf(inside, 0.6) * libm::powf(crest, m.concavity) - hollow
    }

    /// The ridged sum at a point of the ridges' own space, about 0 to 1.
    ///
    /// Musgrave's ridged multifractal ("Texturing and Modeling", 2003, ch. 16): each octave's
    /// ridges are weighted by the ridges of the octaves before it, so spurs branch off the
    /// main ridges and the valleys between stay smooth; each also damped by the slope the
    /// octaves before it built, as Quilez damps his. The ridges' space is first warped by a
    /// broad noise, so main ridges wander rather than running in long smooth arcs.
    fn ridges(&self, q: Vec3) -> f32 {
        let m = &self.mountains;
        let warp = Vec3::new(
            self.ridges.sample(q * 0.5 + Vec3::splat(101.0)),
            self.ridges.sample(q * 0.5 + Vec3::splat(203.0)),
            self.ridges.sample(q * 0.5 + Vec3::splat(307.0)),
        );
        let q = q + warp * WARP;
        let (mut sum, mut amplitude, mut norm, mut frequency) = (0.0, 1.0, 0.0, 1.0);
        let mut weight = 1.0;
        let mut slope = Vec3::ZERO;
        for octave in 0..OCTAVES {
            let shift = Vec3::splat(octave as f32 * 19.19);
            let (n, g) = self.ridges.sample_with_gradient(q * frequency + shift);
            let r = 1.0 - n.abs();
            let ridge = libm::powf(r, m.sharpness) * weight;
            // Quilez's damping: an octave counts for less where those before it built slope.
            // The slope is the noise's own, smooth, not the ridges', which turns over at every
            // crest and would make the next octaves jump there.
            let damp = 1.0 / (1.0 + m.erosion * slope.length_squared());
            sum += amplitude * ridge * damp;
            slope += amplitude * g * frequency * weight;
            norm += amplitude;
            weight = (ridge * BRANCHING).clamp(0.0, 1.0);
            amplitude *= GAIN;
            frequency *= 2.0;
        }
        sum / norm
    }

    /// What the mountains make of the ground in a direction from the centre of a planet of so
    /// many metres' radius.
    pub fn rise(&self, direction: Vec3, radius_m: f32) -> Rise {
        let up = direction;
        let point = direction * radius_m;
        let ridged = self.ridged(point);
        if self.inside(point) <= 0.0 {
            return Rise::default();
        }
        let m = &self.mountains;
        if m.gullies_m <= 0.0 {
            return Rise {
                height_m: ridged,
                bed: 0.0,
            };
        }
        // How the ridges rise round the point, over a few metres along each of the world's
        // axes, which unlike any pair of directions across the ground never turn over, then
        // laid level.
        // Centred, so at a summit or in a hollow, where the ground has no way down, it is
        // level and no gully is cut.
        let across = |axis: Vec3| {
            self.ridged(point + axis * SLOPE_STEP_M) - self.ridged(point - axis * SLOPE_STEP_M)
        };
        let rise =
            Vec3::new(across(Vec3::X), across(Vec3::Y), across(Vec3::Z)) / (2.0 * SLOPE_STEP_M);
        let mut slope = rise - up * rise.dot(up);
        let steepness = slope.length();
        // The gullies, each set following the slope the ones before it left.
        let mut carved = 0.0;
        let mut deepest = 0.0;
        if steepness > 0.02 {
            // Full depth from a gentle slope up, so the depth does not follow every change of
            // the fall near a crest.
            let fade = smoothstep(0.02, 0.4, steepness);
            for (k, (scale, depth)) in GULLY_SETS.iter().enumerate() {
                let spacing = m.gully_spacing_m * scale;
                let (stripes, across, along) = self.stripes(point, up, slope, spacing, k as i32);
                let depth = m.gullies_m * depth * fade;
                // 0 on the ribs between gullies, the whole depth down a gully's line.
                carved += depth * (stripes - 1.0) * 0.5;
                deepest += depth;
                // The next, finer set follows the gully walls this set cut, so they branch;
                // turned by at most half the fall, so the way down never vanishes and turns
                // over, which would make the finer stripes jump.
                let turn = (depth * 0.5 * along).clamp(-0.5, 0.5) * steepness;
                slope += across * turn;
            }
        }
        Rise {
            height_m: ridged + carved,
            bed: if deepest > 0.0 {
                -carved / deepest
            } else {
                0.0
            },
        }
    }

    /// Stripes along the downhill direction round a point, laid cell by cell: each cell of
    /// `spacing` has a point of its own, and its stripes run through it across the slope, so
    /// they line up with the ground's fall wherever it turns. Gives -1 along a gully's line to
    /// 1 on the rib between, the slope across the stripes it falls toward, and how fast the
    /// stripes change across, a metre.
    fn stripes(
        &self,
        point: Vec3,
        up: Vec3,
        slope: Vec3,
        spacing: f32,
        set: i32,
    ) -> (f32, Vec3, f32) {
        let fall = slope.normalize_or(up.any_orthonormal_vector());
        let across = up.cross(fall).normalize_or(up.any_orthonormal_vector());
        let cell = point / spacing;
        let base = cell.floor();
        let (mut weights, mut cosine, mut sine) = (0.0, 0.0, 0.0);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let c = base + Vec3::new(dx as f32, dy as f32, dz as f32);
                    let jitter =
                        self.gullies
                            .point_in(c.x as i32 + set * 7919, c.y as i32, c.z as i32);
                    let d = cell - (c + jitter);
                    // Within one cell only: any point nearer than that is in the cells round, so
                    // the stripes never jump as the point crosses into another cell.
                    let far = d.length_squared();
                    if far >= 1.0 {
                        continue;
                    }
                    let w = (1.0 - far) * (1.0 - far) * (1.0 - far);
                    let phase = d.dot(across) * core::f32::consts::TAU;
                    weights += w;
                    cosine += w * libm::cosf(phase);
                    sine += w * libm::sinf(phase);
                }
            }
        }
        // Where no cell's point is near, the stripes fade to a rib, not a jump.
        let value = (cosine + FADE_TO_RIB) / (weights + FADE_TO_RIB);
        let change = -(sine / (weights + FADE_TO_RIB)) * core::f32::consts::TAU / spacing;
        (value, across, change)
    }
}

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How many standard deviations a normal variable passes only a share of the time
/// (Abramowitz and Stegun 26.2.23).
fn upper_quantile(share: f32) -> f32 {
    let share = share.clamp(1e-4, 1.0 - 1e-4);
    let tail = |p: f32| {
        let t = libm::sqrtf(-2.0 * libm::logf(p));
        t - (2.515_517 + 0.802_853 * t + 0.010_328 * t * t)
            / (1.0 + 1.432_788 * t + 0.189_269 * t * t + 0.001_308 * t * t * t)
    };
    if share <= 0.5 {
        tail(share)
    } else {
        -tail(1.0 - share)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADIUS: f32 = 30_000.0;

    fn range() -> Range {
        Range::new(Mountains::START, Seed(1))
    }

    /// Directions spread evenly over the sphere (a Fibonacci lattice).
    fn directions(count: usize) -> impl Iterator<Item = Vec3> {
        (0..count).map(move |i| {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
            let around = i as f32 * 2.399_963;
            let flat = libm::sqrtf(1.0 - y * y);
            Vec3::new(flat * libm::cosf(around), y, flat * libm::sinf(around))
        })
    }

    #[test]
    fn the_range_noise_spreads_as_the_share_assumes() {
        let range = range();
        let n = 20_000;
        let inside = directions(n)
            .filter(|&d| range.inside(d * RADIUS) > 0.5)
            .count() as f32
            / n as f32;
        let share = Mountains::START.share;
        assert!(
            (inside - share).abs() < 0.1,
            "{inside} of the land, not {share}"
        );
    }

    #[test]
    fn ranges_stand_high_and_the_land_out_of_them_is_untouched() {
        let range = range();
        let mut highest: f32 = 0.0;
        for d in directions(20_000) {
            let rise = range.rise(d, RADIUS);
            if range.inside(d * RADIUS) <= 0.0 {
                assert_eq!(rise, Rise::default());
            }
            assert!(rise.height_m <= Mountains::START.highest_m());
            highest = highest.max(rise.height_m);
        }
        let m = Mountains::START;
        assert!(highest > m.base_m + m.relief_m * 0.5, "{highest}");
    }

    #[test]
    fn gullies_only_cut_down_and_their_beds_lie_lowest() {
        let range = range();
        let (mut beds, mut ribs) = ((0.0, 0), (0.0, 0));
        for d in directions(20_000) {
            let ridged = range.ridged(d * RADIUS);
            let rise = range.rise(d, RADIUS);
            assert!(rise.height_m <= ridged + 1e-3, "{rise:?} over {ridged}");
            let cut = ridged - rise.height_m;
            if rise.bed > 0.7 {
                beds = (beds.0 + cut, beds.1 + 1);
            } else if rise.bed < 0.3 && ridged > 0.0 {
                ribs = (ribs.0 + cut, ribs.1 + 1);
            }
        }
        assert!(beds.1 > 0 && ribs.1 > 0);
        assert!(beds.0 / beds.1 as f32 > 3.0 * ribs.0 / ribs.1 as f32);
    }

    #[test]
    fn the_ridged_sum_spreads_as_valleys_and_crests_assume() {
        // A quarter of the ground is valley floor, and only the highest few in a hundred
        // reach the crests.
        let range = range();
        let mut v: Vec<f32> = directions(20_000)
            .map(|d| range.ridges(d * RADIUS / 5000.0))
            .collect();
        v.sort_by(f32::total_cmp);
        let share_under = |x: f32| v.iter().filter(|&&r| r < x).count() as f32 / v.len() as f32;
        assert!(
            (0.15..0.4).contains(&share_under(VALLEY)),
            "{}",
            share_under(VALLEY)
        );
        assert!(
            (0.97..1.0).contains(&share_under(CREST)),
            "{}",
            share_under(CREST)
        );
    }

    #[test]
    fn the_mountains_never_jump() {
        // Ground two centimetres apart is never more than a quarter of a metre apart, as on a
        // crease steeper than any wall: no step where a cell, a set of gullies or a measuring
        // direction changes, which would stand as a ledge however close one looked.
        let range = range();
        let mut worst = (0.0, Vec3::ZERO);
        for d in directions(200_000) {
            if range.inside(d * RADIUS) <= 0.0 {
                continue;
            }
            let apart = |cm: f32| {
                let step = d.any_orthonormal_vector() * (cm * 0.005 / RADIUS);
                (range.rise((d + step).normalize(), RADIUS).height_m
                    - range.rise((d - step).normalize(), RADIUS).height_m)
                    .abs()
            };
            let near = apart(2.0);
            // A crease's rise shrinks with the distance, ten times over ten times closer; a
            // step's stays.
            if near > 0.25 && apart(0.2) > 0.5 * near && near > worst.0 {
                worst = (near, d);
            }
        }
        assert!(worst.0 == 0.0, "a step of {} m at {}", worst.0, worst.1);
    }

    #[test]
    fn flat_land_in_a_range_has_no_gullies() {
        // Gullies follow a slope; where there is none, as on the valleys' floors, nothing cuts.
        let range = range();
        for d in directions(20_000) {
            let point = d * RADIUS;
            if range.inside(point) < 1.0 {
                continue;
            }
            // Level over a gully set's reach every way round, measured another way than the
            // gullies measure it: across the ground rather than along the world's axes.
            let (a, b) = (
                d.any_orthonormal_vector(),
                d.any_orthonormal_vector().cross(d),
            );
            let r = range.ridged(point);
            let reach = SLOPE_STEP_M * 2.0;
            let level = [a, -a, b, -b, a + b, a - b, -a + b, -a - b]
                .iter()
                .all(|&way| {
                    (range.ridged(point + way.normalize() * reach) - r).abs() < 0.005 * reach
                });
            if level {
                assert!((range.rise(d, RADIUS).height_m - r).abs() < 1e-3);
            }
        }
    }
}
