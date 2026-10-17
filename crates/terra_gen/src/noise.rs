use crate::Seed;
use crate::seed::lattice;
use glam::{IVec3, Vec3};

/// Smooth three-dimensional noise (Perlin's improved gradient noise).
///
/// Sampling in three dimensions means a planet has no seams or poles: use the point's
/// position, or its direction from the centre, as the input.
#[derive(Clone, Copy, Debug)]
pub struct Noise {
    seed: u64,
}

impl Noise {
    pub fn new(seed: Seed) -> Self {
        Noise { seed: seed.0 }
    }

    /// Noise at a point, in about `-1.0..1.0`. Features are about one unit across, so
    /// scale the input to choose their size.
    pub fn sample(&self, point: Vec3) -> f32 {
        self.sample_shifted(point, IVec3::ZERO)
    }

    /// [`Noise::sample`] at `point + shift`, exactly, with the whole cells of the shift kept
    /// apart from the point: a single holds a point a few units across to a ten-millionth,
    /// but one shifted by hundreds of cells only to a few hundred-thousandths, which steps
    /// noise sampled far from the origin by that much (a mountain's warp stepped every
    /// 0.6 m). Shift whole cells this way and add only fractions to the point.
    pub fn sample_shifted(&self, point: Vec3, shift: IVec3) -> f32 {
        let cell = point.floor();
        let (x, y, z) = (
            cell.x as i32 + shift.x,
            cell.y as i32 + shift.y,
            cell.z as i32 + shift.z,
        );
        let f = point - cell;
        let (u, v, w) = (fade(f.x), fade(f.y), fade(f.z));

        let corner = |dx: i32, dy: i32, dz: i32| {
            let offset = f - Vec3::new(dx as f32, dy as f32, dz as f32);
            gradient(lattice(self.seed, x + dx, y + dy, z + dz), offset)
        };
        let x00 = lerp(corner(0, 0, 0), corner(1, 0, 0), u);
        let x10 = lerp(corner(0, 1, 0), corner(1, 1, 0), u);
        let x01 = lerp(corner(0, 0, 1), corner(1, 0, 1), u);
        let x11 = lerp(corner(0, 1, 1), corner(1, 1, 1), u);
        lerp(lerp(x00, x10, v), lerp(x01, x11, v), w)
    }

    /// [`Noise::sample`], and how it changes with the point: its gradient, worked out from
    /// the same corners as the value is (Quilez, "Value noise derivatives", for the improved
    /// noise's trilinear blend and quintic fade). Generation that shapes the land by its own
    /// slope needs it, as derivative-damped fractal noise does.
    pub fn sample_with_gradient(&self, point: Vec3) -> (f32, Vec3) {
        self.sample_with_gradient_shifted(point, IVec3::ZERO)
    }

    /// [`Noise::sample_with_gradient`] at `point + shift`, exactly, as
    /// [`Noise::sample_shifted`].
    pub fn sample_with_gradient_shifted(&self, point: Vec3, shift: IVec3) -> (f32, Vec3) {
        let cell = point.floor();
        let (x, y, z) = (
            cell.x as i32 + shift.x,
            cell.y as i32 + shift.y,
            cell.z as i32 + shift.z,
        );
        let f = point - cell;
        let (u, v, w) = (fade(f.x), fade(f.y), fade(f.z));
        let (du, dv, dw) = (fade_slope(f.x), fade_slope(f.y), fade_slope(f.z));
        let corner = |dx: i32, dy: i32, dz: i32| {
            let offset = f - Vec3::new(dx as f32, dy as f32, dz as f32);
            let toward = direction(lattice(self.seed, x + dx, y + dy, z + dz));
            (toward.dot(offset), toward)
        };
        let (a, ga) = corner(0, 0, 0);
        let (b, gb) = corner(1, 0, 0);
        let (c, gc) = corner(0, 1, 0);
        let (d, gd) = corner(1, 1, 0);
        let (e, ge) = corner(0, 0, 1);
        let (g, gg) = corner(1, 0, 1);
        let (h, gh) = corner(0, 1, 1);
        let (k, gk) = corner(1, 1, 1);
        let k1 = b - a;
        let k2 = c - a;
        let k3 = e - a;
        let k4 = a - b - c + d;
        let k5 = a - c - e + h;
        let k6 = a - b - e + g;
        let k7 = -a + b + c - d + e - g - h + k;
        let value =
            a + k1 * u + k2 * v + k3 * w + k4 * u * v + k5 * v * w + k6 * w * u + k7 * u * v * w;
        let blend = ga
            + u * (gb - ga)
            + v * (gc - ga)
            + w * (ge - ga)
            + u * v * (ga - gb - gc + gd)
            + v * w * (ga - gc - ge + gh)
            + w * u * (ga - gb - ge + gg)
            + u * v * w * (-ga + gb + gc - gd + ge - gg - gh + gk);
        let slope = Vec3::new(
            du * (k1 + k4 * v + k6 * w + k7 * v * w),
            dv * (k2 + k5 * w + k4 * u + k7 * w * u),
            dw * (k3 + k6 * u + k5 * v + k7 * u * v),
        );
        (value, blend + slope)
    }

