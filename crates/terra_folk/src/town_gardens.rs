//! The ground of a walled town's gardens (D381, `terra_town::town::Gardens`): beds of
//! vegetables in rows with earth between, greens kept short and lush, and ground fresh dug,
//! each plot framed by a path trodden pale, laid out as the house whose it is is finished;
//! averaged over the floor's texel, so from high up a plot reads as its mean, not as torn
//! blotches of whichever row a texel's middle fell on.

use std::collections::HashMap;
use terra_town::town::{BALK_M, GardenKind, Gardens};
use terra_world::cover::Growing;

/// How far apart a garden's beds are, middle to middle, and the share of that which is earth
/// between them.
const BED_PITCH_M: f32 = 1.1;
const BED_EARTH: f32 = 0.4;

/// The gardens laid out so far, by their cells on the town's grid.
#[derive(Clone, Default)]
pub struct GardenGround {
    grid: Gardens,
    cells: HashMap<[i32; 2], GardenKind>,
}

impl GardenGround {
    /// The gardens of a town whose homes `laid` says are finished.
    pub fn new(gardens: &Gardens, laid: impl Fn(usize) -> bool) -> Self {
        GardenGround {
            grid: Gardens {
                plots: Vec::new(),
                ..gardens.clone()
            },
            cells: gardens
                .plots
                .iter()
                .filter(|g| laid(g.home))
                .map(|g| (g.cell, g.kind))
                .collect(),
        }
    }

    /// What grows at a point in the town's metres east and south, if it is in a garden, on
    /// average over a square `texel_m` wide round it (0 the point's): a picture of the ground
    /// with texels wider than the beds' rows holds each plot's mean, its earth and its greens
    /// mixed, and its paths as their share of the texel (`CoverSource::growing_over`).
    pub fn grow(&self, p: bevy::math::Vec2, texel_m: f32, growing: &mut Growing) {
        if self.cells.is_empty() {
            return;
        }
        let (cell, within) = self.grid.cell_at(p);
        let Some(&kind) = self.cells.get(&cell) else {
            return;
        };
        let size = Gardens::plot_m();
        let m = within * size;
        // How much of the square is the plot within its path, along and across, the grid's
        // plots side by side; and of that, along it, the earth between the beds' rows.
        let inside = covered(m.x, texel_m, size.x, BALK_M, size.x - BALK_M)
            * covered(m.y, texel_m, size.y, BALK_M, size.y - BALK_M);
        let earth = covered(
            m.x - BALK_M,
            texel_m,
            BED_PITCH_M,
            0.0,
            BED_PITCH_M * BED_EARTH,
        );
        // Between the plots a path of earth trodden pale by the gardeners' feet, a frame round
        // each bed, as a burgage plot's were walked (the review of round 14 asked for a
        // path's edge and lighter ground between them).
        let mut path = *growing;
        path.grass *= PATH_GRASS;
        path.trodden = path.trodden.max(PATH_TRODDEN);
        path.tall = 0.0;
        path.shrub = 0.0;
        let mut within = *growing;
        within.tall = 0.0;
        within.shrub = 0.0;
        match kind {
            GardenKind::Beds => {
                // Earth hoed between the rows, and a row of greens: short, thick and lush.
                let mut hoed = within;
                hoed.grass = 0.05;
                hoed.trodden = hoed.trodden.max(0.55);
                let mut row = within;
                row.grass = 1.0;
                row.lush = 1.0;
                within = mix(&row, &hoed, earth);
            }
            GardenKind::Green => {
                within.grass = within.grass.max(0.95);
                within.lush = 1.0;
            }
            GardenKind::Dug => {
                within.grass *= 0.15;
                within.trodden = within.trodden.max(0.5);
                within.roots = within.roots.max(0.2);
            }
        }
        *growing = mix(&path, &within, inside);
    }
}

/// How bare the path between two plots is, the share of the meadow's grass left, and how
/// trodden: past `ground.wesl`'s turned earth toward its packed, so it reads paler than the
/// beds' dark earth.
const PATH_GRASS: f32 = 0.25;
const PATH_TRODDEN: f32 = 0.72;

