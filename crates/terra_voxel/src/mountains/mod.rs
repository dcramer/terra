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
use terra_gen::smoothstep;
use terra_gen::{Noise, Seed, layer_shift};

mod faces;
mod horns;
pub use faces::Faces;
use faces::RIB_FADE;

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
    /// How far apart a range's separate tops stand, about, in metres, and how deeply the ground
    /// between them is lowered, as a share of the height the ridges raise it over their floor:
    /// 0 leaves every crest standing whole, one wall along a range's front; toward 1 the range
    /// breaks into summits of different heights with saddles between, lower ones in front of
    /// higher ones behind, as the Tatras' and the Dolomites' do seen from a valley.
    pub tops_m: f32,
    pub tops: f32,
    /// How far the ribs between a wall's couloirs are shaped by its face, 0 to 1: 0 every wall
    /// cut alike, round gullies and ribs of one width, combed like a draped sheet; toward 1 on
    /// some faces the gullies narrow to slots between broad buttresses standing out, and on
    /// others they widen between sharp arêtes, faces a kilometre or two across.
    pub buttresses: f32,
    /// How far the coarsest gullies' spacing differs by face, 0 to 1: 0 every wall combed at
    /// `gully_spacing_m`, one width from 3 to 6 km off however the face runs; toward 1 some faces,
    /// a kilometre or so across, cut by couloirs `WIDE_GULLIES` times as far apart and deeper.
    /// Two spacings blended by a face noise, not one scaled smoothly: a spacing that changes
    /// across a face sweeps the stripes' phase, kilometres from the centre, into a moiré.
    pub wide_gullies: f32,
    /// How far some faces are smooth slabs, 0 to 1: their coarser gullies cut to a tenth and a
    /// half, so a wall shows a broad unbroken sheet of rock between fluted ones.
    pub slabs: f32,
    /// How far each gully is cut to its own depth, 0 to 1: 0 all of a cell's alike; toward 1
    /// most of them shallow, down to `GULLY_LEAST` of the deepest, and a few deep.
    pub gully_depths: f32,
    /// How high a talus apron piles against a wall's foot, in metres, its gullies running out
    /// into cones down it: 0 a wall dropping bare to its valley's floor.
    pub aprons_m: f32,
    /// How high its horns stand over the crests they rise from, at most, in metres, and how far
    /// apart, about: pyramidal peaks where three or four cirques have eaten back into one top,
    /// their headwalls steep under the point and easing to the cirques' floors, sharp arêtes
    /// between them (`Range::horns`). 0 leaves the crests as the ridges round them, domes.
    pub horns_m: f32,
    pub horns_across_m: f32,
    /// The kinds of rock face its walls stand in (D182, [`Faces`]), with how often each is
    /// taken: each range takes one, its neighbours often another, blended over a range's
    /// edge, never in its middle. Empty, a wall is as the slope leaves it.
    pub faces: &'static [Faces],
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
        tops_m: 1800.0,
        tops: 0.75,
        buttresses: 1.0,
        wide_gullies: 1.0,
        slabs: 1.0,
        gully_depths: 1.0,
        aprons_m: 80.0,
        horns_m: 950.0,
        horns_across_m: 1900.0,
        faces: &[Faces::HORNS, Faces::SAW],
    };

    /// The most the mountains raise the land, in metres.
    pub fn highest_m(&self) -> f32 {
        self.base_m
            + self.relief_m * libm::powf(HIGHEST_CREST, self.concavity) * (1.0 + self.peaks)
            + self.gullies_m
            + self.horns_m
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
    /// How far it lies on a wall its faces step (faces.rs), 0 to 1, by the wall's slope before
    /// its beds were cut: so a ledge between two risers is the wall's rock, not a level band
    /// of turf and snow ruling the range (the r5 review's "contour lines").
    pub wall: f32,
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
    /// Where a range's separate tops stand (`Mountains::tops`).
    tops: Noise,
    /// Which kind of face a range takes, and its beds' warp and dip (`Mountains::faces`).
    strata: Noise,
    /// Spots the front's rules leave as they were (`Range::with_kept`): directions from the
    /// centre.
    kept: Vec<Vec3>,
    /// Those of them standing in a range (`Range::keeping_ranges_round`).
    kept_in_ranges: Vec<Vec3>,
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
/// Round a kept spot the tops' saddles are off within the first distance, in metres, and
/// whole from the second.
const TOPS_KEPT_M: (f32, f32) = (700.0, 1800.0);
/// Round a kept spot in a range, the same for the mountains round it.
const TOPS_KEPT_RANGE_M: (f32, f32) = (3500.0, 6000.0);
/// Over a range's front, from out of it to where its floor has risen (shares of `inside`), its
/// floor's rise is lowered between the tops too, fading out between the two.
const TOPS_FLOOR: (f32, f32) = (0.6, 1.0);
/// Octaves of the tops' ridged sum, and the share of it below which the ground lies in a
/// saddle, lowered whole.
const TOPS_OCTAVES: usize = 3;
const TOPS_SADDLE: (f32, f32) = (0.2, 0.9);
/// The share of `Mountains::tops` the ground between the tops is lowered by in a range's heart,
/// and how far past the range noise's edge its heart begins (`Range::deep_inside`).
const TOPS_HEART: f32 = 0.35;
const TOPS_DEEP: f32 = 0.55;
/// How high a range's front stands at its edge, as a share of what the ridges raise there,
/// rising to the whole by its heart (`Range::deep_inside`): foothills in front of the peaks.
const FOOTHILLS: f32 = 0.6;
/// How far across a face that shapes its gullies one way is, about, in metres, and the powers
/// of two its shaping runs between (`Mountains::buttresses`): from gullies twice as wide as
/// they were, between arêtes, to slots between buttresses three times as broad.
const BUTTRESS_FACES_M: f32 = 1600.0;
const BUTTRESS_POWERS: (f32, f32) = (-1.0, 1.6);
/// The wide couloirs' spacing and depth, as shares of the coarsest set's
/// (`Mountains::wide_gullies`), and the middle set's depth between them; how far across a face
/// that takes them is, in metres, and the
/// face noise's span over which a face goes from one spacing to the other: narrow, as two
/// spacings mixed half and half beat into a lattice.
const WIDE_GULLIES: (f32, f32, f32) = (2.2, 1.5, 0.6);
const WIDE_FACES_M: f32 = 1300.0;
const WIDE_SPAN: (f32, f32) = (-0.12, 0.12);
/// How far across a slab's face is, in metres, where on the face noise it begins and is whole
/// (`Mountains::slabs`), and how much of each of the two coarser sets' depth a slab keeps.
const SLAB_FACES_M: f32 = 1900.0;
const SLAB_SPAN: (f32, f32) = (0.05, 0.13);
const SLAB_KEEPS: [f32; 2] = [0.1, 0.5];
/// The shallowest a gully is cut (`Mountains::gully_depths`), as a share of the deepest, and
/// the power that skews the depths between toward it, so a few stand out deep.
const GULLY_LEAST: f32 = 0.25;
const GULLY_SKEW: f32 = 1.3;
/// Where a talus apron lies, as shares of the climb from the valley to the crest (`along`): from
/// the first, highest at the second, the wall's foot, gone by the third; how much of the
/// gullies' depth it fills, and how deep, as a share of its height, the ground between its
/// cones is lowered.
const APRON: (f32, f32, f32) = (0.28, 0.5, 0.64);
const APRON_FILL: f32 = 0.8;
const APRON_CONES: f32 = 0.5;
/// How much the stripes lean toward a rib, as if a cell's point there weighed so much.
/// Horns (`Range::horns`): from how far up the ridges' crests they rise; how far a horn's
/// faces reach, as a share of the lattice; how far a site strays in its cell; the share of
/// sites that stand; how deep a cirque's bowl is hollowed across its face; the headwall's
/// concave profile; how deep, as a share of `horns_m`, the cirques between the horns are cut
/// into the massif.
const HORNS_FROM: (f32, f32) = (0.42, 0.8);
const HORN_REACH: f32 = 0.44;
const HORN_JITTER: f32 = 0.6;
const HORNS_KEPT: f32 = 0.9;
const CIRQUES: f32 = 0.35;
const HORN_BOWL: f32 = 0.45;
const HORN_PROFILE: f32 = 1.3;

/// The slope, as a rise over the run, over which a stepped wall is rock ledges and all
/// (`Rise::wall`): about 33 to 37 degrees.
const WALL_ROCK: (f32, f32) = (0.65, 0.75);

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
            tops: Noise::new(seed.derive("tops")),
            strata: Noise::new(seed.derive("strata")),
            kept: Vec::new(),
            kept_in_ranges: Vec::new(),
        }
    }

    /// The same range with its front's rules (`Mountains::spurs`, `peaks`) faded out round
    /// these directions from the centre: the places demos are pinned to (D393), which the
    /// rules, reshaping the ranges' fronts kilometres across, would otherwise take with them.
    pub fn with_kept(mut self, spots: impl IntoIterator<Item = Vec3>) -> Self {
        self.kept = spots.into_iter().map(|s| s.normalize()).collect();
        self
    }

    /// The kept spots that stand in a range, on a planet of so many metres' radius, keep the
    /// mountains round them as well as their own ground: a lake's or a fell's peaks are part
    /// of the place, as a valley town's far range is not (`Mountains::tops`).
    pub fn keeping_ranges_round(mut self, radius_m: f32) -> Self {
        self.kept_in_ranges = self
            .kept
            .iter()
            .copied()
            .filter(|spot| self.inside(*spot * radius_m) > 0.5)
            .collect();
        self
    }

    /// How much of the tops' rule a point takes: [`Range::away_from_kept`] within
    /// `TOPS_KEPT_M` of every kept spot, and within `TOPS_KEPT_RANGE_M` of one in a range.
    fn away_for_tops(&self, point: Vec3) -> f32 {
        let length = point.length();
        let direction = point / length.max(1e-3);
        let near = self.kept_in_ranges.iter().fold(1.0, |away: f32, spot| {
            away.min(smoothstep(
                TOPS_KEPT_RANGE_M.0,
                TOPS_KEPT_RANGE_M.1,
                (direction - *spot).length() * length,
            ))
        });
        near.min(self.away_from_kept(point, TOPS_KEPT_M))
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
    /// How deep into a range a point is, 0 at its edge to 1 some kilometres in: as
    /// [`Range::inside`], over a band of the range noise several times wider, since a range's
    /// whole rise from the plain, its front, lies within a couple of kilometres of `inside`'s.
    fn deep_inside(&self, point: Vec3) -> f32 {
        let m = &self.mountains;
        let n = self.ranges.fbm(point / m.range_m, 3, 2.0, 0.5);
        let threshold = RANGE_SD * upper_quantile(m.share);
        smoothstep(threshold - 0.16, threshold + TOPS_DEEP, n)
    }

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

    /// How much the ridges raise a point, before gullies, without the horns' buttress ribs
    /// (`horns::ribbed`): the slope the gullies and the faces follow, so the faces' ledges are
    /// not stepped by a slope turning across every rib.
    fn unribbed(&self, point: Vec3) -> f32 {
        self.ridged_upper_and_foot(point).0
    }

    /// How much the ridges raise a point, before gullies, in metres.
    #[cfg(test)]
    fn ridged(&self, point: Vec3) -> f32 {
        self.ridged_and_upper(point).0
    }

    /// [`Range::ridged`], and how far up into the walls a point is, 0 below them to 1.
    #[cfg(test)]
    fn ridged_and_upper(&self, point: Vec3) -> (f32, f32) {
        let (ridged, upper, _, ribs) = self.ridged_upper_and_foot(point);
        (ridged + ribs, upper)
    }

    /// [`Range::ridged_and_upper`], and how far a point lies in the band at and below a wall's
    /// foot that its talus apron covers (`Mountains::aprons_m`), 0 to 1.
    fn ridged_upper_and_foot(&self, point: Vec3) -> (f32, f32, f32, f32) {
        let m = &self.mountains;
        let (inside, front) = self.inside_and_front(point);
        if inside <= 0.0 {
            return (0.0, 0.0, 0.0, 0.0);
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
        // Under the walls, a talus apron: the band at and below a wall's foot raised, as the
        // rock that falls from a wall piles against its foot, and the gullies that run out of
        // the wall into it (`Range::rise`) leave cones down their lines. The walls dropped to
        // the valley's floor bare, with almost none of the 20 to 33 degree ground a talus
        // makes (the r3 review). Off round the kept spots and their ranges.
        let foot = if m.aprons_m > 0.0 {
            let kept = if self.kept.is_empty() {
                1.0
            } else {
                self.away_for_tops(point)
            };
            kept * smoothstep(APRON.0, APRON.1, along) * (1.0 - smoothstep(APRON.1, APRON.2, along))
        } else {
            0.0
        };
        climb += m.aprons_m / m.relief_m * foot;
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
        let raised = m.relief_m * libm::powf(inside, 0.6) * climb * peaks;
        let height = floor + raised - hollow + summits * libm::powf(inside, 0.6);
        // The range broken into separate tops of different heights, the ground between them
        // lowered, and its outer kilometres lower as a whole, foothills rising in ranks to its
        // heart: the ridges' rise over their floor, and over those outer kilometres the floor's
        // own rise too, the pedestal a range stood on as one wall round it; the floor within,
        // where its valleys hold their lakes, stays.
        let lowered = if m.tops > 0.0 {
            let away = if self.kept.is_empty() {
                1.0
            } else {
                self.away_for_tops(point)
            };
            let deep = if away > 0.0 {
                self.deep_inside(point)
            } else {
                1.0
            };
            let front_floor = floor * (1.0 - smoothstep(TOPS_FLOOR.0, TOPS_FLOOR.1, deep));
            let standing = (raised + summits * libm::powf(inside, 0.6)).max(0.0) + front_floor;
            if away > 0.0 && standing > 0.0 {
                // Deepest over the outer kilometres, the front seen from the plain, and
                // shallower in the heart, whose peaks and cols the ridges already make.
                let depth = m.tops * (TOPS_HEART + (1.0 - TOPS_HEART) * (1.0 - deep));
                let ranks = FOOTHILLS + (1.0 - FOOTHILLS) * smoothstep(0.0, 1.0, deep);
                let kept = ranks * (1.0 - depth * (1.0 - self.tops_at(point)));
                away * (1.0 - kept) * standing
            } else {
                0.0
            }
        } else {
            0.0
        };
        let (horns, ribs) = if m.horns_m > 0.0 {
            self.horns(point, along, inside)
        } else {
            (0.0, 0.0)
        };
        (height - lowered + horns, upper, foot, ribs)
    }

    /// How much a point stands on one of a range's separate tops (`Mountains::tops`), 0
    /// between them to 1 at the highest: a ridged sum of three octaves, as the ridges' own
    /// (Musgrave), at `tops_m`, so the tops are crests with spurs branching off them, rising
    /// to summits and falling to saddles along them, rather than round domes.
    fn tops_at(&self, point: Vec3) -> f32 {
        let q = point / self.mountains.tops_m;
        let (mut sum, mut amplitude, mut norm, mut frequency) = (0.0, 1.0, 0.0, 1.0);
        let mut weight = 1.0;
        for octave in 0..TOPS_OCTAVES {
            let (part, whole) = layer_shift(octave);
            let n = self.tops.sample_shifted(q * frequency + part, whole);
            let r = 1.0 - n.abs();
            let ridge = r * r * weight;
            sum += amplitude * ridge;
            norm += amplitude;
            weight = (ridge * BRANCHING).clamp(0.0, 1.0);
            amplitude *= GAIN;
            frequency *= 2.0;
        }
        ((sum / norm - TOPS_SADDLE.0) / (TOPS_SADDLE.1 - TOPS_SADDLE.0)).clamp(0.0, 1.0)
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
        let (ridged, upper, foot, ribs) = self.ridged_upper_and_foot(point);
        if self.inside(point) <= 0.0 {
            return Rise::default();
        }
        let m = &self.mountains;
        if m.gullies_m <= 0.0 {
            return Rise {
                height_m: ridged + ribs,
                ..Rise::default()
            };
        }
        // How the ridges rise round the point, over a few metres along each of the world's
        // axes, which unlike any pair of directions across the ground never turn over, then
        // laid level.
        // Centred, so at a summit or in a hollow, where the ground has no way down, it is
        // level and no gully is cut.
        let across = |axis: Vec3| {
            self.unribbed(point + axis * SLOPE_STEP_M) - self.unribbed(point - axis * SLOPE_STEP_M)
        };
        let rise =
            Vec3::new(across(Vec3::X), across(Vec3::Y), across(Vec3::Z)) / (2.0 * SLOPE_STEP_M);
        let mut slope = rise - up * rise.dot(up);
        let steepness = slope.length();
        // The gullies, each set following the slope the ones before it left.
        let mut carved = 0.0;
        let mut deepest = 0.0;
        // What the talus aprons' cones take off between them (`Mountains::aprons_m`), apart
        // from the gullies' cuts, whose beds hold snow and gravel.
        let mut piled = 0.0;
        // How far the faces stand here, between the coarsest gullies' lines (`RIB_FADE`).
        let mut rib = 1.0;
        if steepness > 0.02 {
            // How deeply this wall's couloirs are cut, by a noise some kilometres across.
            let face = self.faces.fbm(point / COULOIR_FACES_M, 2, 2.0, 0.5);
            let walls_cut = 1.0 + COULOIR_FACE * (smoothstep(-0.35, 0.35, face) * 2.0 - 1.0);
            // Full depth from a gentle slope up, so the depth does not follow every change of
            // the fall near a crest.
            let fade = smoothstep(0.02, 0.4, steepness);
            let away = self.away_from_kept(point, KEPT_M);
            let (wide, slab, vary, shaped) = self.face_shaping(point);
            for (k, (scale, depth)) in GULLY_SETS.iter().enumerate() {
                let spacing = m.gully_spacing_m * scale;
                let (mut stripes, across, mut along, mut cut) =
                    self.stripes(point, up, slope, spacing, k as i32, (away, vary));
                // The coarsest set on a wide face: another set of stripes farther apart, from
                // cells of their own, blended in by the face (`Mountains::wide_gullies`).
                let mut widened = 1.0;
                if k == 0 && wide > 0.0 {
                    let (s, _, a, c) = self.stripes(
                        point,
                        up,
                        slope,
                        spacing * WIDE_GULLIES.0,
                        GULLY_SETS.len() as i32,
                        (away, vary),
                    );
                    stripes += (s - stripes) * wide;
                    along += (a - along) * wide;
                    cut += (c - cut) * wide;
                    widened += (WIDE_GULLIES.1 - 1.0) * wide;
                }
                if k == 0 {
                    let into = ((1.0 - stripes) * 0.5).max(0.0);
                    rib = 1.0 - smoothstep(RIB_FADE.0, RIB_FADE.1, into);
                }
                // The middle set fainter between wide couloirs, so they read as the face's own.
                if k == 1 {
                    widened *= 1.0 + (WIDE_GULLIES.2 - 1.0) * wide;
                }
                if k < 2 {
                    widened *= 1.0 + (SLAB_KEEPS[k] - 1.0) * slab;
                }
                // Down the walls, couloirs: the same lines cut the deeper, only where the wall
                // falls steeply enough that a line cut so deep still runs downhill all the way,
                // not into pits that would fill with water.
                let couloir =
                    m.couloirs_m * upper * smoothstep(COULOIRS_FROM, COULOIRS_FULL, steepness);
                // Each couloir cut as deep as its own cell's point says and as its wall's face
                // does: cut all alike, a wall's couloirs ribbed it like corduroy, a regular
                // lattice of chevrons from across the valley.
                let depth = (m.gullies_m + couloir * cut * walls_cut) * depth * fade * widened;
                // On a talus apron the gullies run out into fans: their cuts filled with what
                // they bring down, and the coarsest set's lines left standing as cones, the
                // ground between them lower.
                let depth = depth * (1.0 - APRON_FILL * foot);
                if k == 0 && foot > 0.0 {
                    let between = ((1.0 + stripes) * 0.5).clamp(0.0, 1.0);
                    piled -= m.aprons_m * APRON_CONES * foot * fade * between;
                }
                // 0 on the ribs between gullies, the whole depth down a gully's line.
                if shaped != 0.0 && k < 2 {
                    let into = ((1.0 - stripes) * 0.5).max(0.0);
                    carved -= depth * libm::powf(into, libm::exp2f(shaped));
                } else {
                    carved += depth * (stripes - 1.0) * 0.5;
                }
                deepest += depth;
                // The next, finer set follows the gully walls this set cut, so they branch;
                // turned by at most half the fall, so the way down never vanishes and turns
                // over, which would make the finer stripes jump.
                let turn = (depth * 0.5 * along).clamp(-0.5, 0.5) * steepness;
                slope += across * turn;
            }
        }
        let height_m = ridged + carved + piled;
        let step = self.stepped(point, height_m, slope, (upper, rib));
        let height_m = height_m + step + ribs;
        Rise {
            height_m,
            wall: if step != 0.0 {
                smoothstep(WALL_ROCK.0, WALL_ROCK.1, steepness)
            } else {
                0.0
            },
            bed: if deepest > 0.0 {
                -carved / deepest
            } else {
                0.0
            },
        }
    }

    /// How a face shapes its gullies, each 0 round the kept spots and their ranges, whose
    /// ground stays as pinned: how far it takes the wide couloirs (`Mountains::wide_gullies`),
    /// how far it is a slab (`Mountains::slabs`), how far each gully is cut to its own depth
    /// (`Mountains::gully_depths`), and the buttresses' power on how far into a gully a point
    /// is (`Mountains::buttresses`): above 1 narrowing the two coarser sets' gullies to slots
    /// between broad buttresses, below 1 widening them between sharp arêtes.
    fn face_shaping(&self, point: Vec3) -> (f32, f32, f32, f32) {
        let m = &self.mountains;
        let faced =
            m.buttresses > 0.0 || m.wide_gullies > 0.0 || m.slabs > 0.0 || m.gully_depths > 0.0;
        let kept = if !faced || self.kept.is_empty() {
            1.0
        } else {
            self.away_for_tops(point)
        };
        if kept <= 0.0 {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let on_face = |share: f32, scale_m: f32, shift: f32, span: (f32, f32)| {
            if share > 0.0 {
                let n = self
                    .faces
                    .fbm(point / scale_m + Vec3::splat(shift), 2, 2.0, 0.5);
                share * kept * smoothstep(span.0, span.1, n)
            } else {
                0.0
            }
        };
        let wide = on_face(m.wide_gullies, WIDE_FACES_M, 13.3, WIDE_SPAN);
        let slab = on_face(m.slabs, SLAB_FACES_M, 17.9, SLAB_SPAN);
        let b = on_face(m.buttresses, BUTTRESS_FACES_M, 9.1, (-0.35, 0.35));
        let shaped = if m.buttresses > 0.0 {
            m.buttresses * kept * BUTTRESS_POWERS.0 + (BUTTRESS_POWERS.1 - BUTTRESS_POWERS.0) * b
        } else {
            0.0
        };
        (wide, slab, m.gully_depths * kept, shaped)
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
        (away, vary): (f32, f32),
    ) -> (f32, Vec3, f32, f32) {
        let stretch = self.mountains.gully_stretch;
        let long = self.stripes_by(point, up, slope, spacing, set, (stretch, vary));
        if away >= 1.0 || stretch == 1.0 {
            return long;
        }
        let short = self.stripes_by(point, up, slope, spacing, set, (1.0, vary));
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
        (stretch, vary): (f32, f32),
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
                    // Each gully between two ribs cut to its own depth, most shallow and a few
                    // deep, by `vary`: cut alike, a cell's gullies were a comb of one width and
                    // depth (the r3 review). Whole at the ribs, so the ground never jumps.
                    let deep = if vary > 0.0 {
                        let gully = libm::floorf(d.dot(across) * stretch) as i32;
                        let h = self.gullies.point_in(
                            c.x as i32 + set * 7919 + gully * 104_729,
                            c.y as i32 + 31,
                            c.z as i32,
                        );
                        let h = libm::powf(h.x, GULLY_SKEW);
                        1.0 + (GULLY_LEAST + (1.0 - GULLY_LEAST) * h - 1.0) * vary
                    } else {
                        1.0
                    };
                    cosine += w * (1.0 - deep * (1.0 - libm::cosf(phase)));
                    sine += w * deep * libm::sinf(phase);
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
mod tests;
