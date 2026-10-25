//! What a building's maps would take stored smaller, and what that costs them, measured on
//! the maps themselves (research/2026-10-25-wfc-weathering.md). Uncompressed, the atlas is
//! three RGBA8 layers, 12 bytes a texel, 12 KB a face. The ways down measured:
//!
//! - **BC4** per channel (as BC4 or BC5 textures): 4 bits a texel a channel, each 4x4
//!   block keeping its least and most value and eight steps between (Microsoft's block
//!   compression format, decoded by every GPU); the constant fourth channel of the last
//!   layer dropped, 11 channels left.
//! - **Half-size** maps, 16 texels a side: a 2x2 box down, bilinear back up.
//! - Both together.
//! - **Mixed**: the motif channels (`SHARP`) BC4 at full size, the smooth mask channels
//!   BC4 at half size.
//!
//! Errors are in 0..255 steps over every texel of every channel kept, against the maps as
//! drawn, with the channel that loses most named.

use crate::{TILE, Weathered};

/// The channels kept, in layer order (the last layer's fourth is a constant 1, dropped).
pub const CHANNELS: [&str; 11] = [
    "crack", "loss", "moss", "streak", "grime", "wear", "lichen", "damp", "soot", "cavity", "edge",
];

/// The channels drawn as motifs, sharp-edged (the rest are smooth masks).
pub const SHARP: [bool; 11] = [
    true, true, true, true, false, false, true, false, true, false, false,
];

/// One way of storing the maps: the bytes it takes (tightly packed, the lookup included)
/// and what it costs.
#[derive(Clone, Copy, Debug, Default)]
pub struct Way {
    pub bytes: usize,
    pub rms: f32,
    pub max: f32,
    /// The channel with the largest error, and its error.
    pub worst: &'static str,
    pub worst_rms: f32,
}

/// The maps' bytes as they are and in each way.
#[derive(Clone, Copy, Debug, Default)]
pub struct Report {
    /// As uploaded now (the square atlas, its unused slots included, and the lookup).
    pub raw: usize,
    /// Faces whose every kept channel is within two steps of nought: they could share one
    /// blank slot.
    pub blank: usize,
    pub bc4: Way,
    pub half: Way,
    pub half_bc4: Way,
    pub mixed: Way,
}

impl Weathered {
    /// Measures the ways of storing these maps smaller.
    pub fn compression(&self) -> Report {
        let slots = self.slots as usize;
        let lookup = self.lookup.len() * 4;
        let texels = slots * TILE * TILE;
        let kept = CHANNELS.len();
        let mut errors: [Errors; 3] = Default::default();
        let mut blank = 0;
        for slot in 0..slots {
            let mut empty = true;
            for ch in 0..CHANNELS.len() {
                let tile = self.tile(slot, ch);
                empty &= tile.iter().all(|&v| v <= 2.0);
                let half = down(&tile, TILE);
                errors[0].add(ch, &tile, &bc4(&tile, TILE));
                errors[1].add(ch, &tile, &quantise(up(&half, TILE)));
                // Filtered up by the GPU, never rounded again.
                errors[2].add(ch, &tile, &up(&bc4(&half, TILE / 2), TILE));
            }
            blank += usize::from(empty);
        }
        let sharp = SHARP.iter().filter(|&&s| s).count();
        let mixed = Errors::pick(&errors[0], &errors[2]);
        let [bc4_e, half_e, both_e] = errors;
        Report {
            raw: self.bytes(),
            blank,
            bc4: bc4_e.way(texels * kept / 2 + lookup),
            half: half_e.way(texels / 4 * 12 + lookup),
            half_bc4: both_e.way(texels / 4 * kept / 2 + lookup),
            mixed: mixed.way(texels * sharp / 2 + texels / 4 * (kept - sharp) / 2 + lookup),
        }
    }

    /// One channel of one face's map, row by row.
    fn tile(&self, slot: usize, channel: usize) -> Vec<f32> {
        let per_row = self.per_row as usize;
        let side = self.side();
        let (sx, sy) = (slot % per_row, slot / per_row);
        let layer = &self.atlas[channel / 4];
        (0..TILE * TILE)
            .map(|i| {
                let (x, y) = (i % TILE, i / TILE);
                f32::from(layer[((sy * TILE + y) * side + sx * TILE + x) * 4 + channel % 4])
            })
            .collect()
    }
}

/// Squared errors summed, by channel and over all.
#[derive(Clone, Debug, Default)]
struct Errors {
    sum: [f64; 11],
    count: [usize; 11],
    max: [f32; 11],
}

