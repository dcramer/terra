//! The volumetric clouds' 3D noise (docs/sky.md stage 2, D515): one tiling texture of
//! Perlin-Worley and Worley noise, as Horizon's Nubis bakes it (Schneider, "The real-time
//! volumetric cloudscapes of Horizon Zero Dawn", SIGGRAPH 2015 Advances; Hillaire, "Physically
//! based sky, atmosphere and cloud rendering in Frostbite", 2016). Worley noise, inverted, is
//! round billows; Perlin noise remapped by it is the heaps' connected body.
//!
//! - **r**: Perlin-Worley, the cloud's large shape (Perlin fBm of 4, 8 and 16 cells over the
//!   tile, remapped by the Worley fBm of 4, 8 and 16 cells: `remap(perlin, 0, 1, worley, 1)`).
//! - **g, b, a**: Worley fBm at 4, 8 and 16 cells over the tile, from coarse to fine: the shape's
//!   erosion and, tiled finer, the edges' detail.
//!
//! Every value is a function of the texel alone (a fixed seed, `terra_gen::hash`), so every
//! machine bakes the same texture, and each noise repeats over the tile so the texture wraps
//! with no seam. Its mipmaps are box-filtered, so far clouds sample it without shimmer.

use terra_gen::hash;

/// The texture's side in texels.
pub const SIDE: u32 = 64;
/// Its mip levels, down to one texel.
pub const LEVELS: u32 = SIDE.ilog2() + 1;
/// The bake's seed: a fixed one, so every machine's clouds are alike.
const SEED: u32 = 0x5c1d_c10d;

/// Where a lattice point lies within a tile of `cells`, wrapped, so the noise tiles.
fn wrap(i: i32, cells: i32) -> u32 {
    i.rem_euclid(cells) as u32
}

/// Worley noise over a tile of `cells` a side at `p` (0 to 1 across the tile), inverted so it is
/// 1 at a feature point and 0 a cell away: one feature point a cell, the distance to the nearest.
pub fn worley(p: [f32; 3], cells: u32, salt: u32) -> f32 {
    let n = cells as i32;
    let q = p.map(|v| v * cells as f32);
    let base = q.map(|v| libm::floorf(v) as i32);
    let mut nearest = f32::MAX;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let c = [base[0] + dx, base[1] + dy, base[2] + dz];
                let (i, j, k) = (wrap(c[0], n), wrap(c[1], n), wrap(c[2], n));
                let s = SEED ^ salt.wrapping_mul(0x9e37_79b9);
                let at = [
                    c[0] as f32 + hash::unit3(s, i, j, k),
                    c[1] as f32 + hash::unit3(s.wrapping_add(1), i, j, k),
                    c[2] as f32 + hash::unit3(s.wrapping_add(2), i, j, k),
                ];
                let d2 = (0..3).map(|a| (at[a] - q[a]) * (at[a] - q[a])).sum::<f32>();
                nearest = nearest.min(d2);
            }
        }
    }
    (1.0 - libm::sqrtf(nearest)).clamp(0.0, 1.0)
}

