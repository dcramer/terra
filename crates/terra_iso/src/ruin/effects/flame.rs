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

/// The flipbook's frames as materials, bright for the flames.
#[derive(Resource)]
pub struct Book {
    pub bright: Vec<Handle<StandardMaterial>>,
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

/// How a scene's fires spark and smoke. The default is as the fires stood (the marsh's camp
/// fire keeps it); the ruin inserts its braziers' (ruin/fire.rs's `plume`) before the plugin.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Plume {
    /// Embers a fire for each metre of its flame's size.
    pub embers: f32,
    /// An ember's width (m), and its streak's least and most length (m) and how many seconds
    /// of its motion the streak spans.
    pub spark: f32,
    pub streak: (f32, f32),
    pub trail: f32,
    /// How fast an ember is thrown up (m/s x size / 0.7), and how long it lives (s).
    pub rise: (f32, f32),
    pub life: (f32, f32),
    /// Smoke puffs a fire for each metre of size, and how long each lives (s).
    pub puffs: f32,
    pub smoke_life: (f32, f32),
    /// A puff's width at birth and what it grows by over its life (x size), how fast it
    /// climbs (m/s x size / 0.7) and its most alpha.
    pub wide: (f32, f32),
    pub climb: f32,
    pub alpha: f32,
    /// The smoke low, lit by its fire, and its own colour above (linear, unlit: not exposed).
    pub lit: Vec3,
    pub dark: Vec3,
    /// How far through its life a puff has climbed out of the fire's light (0..1).
    pub cool: f32,
    /// The air the smoke leans on (x, z, m/s); embers drift on it as they slow.
    pub wind: Vec2,
}