impl Errors {
    /// The sharp channels from `sharp`, the smooth ones from `smooth`.
    fn pick(sharp: &Errors, smooth: &Errors) -> Errors {
        let mut out = smooth.clone();
        for (c, _) in SHARP.iter().enumerate().filter(|(_, s)| **s) {
            out.sum[c] = sharp.sum[c];
            out.count[c] = sharp.count[c];
            out.max[c] = sharp.max[c];
        }
        out
    }

    fn add(&mut self, channel: usize, was: &[f32], now: &[f32]) {
        for (a, b) in was.iter().zip(now) {
            let d = (a - b).abs();
            self.sum[channel] += f64::from(d * d);
            self.max[channel] = self.max[channel].max(d);
        }
        self.count[channel] += was.len();
    }

    fn way(&self, bytes: usize) -> Way {
        let rms = |sum: f64, count: usize| libm::sqrt(sum / count.max(1) as f64) as f32;
        let (worst, worst_rms) = (0..CHANNELS.len())
            .map(|c| (CHANNELS[c], rms(self.sum[c], self.count[c])))
            .fold(("", 0.0), |a, b| if b.1 > a.1 { b } else { a });
        Way {
            bytes,
            rms: rms(self.sum.iter().sum(), self.count.iter().sum()),
            max: self.max.iter().copied().fold(0.0, f32::max),
            worst,
            worst_rms,
        }
    }
}

/// A tile `n` a side through BC4 and back (`crate::bc4`): rounded to bytes, encoded, and
/// decoded as the GPU does, unrounded.
pub fn bc4(tile: &[f32], n: usize) -> Vec<f32> {
    let bytes: Vec<u8> = tile.iter().map(|&v| byte(v)).collect();
    crate::bc4::decode(&crate::bc4::encode(&bytes, n, n), n, n)
}

/// A value in 0..255 steps as a byte, rounded.
fn byte(v: f32) -> u8 {
    libm::roundf(v).clamp(0.0, 255.0) as u8
}

/// The channels the material samples at full size (the `SHARP` ones) and at half size,
/// by their place in `CHANNELS`.
pub const MOTIF: [usize; 6] = [0, 1, 2, 3, 6, 8];
pub const MASKS: [usize; 5] = [4, 5, 7, 9, 10];

/// The maps as the material samples them, the mixed way: a plane a channel; the motif
/// channels' (`MOTIF`) `side` texels square, each face's map where it is in the atlas; the
/// mask channels' (`MASKS`) half that, each face's map halved by a 2x2 box.
#[derive(Clone, Debug, Default)]
pub struct Planes {
    pub side: usize,
    /// `MOTIF.len()` planes, `side * side` bytes each, row by row.
    pub motif: Vec<u8>,
    /// `MASKS.len()` planes, `(side / 2)^2` bytes each.
    pub masks: Vec<u8>,
}

impl Planes {
    /// Both arrays' planes in BC4, each plane's blocks after the one before's.
    pub fn bc4(&self) -> (Vec<u8>, Vec<u8>) {
        let encode = |planes: &[u8], side: usize| -> Vec<u8> {
            planes
                .chunks_exact(side * side)
                .flat_map(|plane| crate::bc4::encode(plane, side, side))
                .collect()
        };
        (
            encode(&self.motif, self.side),
            encode(&self.masks, self.side / 2),
        )
    }
}

impl Weathered {
    /// The maps in the mixed layout's planes (`Planes`), not yet encoded.
    pub fn planes(&self) -> Planes {
        let side = self.side();
        let (half, h) = (side / 2, TILE / 2);
        let per_row = self.per_row as usize;
        let mut motif = Vec::with_capacity(MOTIF.len() * side * side);
        for &ch in &MOTIF {
            let layer = &self.atlas[ch / 4];
            motif.extend((0..side * side).map(|i| layer[i * 4 + ch % 4]));
        }
        let mut masks = vec![0u8; MASKS.len() * half * half];
        for (plane, &ch) in masks.chunks_exact_mut(half * half).zip(&MASKS) {
            for slot in 0..self.slots as usize {
                let (sx, sy) = (slot % per_row, slot / per_row);
                for (i, v) in down(&self.tile(slot, ch), TILE).into_iter().enumerate() {
                    plane[(sy * h + i / h) * half + sx * h + i % h] = byte(v);
                }
            }
        }
        Planes { side, motif, masks }
    }
}

/// A tile `n` a side halved by a 2x2 box.
pub fn down(tile: &[f32], n: usize) -> Vec<f32> {
    let h = n / 2;
    (0..h * h)
        .map(|i| {
            let (x, y) = (2 * (i % h), 2 * (i / h));
            (tile[y * n + x]
                + tile[y * n + x + 1]
                + tile[(y + 1) * n + x]
                + tile[(y + 1) * n + x + 1])
                / 4.0
        })
        .collect()
}

