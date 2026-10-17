//! The land a world is made of, by its rules (docs/demos.md, D321 to D326): its generated
//! ground and its climate taken together, which neither knows of the other. Where a place is
//! on the world and how its ground leans ([`frame`]), what the climate is there by those
//! ([`Land::place`]), and a region of it sampled on a grid, the field maps' and the site
//! queries' ground ([`region`]).
//!
//! Plain Rust with no engine (D28): what is decided here (where the tree line runs, how deep
//! the snow lies) changes play, so every machine works it out the same. `terra_world` draws
//! it; `cargo xtask fields` maps it without the GPU (D326).

pub mod drift;
pub mod frame;
pub mod ground;
pub mod heath;
pub mod lakes;
pub mod maps;
pub mod region;
pub mod round;
pub mod site;
pub mod snow;
pub mod town;
pub mod tundra;
pub mod water;
pub mod woods;

use frame::{Frame, START_LATITUDE};
use glam::Vec3;
use terra_climate::{Climate, Fields, Place};
use terra_voxel::GeneratedPlanet;

/// Ground whose year's mean is under this, in degrees Celsius, stays frozen at depth through
/// the summer: permafrost, as continuous permafrost begins near a mean of -2 to -5 °C.
pub const PERMAFROST_C: f32 = -2.0;

/// Where the planet's own start is, from its centre: the pole of the cube the ground is laid
/// on, which the free camera takes as up.
pub const PLANET_START: Vec3 = Vec3::Y;

/// A world's land: its ground as generated, its climate, and the frame it turns in.
#[derive(Clone, Debug)]
pub struct Land {
    pub ground: GeneratedPlanet,
    pub fields: Fields,
    pub frame: Frame,
}

impl Land {
    /// The land of a ground and a climate, the world turned as the sky is over its start
    /// (`frame`), the climate's fields from the ground's seed.
    pub fn new(ground: GeneratedPlanet, climate: Climate) -> Self {
        let year = climate.year;
        let declination = terra_climate::declination(year.tilt_deg.to_radians(), year.phase(0));
        let (frame, _, _) = Frame::turned_for(PLANET_START, START_LATITUDE, declination);
        // On a world to scale, the ground is patterned where its climate keeps it frozen at
        // depth: the year's mean under PERMAFROST_C, which falls with latitude and height
        // as the climate's base temperature does (docs/demos.md §2).
        let seed = ground.spec().seed;
        let ground = if frame::has_latitudes(ground.spec().radius_m) {
            let t = &climate.temperature;
            ground.with_patterned(terra_voxel::Patterned {
                pole: frame.pole(),
                sine_squared_c: t.equator_c - t.pole_c,
                per_m_c: t.lapse_c_per_km / 1000.0,
                at_c: PERMAFROST_C - t.equator_c,
                seed: seed.derive("ice wedges").0,
                ..terra_voxel::Patterned::ICE_WEDGES
            })
        } else {
            ground
        };
        // The demos' pinned places kept as they are by the mountains' front rules, which
        // reshape the ranges' edges kilometres across (D393, D395).
        let radius_m = ground.spec().radius_m;
        let kept: Vec<Vec3> = site::demo_pins(radius_m)
            .into_iter()
            .map(|(_, km)| site::along(&frame, radius_m, PLANET_START, km * 1000.0))
            .collect();
        let ground = ground.with_kept_spots(kept);
        let fields = Fields::new(climate, ground.spec().seed);
        Self {
            ground,
            fields,
            frame,
        }
    }

    pub fn radius_m(&self) -> f32 {
        self.ground.spec().radius_m
    }

    /// The distance from the centre to the sea's surface; the nominal radius on a world with
    /// no sea, which heights over the sea are then counted from.
    pub fn sea_radius_m(&self) -> f32 {
        self.ground.sea_radius_m().unwrap_or(self.radius_m())
    }

