//! The ruin's flames as the ARPGs draw theirs: a flipbook of a burning flame (32 frames made
//! here from rising, looping noise: white-hot at its root, yellow, orange, a red ragged edge
//! breaking into tongues) played on three upright cards a fire, each at its own rate and
//! phase so no loop shows; a white-hot core under them for the bloom; embers that streak up
//! on the hot air and drift off on the wind; and smoke that rises off the flame's top, lit
//! warm from below, widening, greying and fading. They stand where fire.rs's fires are
//! (kit.rs finds them), over its halos and lights, in place of its tongue cards.

use super::kit::{Cam, Clock, Fires, Noise, glow, image, smooth};
use crate::forest::Rng;
use bevy::prelude::*;

/// The flipbook: columns, rows, a frame's pixels, and frames a second.
const COLS: u32 = 8;
const ROWS: u32 = 4;
const FW: u32 = 64;
const FH: u32 = 128;
const FPS: f32 = 24.0;
pub const FRAMES: usize = (COLS * ROWS) as usize;
/// A flame card's brightness (the texture's white-hot root times this, in linear light).
const HOT: f32 = 3.2;

/// The flipbook's frames as materials, bright for the flames and dim for their reflections
/// (pool.rs's).
#[derive(Resource)]
pub struct Book {
    pub bright: Vec<Handle<StandardMaterial>>,
    pub dim: Vec<Handle<StandardMaterial>>,
}

/// A flame card: its foot, height, rate and phase through the book, and lean.
#[derive(Component)]
pub struct Flip {
    pub foot: Vec3,
    pub tall: f32,
    pub rate: f32,
    pub phase: f32,
    pub lean: f32,
}

#[derive(Component)]
struct Core {
    at: Vec3,
    size: f32,
    seed: f32,
}

#[derive(Component)]
struct Ember {
    foot: Vec3,
    life: f32,
    phase: f32,
    seed: f32,
    rise: f32,
}

#[derive(Component)]
struct Puff {
    foot: Vec3,
    size: f32,
    life: f32,
    phase: f32,
    seed: f32,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, book)
        .add_systems(PostStartup, spawn.after(super::kit::Found))
        .add_systems(Update, (flip, core, embers, smoke));
}

/// Which frame a card shows at `t`.
pub fn frame(t: f32, rate: f32, phase: f32) -> usize {
    ((t * rate + phase) * FPS).rem_euclid(FRAMES as f32) as usize % FRAMES
}

fn book(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let atlas = images.add(flipbook());
    let make = |materials: &mut Assets<StandardMaterial>, k: f32| -> Vec<_> {
        (0..FRAMES as u32)
            .map(|f| {
                let (c, r) = (f % COLS, f / COLS);
                materials.add(StandardMaterial {
                    uv_transform: bevy::math::Affine2::from_scale_angle_translation(
                        Vec2::new(1.0 / COLS as f32, 1.0 / ROWS as f32),
                        0.0,
                        Vec2::new(c as f32 / COLS as f32, r as f32 / ROWS as f32),
                    ),
                    double_sided: true,
                    cull_mode: None,
                    ..glow(LinearRgba::rgb(k, k, k), Some(atlas.clone()))
                })
            })
            .collect()
    };
    let bright = make(&mut materials, HOT);
    let dim = make(&mut materials, HOT * 0.3);
    commands.insert_resource(Book { bright, dim });
}

/// The flipbook's atlas: each frame a flame whose noise has risen a 32nd further, so the
/// last frame runs into the first.
pub fn flipbook() -> Image {
    let big = Noise::new(4, 0xf1a3);
    let fine = Noise::new(8, 0x7e11);
    let sway = Noise::new(4, 0x51de);
    image(COLS * FW, ROWS * FH, |x, y| {
        let f = (y / FH) * COLS + x / FW;
        let p = f as f32 / FRAMES as f32;
        let u = (x % FW) as f32 / FW as f32;
        let v = 1.0 - (y % FH) as f32 / FH as f32;
        flame(&big, &fine, &sway, u, v, p)
    })
}

