use crate::Rng;
use crate::hash::splitmix64 as mix;

/// Where every generated thing starts. One seed describes a whole planet; seeds for its
/// parts are derived from it by name or by place, so unrelated parts never share a
/// random stream and adding a new part never changes an existing one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Seed(pub u64);

impl Seed {
    /// The seed for a named purpose, such as `"terrain"` or `"ore"`.
    pub fn derive(self, label: &str) -> Seed {
        // FNV-1a over the label, so the result depends only on the bytes.
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in label.bytes() {
            hash = (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
        Seed(mix(self.0 ^ mix(hash)))
    }

    /// The seed for a place on an integer grid, such as a chunk or a scatter cell.
    pub fn at(self, x: i32, y: i32, z: i32) -> Seed {
        Seed(lattice(self.0, x, y, z))
    }

    /// The seed for the `index`-th item of a kind, such as the fifth tree in a cell.
    pub fn index(self, index: u64) -> Seed {
        Seed(mix(self.0 ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15)))
    }

    /// A random stream that always produces the same numbers for this seed.
    pub fn rng(self) -> Rng {
        Rng::new(self)
    }
}

/// A hash of a seed and an integer grid position.
pub(crate) fn lattice(seed: u64, x: i32, y: i32, z: i32) -> u64 {
    let x = (x as u32 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let y = (y as u32 as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    let z = (z as u32 as u64).wrapping_mul(0x1656_67b1_9e37_79f9);
    mix(mix(seed ^ x) ^ mix(y ^ z.rotate_left(32)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_seeds_are_pinned() {
        // Exact values: if these change, every generated world changes.
        let seed = Seed(1);
        assert_eq!(seed.derive("terrain"), Seed(PINNED_TERRAIN));
        assert_eq!(seed.at(3, -4, 5), Seed(PINNED_AT));
        assert_eq!(seed.index(7), Seed(PINNED_INDEX));
    }

    const PINNED_TERRAIN: u64 = 893_994_868_548_096_648;
    const PINNED_AT: u64 = 14_789_744_563_965_282_753;
    const PINNED_INDEX: u64 = 17_388_166_129_998_380_965;

    #[test]
    fn different_purposes_and_places_get_different_seeds() {
        let seed = Seed(42);
        assert_ne!(seed.derive("terrain"), seed.derive("ore"));
        assert_ne!(seed.at(0, 0, 1), seed.at(0, 1, 0));
        assert_ne!(seed.at(1, 0, 0), seed.at(-1, 0, 0));
        assert_ne!(seed.index(0), seed.index(1));
        assert_ne!(Seed(1).derive("terrain"), Seed(2).derive("terrain"));
    }
}
