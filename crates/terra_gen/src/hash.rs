//! The integer hash mixers, each written once (D396): what turns a number into one that looks
//! unrelated to its neighbours, for seeds, places and draws. Every crate reaches this one, so a
//! mixer is never written out again where it is used; `cargo xtask check` fails if one of
//! these constants appears anywhere else. A caller scatters its inputs into one number (each by
//! its own odd multiplier, as it always has) and hands it here.
//!
//! Each is pure integer arithmetic, the same on every machine. SplitMix64 is from Steele, Lea
//! and Flood (2014) with Vigna's constants, and the 32-bit mixers are Chris Wellons's, from his
//! hash prospector; both are in the public domain.

/// SplitMix64's finaliser: scrambles 64 bits so that nearby inputs give unrelated outputs.
/// SplitMix64's stream is this over a state stepped by `0x9e37_79b9_7f4a_7c15`.
pub fn splitmix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Wellons's lowbias32: two rounds of shift, multiply, shift, the best-mixed 32-bit hash of
/// that shape he found.
pub fn lowbias32(x: u32) -> u32 {
    let x = lowbias32_round(x);
    let x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// [`lowbias32`]'s first round alone: cheaper and less well mixed, kept where what was made
/// was made with it.
pub fn lowbias32_round(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^ (x >> 15)
}

/// A two-round 32-bit hash of the prospector's, with different shifts from [`lowbias32`].
pub fn prospector32(x: u32) -> u32 {
    let x = prospector32_round(x);
    let x = x.wrapping_mul(0x297a_2d39);
    x ^ (x >> 15)
}

/// [`prospector32`]'s first round alone: cheaper and less well mixed, kept where what was
/// made was made with it.
pub fn prospector32_round(mut x: u32) -> u32 {
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^ (x >> 12)
}

/// A number from a list of numbers, the same on every machine: SplitMix64's stream over
/// them, a piece's version from its place (building's layout and drawing, its settlements).
pub fn of(parts: &[u32]) -> u32 {
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    for &part in parts {
        state ^= part as u64;
        state = splitmix64(state.wrapping_add(0x9e37_79b9_7f4a_7c15));
    }
    state as u32
}

/// A number from 0 up to 1, from a hash's high 24 bits.
pub fn unit(hash: u32) -> f32 {
    (hash >> 8) as f32 / (1u32 << 24) as f32
}

/// The 32-bit hash of a place, the same on every machine: three numbers (a cube's x, y and
/// z, a texel and a salt) and a seed, each scattered by its own odd multiplier and then
/// mixed, so neighbours differ. Building's damage, loss's shading, broken timber, the cell
/// maps and the damage mask share it.
pub fn mix3(seed: u32, a: u32, b: u32, c: u32) -> u32 {
    prospector32_round(
        seed ^ a.wrapping_mul(0x9E37_79B1)
            ^ b.wrapping_mul(0x85EB_CA77)
            ^ c.wrapping_mul(0xC2B2_AE3D),
    )
}

/// [`mix3`] as a number from 0 to 1, from its low 16 bits.
pub fn unit3(seed: u32, a: u32, b: u32, c: u32) -> f32 {
    (mix3(seed, a, b, c) & 0xFFFF) as f32 / 65535.0
}

/// FNV-1a over a key's numbers' bits, a number at a step: quick, and it needs no crate. For
/// keys of places and cubes, never for anything a stranger chooses.
#[derive(Default)]
pub struct Fnv1a(u64);

/// A hash map keyed quickly ([`Fnv1a`]): for places and cubes, never for anything a stranger
/// chooses.
pub type QuickMap<K, V> = std::collections::HashMap<K, V, std::hash::BuildHasherDefault<Fnv1a>>;

/// A hash set keyed quickly, as [`QuickMap`].
pub type QuickSet<K> = std::collections::HashSet<K, std::hash::BuildHasherDefault<Fnv1a>>;

impl std::hash::Hasher for Fnv1a {
    fn finish(&self) -> u64 {
        // The low bits of FNV are weak; folded so a table's buckets spread.
        self.0 ^ (self.0 >> 29)
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn write_u32(&mut self, n: u32) {
        self.0 = (self.0 ^ u64::from(n)).wrapping_mul(0x0100_0000_01b3);
    }
    fn write_i32(&mut self, n: i32) {
        self.write_u32(n as u32);
    }
    // A number a step, not a byte a step: a piece is hashed by many of them.
    fn write_u8(&mut self, n: u8) {
        self.write_u32(u32::from(n));
    }
    fn write_u16(&mut self, n: u16) {
        self.write_u32(u32::from(n));
    }
    fn write_u64(&mut self, n: u64) {
        self.write_u32(n as u32);
        self.write_u32((n >> 32) as u32);
    }
    fn write_i64(&mut self, n: i64) {
        self.write_u64(n as u64);
    }
    fn write_usize(&mut self, n: usize) {
        self.write_u64(n as u64);
    }
    fn write_isize(&mut self, n: isize) {
        self.write_u64(n as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mixers as they were written out where they were used, before they came here, each
    /// copied as it stood (the shifts and multiplies only; each caller kept its own inputs).
    /// Every caller now hands the same number to one of the functions above, so these
    /// agreeing over many inputs is what keeps every place, seed and draw as it was.
    mod before {
        /// terra_gen's `seed::mix`, terra_voxel's `patterned::hash`, terra_world's
        /// `cover::blades::hash`, terra_build's `layout::splitmix` and `scenes::fuzz`'s `Rng`,
        /// and terra_life's `life::unit`.
        pub fn splitmix64(mut z: u64) -> u64 {
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        /// terra_world's `waves::scatter` (after adding its golden ratio), the sandbox's
        /// `lush::far::hash` and the game's `townsfolk::draw`.
        pub fn lowbias32(mut x: u32) -> u32 {
            x ^= x >> 16;
            x = x.wrapping_mul(0x7feb_352d);
            x ^= x >> 15;
            x = x.wrapping_mul(0x846c_a68b);
            x ^= x >> 16;
            x
        }

        /// terra_world's `scene` lattice and the game's `settlement` and `town` rolls.
        pub fn lowbias32_round(mut x: u32) -> u32 {
            x ^= x >> 16;
            x = x.wrapping_mul(0x7feb_352d);
            x ^= x >> 15;
            x
        }

        /// terra_build's `fire::luck`.
        pub fn prospector32(mut h: u32) -> u32 {
            h ^= h >> 15;
            h = h.wrapping_mul(0x2c1b_3c6d);
            h ^= h >> 12;
            h = h.wrapping_mul(0x297a_2d39);
            h ^= h >> 15;
            h
        }

        /// terra_build's `layout::mix3`, terra_life's `animals::unit` (after its FNV step) and
        /// the game's `town::patch`.
        pub fn prospector32_round(mut h: u32) -> u32 {
            h ^= h >> 15;
            h = h.wrapping_mul(0x2c1b_3c6d);
            h ^= h >> 12;
            h
        }
    }

    /// The inputs tried: the edges, every power of two and its neighbours, and a million
    /// spread over the whole range by a stride prime to it.
    fn inputs() -> impl Iterator<Item = u64> {
        let edges = [0, 1, u64::MAX, u64::MAX - 1, u32::MAX as u64, 1 << 32];
        let powers = (0..64).flat_map(|bit| {
            let p = 1u64 << bit;
            [p - 1, p, p + 1]
        });
        let spread = (0..1_000_000u64).map(|i| i.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        edges.into_iter().chain(powers).chain(spread)
    }

    #[test]
    fn each_mixer_gives_what_it_gave_where_it_was_written_out() {
        for x in inputs() {
            assert_eq!(splitmix64(x), before::splitmix64(x), "splitmix64({x})");
            let x = x as u32 ^ (x >> 32) as u32;
            assert_eq!(lowbias32(x), before::lowbias32(x), "lowbias32({x})");
            assert_eq!(lowbias32_round(x), before::lowbias32_round(x));
            assert_eq!(
                prospector32(x),
                before::prospector32(x),
                "prospector32({x})"
            );
            assert_eq!(prospector32_round(x), before::prospector32_round(x));
        }
    }

    /// Wellons's published lowbias32 inverts, so it never maps two inputs to one: a check of
    /// it against the published inverse (lowbias32_r), known another way than by running it.
    #[test]
    fn lowbias32_is_undone_by_its_published_inverse() {
        let inverse = |mut x: u32| {
            x ^= x >> 16;
            x = x.wrapping_mul(0x43021123);
            x ^= (x >> 15) ^ (x >> 30);
            x = x.wrapping_mul(0x1d69e2a5);
            x ^= x >> 16;
            x
        };
        for x in inputs() {
            let x = x as u32;
            assert_eq!(inverse(lowbias32(x)), x);
        }
    }

    /// Neighbouring inputs land far apart: about half the output bits flip for a one-bit
    /// change, which is what a mixer is for.
    #[test]
    fn a_one_bit_change_flips_about_half_the_bits() {
        let (mut flips, mut tries) = (0u64, 0u64);
        for x in (0..20_000u64).map(|i| i.wrapping_mul(0x9e37_79b9_7f4a_7c15)) {
            for bit in 0..64 {
                flips += u64::from((splitmix64(x) ^ splitmix64(x ^ (1 << bit))).count_ones());
                tries += 64;
            }
            let x = x as u32;
            for bit in 0..32 {
                flips += 2 * u64::from((lowbias32(x) ^ lowbias32(x ^ (1 << bit))).count_ones());
                flips +=
                    2 * u64::from((prospector32(x) ^ prospector32(x ^ (1 << bit))).count_ones());
                tries += 128;
            }
        }
        let share = flips as f64 / tries as f64;
        assert!((0.49..0.51).contains(&share), "{share}");
    }

    /// A place's hash spreads its numbers over the whole of 0 to 1: across a 16 by 16 patch
    /// of neighbours, each tenth of the range is hit, and no two neighbours in a row match.
    #[test]
    fn a_places_hash_spreads_over_its_range_and_neighbours_differ() {
        let mut tenths = [0usize; 10];
        for x in 0..16u32 {
            for z in 0..16u32 {
                let here = unit3(3, x, 1, z);
                assert!((0.0..=1.0).contains(&here));
                assert_ne!(here, unit3(3, x + 1, 1, z), "at {x}, {z}");
                tenths[((here * 10.0) as usize).min(9)] += 1;
            }
        }
        assert!(tenths.iter().all(|&n| n > 10), "{tenths:?}");
    }

    #[test]
    fn a_list_hashes_the_same_on_every_machine() {
        // Pinned, so a change to the hash, which would move every log in every base, is
        // seen (D30).
        assert_eq!(of(&[]), 0x7f4a_7c15);
        assert_eq!(
            [of(&[0, 3, 7]), of(&[1, 3, 7]), of(&[0, 3, 8])],
            [0x85a8_47f3, 0x94d9_ad8c, 0xd1c1_928d]
        );
    }
}