/// One pixel of the flame at `u` across and `v` up its card, `p` through its loop.
fn flame(big: &Noise, fine: &Noise, sway: &Noise, u: f32, v: f32, p: f32) -> [f32; 4] {
    // The noise rises a whole period of its table over the loop.
    let n1 = big.at(Vec2::new(u * 3.0, v * 2.0 - p * 4.0));
    let n2 = fine.at(Vec2::new(u * 6.0, v * 4.0 - p * 8.0));
    let cx = 0.5 + (sway.at(Vec2::new(1.5, v * 1.5 - p * 4.0)) - 0.5) * 0.35 * v;
    let half = 0.38 * (1.0 - v).powf(0.8) * (0.35 + 0.65 * smooth(0.0, 0.2, v)) + 1e-3;
    let turb = 0.65 * n1 + 0.35 * n2;
    let mut d = 1.0 - (u - cx).abs() / half;
    d += (turb - 0.5) * (0.5 + 1.4 * v);
    d -= smooth(0.5, 1.0, v) * 0.5;
    let a = smooth(0.0, 0.35, d);
    let heat = (d * (1.0 - 0.7 * v)).clamp(0.0, 1.0);
    let ramp = [
        (0.0, Vec3::new(0.5, 0.05, 0.01)),
        (0.35, Vec3::new(1.0, 0.32, 0.04)),
        (0.6, Vec3::new(1.0, 0.68, 0.22)),
        (0.85, Vec3::new(1.0, 0.95, 0.8)),
        (1.0, Vec3::new(1.0, 1.0, 0.95)),
    ];
    let mut c = ramp[0].1;
    for w in ramp.windows(2) {
        if heat >= w[0].0 {
            c = w[0].1.lerp(w[1].1, ((heat - w[0].0) / (w[1].0 - w[0].0)).min(1.0));
        }
    }
    let c = c * (0.3 + 0.7 * heat);
    let s = Color::linear_rgb(c.x, c.y, c.z).to_srgba();
    [s.red, s.green, s.blue, a]
}

fn spawn(
    mut commands: Commands,
    fires: Res<Fires>,
    book: Res<Book>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut rng = Rng::new(0xf1a9e);
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    let streak = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let soft = images.add(crate::motes::soft_image());
    let puff_image = images.add(puff());
    let core_mat = materials.add(glow(LinearRgba::rgb(5.0, 3.4, 1.5), Some(soft.clone())));
    let ember_mat = materials.add(glow(LinearRgba::rgb(9.0, 2.6, 0.4), None));
    for &(at, size) in &fires.0 {
        // The flame stands taller than fire.rs's tongues: a brazier's a metre, a torch's 0.65.
        let tall = size * 1.5;
        for (i, (dx, k)) in [(0.0, 1.0), (-0.3, 0.72), (0.32, 0.66)].iter().enumerate() {
            commands.spawn((
                Mesh3d(card.clone()),
                MeshMaterial3d(book.bright[0].clone()),
                Transform::default(),
                bevy::light::NotShadowCaster,
                Flip {
                    foot: at + Vec3::new(dx * size * 0.5, -0.05 * tall, 0.02 * i as f32),
                    tall: tall * k,
                    rate: rng.range(0.85, 1.15),
                    phase: rng.range(0.0, 2.0),
                    lean: dx * 0.25,
                },
            ));
        }
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(core_mat.clone()),
            Transform::default(),
            bevy::light::NotShadowCaster,
            Core {
                at: at + Vec3::Y * size * 0.22,
                size: size * 0.7,
                seed: rng.f() * 30.0,
            },
        ));
        for _ in 0..(size * 30.0) as usize {
            commands.spawn((
                Mesh3d(streak.clone()),
                MeshMaterial3d(ember_mat.clone()),
                Transform::from_scale(Vec3::ZERO),
                bevy::light::NotShadowCaster,
                Ember {
                    foot: at + Vec3::Y * size * 0.3,
                    life: rng.range(1.4, 2.8),
                    phase: rng.range(0.0, 3.0),
                    seed: rng.f() * 100.0,
                    rise: rng.range(0.9, 1.6) * size / 0.7,
                },
            ));
        }
        for i in 0..(size * 13.0) as usize {
            commands.spawn((
                Mesh3d(card.clone()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::NONE,
                    base_color_texture: Some(puff_image.clone()),
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                })),
                Transform::default(),
                bevy::light::NotShadowCaster,
                Puff {
                    foot: at + Vec3::Y * tall * 0.75,
                    size,
                    life: rng.range(3.5, 5.0),
                    phase: i as f32 * 0.55 + rng.range(0.0, 0.3),
                    seed: rng.f() * 10.0,
                },
            ));
        }
    }
}

/// A smoke puff: a soft blob, its edge broken by noise.
fn puff() -> Image {
    let n = Noise::new(6, 0x5a0e);
    let s = 64;
    image(s, s, |x, y| {
        let p = (Vec2::new(x as f32, y as f32) + 0.5) / s as f32 * 2.0 - 1.0;
        let r = p.length() + (n.at((p + 1.0) * 3.0) - 0.5) * 0.5;
        let a = smooth(1.0, 0.2, r);
        [1.0, 1.0, 1.0, a * a]
    })
}

