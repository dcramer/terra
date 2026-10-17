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
}