/// A half tile back up to `n` a side, bilinear between texel middles, clamped at its edges
/// (a face's map is sampled half a texel in, so faces do not bleed).
pub fn up(half: &[f32], n: usize) -> Vec<f32> {
    let h = n / 2;
    let last = (h - 1) as f32;
    (0..n * n)
        .map(|i| {
            let fx = ((i % n) as f32 * 0.5 - 0.25).clamp(0.0, last);
            let fy = ((i / n) as f32 * 0.5 - 0.25).clamp(0.0, last);
            let (x0, y0) = (fx as usize, fy as usize);
            let (x1, y1) = ((x0 + 1).min(h - 1), (y0 + 1).min(h - 1));
            let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
            let top = half[y0 * h + x0] * (1.0 - tx) + half[y0 * h + x1] * tx;
            let bottom = half[y1 * h + x0] * (1.0 - tx) + half[y1 * h + x1] * tx;
            top * (1.0 - ty) + bottom * ty
        })
        .collect()
}

/// Values rounded to bytes, as a texture stores them.
fn quantise(tile: Vec<f32>) -> Vec<f32> {
    tile.into_iter().map(libm::roundf).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the material samples, decoded from the bytes uploaded and read back face by
    /// face as the shader does (the masks bilinear up), against the maps as drawn: the
    /// error's rms and max.
    fn uploaded_error(w: &Weathered) -> (f32, f32) {
        let planes = w.planes();
        let (motif, masks) = planes.bc4();
        let (side, half, h) = (planes.side, planes.side / 2, TILE / 2);
        assert_eq!(motif.len(), MOTIF.len() * side * side / 2);
        assert_eq!(masks.len(), MASKS.len() * half * half / 2);
        let decode = |bytes: &[u8], side: usize| -> Vec<Vec<f32>> {
            bytes
                .chunks_exact(side * side / 2)
                .map(|p| crate::bc4::decode(p, side, side))
                .collect()
        };
        let (motif, masks) = (decode(&motif, side), decode(&masks, half));
        let per_row = w.per_row as usize;
        let (mut sum, mut count, mut max) = (0.0f64, 0usize, 0.0f32);
        for slot in 0..w.slots as usize {
            let (sx, sy) = (slot % per_row, slot / per_row);
            for ch in 0..CHANNELS.len() {
                let back = match MOTIF.iter().position(|&c| c == ch) {
                    Some(m) => (0..TILE * TILE)
                        .map(|i| motif[m][(sy * TILE + i / TILE) * side + sx * TILE + i % TILE])
                        .collect(),
                    None => {
                        let m = MASKS.iter().position(|&c| c == ch).expect("a mask");
                        let small: Vec<f32> = (0..h * h)
                            .map(|i| masks[m][(sy * h + i / h) * half + sx * h + i % h])
                            .collect();
                        up(&small, TILE)
                    }
                };
                for (a, b) in w.tile(slot, ch).iter().zip(&back) {
                    let d = (a - b).abs();
                    sum += f64::from(d * d);
                    max = max.max(d);
                    count += 1;
                }
            }
        }
        (libm::sqrt(sum / count as f64) as f32, max)
    }

    #[test]
    fn what_is_uploaded_decodes_to_the_error_the_mixed_way_measured() {
        // A stone wall 6 cubes long and 4 high, standing alone.
        let wall = |at: glam::IVec3| {
            if (0..=5).contains(&at.x) && (0..=3).contains(&at.y) && at.z == 0 {
                crate::Stuff::Stone
            } else {
                crate::Stuff::Air
            }
        };
        let settings = crate::Settings {
            mode: crate::Mode::Wfc,
            age: 50.0,
            seed: 3,
            tries: 10,
        };
        let w = crate::weather(
            &wall,
            glam::IVec3::new(-1, 0, -1),
            glam::IVec3::new(6, 4, 1),
            &settings,
        );
        let (rms, max) = uploaded_error(&w);
        let measured = w.compression().mixed;
        assert!(
            (rms - measured.rms).abs() < 1e-3,
            "{rms} vs {}",
            measured.rms
        );
        assert!(
            (max - measured.max).abs() < 1e-3,
            "{max} vs {}",
            measured.max
        );
        assert!(rms > 0.0 && rms < 5.0, "{rms}");
    }

    #[test]
    fn half_size_keeps_smooth_maps_and_loses_thin_lines() {
        let n = 32;
        let flat = vec![120.0; n * n];
        assert_eq!(quantise(up(&down(&flat, n), n)), flat);
        // A one-texel line keeps at most about half its strength.
        let line: Vec<f32> = (0..n * n)
            .map(|i| if i % n == 9 { 255.0 } else { 0.0 })
            .collect();
        let back = up(&down(&line, n), n);
        let peak = (0..n).map(|x| back[5 * n + x]).fold(0.0, f32::max);
        assert!(peak < 160.0 && peak > 60.0, "{peak}");
    }
}
