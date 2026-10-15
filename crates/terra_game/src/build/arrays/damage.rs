//! The damage mask every material shares (D290, D305): a texture set made once, by rule, a
//! layer of the texture arrays, that a damaged cube's surfaces reveal by how much of its
//! health it has lost, whatever its material, as engines draw damage with a shared mask and
//! a threshold per pixel (Space Engineers' deformation masks, Unreal's damage decals):
//! hairline cracks first, then the cracks opening and chips knocked out along them, then
//! whole flakes gone. Its height is each texel's threshold, how much must be lost before it
//! breaks (low first along the cracks); its normal the crack's walls and the chips' scars.

use super::SIDE;

/// How many cells of cracks across a copy of the mask, at its two scales.
const CELLS: [usize; 2] = [5, 13];

/// A number from 0 to 1 for a cell, the same every run.
fn hash(x: i64, y: i64, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x85EB_CA77)
        ^ salt.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// How far a point (in cells, wrapping round `cells`) is from the nearest edge between two
/// of a tiling's Voronoi cells, in cells: a crack network that tiles.
fn edge(px: f32, py: f32, cells: usize, salt: u32) -> f32 {
    let n = cells as i64;
    let (cx, cy) = (px.floor() as i64, py.floor() as i64);
    let mut near = [(f32::MAX, 0.0f32, 0.0f32); 2];
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (gx, gy) = (cx + dx, cy + dy);
            let (wx, wy) = (gx.rem_euclid(n), gy.rem_euclid(n));
            let sx = gx as f32 + 0.15 + 0.7 * hash(wx, wy, salt);
            let sy = gy as f32 + 0.15 + 0.7 * hash(wx, wy, salt + 7);
            let d = (sx - px).powi(2) + (sy - py).powi(2);
            if d < near[0].0 {
                near[1] = near[0];
                near[0] = (d, sx, sy);
            } else if d < near[1].0 {
                near[1] = (d, sx, sy);
            }
        }
    }
    // Distance to the bisector between the two nearest sites.
    let (a, b) = (near[0], near[1]);
    let (mx, my) = ((a.1 + b.1) * 0.5, (a.2 + b.2) * 0.5);
    let (nx, ny) = (b.1 - a.1, b.2 - a.2);
    let len = (nx * nx + ny * ny).sqrt().max(1e-6);
    ((px - mx) * nx + (py - my) * ny).abs() / len
}

/// Smooth value noise that tiles over `cells`.
fn noise(px: f32, py: f32, cells: usize, salt: u32) -> f32 {
    let n = cells as i64;
    let (x0, y0) = (px.floor(), py.floor());
    let (tx, ty) = (px - x0, py - y0);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let at = |x: i64, y: i64| hash(x.rem_euclid(n), y.rem_euclid(n), salt);
    let (x0, y0) = (x0 as i64, y0 as i64);
    let top = at(x0, y0) + (at(x0 + 1, y0) - at(x0, y0)) * s(tx);
    let bottom = at(x0, y0 + 1) + (at(x0 + 1, y0 + 1) - at(x0, y0 + 1)) * s(tx);
    top + (bottom - top) * s(ty)
}

/// Each texel's threshold, from 0.1 (breaks at the first quarter lost) to 1 (only when the
/// cube is all but gone), `side` square, tiling.
pub fn thresholds(side: usize) -> Vec<f32> {
    let mut out = vec![1.0f32; side * side];
    for y in 0..side {
        for x in 0..side {
            let (u, v) = (x as f32 / side as f32, y as f32 / side as f32);
            // Wobble the cracks so they are not straight.
            let wob = |c: usize| {
                let k = c as f32;
                (
                    u * k + 0.35 * (noise(u * 8.0, v * 8.0, 8, 3) - 0.5),
                    v * k + 0.35 * (noise(u * 8.0, v * 8.0, 8, 5) - 0.5),
                )
            };
            let (ax, ay) = wob(CELLS[0]);
            let (bx, by) = wob(CELLS[1]);
            // The big cracks open first and widest; the fine ones join them later.
            let big = edge(ax, ay, CELLS[0], 11) / 0.06;
            let fine = 0.35 + edge(bx, by, CELLS[1], 23) / 0.08;
            let crack = big.min(fine);
            // Chips: blobs knocked out along and between the cracks, later on.
            let chip = 0.55 + 0.9 * (1.0 - noise(u * 9.0, v * 9.0, 9, 41)).powi(2)
                - 0.25 * noise(u * 31.0, v * 31.0, 31, 43);
            let t = (0.1 + 0.9 * crack.min(1.0)).min(chip.max(0.45));
            out[y * side + x] = t.clamp(0.1, 1.0);
        }
    }
    out
}

/// The mask as the arrays' four maps, RGBA a texel: colour and ARM neutral (the shader
/// darkens and roughens what breaks), the normal from the thresholds' slopes (each crack's
/// walls), the height the thresholds.
pub fn maps() -> [Vec<u8>; 4] {
    let side = SIDE as usize;
    let t = thresholds(side);
    let at = |x: usize, y: usize| t[(y % side) * side + (x % side)];
    let mut colour = vec![255u8; side * side * 4];
    let mut normal = vec![0u8; side * side * 4];
    let mut arm = vec![255u8; side * side * 4];
    let mut height = vec![0u8; side * side * 4];
    for y in 0..side {
        for x in 0..side {
            let i = (y * side + x) * 4;
            // Lower thresholds are deeper in the crack: the slope of the threshold is the
            // slope of its walls.
            let dx = at(x + 1, y) - at(x + side - 1, y);
            let dy = at(x, y + 1) - at(x, y + side - 1);
            let (nx, ny) = (dx * 6.0, dy * 6.0);
            let nz = 1.0f32;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            normal[i] = ((nx / len * 0.5 + 0.5) * 255.0) as u8;
            normal[i + 1] = ((-ny / len * 0.5 + 0.5) * 255.0) as u8;
            normal[i + 2] = ((nz / len * 0.5 + 0.5) * 255.0) as u8;
            normal[i + 3] = 255;
            let v = (at(x, y) * 255.0) as u8;
            height[i..i + 4].copy_from_slice(&[v, v, v, 255]);
            colour[i + 3] = 255;
            arm[i + 2] = 0;
        }
    }
    [colour, normal, arm, height]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn more_breaks_as_more_is_lost_cracks_first_and_it_tiles() {
        let side = 128;
        let t = thresholds(side);
        let broken = |lost: f32| t.iter().filter(|&&v| v < lost).count() as f32 / t.len() as f32;
        let bands = [0.25, 0.5, 0.75, 1.01].map(broken);
        // Nothing breaks with nothing lost; a hairline's worth at a quarter; most of it when
        // the cube is all but gone; never less with more lost.
        assert_eq!(broken(0.0), 0.0);
        assert!(bands[0] > 0.02 && bands[0] < 0.3, "{bands:?}");
        assert!(bands[3] > 0.95, "{bands:?}");
        assert!(bands.windows(2).all(|w| w[0] <= w[1]), "{bands:?}");
        // Its opposite edges agree, as it tiles across every surface.
        let gap = |a: usize, b: usize| {
            (0..side)
                .map(|y| (t[y * side + a] - t[y * side + b]).abs())
                .sum::<f32>()
                / side as f32
        };
        let (edge, inner) = (gap(0, side - 1), gap(side / 2, side / 2 - 1));
        assert!(edge < inner * 2.5 + 0.02, "{edge} {inner}");
    }
}