/// Each card upright to the camera, on its own frame of the book, breathing taller and
/// shorter a little.
fn flip(
    clock: Res<Clock>,
    cam: Res<Cam>,
    book: Res<Book>,
    mut cards: Query<(&Flip, &mut Transform, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let t = clock.t;
    for (f, mut tr, mut mat) in &mut cards {
        let i = frame(t, f.rate, f.phase);
        if mat.0 != book.bright[i] {
            mat.0 = book.bright[i].clone();
        }
        let h = f.tall * (1.0 + 0.06 * (t * 7.0 + f.phase * 9.0).sin());
        tr.translation = f.foot + Vec3::Y * h * 0.5;
        tr.rotation = cam.upright * Quat::from_rotation_z(f.lean);
        tr.scale = Vec3::new(h * 0.55, h, 1.0);
    }
}

fn core(clock: Res<Clock>, cam: Res<Cam>, mut cores: Query<(&Core, &mut Transform)>) {
    let t = clock.t;
    for (c, mut tr) in &mut cores {
        let w = 1.0 + 0.12 * (t * 17.0 + c.seed).sin() + 0.08 * (t * 9.0 + c.seed * 2.0).sin();
        tr.translation = c.at;
        tr.rotation = cam.rot;
        tr.scale = Vec3::new(c.size * w, c.size * 0.8 * w, 1.0);
    }
}

/// Where an ember is `a` seconds up: thrown up on the hot air, slowing, swirling, off on the
/// wind.
fn ember_at(e: &Ember, a: f32) -> Vec3 {
    let s = e.seed;
    let start = Vec3::new((s * 7.1).sin(), 0.0, (s * 3.7).cos()) * 0.12;
    let swirl = Vec3::new((a * 3.0 + s).sin(), 0.0, (a * 2.3 + s * 1.7).cos()) * 0.18 * a;
    e.foot
        + start
        + swirl
        + Vec3::new(0.28, 0.0, 0.12) * a * a
        + Vec3::Y * e.rise * (a - 0.18 * a * a)
}

fn embers(clock: Res<Clock>, mut embers: Query<(&Ember, &mut Transform)>) {
    let t = clock.t;
    for (e, mut tr) in &mut embers {
        let a = (t + e.phase).rem_euclid(e.life);
        let p = ember_at(e, a);
        let v = (p - ember_at(e, (a - 0.03).max(0.0))) / 0.03;
        let k = 1.0 - a / e.life;
        let blink = 0.75 + 0.25 * (t * 23.0 + e.seed).sin();
        let w = 0.022 * k.powf(0.6) * blink;
        tr.translation = p;
        tr.rotation = Quat::from_rotation_arc(Vec3::Z, v.normalize_or(Vec3::Y));
        tr.scale = Vec3::new(w, w, (v.length() * 0.05).clamp(0.03, 0.09) * k);
    }
}

fn smoke(
    clock: Res<Clock>,
    cam: Res<Cam>,
    mut puffs: Query<(&Puff, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = clock.t;
    for (p, mut tr, mat) in &mut puffs {
        let a = (t + p.phase).rem_euclid(p.life);
        let k = a / p.life;
        let wide = p.size * (0.45 + 2.6 * k);
        tr.translation = p.foot
            + Vec3::Y * (0.55 * a * p.size / 0.7)
            + Vec3::new(0.32, 0.0, 0.14) * a
            + Vec3::new((a + p.seed).sin(), 0.0, (a * 0.7 + p.seed).cos()) * 0.08 * a;
        tr.rotation = cam.rot * Quat::from_rotation_z(p.seed + a * 0.25);
        tr.scale = Vec3::new(wide, wide, 1.0);
        let warm = Vec3::new(0.34, 0.15, 0.06).lerp(Vec3::new(0.05, 0.05, 0.056), smooth(0.0, 0.35, k));
        let alpha = 0.42 * smooth(0.0, 0.12, k) * (1.0 - k).powf(1.5);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.base_color = Color::linear_rgba(warm.x, warm.y, warm.z, alpha);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The flame is white-hot at its root's middle, ragged and dark toward its edge and tip,
    /// clear beside it.
    #[test]
    fn flame_is_hot_at_root() {
        let (b, f, s) = (Noise::new(4, 1), Noise::new(8, 2), Noise::new(4, 3));
        let root = flame(&b, &f, &s, 0.5, 0.15, 0.0);
        let side = flame(&b, &f, &s, 0.02, 0.15, 0.0);
        assert!(root[3] > 0.9 && root[0] > 0.7, "root {root:?}");
        assert!(side[3] < 0.05, "side {side:?}");
        // Across the card, less of the tip burns than of the root.
        let cover = |v: f32| (0..32).map(|i| flame(&b, &f, &s, i as f32 / 32.0, v, 0.3)[3]).sum::<f32>();
        assert!(cover(0.92) < 0.5 * cover(0.2), "{} {}", cover(0.92), cover(0.2));
    }

    /// The book plays on and wraps.
    #[test]
    fn frames_wrap() {
        assert_eq!(frame(0.0, 1.0, 0.0), 0);
        assert_eq!(frame(FRAMES as f32 / FPS, 1.0, 0.0), 0);
        assert!(frame(0.5, 1.0, 0.0) < FRAMES);
    }
}
