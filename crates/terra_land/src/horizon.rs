//! A horizon map of the land round a point (D416): over a grid, how high the land's horizon
//! stands seen from the ground in each of [`WAYS`] ways, so a shader shades ground, trees and
//! water the sun is under the horizon from, kilometres past the shadow map's reach, for a
//! lookup a pixel and nothing a frame; and the lake mirrors its far slopes lit as they stand.
//!
//! Horizon mapping is Max's ("Horizon mapping: shadows for bump-mapped surfaces", The Visual
//! Computer, 1988), taken to terrain as Timonen and Westerholm's "Scalable Height Field
//! Self-Shadowing" (Eurographics 2010) and Andrew Willmott's height-map shadows for SimCity's
//! terrain do: marched once over a heightfield, read for any sun. It stands where Unreal draws
//! its landscape's far shadow cascades past the dynamic shadow distance (`bCastFarShadow`), an
//! extra shadow map drawn again as the sun moves, which Bevy's four cascades leave no room for.
//!
//! The bake is a rule for any world: heights over a sphere, sampled once onto a grid on the
//! plane touching it at the map's middle, then marched from each cell along great circles, so
//! the planet's curve is in the geometry, not a correction.

use glam::{Vec2, Vec3};

/// How many ways round each cell the horizon is taken in, the first the cell's east, turning
/// toward its north.
pub const WAYS: usize = 16;
/// The elevations a map holds, in radians: lower is a crest's horizon, which the sun is always
/// over; higher than a sun ever stands that a slope shades.
pub const LOWEST: f32 = -0.1;
pub const HIGHEST: f32 = 0.7;

/// Where a map lies on its world: the direction of its middle, its east and north there, and
/// the world's radius. A point is on the map where its direction meets the plane touching the
/// sphere at the middle (a gnomonic projection, as the ground's land map is read).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapFrame {
    pub middle: Vec3,
    pub east: Vec3,
    pub north: Vec3,
    pub radius_m: f32,
}

impl MapFrame {
    /// Where on the map, in metres east and north of its middle, a direction from the centre
    /// lies.
    pub fn to_map(&self, direction: Vec3) -> Vec2 {
        let scale = self.radius_m / direction.dot(self.middle).max(1e-6);
        Vec2::new(direction.dot(self.east), direction.dot(self.north)) * scale
    }

    /// The direction from the centre of a point on the map.
    pub fn from_map(&self, at: Vec2) -> Vec3 {
        (self.middle * self.radius_m + self.east * at.x + self.north * at.y).normalize()
    }

    /// East and north at a direction: the map's own, laid level there (the shader's rule).
    pub fn local(&self, up: Vec3) -> (Vec3, Vec3) {
        let east = (self.east - up * self.east.dot(up)).normalize();
        let north =
            (self.north - up * self.north.dot(up) - east * self.north.dot(east)).normalize();
        (east, north)
    }
}

/// The horizon round every cell of a square grid.
#[derive(Clone, Debug)]
pub struct HorizonMap {
    pub frame: MapFrame,
    /// Cells a side, and how wide each, in metres on the map's plane. Row 0 is the north edge.
    pub cells: usize,
    pub cell_m: f32,
    /// Each cell's horizon in each way, radians: `(row * cells + column) * WAYS + way`.
    elevation: Vec<f32>,
}

impl HorizonMap {
    /// The horizon over `cells` by `cells` of `cell_m` round the frame's middle, looked for
    /// `reach_m` out, the ground standing `height(direction)` over the radius.
    pub fn bake(
        frame: MapFrame,
        cells: usize,
        cell_m: f32,
        reach_m: f32,
        height: impl Fn(Vec3) -> f32 + Sync,
    ) -> Self {
        let heights = Heights::sample(&frame, cells as f32 * cell_m * 0.5, cell_m, reach_m, height);
        let steps = steps(cell_m, reach_m, frame.radius_m);
        let rows: Vec<usize> = (0..cells).collect();
        let half = cells as f32 * cell_m * 0.5;
        let elevation = crate::maps::par_map(&rows, |&row| {
            let mut out = Vec::with_capacity(cells * WAYS);
            for column in 0..cells {
                let at = Vec2::new(
                    (column as f32 + 0.5) * cell_m - half,
                    half - (row as f32 + 0.5) * cell_m,
                );
                out.extend(horizon_at(&frame, &heights, &steps, at));
            }
            out
        })
        .concat();
        Self {
            frame,
            cells,
            cell_m,
            elevation,
        }
    }

