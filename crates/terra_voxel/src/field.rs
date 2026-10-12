//! The ground as a field: at any point, how far it is from the surface and what it is
//! made of. A generated planet answers from its seed alone; edits are laid over it in
//! [`crate::planet`].

use glam::Vec3;
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
pub trait Field: Send + Sync {
    fn sample(&self, point: Vec3) -> Sample;
}

/// A planet generated from its seed: large hills, with finer detail on them, soil over
/// stone, sand by the sea, and veins of ore in the stone.
///
/// The large hills are the stand-in planet's (`terra_core::StandInPlanet`), from the same
/// seed, so the same seed gives the same lie of the land before and after voxels.
#[derive(Clone, Debug)]
pub struct GeneratedPlanet {
    spec: PlanetSpec,
    hills: Noise,
    detail: Noise,
    ore: Noise,
    relief_m: f32,
    sea_level_m: Option<f32>,
}

impl GeneratedPlanet {
    /// How many large hills fit around the planet, as on the stand-in.
    const FEATURES_AROUND: f32 = 4.0;
    /// The finer detail: how tall, and how far apart its features are.
    const DETAIL_M: f32 = 0.8;
    const DETAIL_SCALE_M: f32 = 24.0;
    /// How deep the top layer of soil or sand goes.
    const TOPSOIL_M: f32 = 1.5;
    /// Sand instead of soil this close above the sea.
    const BEACH_M: f32 = 1.5;
    /// How big ore veins are, and how much of the stone they fill: a vein where the noise
    /// is above the threshold.
    const ORE_SCALE_M: f32 = 9.0;
    const ORE_THRESHOLD: f32 = 0.45;

    pub fn new(spec: PlanetSpec, relief_m: f32, sea_level_m: Option<f32>) -> Self {
        Self {
            spec,
            hills: Noise::new(spec.seed.derive("stand-in hills")),
            detail: Noise::new(spec.seed.derive("ground detail")),
            ore: Noise::new(spec.seed.derive("ore veins")),
            relief_m,
            sea_level_m,
        }
    }

    pub fn spec(&self) -> PlanetSpec {
        self.spec
    }

    /// Distance from the centre to the sea's surface, if there is a sea.
    pub fn sea_radius_m(&self) -> Option<f32> {
        self.sea_level_m.map(|level| self.spec.radius_m + level)
    }

    /// How high the generated ground stands above the nominal radius in a direction.
    pub fn height_m(&self, direction: Vec3) -> f32 {
        let hills = self.relief_m
            * self
                .hills
                .fbm(direction * Self::FEATURES_AROUND, 5, 2.0, 0.5);
        let detail = Self::DETAIL_M
            * self.detail.fbm(
                direction * (self.spec.radius_m / Self::DETAIL_SCALE_M),
                3,
                2.0,
                0.5,
            );
        hills + detail
    }
}

impl Field for GeneratedPlanet {
    fn sample(&self, point: Vec3) -> Sample {
        let from_centre = point.length();
        if from_centre < 1e-3 {
            return Sample {
                distance: -self.spec.radius_m,
                material: material::STONE,
            };
        }
        let height = self.height_m(point / from_centre);
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
    fn above_is_air_below_is_ground() {
        let planet = planet();
        let up = Vec3::Y;
        let surface = up * (2000.0 + planet.height_m(up));
        assert!(planet.sample(surface + up * 5.0).distance > 4.9);
        assert!(planet.sample(surface - up * 5.0).distance < -4.9);
        assert!(planet.sample(surface).distance.abs() < 1e-2);
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
    fn the_same_seed_gives_the_same_ground() {
        let point = Vec3::new(13.0, 2101.0, -40.0);
        assert_eq!(planet().sample(point), planet().sample(point));
    }
}
