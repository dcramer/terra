//! The drifts of a meadow's mix (D381), laid by the world's seed (D596).

use bevy::prelude::*;
use terra_world::planet::Planet;

/// Smooth value noise from 0 to 1 over a place, a unit a cell: the drifts of a meadow's mix,
/// laid by the world's seed (`meadow_seed`, D596), so another world's meadow drifts elsewhere.
pub(super) fn patch(seed: u32, p: Vec2) -> f32 {
    let hash = |x: i32, y: i32| {
        let h = terra_gen::hash::prospector32_round(
            seed ^ (x as u32).wrapping_mul(0x9e37_79b9) ^ (y as u32).wrapping_mul(0x85eb_ca6b),
        );
        (h & 0xffff) as f32 / 65535.0
    };
    let (x, y) = (libm::floorf(p.x), libm::floorf(p.y));
    let (fx, fy) = (p.x - x, p.y - y);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (x, y) = (x as i32, y as i32);
    let top = hash(x, y) + (hash(x + 1, y) - hash(x, y)) * sx;
    let bottom = hash(x, y + 1) + (hash(x + 1, y + 1) - hash(x, y + 1)) * sx;
    top + (bottom - top) * sy
}

/// The seed a world's meadows drift by: its own, derived for them (D596).
pub(super) fn meadow_seed(planet: &Planet) -> u32 {
    planet.seed().derive("meadow").0 as u32
}
