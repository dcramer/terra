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

use glam::{IVec3, Vec3};
use terra_gen::{Noise, Seed, layer_shift};

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
    /// How far ice cut its upper slopes into walls, 0 to 1: 0 leaves a slope's climb as its
    /// concavity bends it; 1 stands the upper part of every slope forward as a wall rising
    /// steeply from above its valley's floor and easing to the crest, as a cirque's headwall
    /// and a trough's sides stand over their floors and lakes.
    pub walls: f32,
    /// How much deeper its gullies are cut where they run down those walls, in metres: the
    /// couloirs that split a wall into ribs and feed the scree fans under it.
    pub couloirs_m: f32,
    /// How far a range's front, where it rises from the plain, runs out in spurs and is cut
    /// back by the valleys between them, 0 to 1: 0 a rampart rising evenly round the range
    /// where the broad noise puts its edge, a curtain across any view of it; 1 spurs some
    /// kilometres apart standing out from it as foothills.
    pub spurs: f32,
    /// How much its front's crest rises to peaks and falls to cols between them, as a share of
    /// its height, kilometres apart.
    pub peaks: f32,
    /// How many of a set's gullies one cell of their stripes spans, across and down the fall,
    /// so how far each runs on down a wall before the next cell's take over: at 1 the cells'
    /// stripes, blended at random phases, broke every gully off within its own width, an
    /// egg-crate of even round hollows over a wall rather than gullies running long (D395).
    pub gully_stretch: f32,
    /// How deep its crests are cut into cols and notches between the summits left standing, in
    /// metres, about a kilometre apart, up its walls only (D395).
    pub summits_m: f32,
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
        walls: 0.5,
        couloirs_m: 40.0,
        spurs: 0.0,
        peaks: 0.0,
        gully_stretch: 2.5,
        summits_m: 380.0,
    };

    /// The most the mountains raise the land, in metres.
    pub fn highest_m(&self) -> f32 {
        self.base_m
            + self.relief_m * libm::powf(HIGHEST_CREST, self.concavity) * (1.0 + self.peaks)
            + self.gullies_m
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
    /// How deeply one wall's couloirs are cut against the next's.
    faces: Noise,
    /// Spots the front's rules leave as they were (`Range::with_kept`): directions from the
    /// centre.
    kept: Vec<Vec3>,
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
/// How fast the tops climb past the crests' share, as a share of the rise below it; and the
/// most the ridged sum reaches, 1, in those terms.
const ABOVE_CREST: f32 = 0.45;
/// The walls rise from this share of the way from valley to crest, standing full from that:
/// above the valleys' floors and the lakes in them, which stay as the ridges leave them.
const WALLS_FROM: f32 = 0.5;
const WALLS_FULL: f32 = 0.8;
/// Couloirs are cut from this fall of the ground (a share of a metre a metre: about 31 degrees),
/// full from that one (45 degrees).
const COULOIRS_FROM: f32 = 0.35;
const COULOIRS_FULL: f32 = 0.8;
/// How a wall bends its slope's climb: what is left of the climb's share to the crest raised
/// to this power, so the wall rises steeply from its foot and eases toward the crest.
const WALL_POWER: f32 = 2.5;
const HIGHEST_CREST: f32 = 1.0 + ((1.0 - VALLEY) / (CREST - VALLEY) - 1.0) * ABOVE_CREST;
/// How far under the valleys' share of the ridged sum, as a share of the valleys' to the
/// crests', the floors are hollowed deepest.
const HOLLOWS: f32 = 0.45;
/// The least a couloir is cut, as a share of the deepest, and how far a wall's couloirs are cut
/// deeper or shallower than the theme's by its face, over faces this far across.
const COULOIR_LEAST: f32 = 0.2;
const COULOIR_FACE: f32 = 0.6;
const COULOIR_FACES_M: f32 = 2200.0;
/// A range's front: from out of the range to where its floor has risen this far (shares of
/// `inside`), fading out between the two.
const FRONT: (f32, f32) = (0.3, 0.6);
/// How far apart its spurs are about, in metres, and how far they shift the range noise at
/// `spurs` 1 (its edge's band is 0.32 of it), about the ridged noise's middle.
const SPURS_M: f32 = 2800.0;
const SPUR_SHIFT: f32 = 0.12;
const SPUR_MIDDLE: f32 = 0.45;
/// How far apart the front's peaks are about, in metres.
const PEAKS_M: f32 = 3200.0;
/// How far apart the crests' summits are about, in metres, and from how far up the walls
/// (shares of the way from valley to crest) they rise, whole from the second.
const SUMMITS_M: f32 = 1400.0;
const SUMMITS_FROM: (f32, f32) = (0.6, 1.0);
/// Round a kept spot the front's rules are off within the first distance, in metres, and
/// whole from the second.
const KEPT_M: (f32, f32) = (700.0, 1800.0);
/// The same for the crests' cols, which change the skyline over a place kilometres round it:
/// cut within 3 km of the lake, they reshaped the domes over it.
const SUMMITS_KEPT_M: (f32, f32) = (2500.0, 4000.0);
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
            faces: Noise::new(seed.derive("faces")),
            kept: Vec::new(),
        }
    }

    /// The same range with its front's rules (`Mountains::spurs`, `peaks`) faded out round
    /// these directions from the centre: the places demos are pinned to (D393), which the
    /// rules, reshaping the ranges' fronts kilometres across, would otherwise take with them.
    pub fn with_kept(mut self, spots: impl IntoIterator<Item = Vec3>) -> Self {
        self.kept = spots.into_iter().map(|s| s.normalize()).collect();
        self
    }

    /// How much of the newer rules a point takes, 0 within `within`'s first distance of a kept
    /// spot, in metres, to 1 past its second.
    fn away_from_kept(&self, point: Vec3, within: (f32, f32)) -> f32 {
        let length = point.length();
        let direction = point / length.max(1e-3);
        self.kept.iter().fold(1.0, |away: f32, spot| {
            away.min(smoothstep(
                within.0,
                within.1,
                (direction - *spot).length() * length,
            ))
        })
    }

    /// How much of a range a point is in, 0 out of it to 1 in its heart.
    pub fn inside(&self, point: Vec3) -> f32 {
        self.inside_and_front(point).0
    }

    /// [`Range::inside`], and how far a point is in the range's front (`Mountains::spurs`): 1
    /// out of the range and up its outer slopes, 0 from where its floor has risen. Over the
    /// front the range's edge is shifted out along a ridged noise's lines, as spurs, and in
    /// between them, as the valleys draining the range, so its foot is a ragged line of spurs
    /// and valley mouths rather than a rampart following the broad noise round.
    fn inside_and_front(&self, point: Vec3) -> (f32, f32) {
        let m = &self.mountains;
        let n = self.ranges.fbm(point / m.range_m, 3, 2.0, 0.5);
        let threshold = RANGE_SD * upper_quantile(m.share);
        let edge = |n: f32| smoothstep(threshold - 0.16, threshold + 0.16, n);
        let plain = edge(n);
        let mut front = 1.0 - smoothstep(FRONT.0, FRONT.1, plain);
        if front > 0.0 && (m.spurs > 0.0 || m.peaks > 0.0) && !self.kept.is_empty() {
            front *= self.away_from_kept(point, KEPT_M);
        }
        if m.spurs <= 0.0 || front <= 0.0 || n < threshold - 0.16 - SPUR_SHIFT {
            return (plain, front);
        }
        let ridge = 1.0 - self.faces.sample(point / SPURS_M).abs();
        let shift = m.spurs * SPUR_SHIFT * front * (ridge * ridge - SPUR_MIDDLE);
        (edge(n + shift), front)
    }

    /// How much the ridges raise a point, before gullies, in metres.
    fn ridged(&self, point: Vec3) -> f32 {
        self.ridged_and_upper(point).0
    }

    /// [`Range::ridged`], and how far up into the walls a point is, 0 below them to 1.
    fn ridged_and_upper(&self, point: Vec3) -> (f32, f32) {
        let m = &self.mountains;
        let (inside, front) = self.inside_and_front(point);
        if inside <= 0.0 {
            return (0.0, 0.0);
        }
        let ridged = self.ridges(point / m.ridges_m);
        // The range's floor rises gently with how far into it a point is; its ridges stand on
        // it, the lowest share of the sum its valleys, the rest rising faster toward the crests.
        let along = (ridged - VALLEY) / (CREST - VALLEY);
        // Past the crests' share the ridged sum still rises toward its ridge lines, so the tops
        // keep climbing, more slowly, to arêtes and horns rather than levelling into a table.
        let crest = if along > 1.0 {
            1.0 + (along - 1.0) * ABOVE_CREST
        } else {
            along.max(0.0)
        };
        let mut climb = libm::powf(crest, m.concavity);
        // The upper slopes stood forward as walls: the climb's share below the crests bent
        // convex, steep from the wall's foot and easing to the crest; the valleys below the
        // walls' foot as they were.
        let upper = smoothstep(WALLS_FROM, WALLS_FULL, along);
        if m.walls > 0.0 && upper > 0.0 {
            let below = climb.min(1.0);
            let cut = 1.0 - libm::powf(1.0 - below, WALL_POWER) + (climb - below);
            climb += m.walls * upper * (cut - climb);
        }
        // The range's floor rises over its outer part and is level within, where its valleys
        // hold their lakes.
        let floor = m.base_m * smoothstep(0.0, FLOOR_RISES, inside);
        // The floors hollowed where the ridged sum falls lowest, furthest from every ridge, as
        // glaciers overdeepen a cirque's floor and a lake fills it.
        let hollow = m.basins_m * smoothstep(0.0, -HOLLOWS, along) * inside;
        // Along the front, its crest rises to peaks and falls to cols kilometres apart.
        let peaks = if m.peaks > 0.0 && front > 0.0 {
            let p = self
                .faces
                .fbm(point / PEAKS_M + Vec3::splat(2.3), 2, 2.0, 0.5);
            1.0 + m.peaks * front * (smoothstep(-0.3, 0.3, p) * 2.0 - 1.0)
        } else {
            1.0
        };
        // Its crests broken into summits and the cols and notches between them, a kilometre or
        // so apart: the ridged sum alone left every crest one smooth arc, a curtain's top seen
        // from across a valley (D395). Only up the walls, so no valley's floor or lake moves.
        let crested = smoothstep(SUMMITS_FROM.0, SUMMITS_FROM.1, along);
        let summits = if m.summits_m > 0.0 && crested > 0.0 {
            let s = self
                .faces
                .fbm(point / SUMMITS_M + Vec3::splat(5.7), 3, 2.1, 0.5);
            -m.summits_m * crested * (1.0 - smoothstep(-0.35, 0.35, s))
        } else {
            0.0
        };
        let summits = if summits != 0.0 && !self.kept.is_empty() {
            summits * self.away_from_kept(point, SUMMITS_KEPT_M)
        } else {
            summits
        };
        (
            floor + m.relief_m * libm::powf(inside, 0.6) * climb * peaks - hollow
                + summits * libm::powf(inside, 0.6),
            upper,
        )
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
        // The warp's three noises a few hundred cells apart, shifted by whole cells apart from
        // the point (`Noise::sample_shifted`): added to it, they held it only to 0.6 m, and the
        // warp stepped the ridges by up to 0.15 m at every one.
        let warp = Vec3::new(
            self.ridges.sample_shifted(q * 0.5, IVec3::splat(101)),
            self.ridges.sample_shifted(q * 0.5, IVec3::splat(203)),
            self.ridges.sample_shifted(q * 0.5, IVec3::splat(307)),
        );
        let q = q + warp * WARP;
        let (mut sum, mut amplitude, mut norm, mut frequency) = (0.0, 1.0, 0.0, 1.0);
        let mut weight = 1.0;
        let mut slope = Vec3::ZERO;
        for octave in 0..OCTAVES {
            let (part, whole) = layer_shift(octave);
            let (n, g) = self
                .ridges
                .sample_with_gradient_shifted(q * frequency + part, whole);
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
        let (ridged, upper) = self.ridged_and_upper(point);
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
            // How deeply this wall's couloirs are cut, by a noise some kilometres across.
            let face = self.faces.fbm(point / COULOIR_FACES_M, 2, 2.0, 0.5);
            let walls_cut = 1.0 + COULOIR_FACE * (smoothstep(-0.35, 0.35, face) * 2.0 - 1.0);
            // Full depth from a gentle slope up, so the depth does not follow every change of
            // the fall near a crest.
            let fade = smoothstep(0.02, 0.4, steepness);
            let away = self.away_from_kept(point, KEPT_M);
            for (k, (scale, depth)) in GULLY_SETS.iter().enumerate() {
                let spacing = m.gully_spacing_m * scale;
                let (stripes, across, along, cut) =
                    self.stripes(point, up, slope, spacing, k as i32, away);
                // Down the walls, couloirs: the same lines cut the deeper, only where the wall
                // falls steeply enough that a line cut so deep still runs downhill all the way,
                // not into pits that would fill with water.
                let couloir =
                    m.couloirs_m * upper * smoothstep(COULOIRS_FROM, COULOIRS_FULL, steepness);
                // Each couloir cut as deep as its own cell's point says and as its wall's face
                // does: cut all alike, a wall's couloirs ribbed it like corduroy, a regular
                // lattice of chevrons from across the valley.
                let depth = (m.gullies_m + couloir * cut * walls_cut) * depth * fade;
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

    /// [`Range::stripes_by`] each cell spanning `Mountains::gully_stretch` gullies, so they run
    /// long down the fall; round a kept spot (`Range::with_kept`) blended to a gully a cell, as
    /// the ground was when its demo was pinned there, `away` 0 within the spot's first
    /// distance.
    fn stripes(
        &self,
        point: Vec3,
        up: Vec3,
        slope: Vec3,
        spacing: f32,
        set: i32,
        away: f32,
    ) -> (f32, Vec3, f32, f32) {
        let stretch = self.mountains.gully_stretch;
        let long = self.stripes_by(point, up, slope, spacing, set, stretch);
        if away >= 1.0 || stretch == 1.0 {
            return long;
        }
        let short = self.stripes_by(point, up, slope, spacing, set, 1.0);
        let mix = |a: f32, b: f32| a + (b - a) * away;
        (
            mix(short.0, long.0),
            long.1,
            mix(short.2, long.2),
            mix(short.3, long.3),
        )
    }

    /// Stripes along the downhill direction round a point, laid cell by cell: each cell of
    /// `stretch` times `spacing` has a point of its own, and its stripes, `spacing` apart, run
    /// through it across the slope, so they line up with the ground's fall wherever it turns,
    /// each running on down the fall for about a cell before the next cell's take over. Gives
    /// -1 along a gully's line to
    /// 1 on the rib between, the slope across the stripes it falls toward, and how fast the
    /// stripes change across, a metre; and how deeply the gullies there are cut, a share of the
    /// deepest, by their cells.
    fn stripes_by(
        &self,
        point: Vec3,
        up: Vec3,
        slope: Vec3,
        spacing: f32,
        set: i32,
        stretch: f32,
    ) -> (f32, Vec3, f32, f32) {
        let fall = slope.normalize_or(up.any_orthonormal_vector());
        let across = up.cross(fall).normalize_or(up.any_orthonormal_vector());
        let cell = point / (spacing * stretch);
        let base = cell.floor();
        let (mut weights, mut cosine, mut sine, mut cut) = (0.0, 0.0, 0.0, 0.0);
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
                    let phase = d.dot(across) * core::f32::consts::TAU * stretch;
                    weights += w;
                    // How deep this cell's gully is cut, from its point: a few couloirs deep,
                    // most shallower.
                    let deep = jitter.x * 7.31 + jitter.y * 3.17 + jitter.z * 1.93;
                    let deep = deep - libm::floorf(deep);
                    cut += w * (COULOIR_LEAST + (1.0 - COULOIR_LEAST) * deep * deep);
                    cosine += w * libm::cosf(phase);
                    sine += w * libm::sinf(phase);
                }
            }
        }
        // Where no cell's point is near, the stripes fade to a rib, not a jump.
        let value = (cosine + FADE_TO_RIB) / (weights + FADE_TO_RIB);
        let change = -(sine / (weights + FADE_TO_RIB)) * core::f32::consts::TAU / spacing;
        let cut = (cut + FADE_TO_RIB * COULOIR_LEAST) / (weights + FADE_TO_RIB);
        (value, across, change, cut)
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
            // A crease's rise shrinks with the distance, ten times over ten times closer, and
            // grows as much ten times farther; a step's stays. Both ways, since closer than a
            // centimetre a direction on a planet this size is near a single's precision (2 mm).
            let step = apart(0.2) > 0.5 * near || apart(20.0) < 2.5 * near;
            if near > 0.25 && step && near > worst.0 {
                worst = (near, d);
            }
        }
        assert!(worst.0 == 0.0, "a step of {} m at {}", worst.0, worst.1);
    }

    #[test]
    fn walls_raise_only_the_upper_slopes_and_couloirs_cut_only_down_the_walls() {
        let m = Mountains::START;
        assert!(m.walls > 0.0 && m.couloirs_m > 0.0);
        let plain = Range::new(
            Mountains {
                walls: 0.0,
                couloirs_m: 0.0,
                ..m
            },
            Seed(1),
        );
        let uncut = Range::new(
            Mountains {
                couloirs_m: 0.0,
                ..m
            },
            Seed(1),
        );
        let walled = range();
        let (mut raised, mut deeper) = (0, 0);
        for d in directions(20_000) {
            let point = d * RADIUS;
            let (low, _) = plain.ridged_and_upper(point);
            let (high, upper) = walled.ridged_and_upper(point);
            let carved = |range: &Range, ridged: f32| ridged - range.rise(d, RADIUS).height_m;
            if upper == 0.0 {
                // The valleys, their floors and lakes, as the ridges leave them.
                assert_eq!(low, high);
                assert!((carved(&walled, high) - carved(&uncut, high)).abs() < 1e-3);
                continue;
            }
            // A wall stands its slope's upper part forward, never lower than it was.
            assert!(high >= low - 1e-3, "{high} under {low}");
            if high > low + 50.0 {
                raised += 1;
            }
            if carved(&walled, high) > carved(&uncut, high) + 10.0 {
                deeper += 1;
            }
        }
        assert!(raised > 100, "{raised} raised");
        assert!(deeper > 100, "{deeper} cut deeper");
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

    #[test]
    fn the_ground_is_smooth_within_a_voxel() {
        // How far the rise over 2 cm departs from a tenth of the rise over 20 cm: none on a
        // smooth slope. A single holds a point on a planet this size to about 2 mm, and the
        // gullies' depths follow the slope measured over 40 m, so each point's height is rough
        // by a little; rough by half a voxel, the mesh would show it. The ridges' warp, its
        // noises shifted hundreds of cells by adding to the point, stepped by up to 0.15 m.
        let range = range();
        let mut rough = Vec::new();
        for d in directions(100_000) {
            if range.inside(d * RADIUS) <= 0.0 {
                continue;
            }
            let rise = |cm: f32| {
                let step = d.any_orthonormal_vector() * (cm * 0.005 / RADIUS);
                range.rise((d + step).normalize(), RADIUS).height_m
                    - range.rise((d - step).normalize(), RADIUS).height_m
            };
            rough.push((rise(2.0) - rise(20.0) / 10.0).abs());
        }
        rough.sort_by(f32::total_cmp);
        let share = |f: f32| rough[((rough.len() - 1) as f32 * f) as usize];
        assert!(share(0.99) < 0.05, "1% rough by {} m", share(0.99));
        assert!(share(0.999) < 0.1, "0.1% rough by {} m", share(0.999));
    }
}
