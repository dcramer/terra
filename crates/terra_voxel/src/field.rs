//! The ground as a field: at any point, how far it is from the surface and what it is
//! made of. A generated planet answers from its seed alone; edits are laid over it in
//! [`crate::planet`].

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

    /// Each material, and the name data files know it by.
    pub const NAMED: [(MaterialId, &str); 5] = [
        (AIR, "Air"),
        (SOIL, "Soil"),
        (STONE, "Stone"),
        (ORE, "Ore"),
        (SAND, "Sand"),
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
}

/// The heights of the land at lattice points already worked out, for asks nearby.
pub struct HeightMemo {
    heights: HashMap<(u8, u32, u32), f32, BuildHasherDefault<LatticeHasher>>,
    /// Blend between every so many lattice points, for ground seen coarsely: 1 is the
    /// ground itself.
    step: u32,
}

impl HeightMemo {
    /// A memo for asking about ground seen at a coarseness: voxels so many metres apart.
    /// The lattice points used are the ground's own, fewer of them.
    pub fn coarse(spacing_m: f32) -> Self {
        let step = (spacing_m / GeneratedPlanet::LATTICE_M).max(1.0) as u32;
        Self {
            heights: HashMap::default(),
            step: step.next_power_of_two(),
        }
    }
}

impl Default for HeightMemo {
    fn default() -> Self {
        Self::coarse(GeneratedPlanet::LATTICE_M)
    }
}

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
    relief_m: f32,
    /// How tall the finer detail is.
    detail_m: f32,
    sea_level_m: Option<f32>,
    /// Lattice points along each edge of a face.
    lattice: u32,
}

impl GeneratedPlanet {
    /// How many large hills fit around the planet, as on the stand-in.
    const FEATURES_AROUND: f32 = 4.0;
    /// The finer detail: how tall, and how far apart its features are.
    const DETAIL_M: f32 = 0.8;
    const DETAIL_SCALE_M: f32 = 24.0;
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
            relief_m,
            detail_m: Self::DETAIL_M,
            sea_level_m,
            lattice: (quarter_round / Self::LATTICE_M).ceil().max(1.0) as u32,
        }
    }

    /// The same planet without its finer detail: with no relief, perfectly smooth, as a
    /// test level wants.
    pub fn smooth(mut self) -> Self {
        self.detail_m = 0.0;
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

    /// The height at a lattice point, worked out from the noise.
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
                self.exact_height(cube_direction(face, u(i), u(j)))
            })
        };
        let (a, b, c, d) = (node(i, j), node(i2, j), node(i, j2), node(i2, j2));
        let near = a + (b - a) * fi;
        let far = c + (d - c) * fi;
        near + (far - near) * fj
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
        let height = self.height_in(point / from_centre, memo);
        let distance = from_centre - (self.spec.radius_m + height);
        let depth = -distance;
        let material = if depth < Self::TOPSOIL_M {
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

    const PINNED_GROUND: [(u32, u16); 3] =
        [(1_101_221_632, 4), (3_253_984_704, 2), (1_088_236_800, 1)];
}
