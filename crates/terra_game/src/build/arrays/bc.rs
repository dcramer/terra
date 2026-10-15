//! Block compression for the construction materials' texture arrays (D290): BC7 for colour,
//! BC1 for occlusion-roughness-metal, BC4 for height and BC5 for a normal's two components,
//! the formats every desktop GPU samples directly, as shipped games store their textures:
//! BC7 colour and BC5 normals are the industry's standard for a material's main maps, BC1
//! colour's four shades a block of 5:6:5 its cheap tier (research/texture-detail-2026-10-15.md).
//! Each texture set is compressed once, as it is made resident. BC1 and BC4 are the real-time
//! encoders: each block's colours from the corners of the box its pixels span, inset a little,
//! each pixel the nearest of the four or eight between (J.M.P. van Waveren, "Real-Time DXT
//! Compression", Intel 2006). BC7 is its mode 6 alone, one line through RGBA at 7 bits and a
//! shared bit an end, sixteen steps along it: its endpoints along the block's principal axis,
//! then fitted by least squares to the steps chosen, as Rich Geldreich's bc7enc does it
//! (MIT or public domain), mode 6 being the one most of a natural texture's blocks take.

/// How many mipmap levels a picture `side` square has, down to 4 by 4, the least a block
/// covers.
pub fn levels(side: u32) -> u32 {
    side.max(4).ilog2() - 1
}

/// How a map's smaller copies are averaged: colour in linear light, as the screen adds it
/// (averaging its sRGB values darkens every level), a normal map's directions made unit
/// again, anything else as stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mips {
    Linear,
    Srgb,
    Normal,
}

/// sRGB's transfer, a byte to linear light.
fn to_linear(v: u8) -> f32 {
    let c = f32::from(v) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        libm::powf((c + 0.055) / 1.055, 2.4)
    }
}

fn to_srgb(l: f32) -> u8 {
    let c = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * libm::powf(l, 1.0 / 2.4) - 0.055
    };
    (c * 255.0).round().clamp(0.0, 255.0) as u8
}

/// An RGBA picture `side` square and each smaller copy to 4 by 4, each pixel the mean of the
/// four under it, averaged as `mips` says.
pub fn chain(pixels: &[u8], side: u32, mips: Mips) -> Vec<Vec<u8>> {
    let linear: [f32; 256] = std::array::from_fn(|v| to_linear(v as u8));
    let mut out = vec![pixels.to_vec()];
    let mut size = side as usize;
    for _ in 1..levels(side) {
        let last = out.last().map_or(&[][..], Vec::as_slice);
        let half = size / 2;
        let mut next = vec![0u8; half * half * 4];
        for y in 0..half {
            for x in 0..half {
                let texel = |c: usize| {
                    [(0, 0), (1, 0), (0, 1), (1, 1)]
                        .map(|(dx, dy)| last[((y * 2 + dy) * size + x * 2 + dx) * 4 + c])
                };
                let at = (y * half + x) * 4;
                match mips {
                    Mips::Linear => {
                        for c in 0..4 {
                            let sum: u32 = texel(c).iter().map(|&v| u32::from(v)).sum();
                            next[at + c] = ((sum + 2) / 4) as u8;
                        }
                    }
                    Mips::Srgb => {
                        for c in 0..3 {
                            let sum: f32 = texel(c).iter().map(|&v| linear[usize::from(v)]).sum();
                            next[at + c] = to_srgb(sum / 4.0);
                        }
                        let sum: u32 = texel(3).iter().map(|&v| u32::from(v)).sum();
                        next[at + 3] = ((sum + 2) / 4) as u8;
                    }
                    Mips::Normal => {
                        let mut n = [0f32; 3];
                        for (c, axis) in n.iter_mut().enumerate() {
                            *axis = texel(c)
                                .iter()
                                .map(|&v| f32::from(v) / 127.5 - 1.0)
                                .sum::<f32>();
                        }
                        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
                        for c in 0..3 {
                            next[at + c] =
                                ((n[c] / len + 1.0) * 127.5).round().clamp(0.0, 255.0) as u8;
                        }
                        next[at + 3] = 255;
                    }
                }
            }
        }
        out.push(next);
        size = half;
    }
    out
}

