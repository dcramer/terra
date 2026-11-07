//! The land: a heightfield over the clearing, a rocky ridge to the north, a knoll to the
//! west and a pond to the east, drawn in the game's ground layers (`materials/ground`, a
//! stack of scans) blended per vertex by what lies there: grass in the open, leaves, duff
//! and roots under the trees, moss by the rocks, bare rock where it is steep, mud at the
//! water. Everyone stands on it; the search map is blocked where it is too steep or wet.
//! The biome's ground layers (kit/ground.rs) lay the scans by these maps and the kit's rules.

use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// The names of the six maps `Land::mesh`'s `cover` gives each vertex, in its order (the
/// colour's four, then the second texture coordinate's two), as a biome's ground names
/// them (`Site("grass")`, kit/ground.rs).
pub const SITES: [&str; 6] = ["grass", "leaves", "moss", "rock", "mud", "roots"];

pub fn plugin(app: &mut App) {
    app.add_plugins(crate::kit::ground::plugin)
        .insert_resource(Land::new());
}

/// Half the ground mesh's side, metres, and its cell.
pub const EXTENT: f32 = 56.0;
const STEP: f32 = 0.5;
/// The clearing's middle, where the camp is.
pub const CAMP: Vec2 = Vec2::new(-1.0, 2.0);

/// How deep in the woods a point is: 0 in the clearing, 1 among the trees, its edge
/// ragged.
pub fn woods(p: Vec2) -> f32 {
    let d = (p - CAMP).length() + 6.0 * (fbm(p * 0.09, 23) - 0.5);
    smooth(6.0, 10.0, d)
}

/// How trodden bare the ground is: the camp's earth about the fire, the paths worn from it
/// to the pond and out west through the wood, and a few scuffed patches in the clearing.
pub fn bare(p: Vec2) -> f32 {
    let fire = crate::scene::FIRE;
    let wobble = fbm(p * 0.6, 91) - 0.5;
    // Its edge ragged at two sizes, so it is no disc: lobes where feet went, tufts biting in.
    let ragged = fbm(p * 2.4, 97) - 0.5;
    let camp = smooth(4.3, 1.0, p.distance(fire) + wobble * 2.6 + ragged * 1.3);
    let path = |a: Vec2, b: Vec2, w: f32| {
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
        let d = p.distance(a + ab * t) + wobble * 0.8;
        smooth(w, 0.0, d) * (0.55 + 0.45 * smooth(0.3, 0.6, fbm(p * 0.9, 95)))
    };
    let paths = path(fire, POND + Vec2::new(-4.5, -1.5), 0.75)
        .max(path(fire, Vec2::new(-9.0, 5.5), 0.65))
        .max(path(Vec2::new(-9.0, 5.5), Vec2::new(-20.0, 9.0), 0.55));
    let scuffs = smooth(0.68, 0.8, fbm(p * 0.35, 93)) * (1.0 - woods(p));
    // The forest's fight's clearing (wood/clearing.rs), between the camp and the pond.
    let fight = crate::wood::clearing::trodden(p);
    (camp.max(paths * 0.7).max(scuffs * 0.5).max(fight)).clamp(0.0, 1.0)
}

/// The pond's water, metres.
pub const WATER: f32 = -0.55;
pub const POND: Vec2 = Vec2::new(13.0, 7.0);

/// A small integer hash to 0..1.
pub fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 0x0100_0000 as f32
}

/// Smooth value noise, 0..1.
pub fn noise(p: Vec2, seed: u32) -> f32 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let (x, y) = (i.x as i32, i.y as i32);
    let a = hash(x, y, seed);
    let b = hash(x + 1, y, seed);
    let c = hash(x, y + 1, seed);
    let d = hash(x + 1, y + 1, seed);
    a + (b - a) * u.x + (c - a) * u.y + (a - b - c + d) * u.x * u.y
}

pub fn fbm(p: Vec2, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut q = p;
    for o in 0..4 {
        sum += amp * noise(q, seed + o);
        q = q * 2.03 + Vec2::new(17.1, 9.3);
        amp *= 0.5;
    }
    sum / 0.9375
}

