//! A packed kind's shape on its surface (D236): its kit's `Packing` as lumps, lifts and
//! broken facets pushing each point out along its normal.

use glam::Vec3;
use terra_build::kit;

/// Earth's packing in a surface's shape: its kit's `Packing`, and how tall its lifts are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Packed {
    pub packing: kit::Packing,
    pub lift: f32,
}

impl Packed {
    /// How far the surface is pushed out at a point by its packing: lumps of two sizes, and
    /// each lift rounded out between its seams, a little flatter in its middle.
    pub(super) fn at(&self, p: Vec3, outward: Vec3, stepping: f32) -> f32 {
        use crate::dress::smooth;
        let k = &self.packing;
        let lumps = if k.lumps > 0.0 {
            let size = k.lump.max(0.05);
            (smooth(p, size, 41) * 0.65 + smooth(p, size * 0.45, 43) * 0.35 - 0.5) * 2.0 * k.lumps
        } else {
            0.0
        };
        let ridge = if k.ridge > 0.0 {
            let y = crate::dress::lift_at(p, self.lift.max(0.05));
            let within = y - libm::floorf(y);
            let belly = 1.0 - (2.0 * within - 1.0) * (2.0 * within - 1.0);
            (libm::powf(belly, 0.6) - 0.6) * k.ridge
        } else {
            0.0
        };
        // Each lift a little proud or shy of the one under it, eased over its seam, out of
        // a wall's upright faces and round its arrises, not up off its top, and by `stepping`
        // of it, as near a cut it eases to nothing.
        let step = if k.step > 0.0 && stepping > 0.0 {
            let y = crate::dress::lift_at(p, self.lift.max(0.05));
            let layer = libm::floorf(y);
            let within = y - layer;
            let own = |l: f32| {
                (terra_gen::hash::unit(terra_gen::hash::of(&[l as i32 as u32, 61])) - 0.5)
                    * 2.0
                    * k.step
            };
            let t = (within / 0.12).clamp(0.0, 1.0);
            let t = t * t * (3.0 - 2.0 * t);
            let upright = ((0.2 - outward.y.abs()) / 0.2).clamp(0.0, 1.0);
            (own(layer - 1.0) + (own(layer) - own(layer - 1.0)) * t) * upright * stepping
        } else {
            0.0
        };
        // Broken faces on the walls and the roof, eased to nothing up a floor's top: there the
        // dug ground lies 3 cm over the cubes, and the facets stood up through it in pale
        // patches (mine round 10).
        let facets = if k.facets > 0.0 {
            let off_top = (1.0 - outward.y).clamp(0.0, 1.0);
            (crate::dress::faceted(p, k.facet.max(0.05), 47) - 0.5) * 2.0 * k.facets * off_top
        } else {
            0.0
        };
        lumps + ridge + step + facets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_earths_lifts_each_stand_a_little_proud_or_shy_of_the_next() {
        let packed = Packed {
            packing: kit::Packing {
                lumps: 0.0,
                lump: 0.5,
                ridge: 0.0,
                step: 0.03,
                facets: 0.0,
                facet: 0.0,
            },
            lift: 0.5,
        };
        // The middle of each of eight lifts up a line on a wall: within the step either way,
        // and not all alike, so the wall's outline steps.
        let middles: Vec<f32> = (0..8)
            .map(|lift| {
                let mut p = Vec3::new(0.3, 0.0, 0.0);
                // Up the line to where the lift's middle is, as the seams wander.
                p.y = (0..200)
                    .map(|i| i as f32 * 0.025)
                    .find(|&y| {
                        let at = crate::dress::lift_at(Vec3::new(0.3, y, 0.0), 0.5);
                        libm::floorf(at) as i32 == lift && at - libm::floorf(at) > 0.5
                    })
                    .unwrap();
                packed.at(p, Vec3::X, 1.0)
            })
            .collect();
        assert!(
            middles.iter().all(|m| m.abs() <= 0.03 + 1e-5),
            "{middles:?}"
        );
        let (low, high) = middles
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        assert!(high - low > 0.02, "{middles:?}");
    }

    /// Facets break a wall's face and its roof's, and leave a floor's top flat, where the dug
    /// ground lies 3 cm over it (mine round 10: they stood up through it in pale patches).
    #[test]
    fn facets_break_walls_and_roofs_and_leave_a_floor_flat() {
        let packing = kit::Packing {
            lumps: 0.0,
            lump: 0.5,
            ridge: 0.0,
            step: 0.0,
            facets: 0.08,
            facet: 0.8,
        };
        let packed = Packed { packing, lift: 0.5 };
        let spread = |outward: Vec3| {
            let along: Vec<f32> = (0..400)
                .map(|i| Vec3::new(i as f32 * 0.02, 0.3, i as f32 * 0.013))
                .map(|p| packed.at(p, outward, 1.0))
                .collect();
            let (low, high) = along
                .iter()
                .fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
            high - low
        };
        assert!(spread(Vec3::X) > 0.05, "a wall: {}", spread(Vec3::X));
        assert!(
            spread(Vec3::NEG_Y) > 0.05,
            "a roof: {}",
            spread(Vec3::NEG_Y)
        );
        assert!(spread(Vec3::Y) < 1e-6, "a floor: {}", spread(Vec3::Y));
    }
}
