//! The ground as a field: at any point, how far it is from the surface and what it is
//! made of. A generated planet answers from its seed alone; edits are laid over it in
//! [`crate::planet`].

use crate::mountains::{Mountains, Range};
use crate::patterned::Patterned;
use glam::Vec3;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use terra_core::{MaterialId, PlanetSpec};
use terra_gen::Noise;

/// What the ground is made of. Which materials exist is design (D43); these are the few
/// the MVP needs, laid down by rule.
pub mod material {
    use terra_core::MaterialId;

    /// Nothing: above the ground.
    pub const AIR: MaterialId = MaterialId(0);
    /// The top layer where things grow.
    pub const SOIL: MaterialId = MaterialId(1);
    /// What most of the planet is.
    pub const STONE: MaterialId = MaterialId(2);
    /// Veins in the stone, worth mining.
    pub const ORE: MaterialId = MaterialId(3);
    /// The top layer by the sea.
    pub const SAND: MaterialId = MaterialId(4);
    /// Broken rock lying under a cliff, in mountains (D322).
    pub const SCREE: MaterialId = MaterialId(5);
    /// Water-worn stones in a gully's bed and on a gentle shore.
    pub const GRAVEL: MaterialId = MaterialId(6);

    /// Each material, and the name data files know it by.
    pub const NAMED: [(MaterialId, &str); 7] = [
        (AIR, "Air"),
        (SOIL, "Soil"),
        (STONE, "Stone"),
        (ORE, "Ore"),
        (SAND, "Sand"),
        (SCREE, "Scree"),
        (GRAVEL, "Gravel"),
    ];

    /// The material a data file names.
    pub fn named(name: &str) -> Option<MaterialId> {
        NAMED
            .iter()
            .find(|(_, called)| *called == name)
            .map(|(material, _)| *material)
    }
}

/// The ground at one point.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    /// How far the point is from the surface, in metres: positive above it, negative
    /// inside it. An estimate that is accurate near the surface.
    pub distance: f32,
    /// What the ground is made of there. Above the ground, what it would be just below.
    pub material: MaterialId,
}

impl Sample {
    pub fn solid(&self) -> bool {
        self.distance < 0.0
    }
}

/// Anything that can say where the ground is and what it is made of.
///
/// Asking about many nearby points, as meshing a chunk does, shares work between them
/// through a memo: start one, ask with it, and drop it when done.
pub trait Field: Send + Sync {
    /// Work kept between asks about nearby points, such as the heights of the land there.
    type Memo: Default;

    /// A memo for asking about ground seen at a coarseness, voxels so many metres apart,
    /// as a distant chunk is. A field may answer such asks more cheaply and less finely.
    fn memo_for(&self, _spacing_m: f32) -> Self::Memo {
        Self::Memo::default()
    }

    /// How far a point is from the surface: positive above, negative inside.
    fn distance_in(&self, point: Vec3, memo: &mut Self::Memo) -> f32;

    /// How far a point is from the surface, and what the ground is made of there.
    fn sample_in(&self, point: Vec3, memo: &mut Self::Memo) -> Sample;

    /// The ground at one point, asked on its own.
    fn sample(&self, point: Vec3) -> Sample {
        self.sample_in(point, &mut Self::Memo::default())
    }

    /// The tangent of the steepest ground it has, about: how much further than its true
    /// distance from a point the ground can be, measured straight up from the centre, which
    /// is what [`crate::may_hold_surface`] allows for. Hills' is about 48 degrees'.
    fn steepest(&self) -> f32 {
        1.5
    }
}

/// The land at lattice points already worked out, for asks nearby.
pub struct HeightMemo {
    heights: HashMap<(u8, u32, u32), Node, BuildHasherDefault<LatticeHasher>>,
    /// Blend between every so many lattice points, for ground seen coarsely: 1 is the
    /// ground itself.
    step: u32,
    /// The ground round about, as coarsely as looking for a cliff uphill of scree needs it,
    /// kept apart so those looks share their lattice points.
    around: Option<Box<HeightMemo>>,
}

