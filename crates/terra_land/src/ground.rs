//! A world's ground as generated, before anyone digs: the same for the game and for the tools
//! that look at its rules without the engine (the field maps), so both see one land.

use terra_core::PlanetSpec;
use terra_voxel::{GeneratedPlanet, Mountains};

/// The height of the world's tallest hills, in metres.
pub const RELIEF_M: f32 = 150.0;
/// Sea level relative to the planet's nominal radius, in metres.
pub const SEA_LEVEL_M: f32 = -20.0;
/// Rolling ground's swells and the bumps on them, in metres, as the lush forest stands on.
pub const ROLLING_M: f32 = 6.0;
pub const ROLLING_DETAIL_M: f32 = 1.1;

/// A world smaller than this holds no mountain range: a range is tens of kilometres across,
/// and on the 2 km test planet one would be the whole world.
pub const MOUNTAINS_FROM_RADIUS_M: f32 = 10_000.0;

/// The world's ground: its hills and its sea, and on a world large enough its mountains, the
/// starting realm's (D322).
pub fn world(spec: PlanetSpec) -> GeneratedPlanet {
    let ground = GeneratedPlanet::new(spec, RELIEF_M, Some(SEA_LEVEL_M));
    if spec.radius_m >= MOUNTAINS_FROM_RADIUS_M {
        ground.with_mountains(Mountains::START)
    } else {
        ground
    }
}

/// Ground with no hills, no sea and no finer detail, as flat as a sphere is: the yard.
pub fn smooth(spec: PlanetSpec) -> GeneratedPlanet {
    GeneratedPlanet::new(spec, 0.0, None).smooth()
}

/// Ground rolling gently, swells a few metres high and smaller bumps on them, no sea.
pub fn rolling(spec: PlanetSpec) -> GeneratedPlanet {
    GeneratedPlanet::new(spec, ROLLING_M, None).with_detail_m(ROLLING_DETAIL_M)
}
