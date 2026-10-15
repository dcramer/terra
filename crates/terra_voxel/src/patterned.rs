//! Patterned ground (docs/demos.md §2): where the ground stays frozen at depth, winter's cold
//! cracks it into a net of polygons and ice wedges grow in the cracks, so the net's edges sink
//! into troughs with low rims either side, and the thaw leaves pools in the polygons' sunken
//! middles and where troughs meet. Real ice-wedge polygons are 10 to 30 m across with under
//! half a metre of relief (Liljedahl et al. 2016; research/2026-10-15-biome-demos.md, tundra).
//!
//! Where the ground is frozen is the climate's to say, not the ground's: the world hands the
//! ground a cold that grows with latitude and height, worked out from its climate
//! (`terra_land`), and the ground lays the net where it is cold enough, a metre-scale relief on
//! the lattice as the finer detail is. The net is a slice through a 3D Voronoi diagram of
//! jittered points, so its polygons are four- to seven-sided and of mixed sizes, as real ones
//! are.

use glam::{IVec3, Vec3};

/// Patterned ground: where it lies and how it is shaped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Patterned {
    /// The world's pole, toward which latitude is counted.
    pub pole: Vec3,
    /// How cold the ground is past the edge of permafrost, in degrees Celsius, as
    /// `sine_squared_c * sin²(latitude) + per_m_c * height over the sea + at_c`: above 0 it is
    /// frozen at depth.
    pub sine_squared_c: f32,
    pub per_m_c: f32,
    pub at_c: f32,
    /// How much colder than the edge the net is at its full depth.
    pub full_c: f32,
    /// How far apart the polygons' middles are, in metres.
    pub cell_m: f32,
    /// How deep and how wide (from the crack out) a trough is, and how high its rims stand.
    pub trough_m: f32,
    pub trough_wide_m: f32,
    pub rim_m: f32,
    /// How deep a polygon's middle sinks below its rims, and the share of polygons whose
    /// middle has thawed into a pool, and how deep that is.
    pub centre_m: f32,
    pub pools: f32,
    pub pool_m: f32,
    /// The pattern's own seed.
    pub seed: u64,
}

impl Patterned {
    /// Ice-wedge polygons as low arctic tundra has them: some 22 m across, troughs 0.4 m deep
    /// and 2 m wide each side of the crack, rims 0.15 m, the middles 0.2 m low and a fifth of
    /// them pools 0.8 m deep.
    pub const ICE_WEDGES: Patterned = Patterned {
        pole: Vec3::Y,
        sine_squared_c: 0.0,
        per_m_c: 0.0,
        at_c: -1.0,
        full_c: 3.0,
        cell_m: 22.0,
        trough_m: 0.4,
        trough_wide_m: 2.0,
        rim_m: 0.15,
        centre_m: 0.2,
        pools: 0.2,
        pool_m: 0.8,
        seed: 0,
    };

    /// How fully the net is laid in a direction from the centre, at a height over the sea: 0
    /// where the ground thaws through, 1 where it is cold enough for the full net.
    pub fn strength(&self, direction: Vec3, over_sea_m: f32) -> f32 {
        let sine = direction.dot(self.pole);
        let cold = self.sine_squared_c * sine * sine + self.per_m_c * over_sea_m + self.at_c;
        smoothstep(0.0, self.full_c, cold)
    }

