//! What the ruin's effects share: the clock every effect is a function of (so a still is
//! one chosen moment and a film plays them), the camera's facing, the fires' places, bursts
//! of spark streaks, and textures made in code.

use crate::Args;
use crate::camera::MainCamera;
use crate::forest::Rng;
use crate::motes::{Halo, Tongue};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// The effects' time: the game's in a film or a window, held still in a still (`--shot`
/// without `--film`), when each effect stands at the moment its module names.
#[derive(Resource, Default)]
pub struct Clock {
    pub t: f32,
    pub still: bool,
}

/// The camera's rotation, its turn about the vertical alone (for upright cards), and place.
#[derive(Resource, Default)]
pub struct Cam {
    pub rot: Quat,
    pub upright: Quat,
    pub pos: Vec3,
}

/// The ruin's fires (fire.rs's): the bed's middle and the flame's height, found by their
/// halos, which fire.rs stands at the bed plus 0.35 of the flame and sizes 2.6 flames.
#[derive(Resource, Default)]
pub struct Fires(pub Vec<(Vec3, f32)>);

/// After this, `Fires` holds them (PostStartup).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Found;

pub fn plugin(app: &mut App) {
    app.init_resource::<Clock>()
        .init_resource::<Cam>()
        .init_resource::<Fires>()
        .add_systems(PostStartup, find_fires.in_set(Found))
        .add_systems(PreUpdate, tick)
        .add_systems(Update, burst);
}

fn tick(
    time: Res<Time>,
    args: Res<Args>,
    mut clock: ResMut<Clock>,
    mut cam: ResMut<Cam>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
) {
    clock.still = args.shot.is_some() && args.film == 0;
    clock.t = time.elapsed_secs();
    if let Ok(c) = camera.single() {
        let b = c.back();
        cam.rot = c.rotation();
        cam.upright = Quat::from_rotation_y(b.x.atan2(b.z));
        cam.pos = c.translation();
    }
}

/// Reads the fires off fire.rs's halos, and puts out fire.rs's tongue cards: flame.rs draws
/// the flames now.
fn find_fires(
    mut fires: ResMut<Fires>,
    halos: Query<&Transform, With<Halo>>,
    mut tongues: Query<&mut Visibility, With<Tongue>>,
) {
    for t in &halos {
        let size = t.scale.x / 2.6;
        fires.0.push((t.translation - Vec3::Y * size * 0.35, size));
    }
    fires.0.sort_by(|a, b| b.1.total_cmp(&a.1));
    for mut v in &mut tongues {
        *v = Visibility::Hidden;
    }
}

/// A burst of spark streaks: where it is and how long since it went off (negative: not yet).
#[derive(Component)]
pub struct Burst {
    pub at: Vec3,
    pub age: f32,
}

/// One streak of a burst: its way out, speed (m/s), life (s) and thickness (m).
#[derive(Component)]
pub struct Spark {
    pub burst: Entity,
    pub dir: Vec3,
    pub speed: f32,
    pub life: f32,
    pub width: f32,
}

/// Throws `n` streaks out of a burst, in a cone about `out` (`spread` 0 a line, 1 a sphere).
#[allow(clippy::too_many_arguments)]
pub fn sparks(
    commands: &mut Commands,
    burst: Entity,
    mesh: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    rng: &mut Rng,
    n: usize,
    out: Vec3,
    spread: f32,
    speed: (f32, f32),
    width: f32,
) {
    for _ in 0..n {
        let r = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
        let dir = (out.normalize() + r * spread).normalize_or(Vec3::Y);
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_scale(Vec3::ZERO),
            bevy::light::NotShadowCaster,
            Spark {
                burst,
                dir,
                speed: rng.range(speed.0, speed.1),
                life: rng.range(0.22, 0.5),
                width: width * rng.range(0.6, 1.2),
            },
        ));
    }
}

/// Where a streak is `a` seconds out, and its velocity: thrown, slowed by the air (drag
/// `K`), pulled down, skidding on the floor.
pub fn spark_at(dir: Vec3, speed: f32, a: f32) -> (Vec3, Vec3) {
    const K: f32 = 5.0;
    const G: f32 = 7.0;
    let e = (-K * a).exp();
    let mut p = dir * speed * (1.0 - e) / K - Vec3::Y * 0.5 * G * a * a;
    let v = dir * speed * e - Vec3::Y * G * a;
    p.y = p.y.max(-10.0);
    (p, v)
}

/// Every streak along its way: stretched along its speed (a motion streak), thinning to
/// nothing at the end of its life.
fn burst(bursts: Query<&Burst>, mut sparks: Query<(&Spark, &mut Transform)>) {
    for (s, mut tr) in &mut sparks {
        let Ok(b) = bursts.get(s.burst) else { continue };
        let a = b.age;
        if !(0.0..s.life).contains(&a) {
            tr.scale = Vec3::ZERO;
            continue;
        }
        let (p, v) = spark_at(s.dir, s.speed, a);
        let mut at = b.at + p;
        // Off the floor, not through it.
        at.y = at.y.max(0.03);
        let k = 1.0 - a / s.life;
        let len = (v.length() * 0.035).clamp(0.03, 0.4);
        tr.translation = at;
        tr.rotation = Quat::from_rotation_arc(Vec3::Z, v.normalize_or(Vec3::Y));
        tr.scale = Vec3::new(s.width * k, s.width * k, len * k.sqrt());
    }
}

