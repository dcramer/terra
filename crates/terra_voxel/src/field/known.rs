//! The lattice points a planet's land has been worked out at, shared by every memo.

use super::Node;
use glam::Vec3;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::RwLock;

/// The lattice points a planet's land has been worked out at, shared by every memo asked of
/// it on any thread, so a point worked out for one chunk, one look round or one tree is not
/// worked out again for the next: a lattice point is the same land whoever asks and at what
/// coarseness (`land_in`'s nodes sit on the one lattice), so what any memo answers is the
/// same with it or without. Asked first: at the lake, the first view's 8181 chunks each made
/// their own memo, and the chunks stacked up a wall and the looks for cliffs and aprons
/// uphill asked the same points over and over, at about 100 µs a point with mountains.
///
/// Sharded, as Unreal's derived data cache and every concurrent memo table are, so threads
/// meshing side by side rarely wait on one another; a shard that grows past its share of
/// `KNOWN_MOST` is emptied, which bounds the memory a long walk adds and changes no answer.
///
/// It holds the mountains' slopes measured at their sparser lattice points too
/// (`super::slopes`), each kind apart.
#[derive(Default)]
pub(super) struct Known {
    nodes: Shards<Node>,
    /// The gullies' slope (`super::slopes::SLOPE_EVERY`).
    pub(super) slopes: Shards<Vec3>,
    /// The faces' broad slope (`super::slopes::BROAD_EVERY`).
    pub(super) broad: Shards<Vec3>,
}

/// One kind of value worked out at lattice points, sharded.
pub(super) struct Shards<V> {
    shards: Box<[RwLock<LatticeMap<V>>]>,
}

type LatticeMap<V> = HashMap<(u8, u32, u32), V, BuildHasherDefault<LatticeHasher>>;

/// Lattice points the shared memo holds at most, over its shards: about 28 bytes each held,
/// so some 230 MB.
const KNOWN_MOST: usize = 8 << 20;
const KNOWN_SHARDS: usize = 64;

impl Known {
    /// The land at a lattice point, worked out by `exact` only if no one has yet.
    pub(super) fn get_or(&self, key: (u8, u32, u32), exact: impl FnOnce() -> Node) -> Node {
        self.nodes.get_or(key, exact)
    }
}

impl<V: Copy> Shards<V> {
    fn shard(&self, key: (u8, u32, u32)) -> &RwLock<LatticeMap<V>> {
        let mut hasher = LatticeHasher::default();
        hasher.write_u8(key.0);
        hasher.write_u32(key.1);
        hasher.write_u32(key.2);
        // The hash's high bits, which its multiply mixes best.
        &self.shards[(hasher.finish() >> 58) as usize % KNOWN_SHARDS]
    }

    /// The value at a lattice point, worked out by `exact` only if no one has yet.
    pub(super) fn get_or(&self, key: (u8, u32, u32), exact: impl FnOnce() -> V) -> V {
        let shard = self.shard(key);
        if let Some(value) = shard.read().ok().and_then(|held| held.get(&key).copied()) {
            return value;
        }
        // Worked out outside the lock: two threads asking at once both work it out, alike.
        let value = exact();
        if let Ok(mut held) = shard.write() {
            if held.len() >= KNOWN_MOST / KNOWN_SHARDS {
                held.clear();
            }
            held.insert(key, value);
        }
        value
    }
}

impl<V> Default for Shards<V> {
    fn default() -> Self {
        Self {
            shards: (0..KNOWN_SHARDS)
                .map(|_| RwLock::new(LatticeMap::default()))
                .collect(),
        }
    }
}

/// A copy of a planet starts knowing nothing: a builder may change its land.
impl Clone for Known {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for Known {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Known")
    }
}

/// A quick hash for lattice points, which are small integers.
#[derive(Default)]
pub(super) struct LatticeHasher(u64);

impl Hasher for LatticeHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(byte as u64);
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.write_u64(value as u64);
    }

    fn write_u32(&mut self, value: u32) {
        self.write_u64(value as u64);
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Field, GeneratedPlanet};
    use glam::Vec3;
    use terra_core::PlanetSpec;
    use terra_gen::Seed;

    #[test]
    fn the_land_is_the_same_asked_alone_or_from_many_threads_at_every_coarseness() {
        // What one planet's shared lattice points answer, asked at once from every core at
        // coarse and fine spacings, is what a planet of the same seed that never shared a
        // point answers, bit for bit: with hills, and with mountains, whose points are the
        // dearest.
        let hills = |radius_m: f32| {
            GeneratedPlanet::new(
                PlanetSpec {
                    seed: Seed(7),
                    radius_m,
                },
                150.0,
                Some(-20.0),
            )
        };
        let mountains = || hills(30_000.0).with_mountains(crate::Mountains::START);
        let small = || hills(2000.0);
        let planets: [(&dyn Fn() -> GeneratedPlanet, f32); 2] =
            [(&small, 2040.0), (&mountains, 30_400.0)];
        for (planet, out_m) in planets {
            let points: Vec<Vec3> = (0..2000)
                .map(|k| {
                    let t = k as f32 * 0.37;
                    Vec3::new(libm::cosf(t), 0.3 * libm::sinf(t * 1.3), libm::sinf(t)) * out_m
                })
                .collect();
            let shared = planet();
            let spacings = [0.5, 1.0, 4.0, 32.0];
            let asked = terra_gen::par::map(&points, |&point| {
                spacings.map(|spacing| shared.distance_in(point, &mut shared.memo_for(spacing)))
            });
            let alone = planet();
            for (point, asked) in points.iter().zip(asked) {
                for (spacing, got) in spacings.iter().zip(asked) {
                    // A copy shares nothing (`Known`'s Clone), so each answer is worked out
                    // afresh.
                    let fresh = alone.clone();
                    let want = fresh.distance_in(*point, &mut fresh.memo_for(*spacing));
                    assert_eq!(
                        got.to_bits(),
                        want.to_bits(),
                        "at {point} {spacing} m apart"
                    );
                }
            }
        }
    }
}