/// The 16 pixels of the block at (bx, by), RGBA.
fn block(pixels: &[u8], side: usize, bx: usize, by: usize) -> [[u8; 4]; 16] {
    let mut out = [[0u8; 4]; 16];
    for (i, p) in out.iter_mut().enumerate() {
        let at = ((by * 4 + i / 4) * side + bx * 4 + i % 4) * 4;
        p.copy_from_slice(&pixels[at..at + 4]);
    }
    out
}

fn to565(c: [i32; 3]) -> u16 {
    let r = (c[0].clamp(0, 255) * 31 + 127) / 255;
    let g = (c[1].clamp(0, 255) * 63 + 127) / 255;
    let b = (c[2].clamp(0, 255) * 31 + 127) / 255;
    ((r << 11) | (g << 5) | b) as u16
}

fn from565(c: u16) -> [i32; 3] {
    let r = i32::from((c >> 11) & 31);
    let g = i32::from((c >> 5) & 63);
    let b = i32::from(c & 31);
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    ]
}

/// One BC1 block in its four-colour mode.
fn bc1_block(px: &[[u8; 4]; 16]) -> [u8; 8] {
    let mut lo = [255i32; 3];
    let mut hi = [0i32; 3];
    let mut sum = [0i32; 3];
    for p in px {
        for c in 0..3 {
            let v = i32::from(p[c]);
            lo[c] = lo[c].min(v);
            hi[c] = hi[c].max(v);
            sum[c] += v;
        }
    }
    // The box's diagonal the colours follow: green or blue falling as red rises runs the
    // other way.
    let mean = sum.map(|m| m / 16);
    for c in 1..3 {
        let covariance: i32 = px
            .iter()
            .map(|p| (i32::from(p[0]) - mean[0]) * (i32::from(p[c]) - mean[c]))
            .sum();
        if covariance < 0 {
            std::mem::swap(&mut lo[c], &mut hi[c]);
        }
    }
    // Inset by a sixteenth of the span, as the palette's ends are rarely met.
    for c in 0..3 {
        let inset = (hi[c] - lo[c]) / 16;
        hi[c] -= inset;
        lo[c] += inset;
    }
    let (mut c0, mut c1) = (to565(hi), to565(lo));
    if c0 < c1 {
        std::mem::swap(&mut c0, &mut c1);
    }
    let mut out = [0u8; 8];
    out[0..2].copy_from_slice(&c0.to_le_bytes());
    out[2..4].copy_from_slice(&c1.to_le_bytes());
    if c0 == c1 {
        return out;
    }
    let (a, b) = (from565(c0), from565(c1));
    let palette = [
        a,
        b,
        [0, 1, 2].map(|c| (2 * a[c] + b[c]) / 3),
        [0, 1, 2].map(|c| (a[c] + 2 * b[c]) / 3),
    ];
    let mut indices = 0u32;
    for (i, p) in px.iter().enumerate() {
        let mut best = (i32::MAX, 0u32);
        for (k, q) in palette.iter().enumerate() {
            let d: i32 = (0..3).map(|c| (i32::from(p[c]) - q[c]).pow(2)).sum();
            if d < best.0 {
                best = (d, k as u32);
            }
        }
        indices |= best.1 << (i * 2);
    }
    out[4..8].copy_from_slice(&indices.to_le_bytes());
    out
}

/// One BC4 block of a channel, in its eight-value mode.
fn bc4_block(px: &[[u8; 4]; 16], channel: usize) -> [u8; 8] {
    let values: [i32; 16] = std::array::from_fn(|i| i32::from(px[i][channel]));
    let lo = values.iter().copied().min().unwrap_or(0);
    let hi = values.iter().copied().max().unwrap_or(0);
    let mut out = [0u8; 8];
    out[0] = hi as u8;
    out[1] = lo as u8;
    if hi == lo {
        return out;
    }
    let palette: [i32; 8] = std::array::from_fn(|k| match k {
        0 => hi,
        1 => lo,
        k => ((8 - k as i32) * hi + (k as i32 - 1) * lo) / 7,
    });
    let mut bits = 0u64;
    for (i, v) in values.iter().enumerate() {
        let k = (0..8).min_by_key(|&k| (palette[k] - v).abs()).unwrap_or(0) as u64;
        bits |= k << (i * 3);
    }
    out[2..8].copy_from_slice(&bits.to_le_bytes()[..6]);
    out
}

