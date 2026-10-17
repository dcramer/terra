use crate::Seed;
use crate::hash::splitmix64 as mix;
use glam::Vec3;

/// A small, fast random stream (SplitMix64). Not for anything secret; for placing rocks.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: Seed) -> Self {
        Rng { state: seed.0 }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix(self.state)
    }

    /// A number in `0.0..1.0`.
    pub fn unit(&mut self) -> f32 {
        // The top 24 bits fit a float exactly, so every platform gets the same value.
        (self.next_u64() >> 40) as f32 * (1.0 / (1u64 << 24) as f32)
    }

    /// A number in `low..high`.
    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }

    /// A whole number in `0..count`.
    pub fn below(&mut self, count: u32) -> u32 {
        (((self.next_u64() >> 32) * count as u64) >> 32) as u32
    }

    /// A direction, uniform over the sphere. Uses no trigonometry.
    pub fn unit_vector(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
            );
            let length_squared = v.length_squared();
            if length_squared > 1e-6 && length_squared <= 1.0 {
                return v / length_squared.sqrt();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stream_is_pinned() {
        let mut rng = Seed(1).rng();
        let first: [u64; 3] = [rng.next_u64(), rng.next_u64(), rng.next_u64()];
        assert_eq!(first, PINNED_STREAM);
        assert_eq!(Seed(1).rng().unit().to_bits(), PINNED_UNIT_BITS);
    }

    const PINNED_STREAM: [u64; 3] = [
        10_451_216_379_200_822_465,
        13_757_245_211_066_428_519,
        17_911_839_290_282_890_590,
    ];
    const PINNED_UNIT_BITS: u32 = 1_058_081_325;

    #[test]
    fn numbers_stay_in_range() {
        let mut rng = Seed(9).rng();
        for _ in 0..10_000 {
            let u = rng.unit();
            assert!((0.0..1.0).contains(&u));
            assert!(rng.below(7) < 7);
            let r = rng.range(-3.0, 5.0);
            assert!((-3.0..5.0).contains(&r));
        }
    }

    #[test]
    fn directions_are_unit_length_and_spread_out() {
        let mut rng = Seed(3).rng();
        let mut sum = Vec3::ZERO;
        for _ in 0..10_000 {
            let v = rng.unit_vector();
            assert!((v.length() - 1.0).abs() < 1e-5);
            sum += v;
        }
        // Uniform directions cancel out on average.
        assert!((sum / 10_000.0).length() < 0.03);
    }
}