    /// A point in a cell of the unit grid, chosen by the seed: the same cell always gives the
    /// same point, each of its coordinates from 0 to 1 within the cell. Jittered points such as
    /// these seed patterns laid cell by cell (Worley's cellular noise, stripes and gullies).
    pub fn point_in(&self, x: i32, y: i32, z: i32) -> Vec3 {
        let hash = lattice(self.seed ^ 0x005e_ed0f_ce11, x, y, z);
        let part = |shift: u32| ((hash >> shift) & 0x1f_ffff) as f32 / 2_097_152.0;
        Vec3::new(part(0), part(21), part(42))
    }

    /// Several layers of noise added together, each finer and fainter than the last.
    /// `lacunarity` is how much finer each layer is (2.0 is usual); `gain` is how much
    /// fainter (0.5 is usual). The result stays in about `-1.0..1.0`.
    pub fn fbm(&self, point: Vec3, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut total = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = 1.0;
        let mut range = 0.0;
        for octave in 0..octaves {
            // Offset each layer so their grids do not line up at the origin.
            let shift = Vec3::splat(octave as f32 * 19.19);
            total += amplitude * self.sample(point * frequency + shift);
            range += amplitude;
            amplitude *= gain;
            frequency *= lacunarity;
        }
        if range > 0.0 { total / range } else { 0.0 }
    }
}