    /// A cell's horizon in a way, radians.
    pub fn at_cell(&self, column: usize, row: usize, way: usize) -> f32 {
        self.elevation[(row * self.cells + column) * WAYS + way]
    }

    /// The map's width, in metres.
    pub fn size_m(&self) -> f32 {
        self.cells as f32 * self.cell_m
    }

    /// How much of a light from `toward` falls on the ground at `direction`, 1 to 0, as the
    /// shaders read the map (`horizon.wesl`, its CPU twin): the horizon between the cells
    /// round it and the two ways either side of the light's, `soft` radians of penumbra. Off
    /// the map, lit.
    pub fn lit(&self, direction: Vec3, toward: Vec3, soft: f32) -> f32 {
        let up = direction.normalize();
        let (east, north) = self.frame.local(up);
        let rises = libm::asinf(toward.dot(up).clamp(-1.0, 1.0));
        let around = libm::atan2f(toward.dot(north), toward.dot(east));
        let t = (around / core::f32::consts::TAU).rem_euclid(1.0) * WAYS as f32;
        let i = t as usize % WAYS;
        let j = (i + 1) % WAYS;
        let uv = self.frame.to_map(up) / self.size_m() + Vec2::splat(0.5);
        if !(0.0..=1.0).contains(&uv.x) || !(0.0..=1.0).contains(&uv.y) {
            return 1.0;
        }
        let way_i = self.bilinear(uv, i);
        let way_j = self.bilinear(uv, j);
        let horizon = way_i + (way_j - way_i) * t.fract();
        terra_gen::smoothstep(horizon - soft, horizon + soft, rises)
    }

    /// One way's horizon between the four cells round a point, `uv` 0 to 1 east and south of
    /// the map's north-west corner, as a texture is filtered.
    fn bilinear(&self, uv: Vec2, way: usize) -> f32 {
        let n = self.cells;
        let x = (uv.x * n as f32 - 0.5).clamp(0.0, (n - 1) as f32);
        let y = ((1.0 - uv.y) * n as f32 - 0.5).clamp(0.0, (n - 1) as f32);
        let (c, r) = (x as usize, y as usize);
        let (c1, r1) = ((c + 1).min(n - 1), (r + 1).min(n - 1));
        let (fx, fy) = (x.fract(), y.fract());
        let top = self.at_cell(c, r, way) * (1.0 - fx) + self.at_cell(c1, r, way) * fx;
        let bottom = self.at_cell(c, r1, way) * (1.0 - fx) + self.at_cell(c1, r1, way) * fx;
        top * (1.0 - fy) + bottom * fy
    }

    /// The map as a texture's bytes: `WAYS / 4` layers of RGBA8, the ways in order, each
    /// elevation from [`LOWEST`] to [`HIGHEST`] as 0 to 255, rows north first.
    pub fn bytes(&self) -> Vec<u8> {
        let n = self.cells * self.cells;
        let mut out = vec![0u8; n * WAYS];
        for cell in 0..n {
            for way in 0..WAYS {
                let e = self.elevation[cell * WAYS + way];
                let unit = ((e - LOWEST) / (HIGHEST - LOWEST)).clamp(0.0, 1.0);
                let layer = way / 4;
                out[(layer * n + cell) * 4 + way % 4] = (unit * 255.0 + 0.5) as u8;
            }
        }
        out
    }
}

/// The ground's heights sampled onto the map's plane, out past its edge by the reach.
struct Heights {
    half_m: f32,
    cell_m: f32,
    side: usize,
    height: Vec<f32>,
}

