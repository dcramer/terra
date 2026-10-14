//! Block compression for the construction materials' texture arrays (D290): BC1 for colour
//! and occlusion-roughness-metal, BC4 for height and BC5 for a normal's two components, the
//! formats every desktop GPU samples directly at an eighth or a quarter of their size, as
//! shipped games store their textures. Each texture set is compressed once, as it is made
//! resident. The encoders are the real-time ones: each block's colours from the corners of
//! the box its pixels span, inset a little, each pixel the nearest of the four or eight
//! between (J.M.P. van Waveren, "Real-Time DXT Compression", Intel 2006).

/// How many mipmap levels a picture `side` square has, down to 4 by 4, the least a block
/// covers.
pub fn levels(side: u32) -> u32 {
    side.max(4).ilog2() - 1
}

/// An RGBA picture `side` square and each smaller copy to 4 by 4, each pixel the mean of the
/// four under it.
pub fn chain(pixels: &[u8], side: u32) -> Vec<Vec<u8>> {
    let mut out = vec![pixels.to_vec()];
    let mut size = side as usize;
    for _ in 1..levels(side) {
        let last = out.last().map_or(&[][..], Vec::as_slice);
        let half = size / 2;
        let mut next = vec![0u8; half * half * 4];
        for y in 0..half {
            for x in 0..half {
                for c in 0..4 {
                    let at = |dx: usize, dy: usize| {
                        u32::from(last[((y * 2 + dy) * size + x * 2 + dx) * 4 + c])
                    };
                    next[(y * half + x) * 4 + c] =
                        ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8;
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

/// How a map is stored on the GPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    /// BC1: colour, or occlusion-roughness-metal; 8 bytes a block of 4 by 4.
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
            Codec::Bc5 => blocks * 16,
            Codec::Raw(n) => (side * side) as usize * n,
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
        let chain = chain(&vec![200u8; 64 * 64 * 4], 64);
        assert_eq!(chain.len(), 5);
        assert_eq!(chain.last().unwrap().len(), 4 * 4 * 4);
        assert!(chain.iter().all(|level| level.iter().all(|&v| v == 200)));
        assert_eq!(Codec::Bc5.bytes(1024), 1024 * 1024);
        assert_eq!(Codec::Bc1.encode(&chain[0], 64).len(), Codec::Bc1.bytes(64));
        assert_eq!(Codec::Raw(2).encode(&chain[0], 64).len(), 64 * 64 * 2);
    }
}
