use crate::{Ground, MaterialId, PlanetSpec, Water};
use glam::Vec3;
use terra_gen::Noise;

/// A planet computed from a formula, with no voxels: a sphere with hills.
///
/// It exists so that everything except the terrain itself can be built and measured
/// before the voxel terrain is ready (D42). It cannot be dug, and it is one material
/// throughout.
#[derive(Clone, Debug)]
pub struct StandInPlanet {
    spec: PlanetSpec,
    hills: Noise,
    /// Height of the tallest hills above the nominal radius, in metres.
    relief_m: f32,
    /// Sea level relative to the nominal radius, if there is a sea.
    sea_level_m: Option<f32>,
}

impl StandInPlanet {
    /// How many large hills fit around the planet. A fixed count, so planets of any size
    /// have the same look.
    const FEATURES_AROUND: f32 = 4.0;

    pub fn new(spec: PlanetSpec, relief_m: f32, sea_level_m: Option<f32>) -> Self {
        StandInPlanet {
            spec,
            hills: Noise::new(spec.seed.derive("stand-in hills")),
            relief_m,
            sea_level_m,
        }
    }

    pub fn spec(&self) -> PlanetSpec {
        self.spec
    }

    /// Height of the ground above the nominal radius in a direction, in metres.
    pub fn height_m(&self, direction: Vec3) -> f32 {
        self.relief_m
            * self
                .hills
                .fbm(direction * Self::FEATURES_AROUND, 5, 2.0, 0.5)
    }
}

impl Ground for StandInPlanet {
    fn distance(&self, point: Vec3) -> f32 {
        let from_centre = point.length();
        if from_centre < 1e-3 {
            return -self.spec.radius_m;
        }
        from_centre - (self.spec.radius_m + self.height_m(point / from_centre))
    }

    fn material(&self, _point: Vec3) -> MaterialId {
        MaterialId::default()
    }
}

impl Water for StandInPlanet {
    fn level(&self, _direction: Vec3) -> Option<f32> {
        self.sea_level_m.map(|sea| self.spec.radius_m + sea)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;

    fn planet() -> StandInPlanet {
        StandInPlanet::new(
            PlanetSpec {
                seed: Seed(1),
                radius_m: 2000.0,
            },
            150.0,
            Some(0.0),
        )
    }

    fn directions() -> impl Iterator<Item = Vec3> {
        let mut rng = Seed(8).rng();
        (0..500).map(move |_| rng.unit_vector())
    }

    #[test]
    fn the_ground_is_where_the_height_says() {
        let planet = planet();
        for dir in directions() {
            let surface = 2000.0 + planet.height_m(dir);
            assert!(planet.height_m(dir).abs() <= 150.0);
            assert!(planet.distance(dir * (surface + 10.0)) > 0.0);
            assert!(planet.distance(dir * (surface - 10.0)) < 0.0);
            assert!(planet.distance(dir * surface).abs() < 0.01);
        }
        assert!(planet.distance(Vec3::ZERO) < 0.0);
    }

    #[test]
    fn a_ray_from_above_lands_on_the_ground() {
        let planet = planet();
        for dir in directions() {
            let origin = dir * 2600.0;
            let hit = planet
                .raycast(origin, -dir, 1000.0)
                .expect("straight down must hit");
            assert!(
                planet.distance(hit.point).abs() < 0.05,
                "missed by {}",
                planet.distance(hit.point)
            );
            assert!((origin.distance(hit.point) - hit.distance).abs() < 0.01);
            // Pointing away from the planet finds nothing.
            assert!(planet.raycast(origin, dir, 1000.0).is_none());
        }
    }

    #[test]
    fn a_grazing_ray_does_not_tunnel_through_a_hill() {
        let planet = planet();
        for dir in directions() {
            let surface = 2000.0 + planet.height_m(dir);
            let origin = dir * (surface + 2.0);
            let along = dir.cross(Vec3::new(0.3, 1.0, 0.2)).normalize();
            let ray = (along - dir * 0.05).normalize();
            if let Some(hit) = planet.raycast(origin, ray, 800.0) {
                // Everything before the hit is above ground.
                for step in 1..40 {
                    let travelled = hit.distance * step as f32 / 40.0;
                    assert!(planet.distance(origin + ray * travelled) > -0.1);
                }
            }
        }
    }

    #[test]
    fn the_surface_faces_roughly_up() {
        let planet = planet();
        for dir in directions() {
            let surface = dir * (2000.0 + planet.height_m(dir));
            let normal = planet.normal(surface);
            assert!((normal.length() - 1.0).abs() < 1e-4);
            assert!(normal.dot(dir) > 0.5, "normal {normal} against up {dir}");
        }
    }

    #[test]
    fn the_sea_is_level_and_optional() {
        let planet = planet();
        assert_eq!(planet.level(Vec3::X), Some(2000.0));
        assert_eq!(planet.level(Vec3::Y), Some(2000.0));
        let dry = StandInPlanet::new(planet.spec(), 150.0, None);
        assert_eq!(dry.level(Vec3::X), None);
    }

    #[test]
    fn the_same_spec_gives_the_same_planet_and_works_as_a_trait_object() {
        let (a, b) = (planet(), planet());
        let ground: Box<dyn Ground> = Box::new(b);
        for dir in directions() {
            let p = dir * 2050.0;
            assert_eq!(a.distance(p), ground.distance(p));
        }
        let other = StandInPlanet::new(
            PlanetSpec {
                seed: Seed(2),
                radius_m: 2000.0,
            },
            150.0,
            None,
        );
        assert!(directions().any(|d| a.height_m(d) != other.height_m(d)));
    }
}