/// BC7 mode 6's sixteen steps along its line, in 64ths.
const BC7_WEIGHTS: [i32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

/// An end of a BC7 mode 6 line as stored: each channel's top 7 bits, and the low bit its
/// four channels share.
#[derive(Clone, Copy, Debug, PartialEq)]
struct End {
    c: [i32; 4],
    p: i32,
}

impl End {
    /// The nearest stored end to a colour, its shared bit whichever is nearer.
    fn of(v: [f32; 4]) -> End {
        let with = |p: i32| {
            let c = v.map(|x| ((x - p as f32) / 2.0).round().clamp(0.0, 127.0) as i32);
            let err: f32 = (0..4)
                .map(|i| (((c[i] << 1) | p) as f32 - v[i]).powi(2))
                .sum();
            (End { c, p }, err)
        };
        let (a, b) = (with(0), with(1));
        if a.1 <= b.1 { a.0 } else { b.0 }
    }

    fn value(self) -> [i32; 4] {
        self.c.map(|c| (c << 1) | self.p)
    }
}

/// The sixteen colours along a mode 6 line.
fn bc7_palette(a: End, b: End) -> [[i32; 4]; 16] {
    let (a, b) = (a.value(), b.value());
    BC7_WEIGHTS.map(|w| std::array::from_fn(|c| ((64 - w) * a[c] + w * b[c] + 32) >> 6))
}

/// Each pixel's nearest step on the line, and the block's squared error.
fn bc7_steps(px: &[[u8; 4]; 16], a: End, b: End) -> ([usize; 16], i64) {
    let palette = bc7_palette(a, b);
    let mut steps = [0usize; 16];
    let mut total = 0i64;
    for (i, p) in px.iter().enumerate() {
        let mut best = (i64::MAX, 0);
        for (k, q) in palette.iter().enumerate() {
            let d: i64 = (0..4)
                .map(|c| i64::from(i32::from(p[c]) - q[c]).pow(2))
                .sum();
            if d < best.0 {
                best = (d, k);
            }
        }
        steps[i] = best.1;
        total += best.0;
    }
    (steps, total)
}

/// One BC7 block in mode 6 (see the module's doc).
fn bc7_block(px: &[[u8; 4]; 16]) -> [u8; 16] {
    let p: [[f32; 4]; 16] = px.map(|q| q.map(f32::from));
    let mean: [f32; 4] = std::array::from_fn(|c| p.iter().map(|q| q[c]).sum::<f32>() / 16.0);
    // The principal axis, by power iteration on the covariance, from the box's diagonal.
    let mut cov = [[0f32; 4]; 4];
    for q in &p {
        for i in 0..4 {
            for j in 0..4 {
                cov[i][j] += (q[i] - mean[i]) * (q[j] - mean[j]);
            }
        }
    }
    let mut axis: [f32; 4] = std::array::from_fn(|c| {
        let (lo, hi) = p.iter().fold((f32::MAX, f32::MIN), |(lo, hi), q| {
            (lo.min(q[c]), hi.max(q[c]))
        });
        hi - lo
    });
    for _ in 0..8 {
        let next: [f32; 4] = std::array::from_fn(|i| (0..4).map(|j| cov[i][j] * axis[j]).sum());
        let len = next.iter().map(|v| v * v).sum::<f32>().sqrt();
        if len < 1e-6 {
            break;
        }
        axis = next.map(|v| v / len);
    }
    let along: Vec<f32> = p
        .iter()
        .map(|q| (0..4).map(|c| (q[c] - mean[c]) * axis[c]).sum())
        .collect();
    let lo = along.iter().copied().fold(f32::MAX, f32::min);
    let hi = along.iter().copied().fold(f32::MIN, f32::max);
    let end = |t: f32| {
        End::of(std::array::from_fn(|c| {
            (mean[c] + t * axis[c]).clamp(0.0, 255.0)
        }))
    };
    let mut ends = (end(lo), end(hi));
    let (mut steps, mut error) = bc7_steps(px, ends.0, ends.1);
    // The ends fitted by least squares to the steps chosen, and the steps chosen again.
    for _ in 0..2 {
        if error == 0 {
            break;
        }
        let (mut aa, mut ab, mut bb) = (0f32, 0f32, 0f32);
        let (mut xa, mut xb) = ([0f32; 4], [0f32; 4]);
        for (q, &k) in p.iter().zip(&steps) {
            let w = BC7_WEIGHTS[k] as f32 / 64.0;
            aa += (1.0 - w) * (1.0 - w);
            ab += (1.0 - w) * w;
            bb += w * w;
            for c in 0..4 {
                xa[c] += (1.0 - w) * q[c];
                xb[c] += w * q[c];
            }
        }
        let det = aa * bb - ab * ab;
        if det.abs() < 1e-6 {
            break;
        }
        let fitted = (
            End::of(std::array::from_fn(|c| {
                ((bb * xa[c] - ab * xb[c]) / det).clamp(0.0, 255.0)
            })),
            End::of(std::array::from_fn(|c| {
                ((aa * xb[c] - ab * xa[c]) / det).clamp(0.0, 255.0)
            })),
        );
        let (again, err) = bc7_steps(px, fitted.0, fitted.1);
        if err >= error {
            break;
        }
        (ends, steps, error) = (fitted, again, err);
    }
    // The first pixel's step must have its top bit clear, as it is stored in three bits.
    if steps[0] >= 8 {
        ends = (ends.1, ends.0);
        steps = steps.map(|k| 15 - k);
    }
    let mut bits = 0u128;
    let mut at = 0;
    let mut put = |value: u128, n: u32| {
        bits |= value << at;
        at += n;
    };
    put(1 << 6, 7);
    for c in 0..4 {
        put(ends.0.c[c] as u128, 7);
        put(ends.1.c[c] as u128, 7);
    }
    put(ends.0.p as u128, 1);
    put(ends.1.p as u128, 1);
    for (i, &k) in steps.iter().enumerate() {
        put(k as u128, if i == 0 { 3 } else { 4 });
    }
    bits.to_le_bytes()
}

/// How a map is stored on the GPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    /// BC7: colour; 16 bytes a block of 4 by 4, sixteen shades a block at 7 bits a channel.
    Bc7,
    /// BC1: occlusion-roughness-metal; 8 bytes a block of 4 by 4.
    Bc1,
    /// BC4: the first channel; 8 bytes a block.
    Bc4,
    /// BC5: the first two channels; 16 bytes a block.
    Bc5,
    /// Uncompressed, the first so many channels: where the GPU samples no BC.
    Raw(usize),
}