pub fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far into the pond a point is: above 0 inside, its edge broken by noise.
pub fn pond(p: Vec2) -> f32 {
    let d = p - POND;
    let r = 5.2 + 1.1 * (fbm(p * 0.25, 41) - 0.5) * 2.0;
    // Longer east-west.
    let e = Vec2::new(d.x / 1.25, d.y).length();
    (r - e) / r
}

/// Where the ridge's foot runs, at `x`.
pub fn ridge_z(x: f32) -> f32 {
    -14.5 - 4.0 * (fbm(Vec2::new(x * 0.06, 3.0), 7) - 0.5) * 2.0
}

/// The ridge's line: metres north of it are negative.
fn ridge_line(p: Vec2) -> f32 {
    p.y + 14.5 + 4.0 * (fbm(Vec2::new(p.x * 0.06, 3.0), 7) - 0.5) * 2.0
}

/// The land's height at a point, as shaped (the mesh samples it).
pub fn shape(p: Vec2) -> f32 {
    let mut h = 1.4 * (fbm(p * 0.035, 1) - 0.5) + 0.35 * (fbm(p * 0.16, 2) - 0.5);
    // The clearing round the camp, eased flat.
    let clearing = smooth(13.0, 6.0, (p - Vec2::new(-1.0, 2.0)).length());
    h = h * (1.0 - 0.7 * clearing);
    // The ridge to the north: a broken rock step, then the slope above.
    let d = -ridge_line(p);
    let ledges = fbm(p * 0.22, 5);
    h += 3.2 * smooth(0.0, 2.2 + ledges * 1.5, d) + 3.5 * smooth(3.0, 14.0, d);
    h += 0.6 * smooth(0.5, 3.0, d) * (fbm(p * 0.5, 6) - 0.5);
    // A knoll of rock to the west.
    let k = (p - Vec2::new(-15.0, 5.0)).length();
    h += 2.2 * smooth(5.0, 2.2, k + 1.5 * (fbm(p * 0.4, 9) - 0.5));
    // The pond's bowl, with a shelving bank.
    let w = pond(p);
    h = h.min(h * (1.0 - smooth(-0.35, 0.0, w)) + WATER + 0.25 - 1.0 * smooth(-0.1, 0.5, w));
    h
}

/// The sampled land: heights on the mesh's grid.
#[derive(Resource, Clone)]
pub struct Land {
    n: usize,
    h: Vec<f32>,
}

impl Land {
    pub fn new() -> Self {
        Self::from_shape(shape)
    }

    /// Another scene's land (the marsh's) on the same grid and in the same layers.
    pub fn from_shape(shape: impl Fn(Vec2) -> f32) -> Self {
        let n = (EXTENT * 2.0 / STEP) as usize + 1;
        let mut h = vec![0.0; n * n];
        for j in 0..n {
            for i in 0..n {
                h[j * n + i] = shape(Self::at(i, j));
            }
        }
        Land { n, h }
    }

    fn at(i: usize, j: usize) -> Vec2 {
        Vec2::new(i as f32 * STEP - EXTENT, j as f32 * STEP - EXTENT)
    }

    fn get(&self, i: i32, j: i32) -> f32 {
        let c = |v: i32| v.clamp(0, self.n as i32 - 1) as usize;
        self.h[c(j) * self.n + c(i)]
    }

    /// The height under a point, as the mesh's triangles have it.
    pub fn height(&self, p: Vec2) -> f32 {
        let g = (p + EXTENT) / STEP;
        let (i, j) = (g.x.floor() as i32, g.y.floor() as i32);
        let (fx, fz) = (g.x - i as f32, g.y - j as f32);
        let a = self.get(i, j);
        let b = self.get(i + 1, j);
        let c = self.get(i, j + 1);
        let d = self.get(i + 1, j + 1);
        // The mesh splits each cell from (i, j) to (i+1, j+1).
        if fx >= fz {
            a + (b - a) * fx + (d - b) * fz
        } else {
            a + (d - c) * fx + (c - a) * fz
        }
    }

