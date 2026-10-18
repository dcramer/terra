//! A lake's bank (D322, D381): what water it stands over, and how high over it the meadow
//! comes down to.

use crate::planet::Planet;
use bevy::prelude::Vec3;

/// How many of the lakes' 10 m cells round a point its bank's water is looked for in, by what
/// grows there and by the ground's shore (`terrain::lying`): far enough that a flat basin's
/// bank has risen clear of the water where the lookup gives out, so neither the bare bank nor
/// the wet band ends in the cells' squares, as they did a cell or two out (D381's lake).
pub const REACH_CELLS: usize = 3;

/// The water a point's bank stands over, its surface's distance from the centre, as
/// `Planet::water_near_m` finds it but looked for [`REACH_CELLS`] round; and how much that
/// water is a lowland lake's (0 for the sea's or a mountain lake's), from its height over the
/// sea as `lakes` measures it.
pub fn near(planet: &Planet, point: Vec3) -> (f32, f32) {
    let sea = planet.sea_radius_m();
    let lake = planet
        .round()
        .and_then(|round| round.water_near(point.normalize_or(Vec3::Y), REACH_CELLS))
        .filter(|&lake| lake > sea);
    lake.map_or((sea, 0.0), |lake| (lake, crate::lakes::lowland(lake - sea)))
}

/// How much grass a bank holds so high over its water: a mountain lake's scoured stones reach
/// a man's height up it; a lowland lake's meadow comes down to a few centimetres over its
/// water, a margin of wet mud below (the ground shader's soft shore), as a lowland mere's bank
/// is (Red Dead Redemption 2's and Kingdom Come's ponds, sedge and grass to the waterline), not
/// a band of bare ground tens of metres wide where the bank is flat.
pub fn grass(above_m: f32, lowland: f32) -> f32 {
    let from = 0.5 + (0.02 - 0.5) * lowland;
    let to = 1.6 + (0.08 - 1.6) * lowland;
    let t = ((above_m - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lowland lake's meadow comes down to its water; a mountain lake's bank is
    /// bare to a man's height, the sea's alike; nothing grows under any water.
    #[test]
    fn a_lowland_banks_grass_comes_down_to_its_water() {
        assert!(grass(0.1, 1.0) > 0.95);
        assert!(grass(0.5, 0.0) < 0.01);
        assert!(grass(1.7, 0.0) > 0.99);
        assert!(grass(0.0, 1.0) == 0.0 && grass(-1.0, 0.0) == 0.0);
    }
}