/// The share of the stretch `width` wide round `x` that falls on `[from, to)` of a pattern
/// repeating every `period`: the box-filtered pulse train (Apodaca and Gritz, "Advanced
/// RenderMan", `filteredpulsetrain`), or whether `x` is on it when `width` is 0.
fn covered(x: f32, width: f32, period: f32, from: f32, to: f32) -> f32 {
    // How much of the pattern is on from 0 to `x`.
    let integral = |x: f32| {
        let whole = libm::floorf(x / period);
        whole * (to - from) + (x - whole * period - from).clamp(0.0, to - from)
    };
    if width <= 1e-4 {
        let at = x - libm::floorf(x / period) * period;
        return if (from..to).contains(&at) { 1.0 } else { 0.0 };
    }
    ((integral(x + width * 0.5) - integral(x - width * 0.5)) / width).clamp(0.0, 1.0)
}

/// `a` toward `b` by `t`, field by field, for what a share of a texel holds.
fn mix(a: &Growing, b: &Growing, t: f32) -> Growing {
    let l = |x: f32, y: f32| x + (y - x) * t;
    Growing {
        grass: l(a.grass, b.grass),
        lush: l(a.lush, b.lush),
        trodden: l(a.trodden, b.trodden),
        roots: l(a.roots, b.roots),
        tall: l(a.tall, b.tall),
        shrub: l(a.shrub, b.shrub),
        ..*a
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec2;
    use terra_town::town::Garden;

    /// One plot of beds, its grid turned off the world's axes as a town's contour turns it.
    fn beds() -> GardenGround {
        let gardens = Gardens {
            origin: Vec2::ZERO,
            along: Vec2::new(0.8, 0.6),
            plots: vec![Garden {
                cell: [0, 0],
                home: 0,
                kind: GardenKind::Beds,
            }],
        };
        GardenGround::new(&gardens, |_| true)
    }

    /// A point `u` along the plot and `v` across it.
    fn at(u: f32, v: f32) -> Vec2 {
        let along = Vec2::new(0.8, 0.6);
        along * u + along.perp() * v
    }

    fn sample(ground: &GardenGround, p: Vec2, texel_m: f32) -> Growing {
        let mut g = Growing {
            grass: 0.8,
            lush: 0.6,
            ..Default::default()
        };
        ground.grow(p, texel_m, &mut g);
        g
    }

    #[test]
    fn from_high_up_a_bed_reads_even_its_rows_averaged_not_torn_into_blotches() {
        // A floor of 2 m texels over the plot's middle, well within its path. Sampled at
        // their middles the rows alias into stripes of earth and green at random; filtered,
        // each texel holds the rows' mean.
        let ground = beds();
        let size = Gardens::plot_m();
        let (mut point, mut filtered) = (Vec::new(), Vec::new());
        for i in 0..2 {
            for j in 0..4 {
                let p = at(2.5 + i as f32 * 1.7, 3.0 + j as f32 * 2.3);
                point.push(sample(&ground, p, 0.0).trodden);
                filtered.push(sample(&ground, p, 2.0).trodden);
            }
        }
        let spread = |v: &[f32]| {
            v.iter().copied().fold(f32::MIN, f32::max) - v.iter().copied().fold(f32::MAX, f32::min)
        };
        assert!(
            spread(&point) > 0.4,
            "the point-sampled rows alias: {point:?}"
        );
        assert!(spread(&filtered) < 0.1, "filtered, even: {filtered:?}");
        // And the mean is the rows' own: the earth's share of the pitch, found by sampling
        // the plot finely at its points.
        let n = 400;
        let fine: f32 = (0..n)
            .map(|k| {
                let u = BALK_M + (size.x - 2.0 * BALK_M) * (k as f32 + 0.5) / n as f32;
                sample(&ground, at(u, 7.0), 0.0).trodden
            })
            .sum::<f32>()
            / n as f32;
        let mean = filtered.iter().sum::<f32>() / filtered.len() as f32;
        assert!((mean - fine).abs() < 0.04, "{mean} against {fine}");
    }

    #[test]
    fn a_path_frames_each_plot_paler_than_its_beds_and_shows_from_high_up() {
        let ground = beds();
        // On the path, near the plot's edge, and in its middle.
        let path = sample(&ground, at(0.2, 7.0), 0.0);
        let bed = sample(&ground, at(4.0, 7.0), 2.0);
        assert!(path.trodden > bed.trodden + 0.2, "{path:?} {bed:?}");
        // A 2 m texel centred on the line between two plots is mostly path.
        let edge = sample(&ground, at(0.05, 7.0), 2.0);
        assert!(edge.trodden > bed.trodden + 0.1, "{edge:?} {bed:?}");
        // Outside any garden nothing changes.
        let away = sample(&ground, at(-30.0, 7.0), 2.0);
        assert_eq!((away.grass, away.trodden), (0.8, 0.0));
    }
}