/// An unlit additive material of a colour in linear light (above 1 drives the bloom).
pub fn glow(c: LinearRgba, texture: Option<Handle<Image>>) -> StandardMaterial {
    StandardMaterial {
        base_color: c.into(),
        base_color_texture: texture,
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..default()
    }
}

/// Value noise that tiles every `period` cells in both directions, from a table of random
/// heights, smoothly blended between cells: 0 to 1.
pub struct Noise {
    period: usize,
    cells: Vec<f32>,
}

impl Noise {
    pub fn new(period: usize, seed: u32) -> Self {
        let mut rng = Rng::new(seed);
        Noise {
            period,
            cells: (0..period * period).map(|_| rng.f()).collect(),
        }
    }

    pub fn at(&self, p: Vec2) -> f32 {
        let n = self.period as i64;
        let cell = |x: i64, y: i64| {
            self.cells[(y.rem_euclid(n) * n + x.rem_euclid(n)) as usize]
        };
        let (x0, y0) = (p.x.floor(), p.y.floor());
        let f = p - Vec2::new(x0, y0);
        let s = f * f * (Vec2::splat(3.0) - 2.0 * f);
        let (x, y) = (x0 as i64, y0 as i64);
        let a = cell(x, y) + (cell(x + 1, y) - cell(x, y)) * s.x;
        let b = cell(x, y + 1) + (cell(x + 1, y + 1) - cell(x, y + 1)) * s.x;
        a + (b - a) * s.y
    }
}

/// The brightness of an impact's star at `p` (the card's -1 to 1): a pinpoint core, four
/// thin long rays and four shorter diagonal ones, over a faint glow; as Diablo IV's and Path
/// of Exile 2's hits flash, a hard star a frame or two long rather than a soft disc.
pub fn star(p: Vec2) -> f32 {
    let ray = |a: f32, b: f32, len: f32, w: f32| {
        let t = (1.0 - a.abs() / len).max(0.0);
        (-b.abs() / w).exp() * t * t * t
    };
    let d = Vec2::new(p.x + p.y, p.y - p.x) * std::f32::consts::FRAC_1_SQRT_2;
    let r = p.length();
    let rays = ray(p.x, p.y, 1.0, 0.018).max(ray(p.y, p.x, 1.0, 0.018))
        + 0.6 * ray(d.x, d.y, 0.55, 0.014).max(ray(d.y, d.x, 0.55, 0.014));
    let core = (-r * r / 0.004).exp();
    let glow = 0.22 * (1.0 - r).max(0.0).powi(4);
    (rays + core + glow).min(1.0)
}

/// The star as a white texture, its brightness in the alpha (for an additive card).
pub fn star_image() -> Image {
    let n = 128;
    image(n, n, |x, y| {
        let p = (Vec2::new(x as f32, y as f32) + 0.5) / n as f32 * 2.0 - 1.0;
        [1.0, 1.0, 1.0, star(p)]
    })
}

/// An sRGB image of `w` by `h` from a function of the pixel to its colour and alpha.
pub fn image(w: u32, h: u32, f: impl Fn(u32, u32) -> [f32; 4]) -> Image {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let c = f(x, y);
            data.extend(c.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8));
        }
    }
    Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

pub fn smooth(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The noise tiles: a period over, it is the same.
    #[test]
    fn noise_tiles() {
        let n = Noise::new(4, 7);
        for p in [Vec2::new(0.3, 0.7), Vec2::new(2.9, 1.1)] {
            assert!((n.at(p) - n.at(p + Vec2::new(4.0, 4.0))).abs() < 1e-5);
            assert!((0.0..=1.0).contains(&n.at(p)));
        }
    }

    /// A streak leaves along its way, slows, and falls: its speed shrinks and points down.
    #[test]
    fn sparks_slow_and_fall() {
        let (p0, v0) = spark_at(Vec3::X, 8.0, 0.02);
        let (p1, v1) = spark_at(Vec3::X, 8.0, 0.4);
        assert!(p1.x > p0.x && p0.x > 0.0);
        assert!(v1.length() < v0.length());
        assert!(v1.y < 0.0 && p1.y < 0.0);
    }

    /// The star is hard, not a disc: full at its core, still bright out along a ray, dark
    /// just off the ray at the same distance, and nothing at the card's corners.
    #[test]
    fn the_star_is_rays_not_a_disc() {
        assert!(star(Vec2::ZERO) > 0.99);
        let on = star(Vec2::new(0.5, 0.0));
        let off = star(Vec2::new(0.5, 0.15));
        assert!(on > 0.1 && off < on / 4.0, "on the ray {on}, off it {off}");
        assert!(star(Vec2::new(0.95, 0.95)) < 0.01);
    }
}