impl Heights {
    fn sample(
        frame: &MapFrame,
        map_half_m: f32,
        cell_m: f32,
        reach_m: f32,
        height: impl Fn(Vec3) -> f32 + Sync,
    ) -> Self {
        // The plane stretches what lies off the middle: the arc out to the reach past the map's
        // edge, on it (its corners' farthest ground clamped to the edge's).
        let arc = (map_half_m + reach_m) / frame.radius_m;
        let half_m = libm::tanf(arc.min(1.2)) * frame.radius_m + cell_m;
        let side = (2.0 * half_m / cell_m) as usize + 2;
        let rows: Vec<usize> = (0..side).collect();
        let height = crate::maps::par_map(&rows, |&j| {
            (0..side)
                .map(|i| {
                    let at = Vec2::new(i as f32 * cell_m - half_m, j as f32 * cell_m - half_m);
                    height(frame.from_map(at))
                })
                .collect::<Vec<_>>()
        })
        .concat();
        Self {
            half_m,
            cell_m,
            side,
            height,
        }
    }

    /// The height at a point of the plane, between the four samples round it.
    fn at(&self, at: Vec2) -> f32 {
        let max = (self.side - 1) as f32;
        let x = ((at.x + self.half_m) / self.cell_m).clamp(0.0, max - 0.001);
        let y = ((at.y + self.half_m) / self.cell_m).clamp(0.0, max - 0.001);
        let (i, j) = (x as usize, y as usize);
        let (fx, fy) = (x - i as f32, y - j as f32);
        let k = j * self.side + i;
        let low = self.height[k] * (1.0 - fx) + self.height[k + 1] * fx;
        let high = self.height[k + self.side] * (1.0 - fx) + self.height[k + self.side + 1] * fx;
        low * (1.0 - fy) + high * fy
    }
}

/// The march's steps out along the ground: a cell at a time near, then each a share of the way
/// out, as far ground needs only its shape; each with the cosine and sine of its arc.
fn steps(cell_m: f32, reach_m: f32, radius_m: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let mut d = cell_m;
    while d <= reach_m {
        let arc = d / radius_m;
        out.push((libm::cosf(arc), libm::sinf(arc)));
        d += cell_m.max(d * 0.03);
    }
    out
}