    /// The ground as generated in a direction from the centre.
    pub fn surface(&self, direction: Vec3) -> Vec3 {
        let direction = direction.normalize();
        direction * (self.radius_m() + self.ground.height_m(direction))
    }

    /// A point as the climate takes it: how high over the sea, its latitude, and how the
    /// ground there, facing `normal`, leans toward the sun (D323).
    pub fn place(&self, at: Vec3, normal: Vec3) -> Place {
        Place {
            at,
            height_m: at.length() - self.sea_radius_m(),
            latitude: self.frame.latitude(at),
            sunward: self.frame.sunward(at, normal),
            catch: 1.0,
        }
    }
}

impl Land {
    /// What the ground at a point on it is, for what grows there (`woods`): rock, loose
    /// stones, soil, or under water by `water` (the distance from the centre to the water's
    /// surface over it, if there is water).
    pub fn footing(&self, point: Vec3, water: Option<f32>) -> woods::Footing {
        use terra_voxel::{Field, material};
        if water.is_some_and(|level| point.length() < level) {
            return woods::Footing::Water;
        }
        match self.ground.sample(point - point.normalize() * 0.3).material {
            material::STONE | material::ORE => woods::Footing::Rock,
            material::SCREE | material::GRAVEL => woods::Footing::Loose,
            _ => woods::Footing::Soil,
        }
    }
}

/// The start's climate, as `assets/data/climate.ron` has it, for tools that run without the
/// engine's loading.
pub fn start_climate() -> Result<Climate, String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/data/climate.ron");
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    ron::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use terra_core::{PlanetSpec, Seed};

    #[test]
    fn the_start_climate_reads_and_its_start_is_at_the_starts_latitude() {
        let land = Land::new(
            ground::world(PlanetSpec {
                seed: Seed(1),
                radius_m: 30_000.0,
            }),
            start_climate().unwrap(),
        );
        let start = land.surface(PLANET_START);
        let place = land.place(start, start.normalize());
        assert!((place.latitude - START_LATITUDE).abs() < 1e-5);
        assert_eq!(place.sunward, 0.0);
        assert!((place.height_m - (start.length() - land.sea_radius_m())).abs() < 1e-3);
    }

    #[test]
    fn the_mountains_newer_rules_leave_the_pinned_places_as_they_were() {
        let spec = PlanetSpec {
            seed: Seed(1),
            radius_m: 30_000.0,
        };
        let climate = start_climate().unwrap();
        let land = Land::new(ground::world(spec), climate.clone());
        // The ranges as they were when the places were pinned: no front rules, a gully a cell.
        let as_pinned = terra_voxel::Mountains {
            spurs: 0.0,
            peaks: 0.0,
            gully_stretch: 1.0,
            ..terra_voxel::Mountains::START
        };
        let ground = GeneratedPlanet::new(spec, ground::RELIEF_M, Some(ground::SEA_LEVEL_M));
        let without = Land::new(ground.with_mountains(as_pinned), climate);
        let pins = site::demo_pins(spec.radius_m);
        assert!(!pins.is_empty());
        let mut changed = 0;
        for (name, km) in pins {
            // At the place and a few hundred metres round it the ground is the same; some
            // kilometres off the newer rules have reshaped it.
            for (east, north) in [(0.0, 0.0), (0.4, 0.0), (0.0, -0.4), (-0.3, 0.3)] {
                let at = land.along(PLANET_START, (km + Vec2::new(east, north)) * 1000.0);
                let (with, plain) = (land.ground.height_m(at), without.ground.height_m(at));
                assert_eq!(with, plain, "{name} {east},{north} km off");
            }
            for turn in 0..16 {
                let angle = turn as f32 * std::f32::consts::TAU / 16.0;
                let off = Vec2::new(libm::cosf(angle), libm::sinf(angle)) * 6.0;
                let at = land.along(PLANET_START, (km + off) * 1000.0);
                changed += usize::from(land.ground.height_m(at) != without.ground.height_m(at));
            }
        }
        assert!(changed > 0, "the newer rules change nothing anywhere");
    }
}