impl HeightMemo {
    /// A memo for asking about ground seen at a coarseness: voxels so many metres apart.
    /// The lattice points used are the ground's own, fewer of them.
    pub fn coarse(spacing_m: f32) -> Self {
        let step = (spacing_m / GeneratedPlanet::LATTICE_M).max(1.0) as u32;
        Self {
            heights: HashMap::default(),
            step: step.next_power_of_two(),
            around: None,
        }
    }

    /// The memo for the ground round about: at least `spacing_m` between its points.
    fn around(&mut self, spacing_m: f32) -> &mut HeightMemo {
        let step = self.step as f32 * GeneratedPlanet::LATTICE_M;
        self.around
            .get_or_insert_with(|| Box::new(HeightMemo::coarse(spacing_m.max(step))))
    }
}

impl Default for HeightMemo {
    fn default() -> Self {
        Self::coarse(GeneratedPlanet::LATTICE_M)
    }
}

/// The land at a lattice point: its height, and shares, 0 to 1, of what lies on it that is not
/// soil: bare rock, scree and gravel, as the mountains lay them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Node {
    height: f32,
    /// How far down into a mountain gully's bed it lies (`Rise::bed`).
    bed: f32,
    /// How far it lies on a wall the mountains' faces step (`Rise::wall`).
    wall: f32,
}

impl Node {
    fn lerp(self, other: Node, t: f32) -> Node {
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Node {
            height: mix(self.height, other.height),
            bed: mix(self.bed, other.bed),
            wall: mix(self.wall, other.wall),
        }
    }
}

/// What lies on mountains (D322): bare rock where the ground is steeper than this, in degrees;
/// scree where it is between these, a talus's angle, and a cliff stands within so many metres
/// uphill, as scree rests at 33 to 37 degrees below the cliff it fell from; gravel in a gully's
/// bed gentler than this. Scree runs up to bare rock's angle: stopped at 38 degrees, a wall's
/// flank between 38 and 40 was soil, which holds none of a wall's fed snow (terra_world
/// `lying`), and showed as dark holes in the horns' snow and a band of forest against the
/// rock (the r21 review). The furthest cliff looked for, 90 m up, widens the talus under a
/// horn's foot, where the forest met the rock in one hard line.
const BARE_ROCK_DEG: f32 = 40.0;
const SCREE_DEG: (f32, f32) = (24.0, BARE_ROCK_DEG);
const CLIFF_WITHIN_M: [f32; 3] = [20.0, 45.0, 90.0];
/// Under a wall the rock it sheds runs out further than its cones, in an apron of talus on
/// gentler ground, where the forest gives out to scattered trees before the wall's foot, as
/// under the Tatras' and the Dolomites' faces: any ground gentler than a talus's where the
/// ground so many metres off, one of four ways round, stands this many metres over it, as
/// steep on average as a talus. Without, the forest ran straight onto the walls (the r13
/// review of D426). The furthest reach widens the apron to a band that reads from the valley,
/// where at 30 to 80 m it was a line (round 15).
const APRON_WITHIN_M: [(f32, f32); 3] = [(100.0, 60.0), (200.0, 115.0), (320.0, 175.0)];
/// How finely the ground uphill is looked at for a cliff, in metres between its points.
const CLIFF_LOOK_M: f32 = 8.0;
const GRAVEL_UNDER_DEG: f32 = 28.0;
/// A bed on ground flatter than this is a dip in a meadow, not a stream's.
const GRAVEL_OVER_DEG: f32 = 5.0;
const GRAVEL_BED: f32 = 0.8;

/// A quick hash for lattice points, which are small integers.
#[derive(Default)]
pub struct LatticeHasher(u64);

