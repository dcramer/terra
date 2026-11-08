//! What the ruin's effects share: the clock every effect is a function of (so a still is
//! one chosen moment and a film plays them), the camera's facing, the fires' places, and
//! textures made in code. The hits' and the spell's flashes, sparks, smoke and lights are the
//! biome kit's presets (kit/effects.rs).

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
        .add_systems(PreUpdate, tick);
}

fn tick(
    time: Res<Time>,
    args: Res<Args>,
    mut clock: ResMut<Clock>,
    mut cam: ResMut<Cam>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
) {
    clock.still = args.still();
    clock.t = time.elapsed_secs();
    if let Ok(c) = camera.single() {
        let b = c.back();
        cam.rot = c.rotation();
        cam.upright = Quat::from_rotation_y(libm::atan2f(b.x, b.z));
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
        let cell = |x: i64, y: i64| self.cells[(y.rem_euclid(n) * n + x.rem_euclid(n)) as usize];
        let (x0, y0) = (p.x.floor(), p.y.floor());
        let f = p - Vec2::new(x0, y0);
        let s = f * f * (Vec2::splat(3.0) - 2.0 * f);
        let (x, y) = (x0 as i64, y0 as i64);
        let a = cell(x, y) + (cell(x + 1, y) - cell(x, y)) * s.x;
        let b = cell(x, y + 1) + (cell(x + 1, y + 1) - cell(x, y + 1)) * s.x;
        a + (b - a) * s.y
    }
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
}
