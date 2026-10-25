//! BC4 (UNORM), one channel in 4x4 blocks of 8 bytes, as every GPU samples it: two endpoint
//! bytes, then sixteen 3-bit indices, the block's texels row by row, the first in the lowest
//! bits of the 48-bit little-endian word. When the first endpoint is greater, the indices
//! name it (0), the second (1) and six steps between (2-7, from the first toward the second);
//! otherwise four steps, 0 and 255. Layout per Microsoft's block compression docs (Direct3D
//! 10 "BC4"); the encoder chooses as stb_dxt's `stb__CompressAlphaBlock` does (Sean Barrett,
//! public domain or MIT): the block's most as the first endpoint, its least as the second,
//! each texel the nearest of the eight values between, so a block's error is at most half a
//! step, its range over fourteen.

/// Bytes a 4x4 block takes.
pub const BLOCK: usize = 8;

/// One block's sixteen texels, row by row, to its eight bytes.
pub fn encode_block(texels: &[u8; 16]) -> [u8; BLOCK] {
    let hi = texels.iter().copied().max().unwrap_or(0);
    let lo = texels.iter().copied().min().unwrap_or(0);
    let mut bits = 0u64;
    if hi > lo {
        let range = f32::from(hi - lo);
        for (k, &v) in texels.iter().enumerate() {
            // Steps up from the least, 0..=7; the most is index 0, the least 1, step s 8 - s.
            let step = libm::roundf(f32::from(v - lo) / range * 7.0) as u64;
            let index = match step {
                7 => 0,
                0 => 1,
                s => 8 - s,
            };
            bits |= index << (3 * k);
        }
    }
    let mut out = [0; BLOCK];
    out[0] = hi;
    out[1] = lo;
    out[2..].copy_from_slice(&bits.to_le_bytes()[..6]);
    out
}

/// One block's eight bytes back to its sixteen texels, in 0..255 steps unrounded (as the
/// GPU filters them).
pub fn decode_block(block: &[u8; BLOCK]) -> [f32; 16] {
    let (r0, r1) = (f32::from(block[0]), f32::from(block[1]));
    let mut word = [0u8; 8];
    word[..6].copy_from_slice(&block[2..]);
    let bits = u64::from_le_bytes(word);
    let value = |i: u64| -> f32 {
        match i {
            0 => r0,
            1 => r1,
            i if block[0] > block[1] => ((8 - i) as f32 * r0 + (i - 1) as f32 * r1) / 7.0,
            6 => 0.0,
            7 => 255.0,
            i => ((6 - i) as f32 * r0 + (i - 1) as f32 * r1) / 5.0,
        }
    };
    std::array::from_fn(|k| value((bits >> (3 * k)) & 7))
}

/// An image `w` by `h` (both multiples of 4), row by row, to its blocks row by row.
pub fn encode(image: &[u8], w: usize, h: usize) -> Vec<u8> {
    debug_assert!(w.is_multiple_of(4) && h.is_multiple_of(4) && image.len() == w * h);
    let mut out = Vec::with_capacity(w * h / 2);
    for by in (0..h).step_by(4) {
        for bx in (0..w).step_by(4) {
            let block = std::array::from_fn(|k| image[(by + k / 4) * w + bx + k % 4]);
            out.extend_from_slice(&encode_block(&block));
        }
    }
    out
}

/// Blocks of an image `w` by `h` back to its texels, row by row.
pub fn decode(bytes: &[u8], w: usize, h: usize) -> Vec<f32> {
    let mut out = vec![0.0; w * h];
    for (b, block) in bytes.chunks_exact(BLOCK).enumerate() {
        let (bx, by) = (b % (w / 4) * 4, b / (w / 4) * 4);
        let texels = decode_block(block.try_into().expect("a block is 8 bytes"));
        for (k, v) in texels.into_iter().enumerate() {
            out[(by + k / 4) * w + bx + k % 4] = v;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pattern with flat, ramped, sharp and noisy blocks.
    fn pattern(w: usize, h: usize) -> Vec<u8> {
        (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                match (x / 4 + y / 4) % 4 {
                    0 => 90,
                    1 => (x * 17 + y * 3) as u8,
                    2 => {
                        if (x + y) % 3 == 0 {
                            255
                        } else {
                            0
                        }
                    }
                    _ => ((x * 37 + y * 91) % 256) as u8,
                }
            })
            .collect()
    }

    #[test]
    fn blocks_take_eight_bytes_and_land_where_they_were() {
        let (w, h) = (16, 8);
        let image = pattern(w, h);
        let bytes = encode(&image, w, h);
        assert_eq!(bytes.len(), w * h / 2);
        // Each block's endpoints are its own most and least, in the image's block order.
        for by in 0..h / 4 {
            for bx in 0..w / 4 {
                let b = (by * w / 4 + bx) * BLOCK;
                let texels: Vec<u8> = (0..16)
                    .map(|k| image[(by * 4 + k / 4) * w + bx * 4 + k % 4])
                    .collect();
                assert_eq!(bytes[b], *texels.iter().max().unwrap());
                assert_eq!(bytes[b + 1], *texels.iter().min().unwrap());
            }
        }
    }

    #[test]
    fn round_trip_is_within_half_a_step_of_each_blocks_range() {
        let (w, h) = (32, 16);
        let image = pattern(w, h);
        let back = decode(&encode(&image, w, h), w, h);
        for by in (0..h).step_by(4) {
            for bx in (0..w).step_by(4) {
                let block: Vec<usize> = (0..16).map(|k| (by + k / 4) * w + bx + k % 4).collect();
                let lo = block.iter().map(|&i| image[i]).min().unwrap();
                let hi = block.iter().map(|&i| image[i]).max().unwrap();
                // Seven steps across the range, so at most half of one off.
                let bound = f32::from(hi - lo) / 14.0 + 1e-3;
                for &i in &block {
                    let err = (back[i] - f32::from(image[i])).abs();
                    assert!(err <= bound, "texel {i}: {err} > {bound}");
                }
                // The extremes come back exactly; a flat block exactly.
                for &i in &block {
                    if image[i] == lo || image[i] == hi {
                        assert_eq!(back[i], f32::from(image[i]));
                    }
                }
            }
        }
    }

    /// A block of two endpoints and these indices for its first texels, the rest 0.
    fn block(r0: u8, r1: u8, indices: &[u64]) -> [u8; BLOCK] {
        let bits: u64 = indices.iter().enumerate().map(|(k, i)| i << (3 * k)).sum();
        let mut out = [r0, r1, 0, 0, 0, 0, 0, 0];
        out[2..].copy_from_slice(&bits.to_le_bytes()[..6]);
        out
    }

    #[test]
    fn decodes_both_modes_as_the_format_says() {
        // First endpoint greater: eight values, index 2 one seventh from the first.
        let t = decode_block(&block(200, 60, &[2, 7, 0, 1]));
        assert_eq!(&t[..5], &[180.0, 80.0, 200.0, 60.0, 200.0]);
        // First endpoint not greater: six values and the two ends.
        let t = decode_block(&block(60, 200, &[2, 6, 1, 7]));
        assert_eq!(&t[..4], &[88.0, 0.0, 200.0, 255.0]);
        // The encoder's indices decode to the texels' nearest values.
        let mut texels = [10u8; 16];
        texels[3] = 80;
        texels[9] = 45;
        let t = decode_block(&encode_block(&texels));
        assert_eq!((t[0], t[3], t[9]), (10.0, 80.0, 50.0));
    }
}