impl Hasher for LatticeHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(byte as u64);
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.write_u64(value as u64);
    }

    fn write_u32(&mut self, value: u32) {
        self.write_u64(value as u64);
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

/// A planet generated from its seed: large hills, with finer detail on them, soil over
/// stone, sand by the sea, and veins of ore in the stone.
///
/// The large hills are the stand-in planet's (`terra_core::StandInPlanet`), from the same
/// seed, so the same seed gives the same lie of the land before and after voxels.
///
/// The land's height is worked out exactly on a lattice of points a metre apart over the
/// planet, six square faces bent round it, and between them it is a smooth blend of the
/// four nearest. That is the ground, for every question asked of it, so walking, digging
/// and meshing all agree; and nearby asks share the lattice points, which makes meshing a
/// chunk cheap. The land has no features finer than a few metres, so the blend loses
/// nothing that shows.
#[derive(Clone, Debug)]
pub struct GeneratedPlanet {
    spec: PlanetSpec,
    hills: Noise,
    detail: Noise,
    ore: Noise,
    /// The frozen ground's hummocky moraine (`rolling_m`).
    moraine: Noise,
    relief_m: f32,
    /// How tall the finer detail is.
    detail_m: f32,
    sea_level_m: Option<f32>,
    /// Lattice points along each edge of a face.
    lattice: u32,
    /// Its mountains, on a world with any (D322).
    range: Option<Range>,
    /// Its patterned ground where it is frozen at depth, on a world whose climate is known.
    patterned: Option<Patterned>,
}

impl GeneratedPlanet {
    /// How many large hills fit around the planet, as on the stand-in.
    const FEATURES_AROUND: f32 = 4.0;
    /// The finer detail: how tall, and how far apart its features are.
    const DETAIL_M: f32 = 0.8;
    const DETAIL_SCALE_M: f32 = 24.0;
    /// The frozen ground's moraine (`rolling_m`): low ridges this tall at most and this far
    /// across, and hummocks and kettles on them.
    const MORAINE_RIDGES_M: f32 = 14.0;
    const MORAINE_RIDGE_SCALE_M: f32 = 520.0;
    const HUMMOCKS_M: f32 = 7.0;
    const HUMMOCK_SCALE_M: f32 = 85.0;
    /// How deep the top layer of soil or sand goes.
    pub(crate) const TOPSOIL_M: f32 = 1.5;
    /// Sand instead of soil this close above the sea.
    const BEACH_M: f32 = 1.5;
    /// How big ore veins are, and how much of the stone they fill: a vein where the noise
    /// is above the threshold.
    const ORE_SCALE_M: f32 = 9.0;
    const ORE_THRESHOLD: f32 = 0.45;
    /// How far apart the lattice's points are, on the ground.
    const LATTICE_M: f32 = 1.0;

    pub fn new(spec: PlanetSpec, relief_m: f32, sea_level_m: Option<f32>) -> Self {
        let quarter_round = std::f32::consts::FRAC_PI_2 * spec.radius_m;
        Self {
            spec,
            hills: Noise::new(spec.seed.derive("stand-in hills")),
            detail: Noise::new(spec.seed.derive("ground detail")),
            ore: Noise::new(spec.seed.derive("ore veins")),
            moraine: Noise::new(spec.seed.derive("moraine")),
            relief_m,
            detail_m: Self::DETAIL_M,
            sea_level_m,
            lattice: (quarter_round / Self::LATTICE_M).ceil().max(1.0) as u32,
            range: None,
            patterned: None,
        }
    }

    /// The same planet with mountains (D322).
    pub fn with_mountains(mut self, mountains: Mountains) -> Self {
        self.range = Some(Range::new(mountains, self.spec.seed));
        self
    }

    /// The same planet with its mountains' front rules faded out round these directions from
    /// the centre (`Range::with_kept`): where demos are pinned.
    pub fn with_kept_spots(mut self, spots: impl IntoIterator<Item = Vec3>) -> Self {
        let radius_m = self.spec.radius_m;
        self.range = self
            .range
            .map(|range| range.with_kept(spots).keeping_ranges_round(radius_m));
        self
    }

    /// The same planet with patterned ground where its world's climate keeps it frozen.
    pub fn with_patterned(mut self, patterned: Patterned) -> Self {
        self.patterned = Some(patterned);
        self
    }

    /// Its patterned ground, if it has any.
    pub fn patterned(&self) -> Option<&Patterned> {
        self.patterned.as_ref()
    }

    /// How far down into a gully's bed the ground in a direction lies, 0 on a rib between
    /// gullies or off the mountains to 1 at the bottom of every set of them (`Rise::bed`), as
    /// the ground is seen through `memo`.
    pub fn gully_bed(&self, direction: Vec3, memo: &mut HeightMemo) -> f32 {
        if self.range.is_none() {
            return 0.0;
        }
        self.land_in(direction.normalize(), memo).bed
    }

    /// Its mountains, if it has any.
    pub fn mountains(&self) -> Option<&Range> {
        self.range.as_ref()
    }

    /// The most the ground stands above or below the nominal radius, in metres, before
    /// digging: its hills, its finer detail and its mountains.
    pub fn reach_m(&self) -> f32 {
        let moraine = if self.patterned.is_some() {
            Self::MORAINE_RIDGES_M + Self::HUMMOCKS_M
        } else {
            0.0
        };
        self.relief_m
            + self.detail_m
            + moraine
            + self.range.as_ref().map_or(0.0, |r| r.mountains.highest_m())
    }

    /// The same planet without its finer detail: with no relief, perfectly smooth, as a
    /// test level wants.
    pub fn smooth(mut self) -> Self {
        self.detail_m = 0.0;
        self
    }

    /// The same planet with its finer detail this tall, for gently uneven ground.
    pub fn with_detail_m(mut self, detail_m: f32) -> Self {
        self.detail_m = detail_m;
        self
    }

    pub fn spec(&self) -> PlanetSpec {
        self.spec
    }

    /// Distance from the centre to the sea's surface, if there is a sea.
    pub fn sea_radius_m(&self) -> Option<f32> {
        self.sea_level_m.map(|level| self.spec.radius_m + level)
    }

    /// How high the ground stands above the nominal radius in a direction.
    pub fn height_m(&self, direction: Vec3) -> f32 {
        self.height_in(direction, &mut HeightMemo::default())
    }

    /// The land at a lattice point, worked out from the noise.
    fn exact(&self, direction: Vec3) -> Node {
        let mut node = Node {
            height: self.exact_height(direction),
            ..Node::default()
        };
        if let Some(range) = &self.range {
            let rise = range.rise(direction, self.spec.radius_m);
            node.height += rise.height_m;
            node.bed = rise.bed;
            node.wall = rise.wall;
        }
        if let Some(patterned) = &self.patterned {
            let over_sea = node.height - self.sea_level_m.unwrap_or(0.0);
            let strength = patterned.strength(direction, over_sea);
            node.height += patterned.relief_m(direction * self.spec.radius_m, strength)
                + self.rolling_m(direction, strength);
        }
        node
    }

    /// How the ice's leavings raise and sink the frozen ground, laid this fully: past the trees
    /// the land is what the ice sheets left, hummocky moraine and its kettle hollows a few metres
    /// high and tens of metres across over low ridges some hundreds of metres apart, as the
    /// fells' feet in Lapland and the Barren Lands are (Benn and Evans, *Glaciers and
    /// Glaciation*). Without it the plateau past the trees was the hills' own long swells, flat
    /// as a table at a walker's eye ("too flat", the developers, 2026-10-15). Laid where the
    /// patterned ground is, so the forest, the lake's basin and the town's fields keep their
    /// ground, and fading in with it.
    fn rolling_m(&self, direction: Vec3, strength: f32) -> f32 {
        if strength <= 0.0 {
            return 0.0;
        }
        let at = direction * self.spec.radius_m;
        let ridges = Self::MORAINE_RIDGES_M
            * self
                .moraine
                .fbm(at / Self::MORAINE_RIDGE_SCALE_M, 2, 2.0, 0.5);
        let hummocks = Self::HUMMOCKS_M
            * self
                .moraine
                .fbm(at / Self::HUMMOCK_SCALE_M + Vec3::splat(41.7), 3, 2.0, 0.5);
        (ridges + hummocks) * strength
    }

    /// How the ground rises in a direction, as the memo's lattice has it: which way is
    /// uphill, level there, and how steep, in degrees.
    fn rising_in(&self, direction: Vec3, memo: &mut HeightMemo) -> (Vec3, f32) {
        let step_m = memo.step as f32 * Self::LATTICE_M;
        let here = self.height_in(direction, memo);
        let (a, b) = (
            direction.any_orthonormal_vector(),
            direction.any_orthonormal_vector().cross(direction),
        );
        let across = step_m / self.spec.radius_m;
        let rise = |v: Vec3, memo: &mut HeightMemo| {
            (self.height_in((direction + v * across).normalize(), memo) - here) / step_m
        };
        let gradient = a * rise(a, memo) + b * rise(b, memo);
        (
            gradient.normalize_or_zero(),
            libm::atanf(gradient.length()).to_degrees(),
        )
    }

    /// What lies on the ground in a direction, where it is not soil: bare rock too steep to
    /// hold anything, scree under a cliff, gravel in a gully's bed (D322). Only in mountains.
    fn lying_in(&self, direction: Vec3, land: Node, memo: &mut HeightMemo) -> Option<MaterialId> {
        self.range.as_ref()?;
        let bed = land.bed;
        let (uphill, slope) = self.rising_in(direction, memo);
        // A stepped wall's ledges are its rock, as its risers are (`Rise::wall`).
        if slope > BARE_ROCK_DEG || land.wall > 0.5 {
            return Some(material::STONE);
        }
        if bed > GRAVEL_BED && (GRAVEL_OVER_DEG..GRAVEL_UNDER_DEG).contains(&slope) {
            return Some(material::GRAVEL);
        }
        let radius = self.spec.radius_m;
        if (SCREE_DEG.0..SCREE_DEG.1).contains(&slope) && uphill != Vec3::ZERO {
            let around = memo.around(CLIFF_LOOK_M);
            let cliff = CLIFF_WITHIN_M.iter().any(|&reach| {
                let above = (direction * radius + uphill * reach).normalize();
                self.rising_in(above, around).1 > BARE_ROCK_DEG
            });
            if cliff {
                return Some(material::SCREE);
            }
        }
        if slope < SCREE_DEG.1 {
            // A wall's foot, however level the ground under it: the ground some way off on
            // one side or another stands as high over it as a wall does.
            let here = self.height_in(direction, memo);
            let around = memo.around(CLIFF_LOOK_M);
            let (a, b) = (
                direction.any_orthonormal_vector(),
                direction.any_orthonormal_vector().cross(direction),
            );
            let under_wall = APRON_WITHIN_M.iter().any(|&(reach, rise)| {
                [a, -a, b, -b].iter().any(|&way| {
                    let there = (direction * radius + way * reach).normalize();
                    self.height_in(there, around) - here > rise
                })
            });
            if under_wall {
                return Some(material::SCREE);
            }
        }
        None
    }

    /// The height at a lattice point of the hills and their finer detail, from the noise.
    fn exact_height(&self, direction: Vec3) -> f32 {
        let hills = self.relief_m
            * self
                .hills
                .fbm(direction * Self::FEATURES_AROUND, 5, 2.0, 0.5);
        let detail = self.detail_m
            * self.detail.fbm(
                direction * (self.spec.radius_m / Self::DETAIL_SCALE_M),
                3,
                2.0,
                0.5,
            );
        hills + detail
    }

    /// The height in a direction: a blend of the four lattice points round it.
    fn height_in(&self, direction: Vec3, memo: &mut HeightMemo) -> f32 {
        self.land_in(direction, memo).height
    }

    /// The land in a direction: a blend of the four lattice points round it.
    fn land_in(&self, direction: Vec3, memo: &mut HeightMemo) -> Node {
        let (face, s, t) = cube_coordinates(direction);
        let cells = self.lattice as f32;
        let step = memo.step.min(self.lattice);
        // The lattice points either side, `step` apart, and how far between them.
        let at = |u: f32| {
            let f = ((u + 1.0) * 0.5 * cells).clamp(0.0, cells);
            let low = ((f as u32 / step) * step).min(self.lattice - 1);
            let high = (low + step).min(self.lattice);
            (low, high, (f - low as f32) / (high - low) as f32)
        };
        let ((i, i2, fi), (j, j2, fj)) = (at(s), at(t));
        let mut node = |i: u32, j: u32| {
            *memo.heights.entry((face, i, j)).or_insert_with(|| {
                let u = |k: u32| k as f32 / cells * 2.0 - 1.0;
                self.exact(cube_direction(face, u(i), u(j)))
            })
        };
        let (a, b, c, d) = (node(i, j), node(i2, j), node(i, j2), node(i2, j2));
        a.lerp(b, fi).lerp(c.lerp(d, fi), fj)
    }
}

/// Which of the cube's six faces a direction falls on, and where on it, each of the two
/// coordinates from -1 to 1, spaced by equal angles so the lattice is near even on the
/// ground.
fn cube_coordinates(direction: Vec3) -> (u8, f32, f32) {
    let a = direction.abs();
    let (face, major, u, v) = if a.x >= a.y && a.x >= a.z {
        (
            if direction.x > 0.0 { 0 } else { 1 },
            a.x,
            direction.y,
            direction.z,
        )
    } else if a.y >= a.z {
        (
            if direction.y > 0.0 { 2 } else { 3 },
            a.y,
            direction.x,
            direction.z,
        )
    } else {
        (
            if direction.z > 0.0 { 4 } else { 5 },
            a.z,
            direction.x,
            direction.y,
        )
    };
    let angle = |w: f32| libm::atanf(w / major.max(1e-12)) / std::f32::consts::FRAC_PI_4;
    (face, angle(u), angle(v))
}

/// The direction at a place on a face of the cube, the inverse of [`cube_coordinates`].
fn cube_direction(face: u8, s: f32, t: f32) -> Vec3 {
    let (u, v) = (
        libm::tanf(s * std::f32::consts::FRAC_PI_4),
        libm::tanf(t * std::f32::consts::FRAC_PI_4),
    );
    let sign = if face.is_multiple_of(2) { 1.0 } else { -1.0 };
    match face / 2 {
        0 => Vec3::new(sign, u, v),
        1 => Vec3::new(u, sign, v),
        _ => Vec3::new(u, v, sign),
    }
    .normalize()
}

impl Field for GeneratedPlanet {
    type Memo = HeightMemo;

    fn memo_for(&self, spacing_m: f32) -> HeightMemo {
        HeightMemo::coarse(spacing_m)
    }

    /// Mountains' walls stand to about 72 degrees (D322), and their faces' risers to about 80
    /// (`terra_voxel::Faces`).
    fn steepest(&self) -> f32 {
        match &self.range {
            Some(range) if !range.mountains.faces.is_empty() => 5.5,
            Some(_) => 3.0,
            None => 1.5,
        }
    }

    fn distance_in(&self, point: Vec3, memo: &mut HeightMemo) -> f32 {
        let from_centre = point.length();
        if from_centre < 1e-3 {
            return -self.spec.radius_m;
        }
        from_centre - (self.spec.radius_m + self.height_in(point / from_centre, memo))
    }

    fn sample_in(&self, point: Vec3, memo: &mut HeightMemo) -> Sample {
        let from_centre = point.length();
        if from_centre < 1e-3 {
            return Sample {
                distance: -self.spec.radius_m,
                material: material::STONE,
            };
        }
        let land = self.land_in(point / from_centre, memo);
        let height = land.height;
        let distance = from_centre - (self.spec.radius_m + height);
        let depth = -distance;
        // In mountains what lies on the surface, a topsoil's depth of it.
        let lying = if depth < Self::TOPSOIL_M {
            self.lying_in(point / from_centre, land, memo)
        } else {
            None
        };
        let material = if let Some(lying) = lying {
            lying
        } else if depth < Self::TOPSOIL_M {
            let beach = self
                .sea_level_m
                .is_some_and(|sea| height < sea + Self::BEACH_M);
            if beach {
                material::SAND
            } else {
                material::SOIL
            }
        } else if self.ore.fbm(point / Self::ORE_SCALE_M, 2, 2.0, 0.5) > Self::ORE_THRESHOLD {
            material::ORE
        } else {
            material::STONE
        };
        Sample { distance, material }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::StandInPlanet;
    use terra_gen::Seed;

    fn planet() -> GeneratedPlanet {
        GeneratedPlanet::new(
            PlanetSpec {
                seed: Seed(7),
                radius_m: 2000.0,
            },
            150.0,
            Some(-20.0),
        )
    }

    #[test]
    fn frozen_ground_rolls_in_hummocks_and_low_ridges_and_thawed_ground_does_not() {
        let world = |at_c: f32| {
            GeneratedPlanet::new(
                PlanetSpec {
                    seed: Seed(7),
                    radius_m: 30_000.0,
                },
                150.0,
                None,
            )
            .smooth()
            .with_patterned(Patterned {
                at_c,
                // No polygons, so only the moraine is measured.
                trough_m: 0.0,
                rim_m: 0.0,
                centre_m: 0.0,
                pool_m: 0.0,
                ..Patterned::ICE_WEDGES
            })
        };
        let (frozen, thawed, bare) = (world(5.0), world(-5.0), planet_without(30_000.0));
        let along = |i: usize| Vec3::new(1.0, 0.02 * i as f32 / 100.0, 0.3).normalize();
        // Along 6 km, the frozen ground over the thawed: within 300 m it rises and falls by
        // metres to some tens, as hummocky moraine does, and never so steeply a walker cannot
        // climb it; the thawed ground is the hills alone.
        let (mut windows, mut steepest) = (Vec::new(), 0.0_f32);
        let step_m = 0.02 / 100.0 * 30_000.0;
        let rises: Vec<f32> = (0..1000)
            .map(|i| frozen.height_m(along(i)) - thawed.height_m(along(i)))
            .collect();
        for (i, pair) in rises.windows(2).enumerate() {
            steepest = steepest.max(((pair[1] - pair[0]) / step_m).abs());
            assert_eq!(thawed.height_m(along(i)), bare.height_m(along(i)));
        }
        for window in rises.chunks((300.0 / step_m) as usize) {
            let (low, high) = window
                .iter()
                .fold((f32::MAX, f32::MIN), |(l, h), &r| (l.min(r), h.max(r)));
            windows.push(high - low);
        }
        windows.sort_by(f32::total_cmp);
        let middling = windows[windows.len() / 2];
        assert!((3.0..30.0).contains(&middling), "{windows:?}");
        assert!(libm::atanf(steepest).to_degrees() < 25.0, "{steepest}");
    }

    fn planet_without(radius_m: f32) -> GeneratedPlanet {
        GeneratedPlanet::new(
            PlanetSpec {
                seed: Seed(7),
                radius_m,
            },
            150.0,
            None,
        )
        .smooth()
    }

    #[test]
    fn the_ground_is_the_stand_ins_with_finer_detail_on_it() {
        // The same seed gives the stand-in's lie of the land, give or take the detail, and
        // the smooth planet is the stand-in itself.
        let planet = planet();
        let smooth = planet.clone().smooth();
        let stand_in = StandInPlanet::new(planet.spec(), 150.0, Some(-20.0));
        let mut rng = Seed(8).rng();
        let mut most_detail: f32 = 0.0;
        for _ in 0..500 {
            let up = rng.unit_vector();
            let height = stand_in.height_m(up);
            let detail = planet.height_m(up) - height;
            assert!(
                detail.abs() <= GeneratedPlanet::DETAIL_M + 0.02,
                "{detail} at {up}"
            );
            assert!((smooth.height_m(up) - height).abs() < 0.02);
            most_detail = most_detail.max(detail.abs());
            // Clear of the detail, above the stand-in's ground is air and below is ground.
            let clear = GeneratedPlanet::DETAIL_M + 0.1;
            assert!(!planet.sample(up * (2000.0 + height + clear)).solid());
            assert!(planet.sample(up * (2000.0 + height - clear)).solid());
        }
        assert!(
            most_detail > GeneratedPlanet::DETAIL_M * 0.25,
            "{most_detail}"
        );
    }

    #[test]
    fn soil_lies_over_stone_and_ore_is_found_in_it() {
        let planet = planet();
        let mut seen_ore = false;
        for i in 0..400 {
            let direction = Vec3::new(i as f32 * 0.37, 1.0, i as f32 * 0.21).normalize();
            let surface = direction * (2000.0 + planet.height_m(direction));
            let top = planet.sample(surface - direction * 0.5).material;
            assert!(top == material::SOIL || top == material::SAND);
            for depth in [4.0, 8.0, 16.0] {
                let deep = planet.sample(surface - direction * depth).material;
                assert!(deep == material::STONE || deep == material::ORE);
                seen_ore |= deep == material::ORE;
            }
        }
        assert!(seen_ore, "some of the stone is ore");
    }

    #[test]
    fn the_lattice_matches_the_noise_and_meets_itself_at_the_seams() {
        let planet = planet();
        for i in 0..200 {
            let direction =
                Vec3::new(i as f32 * 0.31 - 30.0, 7.0, i as f32 * 0.17 - 17.0).normalize();
            let blended = planet.height_m(direction);
            assert!((blended - planet.exact_height(direction)).abs() < 0.02);
        }
        // Either side of an edge between two faces, the land is the same height.
        let edge = Vec3::new(1.0, 1.0, 0.3).normalize();
        let (x_side, y_side) = (edge + Vec3::X * 1e-6, edge + Vec3::Y * 1e-6);
        assert!(
            (planet.height_m(x_side.normalize()) - planet.height_m(y_side.normalize())).abs()
                < 0.01
        );
        // And a direction comes back from where it falls on the cube.
        let (face, s, t) = cube_coordinates(edge);
        assert!(cube_direction(face, s, t).distance(edge) < 1e-5);
    }

    #[test]
    fn the_ground_is_pinned() {
        // Exact bits: the same on every machine, or players would dig different ground
        // (D41).
        let planet = planet();
        let sampled = [
            Vec3::new(13.0, 1.0, -40.0),
            Vec3::new(-0.3, -0.8, 0.5),
            Vec3::new(0.9, 0.2, 0.4),
        ]
        .map(|direction| {
            let sample = planet.sample(direction.normalize() * 2000.0);
            (sample.distance.to_bits(), sample.material.0)
        });
        assert_eq!(sampled, PINNED_GROUND);
    }

    #[test]
    fn level_ground_under_a_wall_is_talus_or_rock_and_open_level_ground_is_not_talus() {
        let radius_m = 30_000.0;
        let planet = GeneratedPlanet::new(
            PlanetSpec {
                seed: Seed(1),
                radius_m,
            },
            150.0,
            Some(-20.0),
        )
        .with_mountains(crate::Mountains::START);
        let height = |d: Vec3| planet.height_m(d.normalize());
        let range = planet.mountains().expect("a range");
        // Each sample: whether it lies under a wall (or on open level ground), what it is made
        // of and where; on every core, as the sweep is long.
        let ks: Vec<u32> = (0..40_000).collect();
        let samples = terra_gen::par::map(&ks, |&k| {
            let d = Vec3::new(
                terra_gen::hash::unit3(11, k, 1, 0) * 2.0 - 1.0,
                terra_gen::hash::unit3(11, k, 2, 0) * 2.0 - 1.0,
                terra_gen::hash::unit3(11, k, 3, 0) * 2.0 - 1.0,
            )
            .normalize();
            if range.inside(d * radius_m) < 0.05 {
                return None;
            }
            let (a, b) = (
                d.any_orthonormal_vector(),
                d.any_orthonormal_vector().cross(d),
            );
            let here = height(d);
            let off = |way: Vec3, m: f32| height(d + way * (m / radius_m)) - here;
            let level = [a, b]
                .iter()
                .all(|&w| (off(w, 10.0) - off(-w, 10.0)).abs() < 2.0);
            if !level || here < 0.0 {
                return None;
            }
            // A wall's height between where the rule looks, one of four ways round.
            let rises = [a, -a, b, -b].map(|w| off(w, 150.0));
            let made_of = || planet.sample(d * (radius_m + here - 0.3)).material;
            if rises.iter().any(|&r| r > 160.0) {
                Some((true, made_of(), d))
            } else if rises.iter().all(|&r| r.abs() < 15.0)
                // Nor a wall as far off as the talus apron reaches (`APRON_WITHIN_M`): a wide
                // bench or valley floor under one is its apron.
                && [a, -a, b, -b].iter().all(|&w| {
                    APRON_WITHIN_M
                        .iter()
                        .all(|&(reach, rise)| off(w, reach) < rise)
                })
            {
                Some((false, made_of(), d))
            } else {
                None
            }
        });
        let (mut under, mut open) = (0, 0);
        for (walled, made_of, d) in samples.into_iter().flatten() {
            if walled {
                under += 1;
                // Talus, or the wall's own rock where the level ground is one of its ledges.
                assert!(
                    matches!(
                        made_of,
                        material::SCREE | material::STONE | material::GRAVEL
                    ),
                    "{made_of:?} at the foot of a wall {d}"
                );
            } else {
                open += 1;
                assert_ne!(made_of, material::SCREE, "on open level ground {d}");
            }
        }
        assert!(under > 0 && open > 0, "under walls {under}, open {open}");
    }

    const PINNED_GROUND: [(u32, u16); 3] =
        [(1_101_221_632, 4), (3_253_984_704, 2), (1_088_236_800, 1)];
}