impl Default for Plume {
    fn default() -> Self {
        Plume {
            embers: 30.0,
            spark: 0.035,
            streak: (0.03, 0.09),
            trail: 0.05,
            rise: (0.9, 1.6),
            life: (1.4, 2.8),
            puffs: 13.0,
            smoke_life: (3.5, 5.0),
            wide: (0.45, 2.6),
            climb: 0.55,
            alpha: 0.6,
            lit: Vec3::new(0.34, 0.15, 0.06),
            dark: Vec3::new(0.035, 0.035, 0.04),
            cool: 0.35,
            wind: Vec2::new(0.32, 0.14),
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Plume>()
        .add_systems(Startup, book)
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
    commands.insert_resource(Book { bright });
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
    let half = 0.38 * libm::powf(1.0 - v, 0.8) * (0.35 + 0.65 * smooth(0.0, 0.2, v)) + 1e-3;
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
            c = w[0]
                .1
                .lerp(w[1].1, ((heat - w[0].0) / (w[1].0 - w[0].0)).min(1.0));
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
    plume: Res<Plume>,
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
        // The flame stands taller than fire.rs's tongues: a brazier's 1.3 m, a torch's 0.85.
        let tall = size * 1.95;
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
        for _ in 0..(size * plume.embers) as usize {
            commands.spawn((
                Mesh3d(streak.clone()),
                MeshMaterial3d(ember_mat.clone()),
                Transform::from_scale(Vec3::ZERO),
                bevy::light::NotShadowCaster,
                Ember {
                    foot: at + Vec3::Y * size * 0.3,
                    life: rng.range(plume.life.0, plume.life.1),
                    phase: rng.range(0.0, 3.0),
                    seed: rng.f() * 100.0,
                    rise: rng.range(plume.rise.0, plume.rise.1) * size / 0.7,
                },
            ));
        }
        let puffs = (size * plume.puffs) as usize;
        // Born evenly through the longest life, so the column is unbroken.
        let gap = plume.smoke_life.1 / puffs.max(1) as f32;
        for i in 0..puffs {
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
                    life: rng.range(plume.smoke_life.0, plume.smoke_life.1),
                    phase: i as f32 * gap + rng.range(0.0, 0.3),
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
        let h = f.tall * (1.0 + 0.06 * libm::sinf(t * 7.0 + f.phase * 9.0));
        tr.translation = f.foot + Vec3::Y * h * 0.5;
        tr.rotation = cam.upright * Quat::from_rotation_z(f.lean);
        tr.scale = Vec3::new(h * 0.55, h, 1.0);
    }
}

fn core(clock: Res<Clock>, cam: Res<Cam>, mut cores: Query<(&Core, &mut Transform)>) {
    let t = clock.t;
    for (c, mut tr) in &mut cores {
        let w =
            1.0 + 0.12 * libm::sinf(t * 17.0 + c.seed) + 0.08 * libm::sinf(t * 9.0 + c.seed * 2.0);
        tr.translation = c.at;
        tr.rotation = cam.rot;
        tr.scale = Vec3::new(c.size * w, c.size * 0.8 * w, 1.0);
    }
}

/// Where an ember is `a` seconds up: thrown up on the hot air, slowing, swirling, off on the
/// wind.
fn ember_at(e: &Ember, wind: Vec2, a: f32) -> Vec3 {
    let s = e.seed;
    let start = Vec3::new(libm::sinf(s * 7.1), 0.0, libm::cosf(s * 3.7)) * 0.12;
    let swirl = Vec3::new(libm::sinf(a * 3.0 + s), 0.0, libm::cosf(a * 2.3 + s * 1.7)) * 0.18 * a;
    e.foot
        + start
        + swirl
        + Vec3::new(wind.x, 0.0, wind.y) * 0.875 * a * a
        + Vec3::Y * e.rise * (a - 0.18 * a * a)
}

/// An ember `a` seconds up: where it is, and its width and streak's length, fading as it
/// dies (`blink` its flicker, 0.5 to 1).
fn ember_shape(e: &Ember, plume: &Plume, a: f32, blink: f32) -> (Vec3, Vec3, f32, f32) {
    let p = ember_at(e, plume.wind, a);
    let v = (p - ember_at(e, plume.wind, (a - 0.03).max(0.0))) / 0.03;
    let k = 1.0 - a / e.life;
    let w = plume.spark * libm::powf(k, 0.6) * blink;
    let long = (v.length() * plume.trail).clamp(plume.streak.0, plume.streak.1) * k;
    (p, v, w, long)
}

fn embers(clock: Res<Clock>, plume: Res<Plume>, mut embers: Query<(&Ember, &mut Transform)>) {
    let t = clock.t;
    for (e, mut tr) in &mut embers {
        let a = (t + e.phase).rem_euclid(e.life);
        let blink = 0.75 + 0.25 * libm::sinf(t * 23.0 + e.seed);
        let (p, v, w, long) = ember_shape(e, &plume, a, blink);
        tr.translation = p;
        tr.rotation = Quat::from_rotation_arc(Vec3::Z, v.normalize_or(Vec3::Y));
        tr.scale = Vec3::new(w, w, long);
    }
}

/// A puff `a` seconds old: where it is, its width, and its colour and alpha (linear).
fn puff_at(p: &Puff, plume: &Plume, a: f32) -> (Vec3, f32, Vec3, f32) {
    let k = a / p.life;
    let wide = p.size * (plume.wide.0 + plume.wide.1 * k);
    let at = p.foot
        + Vec3::Y * (plume.climb * a * p.size / 0.7)
        + Vec3::new(plume.wind.x, 0.0, plume.wind.y) * a
        + Vec3::new(libm::sinf(a + p.seed), 0.0, libm::cosf(a * 0.7 + p.seed)) * 0.08 * a;
    let colour = plume.lit.lerp(plume.dark, smooth(0.0, plume.cool, k));
    let alpha = plume.alpha * smooth(0.0, 0.12, k) * libm::powf(1.0 - k, 1.5);
    (at, wide, colour, alpha)
}

fn smoke(
    clock: Res<Clock>,
    cam: Res<Cam>,
    plume: Res<Plume>,
    mut puffs: Query<(&Puff, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = clock.t;
    for (p, mut tr, mat) in &mut puffs {
        let a = (t + p.phase).rem_euclid(p.life);
        let (at, wide, c, alpha) = puff_at(p, &plume, a);
        tr.translation = at;
        tr.rotation = cam.rot * Quat::from_rotation_z(p.seed + a * 0.25);
        tr.scale = Vec3::new(wide, wide, 1.0);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.base_color = Color::linear_rgba(c.x, c.y, c.z, alpha);
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
        let cover = |v: f32| {
            (0..32)
                .map(|i| flame(&b, &f, &s, i as f32 / 32.0, v, 0.3)[3])
                .sum::<f32>()
        };
        assert!(
            cover(0.92) < 0.5 * cover(0.2),
            "{} {}",
            cover(0.92),
            cover(0.2)
        );
    }

    /// The ruin's braziers smoke and spark where the camera sees it: each ember at least the
    /// 7 cm that shows at 960 px while young, rising and drifting downwind as it dims; the
    /// smoke a column climbing over 2.5 m, leaning downwind, narrow at the flame and wider
    /// above, warm low and dark high.
    #[test]
    fn the_ruins_braziers_smoke_and_spark_downwind() {
        let plume = crate::ruin::fire::plume();
        let wind = Vec3::new(plume.wind.x, 0.0, plume.wind.y).normalize();
        let size = 0.7;
        let e = Ember {
            foot: Vec3::ZERO,
            life: plume.life.0,
            phase: 0.0,
            seed: 3.0,
            rise: plume.rise.0 * size / 0.7,
        };
        let (_, _, young, long) = ember_shape(&e, &plume, 0.1, 1.0);
        assert!(
            young >= 0.065 && long >= young,
            "young ember {young} x {long} m"
        );
        let (p, _, old, _) = ember_shape(&e, &plume, e.life * 0.95, 1.0);
        assert!(p.y > 1.2 && p.dot(wind) > 0.3, "the ember ends at {p}");
        assert!(old < 0.4 * young, "the ember fades: {old} of {young}");
        let puff = Puff {
            foot: Vec3::ZERO,
            size,
            life: plume.smoke_life.1,
            phase: 0.0,
            seed: 1.0,
        };
        let (low, w0, c0, a0) = puff_at(&puff, &plume, puff.life * 0.15);
        let (high, w1, c1, _) = puff_at(&puff, &plume, puff.life * 0.85);
        assert!(
            high.y - low.y > 2.0 && high.y > 2.5,
            "the column climbs {low} to {high}"
        );
        assert!(
            (high - low).dot(wind) > 0.8,
            "the column leans downwind: {low} to {high}"
        );
        assert!(
            w0 < 0.6 && w1 > 2.0 * w0,
            "narrow low, wider high: {w0} to {w1} m"
        );
        assert!(
            c0.x > 4.0 * c1.x && c0.x > c0.z * 3.0,
            "warm low {c0}, dark high {c1}"
        );
        assert!(a0 > 0.25, "the smoke shows low: alpha {a0}");
    }

    /// The book plays on and wraps.
    #[test]
    fn frames_wrap() {
        assert_eq!(frame(0.0, 1.0, 0.0), 0);
        assert_eq!(frame(FRAMES as f32 / FPS, 1.0, 0.0), 0);
        assert!(frame(0.5, 1.0, 0.0) < FRAMES);
    }
}