    /// How much the net raises (above 0) or sinks the ground at a point on it, in metres,
    /// laid this fully.
    pub fn relief_m(&self, point: Vec3, strength: f32) -> f32 {
        if strength <= 0.0 {
            return 0.0;
        }
        let at = point / self.cell_m;
        let cell = at.floor().as_ivec3();
        // The nearest two of the jittered points round it, and the nearest's own number.
        let (mut first, mut second) = ((f32::MAX, Vec3::ZERO, 0u64), (f32::MAX, Vec3::ZERO));
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let c = cell + IVec3::new(dx, dy, dz);
                    let h = hash(self.seed, c);
                    let jitter = Vec3::new(unit(h), unit(h >> 21), unit(h >> 42));
                    let p = c.as_vec3() + Vec3::splat(0.15) + jitter * 0.7;
                    let d = (p - at).length_squared();
                    if d < first.0 {
                        second = (first.0, first.1);
                        first = (d, p, h);
                    } else if d < second.0 {
                        second = (d, p);
                    }
                }
            }
        }
        // How far from the crack between the two, along the line joining them, in metres.
        let gap = (second.1 - first.1).length().max(1e-4);
        let from_crack = (second.0 - first.0) / (2.0 * gap) * self.cell_m;
        let into_middle = first.0.sqrt() * self.cell_m;
        let trough = -self.trough_m * (1.0 - smoothstep(0.0, self.trough_wide_m, from_crack));
        let rim = self.rim_m
            * smoothstep(
                self.trough_wide_m * 0.6,
                self.trough_wide_m * 1.2,
                from_crack,
            )
            * (1.0
                - smoothstep(
                    self.trough_wide_m * 1.4,
                    self.trough_wide_m * 2.6,
                    from_crack,
                ));
        // Low middles inside their rims, a share of them thawed into pools, each its own size.
        let size = 0.55 + 0.45 * unit(first.2 >> 7);
        let reach = self.cell_m * 0.42 * size;
        let middle = 1.0 - smoothstep(reach * 0.3, reach, into_middle);
        let pool = if unit(first.2 >> 13) < self.pools {
            self.pool_m
        } else {
            self.centre_m
        };
        (trough + rim - pool * middle) * strength
    }
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A number from 0 to 1 from a hash's low 21 bits.
fn unit(h: u64) -> f32 {
    (h & 0x1f_ffff) as f32 / 0x1f_ffff as f32
}

/// SplitMix64 over a cell and a seed.
fn hash(seed: u64, c: IVec3) -> u64 {
    let mut z = seed
        ^ (c.x as u32 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (c.y as u32 as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f)
        ^ (c.z as u32 as u64).wrapping_mul(0x1656_67b1_9e37_79f9);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cold() -> Patterned {
        Patterned {
            at_c: 5.0,
            seed: 3,
            ..Patterned::ICE_WEDGES
        }
    }

    #[test]
    fn the_net_lies_only_where_the_ground_is_frozen() {
        let warm = Patterned {
            at_c: -1.0,
            ..cold()
        };
        assert_eq!(warm.strength(Vec3::Y, 0.0), 0.0);
        assert_eq!(cold().strength(Vec3::Y, 0.0), 1.0);
        // Colder with latitude and height, as the world says.
        let by_latitude = Patterned {
            sine_squared_c: 4.0,
            at_c: -2.0,
            ..cold()
        };
        assert!(by_latitude.strength(Vec3::Y, 0.0) > by_latitude.strength(Vec3::X, 0.0));
        assert_eq!(by_latitude.relief_m(Vec3::new(5.0, 0.0, 3.0), 0.0), 0.0);
    }

    #[test]
    fn troughs_run_between_polygons_and_some_middles_hold_pools() {
        // Walk a line across a plane of the net: the ground dips into troughs a polygon's
        // width apart, rises on their rims, and its lowest points are pools in middles.
        let net = cold();
        let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
        let mut troughs = 0;
        let mut was_in = false;
        let mut x = 0.0;
        while x < 2000.0 {
            let r = net.relief_m(Vec3::new(x, 7.3, 11.1), 1.0);
            lowest = lowest.min(r);
            highest = highest.max(r);
            let inside = r < -0.3 * net.trough_m;
            if inside && !was_in {
                troughs += 1;
            }
            was_in = inside;
            x += 0.25;
        }
        // Under half a metre high, pools a little deeper, and a trough every 10 to 40 m.
        assert!(highest > 0.05 && highest < 0.5, "{highest}");
        assert!(lowest < -0.6 && lowest > -1.5, "{lowest}");
        let every = 2000.0 / troughs as f32;
        assert!((8.0..45.0).contains(&every), "a trough every {every} m");
    }
}