/// The horizon in every way from a point of the map.
fn horizon_at(frame: &MapFrame, heights: &Heights, steps: &[(f32, f32)], at: Vec2) -> [f32; WAYS] {
    let up = frame.from_map(at);
    let (east, north) = frame.local(up);
    let eye = up * (frame.radius_m + heights.at(at));
    let mut out = [0.0; WAYS];
    for (way, slot) in out.iter_mut().enumerate() {
        let angle = way as f32 / WAYS as f32 * core::f32::consts::TAU;
        let out_way = east * libm::cosf(angle) + north * libm::sinf(angle);
        // The highest sine of elevation along the way, the ground's own curve as it falls.
        let mut best = -1.0f32;
        for &(c, s) in steps {
            let toward = up * c + out_way * s;
            let ground = toward * (frame.radius_m + heights.at(frame.to_map(toward)));
            let seen = ground - eye;
            best = best.max(seen.dot(up) / seen.length());
        }
        *slot = libm::asinf(best.clamp(-1.0, 1.0));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADIUS: f32 = 30_000.0;

    fn frame() -> MapFrame {
        MapFrame {
            middle: Vec3::Y,
            east: Vec3::X,
            north: Vec3::NEG_Z,
            radius_m: RADIUS,
        }
    }

    /// A ridge 900 m high running north and south 700 m east of the middle, its flanks up to
    /// 52° steep, and a hill 500 m high north-west of it, up to 51°.
    fn land(direction: Vec3) -> f32 {
        let at = frame().to_map(direction);
        let ridge = 900.0 * libm::expf(-((at.x - 700.0) / 600.0).powi(2));
        let hill = 500.0 * libm::expf(-((at - Vec2::new(-350.0, 350.0)) / 350.0).length_squared());
        ridge + hill
    }

    /// The map's cells a side and their width, the game's.
    const CELLS: usize = 48;
    const CELL_M: f32 = 24.0;

    fn baked() -> HorizonMap {
        HorizonMap::bake(frame(), CELLS, CELL_M, 3500.0, land)
    }

    fn middle_of(column: usize, row: usize) -> Vec2 {
        let half = CELLS as f32 * CELL_M * 0.5;
        Vec2::new(
            (column as f32 + 0.5) * CELL_M - half,
            half - (row as f32 + 0.5) * CELL_M,
        )
    }

    /// The horizon the long way: from the point, a metre at a time along the great circle, on
    /// the land itself, not its sampled grid, from a cell out: nearer, the ground's own slope
    /// is its normal's to shade.
    fn marched(at: Vec2, way: usize) -> f32 {
        let f = frame();
        let up = f.from_map(at);
        let (east, north) = f.local(up);
        let angle = way as f32 / WAYS as f32 * core::f32::consts::TAU;
        let out = east * libm::cosf(angle) + north * libm::sinf(angle);
        let eye = up * (RADIUS + land(up));
        let mut best = -1.0f32;
        let mut d = CELL_M;
        while d <= 3500.0 {
            let toward = up * libm::cosf(d / RADIUS) + out * libm::sinf(d / RADIUS);
            let seen = toward * (RADIUS + land(toward)) - eye;
            best = best.max(seen.dot(up) / seen.length());
            d += 1.0;
        }
        libm::asinf(best)
    }

    #[test]
    fn the_baked_horizon_is_the_one_marched_a_metre_at_a_time() {
        let map = baked();
        let mut errors: Vec<(f32, usize, usize, usize)> = Vec::new();
        for row in (0..CELLS).step_by(3) {
            for column in (0..CELLS).step_by(3) {
                for way in 0..WAYS {
                    let error =
                        map.at_cell(column, row, way) - marched(middle_of(column, row), way);
                    errors.push((error.abs().to_degrees(), column, row, way));
                }
            }
        }
        errors.sort_by(|a, b| a.0.total_cmp(&b.0));
        let typical = errors[errors.len() * 19 / 20].0;
        let (worst, column, row, way) = errors[errors.len() - 1];
        // Nineteen in twenty within a quarter of a degree, under the sun's own half degree; the
        // worst, the ground a cell off on a steep flank, within what a cell's sampling of a
        // curved slope misses (its height off by half a metre over its 24 m, 1.2° measured).
        assert!(
            typical < 0.25 && worst < 2.0,
            "{typical:.2}° the twentieth worst; the worst {worst:.2}° at {column},{row} way {way}"
        );
    }

    #[test]
    fn flat_ground_sees_only_the_curve_of_the_world() {
        let map = HorizonMap::bake(frame(), 8, 48.0, 3000.0, |_| 120.0);
        for way in 0..WAYS {
            let e = map.at_cell(3, 4, way);
            assert!(e < 0.0 && e > -0.002, "{e}");
        }
    }

    #[test]
    fn a_low_sun_behind_the_ridge_shades_its_foot_and_not_its_crest() {
        let map = baked();
        let f = frame();
        let sun = |toward: Vec3, degrees: f32| {
            let up = f.middle;
            let r = degrees.to_radians();
            (toward * libm::cosf(r) + up * libm::sinf(r)).normalize()
        };
        // West of the ridge, its foot: the sun 5° over the east is behind it, over the west not.
        let foot = f.from_map(Vec2::new(-300.0, -200.0));
        assert!(map.lit(foot, sun(Vec3::X, 5.0), 0.01) < 0.01);
        assert!(map.lit(foot, sun(Vec3::NEG_X, 5.0), 0.01) > 0.99);
        // High enough, the sun clears the ridge.
        assert!(map.lit(foot, sun(Vec3::X, 50.0), 0.01) > 0.99);
        // The crest sees the low eastern sun.
        let crest = f.from_map(Vec2::new(700.0, -200.0));
        assert!(map.lit(crest, sun(Vec3::X, 5.0), 0.01) > 0.99);
        // Off the map, nothing is known, and the ground is lit.
        assert_eq!(
            map.lit(f.from_map(Vec2::new(5000.0, 0.0)), sun(Vec3::X, 1.0), 0.01),
            1.0
        );
    }

    #[test]
    fn the_bytes_hold_each_way_in_its_layer_and_channel() {
        let map = baked();
        let bytes = map.bytes();
        let n = CELLS * CELLS;
        assert_eq!(bytes.len(), n * WAYS);
        let (column, row, way) = (30, 20, 6);
        let byte = bytes[((way / 4) * n + row * CELLS + column) * 4 + way % 4];
        let back = LOWEST + byte as f32 / 255.0 * (HIGHEST - LOWEST);
        assert!((back - map.at_cell(column, row, way).clamp(LOWEST, HIGHEST)).abs() < 0.002);
    }
}