    pub fn normal(&self, p: Vec2) -> Vec3 {
        let e = STEP;
        let dx = self.height(p + Vec2::X * e) - self.height(p - Vec2::X * e);
        let dz = self.height(p + Vec2::Y * e) - self.height(p - Vec2::Y * e);
        Vec3::new(-dx, 2.0 * e, -dz).normalize()
    }

    /// How steep the land is, as the rise over a metre.
    pub fn slope(&self, p: Vec2) -> f32 {
        let n = self.normal(p);
        (1.0 - n.y * n.y).sqrt() / n.y.max(0.05)
    }

    /// Where a ray from the camera first meets the land.
    pub fn hit(&self, ray: Ray3d) -> Option<Vec3> {
        let d = *ray.direction;
        let mut t = 0.0;
        let mut last = ray.origin;
        for _ in 0..600 {
            t += 0.25;
            let p = ray.origin + d * t;
            if p.y <= self.height(p.xz()) {
                // Halve back to the surface.
                let (mut lo, mut hi) = (last, p);
                for _ in 0..12 {
                    let m = (lo + hi) * 0.5;
                    if m.y <= self.height(m.xz()) {
                        hi = m;
                    } else {
                        lo = m;
                    }
                }
                return Some(hi);
            }
            last = p;
        }
        None
    }

    /// The ground mesh, its layers' weights from `cover` (grass, leaves, moss, rock in the
    /// colour; mud and roots in the second texture coordinate).
    /// `cover` gives each vertex its layers' weights (colour, second UV) and, in the first UV,
    /// the shade of the crowns over it and how trodden bare it is.
    pub fn mesh(&self, cover: impl Fn(Vec2, Vec3, f32) -> ([f32; 4], [f32; 2], [f32; 2])) -> Mesh {
        let n = self.n;
        let mut pos = Vec::with_capacity(n * n);
        let mut nor = Vec::with_capacity(n * n);
        let mut uv = Vec::with_capacity(n * n);
        let mut col = Vec::with_capacity(n * n);
        let mut uvb = Vec::with_capacity(n * n);
        for j in 0..n {
            for i in 0..n {
                let p = Self::at(i, j);
                let h = self.h[j * n + i];
                let normal = self.normal(p);
                pos.push([p.x, h, p.y]);
                nor.push(normal.to_array());
                let (c, b, a) = cover(p, normal, h);
                col.push(c);
                uvb.push(b);
                uv.push(a);
            }
        }
        let mut idx = Vec::with_capacity((n - 1) * (n - 1) * 6);
        for j in 0..n - 1 {
            for i in 0..n - 1 {
                let a = (j * n + i) as u32;
                let b = a + 1;
                let c = a + n as u32;
                let d = c + 1;
                idx.extend_from_slice(&[a, d, b, a, c, d]);
            }
        }
        Mesh::new(PrimitiveTopology::TriangleList, default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, uvb)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
            .with_inserted_indices(Indices::U32(idx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clearing_is_walkable_and_the_ridge_and_pond_are_not() {
        let land = Land::new();
        // The camp's ground is gentle.
        for p in [
            Vec2::new(0.0, 2.0),
            Vec2::new(3.0, 5.0),
            Vec2::new(-4.0, 0.0),
        ] {
            assert!(land.slope(p) < 0.35, "{p} slope {}", land.slope(p));
            assert!(land.height(p) > WATER + 0.1);
        }
        // The pond's middle is under water.
        assert!(land.height(POND) < WATER - 0.3);
        // The ridge stands above the clearing.
        assert!(land.height(Vec2::new(0.0, -26.0)) > land.height(Vec2::new(0.0, 2.0)) + 3.0);
    }

    #[test]
    fn a_ray_meets_the_land_where_it_stands() {
        let land = Land::new();
        let at = Vec2::new(2.0, 3.0);
        let top = Vec3::new(at.x, 40.0, at.y);
        let hit = land
            .hit(Ray3d::new(top, Dir3::NEG_Y))
            .expect("a ray straight down meets the land");
        assert!((hit.y - land.height(at)).abs() < 0.01);
    }
}