/// Gradient (Perlin) noise over a tile of `cells` a side at `p`, about -1 to 1.
pub fn perlin(p: [f32; 3], cells: u32, salt: u32) -> f32 {
    const GRADIENTS: [[f32; 3]; 12] = [
        [1.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
        [1.0, -1.0, 0.0],
        [-1.0, -1.0, 0.0],
        [1.0, 0.0, 1.0],
        [-1.0, 0.0, 1.0],
        [1.0, 0.0, -1.0],
        [-1.0, 0.0, -1.0],
        [0.0, 1.0, 1.0],
        [0.0, -1.0, 1.0],
        [0.0, 1.0, -1.0],
        [0.0, -1.0, -1.0],
    ];
    let n = cells as i32;
    let q = p.map(|v| v * cells as f32);
    let base = q.map(|v| libm::floorf(v) as i32);
    let f = [0, 1, 2].map(|a| q[a] - base[a] as f32);
    // Quintic fade, Perlin's improved noise (2002).
    let fade = f.map(|t| t * t * t * (t * (t * 6.0 - 15.0) + 10.0));
    let corner = |dx: i32, dy: i32, dz: i32| {
        let (i, j, k) = (
            wrap(base[0] + dx, n),
            wrap(base[1] + dy, n),
            wrap(base[2] + dz, n),
        );
        let g = GRADIENTS[(hash::mix3(SEED ^ salt, i, j, k) % 12) as usize];
        g[0] * (f[0] - dx as f32) + g[1] * (f[1] - dy as f32) + g[2] * (f[2] - dz as f32)
    };
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(corner(0, 0, 0), corner(1, 0, 0), fade[0]);
    let x10 = lerp(corner(0, 1, 0), corner(1, 1, 0), fade[0]);
    let x01 = lerp(corner(0, 0, 1), corner(1, 0, 1), fade[0]);
    let x11 = lerp(corner(0, 1, 1), corner(1, 1, 1), fade[0]);
    lerp(lerp(x00, x10, fade[1]), lerp(x01, x11, fade[1]), fade[2])
}

/// Worley fBm from `cells` up, three octaves weighted as Nubis's (0.625, 0.25, 0.125).
fn worley_fbm(p: [f32; 3], cells: u32) -> f32 {
    worley(p, cells, cells) * 0.625
        + worley(p, cells * 2, cells * 2) * 0.25
        + worley(p, cells * 4, cells * 4) * 0.125
}

/// The texel at `p` (0 to 1 across the tile): Perlin-Worley, then Worley fBm coarse to fine.
pub fn texel(p: [f32; 3]) -> [f32; 4] {
    let fbm = perlin(p, 4, 101) * 0.5 + perlin(p, 8, 102) * 0.25 + perlin(p, 16, 103) * 0.125;
    // About -0.6 to 0.6 in practice: to 0 to 1.
    let perlin01 = (fbm / 0.875 * 0.75 + 0.5).clamp(0.0, 1.0);
    let w = worley_fbm(p, 4);
    let perlin_worley = w + perlin01 * (1.0 - w);
    [perlin_worley, w, worley_fbm(p, 8), worley_fbm(p, 16)]
}

/// The texture: Rgba8 texels, x fastest then y then z, every mip level after the one before.
pub fn bake() -> Vec<u8> {
    let side = SIDE as usize;
    let slices: Vec<usize> = (0..side).collect();
    let top: Vec<[f32; 4]> = terra_gen::par::map(&slices, |&z| {
        let mut slice = Vec::with_capacity(side * side);
        for y in 0..side {
            for x in 0..side {
                let p = [x, y, z].map(|v| (v as f32 + 0.5) / SIDE as f32);
                slice.push(texel(p));
            }
        }
        slice
    })
    .concat();
    let mut levels = vec![top];
    let mut s = side;
    while s > 1 {
        let half = s / 2;
        let above = levels.last().expect("the level above");
        let mut level = Vec::with_capacity(half * half * half);
        for z in 0..half {
            for y in 0..half {
                for x in 0..half {
                    let mut sum = [0.0; 4];
                    for (dx, dy, dz) in (0..8).map(|i| (i & 1, (i >> 1) & 1, i >> 2)) {
                        let t = above[(2 * z + dz) * s * s + (2 * y + dy) * s + 2 * x + dx];
                        (0..4).for_each(|c| sum[c] += t[c] / 8.0);
                    }
                    level.push(sum);
                }
            }
        }
        levels.push(level);
        s = half;
    }
    levels
        .iter()
        .flatten()
        .flat_map(|t| t.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Points spread through the tile, the same every run.
    fn points() -> Vec<[f32; 3]> {
        (0..200u32)
            .map(|i| [1, 2, 3].map(|s| hash::unit3(7, i, s, 0)))
            .collect()
    }

    #[test]
    fn each_noise_repeats_over_the_tile_so_the_texture_wraps_with_no_seam() {
        for p in points() {
            let moved = [p[0] + 1.0, p[1] - 1.0, p[2] + 2.0];
            for cells in [4, 8, 16] {
                assert!((worley(p, cells, 3) - worley(moved, cells, 3)).abs() < 1e-4);
                assert!((perlin(p, cells, 3) - perlin(moved, cells, 3)).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn worley_is_one_on_a_feature_point_and_falls_away_from_it() {
        // A cell's feature point, found by the same hash the noise places it with.
        let s = SEED ^ 5u32.wrapping_mul(0x9e37_79b9);
        let (i, j, k) = (1, 2, 3);
        let at = [
            (i as f32 + hash::unit3(s, i, j, k)) / 4.0,
            (j as f32 + hash::unit3(s.wrapping_add(1), i, j, k)) / 4.0,
            (k as f32 + hash::unit3(s.wrapping_add(2), i, j, k)) / 4.0,
        ];
        assert!(worley(at, 4, 5) > 0.999);
        let away = [at[0] + 0.1, at[1], at[2]];
        assert!(worley(away, 4, 5) < worley(at, 4, 5));
    }

    #[test]
    fn the_shape_spans_thin_air_to_thick_cloud() {
        let shapes: Vec<f32> = points().into_iter().map(|p| texel(p)[0]).collect();
        let lo = shapes.iter().copied().fold(f32::MAX, f32::min);
        let hi = shapes.iter().copied().fold(f32::MIN, f32::max);
        assert!(lo < 0.45 && hi > 0.75, "the shape spans {lo} to {hi}");
    }
}