/// How far a fractal sum's layer is shifted, 19.19 cells a layer as [`Noise::fbm`] shifts its
/// layers so their grids do not line up, as its fraction of a cell and its whole cells, for
/// [`Noise::sample_shifted`]. (`fbm` adds the shift to the point; its bits are pinned.)
pub fn layer_shift(layer: usize) -> (Vec3, IVec3) {
    let shift = layer as f32 * 19.19;
    let whole = shift.floor();
    (Vec3::splat(shift - whole), IVec3::splat(whole as i32))
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// How fast [`fade`] changes.
fn fade_slope(t: f32) -> f32 {
    30.0 * t * t * (t - 1.0) * (t - 1.0)
}

/// The direction [`gradient`] takes the dot product with, for a hash.
fn direction(hash: u64) -> Vec3 {
    match hash % 12 {
        0 => Vec3::new(1.0, 1.0, 0.0),
        1 => Vec3::new(-1.0, 1.0, 0.0),
        2 => Vec3::new(1.0, -1.0, 0.0),
        3 => Vec3::new(-1.0, -1.0, 0.0),
        4 => Vec3::new(1.0, 0.0, 1.0),
        5 => Vec3::new(-1.0, 0.0, 1.0),
        6 => Vec3::new(1.0, 0.0, -1.0),
        7 => Vec3::new(-1.0, 0.0, -1.0),
        8 => Vec3::new(0.0, 1.0, 1.0),
        9 => Vec3::new(0.0, -1.0, 1.0),
        10 => Vec3::new(0.0, 1.0, -1.0),
        _ => Vec3::new(0.0, -1.0, -1.0),
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The dot product of `offset` with one of twelve directions chosen by `hash`.
fn gradient(hash: u64, offset: Vec3) -> f32 {
    let Vec3 { x, y, z } = offset;
    match hash % 12 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 => -y + z,
        10 => y - z,
        _ => -y - z,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points() -> impl Iterator<Item = Vec3> {
        let mut rng = Seed(5).rng();
        (0..5_000).map(move |_| rng.unit_vector() * rng.range(0.0, 300.0))
    }

    #[test]
    fn samples_are_pinned() {
        // Exact bits: the same on every machine, or worlds would differ between players.
        let noise = Noise::new(Seed(1));
        let sampled = [
            noise.sample(Vec3::new(0.5, 0.25, 0.75)).to_bits(),
            noise.sample(Vec3::new(-12.3, 4.56, 78.9)).to_bits(),
            noise.fbm(Vec3::new(1.1, 2.2, 3.3), 5, 2.0, 0.5).to_bits(),
        ];
        assert_eq!(sampled, PINNED_SAMPLES);
    }

    const PINNED_SAMPLES: [u32; 3] = [1_050_255_136, 1_051_968_361, 1_046_572_127];

    #[test]
    fn a_shift_by_whole_cells_is_the_noise_there_and_keeps_its_precision() {
        let noise = Noise::new(Seed(3));
        for p in points() {
            let p = p / 30.0;
            let shift = IVec3::new(3, -2, 5);
            let there = noise.sample(p + shift.as_vec3());
            assert!((noise.sample_shifted(p, shift) - there).abs() < 1e-4);
            let (value, slope) = noise.sample_with_gradient_shifted(p, shift);
            let (v, g) = noise.sample_with_gradient(p + shift.as_vec3());
            assert!((value - v).abs() < 1e-4 && (slope - g).length() < 1e-3);
        }
        // Shifted 307 cells by adding, a point a millionth of a cell along moves the noise by
        // a whole step of the single's precision there or not at all; shifted apart, smoothly,
        // as far as its slope says.
        let (p, along) = (Vec3::new(1.37, 2.11, 0.53), Vec3::new(1e-6, 0.0, 0.0));
        let (_, slope) = noise.sample_with_gradient_shifted(p, IVec3::splat(307));
        let moved = |k: f32| noise.sample_shifted(p + along * k, IVec3::splat(307));
        for k in 1..8 {
            let expected = slope.x * 1e-6;
            let step = moved(k as f32) - moved(k as f32 - 1.0);
            assert!((step - expected).abs() < 2e-7, "{step} for {expected}");
        }
    }

    #[test]
    fn stays_in_range_and_uses_it() {
        let noise = Noise::new(Seed(2));
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        for p in points() {
            let n = noise.sample(p);
            assert!((-1.0..=1.0).contains(&n), "{n} at {p}");
            let layered = noise.fbm(p, 5, 2.0, 0.5);
            assert!((-1.0..=1.0).contains(&layered));
            low = low.min(n);
            high = high.max(n);
        }
        assert!(low < -0.6 && high > 0.6, "range used: {low}..{high}");
    }

    #[test]
    fn is_smooth() {
        let noise = Noise::new(Seed(2));
        for p in points() {
            let step = Vec3::new(0.001, -0.001, 0.001);
            assert!((noise.sample(p) - noise.sample(p + step)).abs() < 0.01);
        }
    }

    #[test]
    fn is_zero_on_the_grid_and_differs_by_seed() {
        let noise = Noise::new(Seed(2));
        assert_eq!(noise.sample(Vec3::new(3.0, -7.0, 11.0)), 0.0);
        let p = Vec3::new(0.3, 0.6, 0.9);
        assert_ne!(noise.sample(p), Noise::new(Seed(3)).sample(p));
    }

    #[test]
    fn its_gradient_is_how_it_changes_and_its_value_the_samples() {
        let noise = Noise::new(Seed(4));
        let h = 1e-3;
        for p in points().take(500) {
            let (value, gradient) = noise.sample_with_gradient(p);
            assert!((value - noise.sample(p)).abs() < 1e-5, "{value} at {p}");
            // Counted another way: the change over a small step along each axis.
            let along =
                |axis: Vec3| (noise.sample(p + axis * h) - noise.sample(p - axis * h)) / (2.0 * h);
            let measured = Vec3::new(along(Vec3::X), along(Vec3::Y), along(Vec3::Z));
            assert!(
                gradient.distance(measured) < 0.02,
                "{gradient} against {measured} at {p}"
            );
        }
    }

    #[test]
    fn a_cells_point_is_inside_it_and_its_own() {
        let noise = Noise::new(Seed(4));
        let mut seen = std::collections::HashSet::new();
        for x in -5..5 {
            for z in -5..5 {
                let p = noise.point_in(x, 0, z);
                assert!(p.cmpge(Vec3::ZERO).all() && p.cmplt(Vec3::ONE).all(), "{p}");
                assert_eq!(p, noise.point_in(x, 0, z));
                seen.insert((p.x.to_bits(), p.y.to_bits()));
            }
        }
        assert_eq!(seen.len(), 100);
    }
}