impl Codec {
    /// The bytes a level `side` square takes.
    pub fn bytes(self, side: u32) -> usize {
        let blocks = (side.div_ceil(4) * side.div_ceil(4)) as usize;
        match self {
            Codec::Bc1 | Codec::Bc4 => blocks * 8,
            Codec::Bc5 | Codec::Bc7 => blocks * 16,
            Codec::Raw(n) => (side * side) as usize * n,
        }
    }

    /// The bytes a row of blocks takes in a level `side` square, and how many rows it has.
    pub fn rows(self, side: u32) -> (u32, u32) {
        match self {
            Codec::Raw(n) => (side * n as u32, side),
            Codec::Bc1 | Codec::Bc4 => (side.div_ceil(4) * 8, side.div_ceil(4)),
            Codec::Bc5 | Codec::Bc7 => (side.div_ceil(4) * 16, side.div_ceil(4)),
        }
    }

    /// One level of RGBA, `side` square, stored.
    pub fn encode(self, pixels: &[u8], side: u32) -> Vec<u8> {
        if let Codec::Raw(n) = self {
            return pixels
                .chunks_exact(4)
                .flat_map(|p| p[..n].iter().copied())
                .collect();
        }
        let blocks = side as usize / 4;
        let mut out = Vec::with_capacity(self.bytes(side));
        for by in 0..blocks {
            for bx in 0..blocks {
                let px = block(pixels, side as usize, bx, by);
                match self {
                    Codec::Bc7 => out.extend(bc7_block(&px)),
                    Codec::Bc1 => out.extend(bc1_block(&px)),
                    Codec::Bc4 => out.extend(bc4_block(&px, 0)),
                    Codec::Bc5 => {
                        out.extend(bc4_block(&px, 0));
                        out.extend(bc4_block(&px, 1));
                    }
                    Codec::Raw(_) => {}
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A BC1 block decoded as a GPU does, in its four-colour mode.
    fn bc1_decode(block: &[u8]) -> [[i32; 3]; 16] {
        let c0 = u16::from_le_bytes([block[0], block[1]]);
        let c1 = u16::from_le_bytes([block[2], block[3]]);
        let (a, b) = (from565(c0), from565(c1));
        let palette = [
            a,
            b,
            [0, 1, 2].map(|c| (2 * a[c] + b[c]) / 3),
            [0, 1, 2].map(|c| (a[c] + 2 * b[c]) / 3),
        ];
        let bits = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
        std::array::from_fn(|i| {
            if c0 == c1 {
                a
            } else {
                palette[((bits >> (i * 2)) & 3) as usize]
            }
        })
    }

    fn bc4_decode(block: &[u8]) -> [i32; 16] {
        let (hi, lo) = (i32::from(block[0]), i32::from(block[1]));
        let mut bytes = [0u8; 8];
        bytes[..6].copy_from_slice(&block[2..8]);
        let bits = u64::from_le_bytes(bytes);
        std::array::from_fn(|i| match ((bits >> (i * 3)) & 7) as i32 {
            0 => hi,
            1 => lo,
            k => ((8 - k) * hi + (k - 1) * lo) / 7,
        })
    }

    /// A gradient across a block, as a texture's detail is.
    fn gradient() -> [[u8; 4]; 16] {
        std::array::from_fn(|i| {
            let t = (i % 4 * 40 + i / 4 * 12) as u8;
            [40 + t, 30 + t / 2, 200 - t, 255]
        })
    }

    #[test]
    fn a_bc1_block_comes_back_near_what_went_in() {
        let px = gradient();
        let back = bc1_decode(&bc1_block(&px));
        for (p, q) in px.iter().zip(back) {
            for c in 0..3 {
                // Four colours on a line across a span of 156: within half a step of one, and the inset.
                assert!((i32::from(p[c]) - q[c]).abs() <= 28, "{p:?} {q:?}");
            }
        }
    }

    #[test]
    fn a_flat_block_comes_back_flat() {
        let px = [[90u8, 120, 60, 255]; 16];
        for q in bc1_decode(&bc1_block(&px)) {
            assert!((q[0] - 90).abs() <= 4 && (q[1] - 120).abs() <= 2 && (q[2] - 60).abs() <= 4);
        }
        assert_eq!(bc4_decode(&bc4_block(&px, 1)), [120; 16]);
    }

    #[test]
    fn a_bc4_block_is_within_a_fourteenth_of_its_span() {
        let px = gradient();
        for (p, q) in px.iter().zip(bc4_decode(&bc4_block(&px, 0))) {
            assert!((i32::from(p[0]) - q).abs() <= 156 / 14 + 1, "{} {q}", p[0]);
        }
    }

    #[test]
    fn levels_run_down_to_a_block() {
        assert_eq!(levels(1024), 9);
        let chain = chain(&vec![200u8; 64 * 64 * 4], 64, Mips::Srgb);
        assert_eq!(chain.len(), 5);
        assert_eq!(chain.last().unwrap().len(), 4 * 4 * 4);
        assert!(chain.iter().all(|level| level.iter().all(|&v| v == 200)));
        assert_eq!(Codec::Bc5.bytes(1024), 1024 * 1024);
        assert_eq!(Codec::Bc7.bytes(2048), 2048 * 2048);
        assert_eq!(Codec::Bc1.encode(&chain[0], 64).len(), Codec::Bc1.bytes(64));
        assert_eq!(Codec::Bc7.encode(&chain[0], 64).len(), Codec::Bc7.bytes(64));
        assert_eq!(Codec::Raw(2).encode(&chain[0], 64).len(), 64 * 64 * 2);
    }

    /// A BC7 block decoded as the format's specification says, for mode 6.
    fn bc7_decode(block: &[u8]) -> [[i32; 4]; 16] {
        let bits = u128::from_le_bytes(block.try_into().unwrap());
        let mut at = 0;
        let mut take = |n: u32| {
            let v = ((bits >> at) & ((1u128 << n) - 1)) as i32;
            at += n;
            v
        };
        assert_eq!(take(7), 64, "mode 6");
        // Each channel's two ends in turn: R0 R1 G0 G1 B0 B1 A0 A1.
        let stored: [i32; 8] = std::array::from_fn(|_| take(7));
        let ends: [[i32; 4]; 2] = [0, 1].map(|e| std::array::from_fn(|c| stored[c * 2 + e]));
        let p = [take(1), take(1)];
        let ends = [0, 1].map(|e| ends[e].map(|c| (c << 1) | p[e]));
        let steps: [i32; 16] = std::array::from_fn(|i| take(if i == 0 { 3 } else { 4 }));
        let w = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];
        steps.map(|k| {
            let w = w[k as usize];
            std::array::from_fn(|c| ((64 - w) * ends[0][c] + w * ends[1][c] + 32) >> 6)
        })
    }

    /// A block of bark or stone: two tones and grain between, made up by a small hash.
    fn textured(seed: u32) -> [[u8; 4]; 16] {
        let mut h = seed.wrapping_mul(2_654_435_761);
        let mut next = || {
            h ^= h << 13;
            h ^= h >> 17;
            h ^= h << 5;
            (h % 1000) as f32 / 1000.0
        };
        let (a, b) = ([next(), next(), next()], [next(), next(), next()]);
        std::array::from_fn(|_| {
            let t = next();
            let grain = next() * 0.15;
            let c: [u8; 3] = std::array::from_fn(|c| {
                ((a[c] * (1.0 - t) + b[c] * t + grain) * 200.0).clamp(0.0, 255.0) as u8
            });
            [c[0], c[1], c[2], 255]
        })
    }

    fn squared(px: &[[u8; 4]; 16], back: impl Iterator<Item = [i32; 3]>) -> i64 {
        px.iter()
            .zip(back)
            .map(|(p, q)| {
                (0..3)
                    .map(|c| i64::from(i32::from(p[c]) - q[c]).pow(2))
                    .sum::<i64>()
            })
            .sum()
    }

    #[test]
    fn a_bc7_block_comes_back_near_what_went_in_and_flat_stays_flat() {
        let px = gradient();
        for (p, q) in px.iter().zip(bc7_decode(&bc7_block(&px))) {
            for c in 0..4 {
                // Sixteen steps along a span of 156: within half a step and the ends' rounding.
                assert!((i32::from(p[c]) - q[c]).abs() <= 7, "{p:?} {q:?}");
            }
        }
        for q in bc7_decode(&bc7_block(&[[90u8, 121, 60, 255]; 16])) {
            assert!((q[0] - 90).abs() <= 1 && (q[1] - 121).abs() <= 1 && (q[2] - 60).abs() <= 1);
        }
    }

    #[test]
    fn bc7_keeps_a_textured_block_far_closer_than_bc1() {
        // What BC7 is for: a natural texture's blocks, many shades along their grain.
        let (mut seven, mut one) = (0i64, 0i64);
        for seed in 1..400 {
            let px = textured(seed);
            seven += squared(
                &px,
                bc7_decode(&bc7_block(&px))
                    .into_iter()
                    .map(|q| [q[0], q[1], q[2]]),
            );
            one += squared(&px, bc1_decode(&bc1_block(&px)).into_iter());
        }
        assert!(seven * 2 < one, "BC7's error {seven}, BC1's {one}");
    }

    #[test]
    fn colour_is_averaged_in_linear_light_and_normals_stay_unit() {
        // Black and white texels: half the light, which sRGB stores as 188, not 128.
        let checker: Vec<u8> = (0..64)
            .flat_map(|i| {
                let v = if (i % 8 + i / 8) % 2 == 0 { 0 } else { 255 };
                [v, v, v, 255]
            })
            .collect();
        let half = &chain(&checker, 8, Mips::Srgb)[1];
        assert!((i32::from(half[0]) - 188).abs() <= 1, "{}", half[0]);
        // Two normals leaning opposite ways average to straight up, at full length.
        let lean = |x: f32| {
            let z = (1.0 - x * x).sqrt();
            [
                ((x + 1.0) * 127.5) as u8,
                128,
                ((z + 1.0) * 127.5) as u8,
                255,
            ]
        };
        let normals: Vec<u8> = (0..64)
            .flat_map(|i| if i % 2 == 0 { lean(0.6) } else { lean(-0.6) })
            .collect();
        let level = &chain(&normals, 8, Mips::Normal)[1];
        let n: Vec<f32> = level[..3]
            .iter()
            .map(|&v| f32::from(v) / 127.5 - 1.0)
            .collect();
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((len - 1.0).abs() < 0.02 && n[2] > 0.98, "{n:?}");
    }
}
