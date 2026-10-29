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
    /// How far round an inside corner its facets ease in: half a facet, as the faces either
    /// side of a corner meet on it (`clear` in [`Packed::at`]).
    pub(super) fn facet_ease(&self) -> f32 {
        if self.packing.facets > 0.0 {
            self.packing.facet.max(0.05) * 0.5
        } else {
            0.0
        }
    }

    /// How far the surface is pushed out at a point by its packing: lumps of two sizes, and
    /// each lift rounded out between its seams, a little flatter in its middle; its facets
    /// sunk by `clear` of their depth (0 on an inside corner, 1 [`Packed::facet_ease`] off).
    pub(super) fn at(&self, p: Vec3, outward: Vec3, stepping: f32, clear: f32) -> f32 {
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
        // patches (mine round 10). Sunk only, so a post or a prop stood at the face is never
        // swallowed (round 10's either way, at 8 cm, did not read as broken rock), and eased
        // in from an inside corner: there the inside fade's 6 cm would fold a deep facet's
        // edge into a fin.
        let facets = if k.facets > 0.0 {
            let off_top = (1.0 - outward.y).clamp(0.0, 1.0);
            let clear = clear.clamp(0.0, 1.0);
            let eased = clear * clear * (3.0 - 2.0 * clear);
            -crate::dress::faceted(p, k.facet.max(0.05), 47) * k.facets * off_top * eased
        } else {
            0.0
        };
        lumps + ridge + step + facets
    }

    /// How clear a point is of the inside edges near it for its facets: `clear` in
    /// [`Packed::at`], 1 where it has none.
    pub(super) fn clear(&self, p: Vec3, edges: &[(Vec3, Vec3)]) -> f32 {
        let ease = self.facet_ease();
        if ease > 0.0 {
            fade_over(p, edges, ease)
        } else {
            1.0
        }
    }
}

/// How far a point is from the inside edges near it, as a share of `reach`, up to 1.
pub(super) fn fade_over(p: Vec3, edges: &[(Vec3, Vec3)], reach: f32) -> f32 {
    edges
        .iter()
        .map(|&(from, to)| {
            let along = (to - from).normalize();
            let t = (p - from).dot(along).clamp(0.0, terra_build::CELL_M);
            (p.distance(from + along * t) / reach).min(1.0)
        })
        .fold(1.0_f32, f32::min)
}

/// How far off a face its inside edges are gathered: the inside fade's reach, or a
/// packing's facets' ease (`Packed::facet_ease`) within the cubes round it, which
/// `near_inside` looks through.
pub(super) fn edge_reach(packed: Option<Packed>) -> f32 {
    let ease = packed.map_or(0.0, |packed| packed.facet_ease());
    super::INSIDE_FADE_M.max(ease.min(terra_build::CELL_M))
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
                packed.at(p, Vec3::X, 1.0, 1.0)
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
            facets: 0.12,
            facet: 0.8,
        };
        let packed = Packed { packing, lift: 0.5 };
        let spread = |outward: Vec3| {
            let along: Vec<f32> = (0..400)
                .map(|i| Vec3::new(i as f32 * 0.02, 0.3, i as f32 * 0.013))
                .map(|p| packed.at(p, outward, 1.0, 1.0))
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

    /// A blasted drift's broken rock lies behind its neat line, never in front of it: the
    /// facets only sink, by up to their depth, so a timber post or a prop stood at the face
    /// keeps clear of the rock (round 10's either way stood 8 cm proud of it); and they ease
    /// in from an inside corner, nothing on it, all of it half a facet off.
    #[test]
    fn facets_sink_behind_the_face_and_ease_in_from_an_inside_corner() {
        let packing = kit::Packing {
            lumps: 0.0,
            lump: 0.5,
            ridge: 0.0,
            step: 0.0,
            facets: 0.2,
            facet: 0.6,
        };
        let packed = Packed { packing, lift: 0.5 };
        let wall: Vec<f32> = (0..2000)
            .map(|i| Vec3::new(i as f32 * 0.011, 0.3 + (i % 37) as f32 * 0.05, 0.0))
            .map(|p| packed.at(p, Vec3::Z, 1.0, 1.0))
            .collect();
        let (low, high) = wall
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        assert!(high <= 1e-6, "proud of the face by {high}");
        assert!(low >= -0.2 - 1e-6, "sunk past the depth, {low}");
        assert!(high - low > 0.12, "broken over only {}", high - low);
        assert!((packed.facet_ease() - 0.3).abs() < 1e-6);
        for i in 0..50 {
            let p = Vec3::new(i as f32 * 0.07, 0.7, 0.0);
            assert!(packed.at(p, Vec3::Z, 1.0, 0.0).abs() < 1e-6);
            let (near, off) = (
                packed.at(p, Vec3::Z, 1.0, 0.3),
                packed.at(p, Vec3::Z, 1.0, 1.0),
            );
            // A third of the way out, eased in: 0.3² (3 - 0.6) of the way down.
            assert!((near - off * 0.216).abs() < 1e-6, "{near} {off}");
        }
    }
}
