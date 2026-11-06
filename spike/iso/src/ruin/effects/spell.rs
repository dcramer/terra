//! The hero's spell: a volley of three arcane bolts from his off hand at the tree monster,
//! as Path of Exile's and Diablo IV's are drawn: each bolt a small white-hot core drawn out
//! along its flight inside a thin blue halo (the bloom does the rest), a sharp streak behind
//! it (a white inner ribbon in a blue outer one) fading to nothing, its own light casting a
//! blue pool on the stones under it; and where it lands, a hard star flash a few frames
//! long, blue-white sparks thrown back off the bark, and a short bright light (Last Epoch's
//! "a number of lights" in a skill). Every part is a function of the clock: in a still the
//! first bolt has just struck and the other two are in the air.

use super::Stage;
use super::kit::{Burst, Cam, Clock, Noise, glow, image, smooth, sparks, star_image};
use super::mark::{Mark, mark, mark_image};
use crate::forest::Rng;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// The volley repeats every `PERIOD` s; bolt i leaves `i * STAGGER` in and flies `TRAVEL`.
const PERIOD: f32 = 3.2;
const STAGGER: f32 = 0.2;
const TRAVEL: f32 = 0.5;
const BOLTS: usize = 3;
/// The still's moment in the volley: bolt 0 struck 0.08 s ago, 1 and 2 in the air.
const STILL: f32 = 0.58;
/// How far behind a bolt its trail reaches, in the flight's share.
/// 0.35 left "thin streaks with no trail" (the lead's judges, 2026-11-06): longer.
const TAIL: f32 = 0.5;
const TRAIL_POINTS: usize = 16;
/// The core's radius and how far it is drawn out along the flight, the halo's size (m).
const CORE: f32 = 0.075;
const CORE_LENGTH: f32 = 0.36;
const HALO: f32 = 0.45;
/// The bolt's own light, in lumens, lighting the bodies it passes. Riding 2 m up it spread
/// a quarter of the braziers' light evenly over 4 m of floor: a violet wash, no pool (the
/// crop of blind-ruin6/a.png; all three judges: "the bolts light nothing").
const BOLT_LM: f32 = 30_000.0;
/// So a second light rides under each bolt, `POOL_UP` over the floor: at 0.5 m its light
/// falls to a fifth a metre out, a tight cold pool under the spell (d4-lightning).
const POOL_LM: f32 = 25_000.0;
const POOL_UP: f32 = 0.5;
/// The trails' widths at the bolt (m): the outer blue streak and the white one inside it.
/// 0.11 and 0.035 were a hairline at 70 px a metre; as wide as the head, tapering to the tail.
const OUTER: f32 = 0.24;
const INNER: f32 = 0.07;
/// The impact's light at its height, in lumens, and how fast it dies (s).
/// 40k dying in 0.09 s left a faint burst at the still's 0.08 s (the judges).
const FLASH_LM: f32 = 150_000.0;
const FLASH_FALL: f32 = 0.15;
/// The impact's star: its size at its height (m) and how long it lasts (s). A 1.5 m white
/// star for 0.28 s hid the target (and in the forest the caster) in every biome's judge
/// (2026-11-06): now a small blue glint gone in a tenth, the impact carried by the shell,
/// the sparks, the light and the mark, as Diablo IV's and Path of Exile 2's impacts are
/// built (a hot flash a few frames long, an expanding shell, debris, a light, a decal).
const STAR: f32 = 0.6;
const STAR_LIFE: f32 = 0.1;
/// The shell: a ring card swelling from `SHELL.0` to `SHELL.1` m across over `SHELL_LIFE` s.
const SHELL: (f32, f32) = (0.3, 1.7);
const SHELL_LIFE: f32 = 0.26;
/// The impact's light on the floor under it, and how fast it dies (s).
const GLINT_LM: f32 = 70_000.0;
const GLINT_FALL: f32 = 0.3;
/// The frost mark under the impact: its size (m) and life (s).
const MARK: f32 = 1.5;
const MARK_LIFE: f32 = 1.6;

#[derive(Component)]
enum Part {
    Core,
    Glow,
    Light,
    Pool,
    Trail(Handle<Mesh>, f32),
    Flash(Handle<StandardMaterial>),
    Shell(Handle<StandardMaterial>),
    Glint,
    Mark,
    Bang,
}

#[derive(Component)]
struct Bolt(usize, Part);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, fly);
}

/// Where bolt `i` is at `s` of its flight: from the hero's off hand to the bark, rising in
/// an arc and swinging to its own side so the three do not lie in one line.
/// The caster's chest and the target's are the stage's (mod.rs).
fn path(stage: &Stage, i: usize, s: f32) -> Vec3 {
    let (hero, bark) = (stage.hero(), stage.bark());
    let dir = (bark - hero).normalize();
    let from = hero + dir * 0.45;
    let to = bark - dir * 0.35;
    let side = dir.cross(Vec3::Y).normalize();
    let swing = [-0.45, 0.4, 0.05][i];
    from.lerp(to, s)
        + Vec3::Y * 0.7 * 4.0 * s * (1.0 - s)
        + side * swing * (std::f32::consts::PI * s).sin()
}

/// How far bolt `i` is through its flight, and how long since it struck, at volley time `p`.
fn flight(i: usize, p: f32) -> (f32, f32) {
    let left = i as f32 * STAGGER;
    ((p - left) / TRAVEL, p - left - TRAVEL)
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut decals: ResMut<Assets<super::mark::Decal>>,
    land: Option<Res<crate::terrain::Land>>,
    stage: Res<Stage>,
) {
    let stage = &*stage;
    let mut rng = Rng::new(0x5be11);
    let soft = images.add(crate::motes::soft_image());
    let star = images.add(star_image());
    let ring = images.add(shell_image());
    let frost = images.add(mark_image());
    let ball = meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap());
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    let streak = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let core = materials.add(glow(LinearRgba::rgb(32.0, 36.0, 44.0), None));
    let halo = materials.add(glow(LinearRgba::rgb(0.35, 0.85, 3.4), Some(soft.clone())));
    let ribbon_of = |c: LinearRgba| StandardMaterial {
        double_sided: true,
        cull_mode: None,
        ..glow(c, None)
    };
    let outer = materials.add(ribbon_of(LinearRgba::rgb(0.9, 2.2, 8.0)));
    let inner = materials.add(ribbon_of(LinearRgba::rgb(6.0, 9.0, 16.0)));
    let spark = materials.add(glow(LinearRgba::rgb(8.0, 12.0, 24.0), None));
    let off = Transform::from_scale(Vec3::ZERO);
    for i in 0..BOLTS {
        commands.spawn((Mesh3d(ball.clone()), MeshMaterial3d(core.clone()), off, Bolt(i, Part::Core)));
        commands.spawn((Mesh3d(card.clone()), MeshMaterial3d(halo.clone()), off, Bolt(i, Part::Glow)));
        commands.spawn((
            PointLight {
                color: Color::srgb(0.4, 0.6, 1.0),
                intensity: 0.0,
                range: 4.5,
                radius: 0.04,
                ..default()
            },
            Transform::default(),
            Bolt(i, Part::Light),
        ));
        commands.spawn((
            PointLight {
                color: Color::srgb(0.3, 0.55, 1.0),
                intensity: 0.0,
                range: 2.6,
                radius: 0.05,
                ..default()
            },
            Transform::default(),
            Bolt(i, Part::Pool),
        ));
        for (material, width) in [(&outer, OUTER), (&inner, INNER)] {
            let mesh = meshes.add(ribbon(&[], Vec3::ZERO, width));
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::default(),
                NoFrustumCulling,
                bevy::light::NotShadowCaster,
                Bolt(i, Part::Trail(mesh, width)),
            ));
        }
        let flash = materials.add(glow(LinearRgba::rgb(6.0, 8.0, 14.0), Some(star.clone())));
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(flash.clone()),
            off,
            Bolt(i, Part::Flash(flash)),
        ));
        commands.spawn((
            PointLight {
                color: Color::srgb(0.6, 0.75, 1.0),
                intensity: 0.0,
                range: 7.0,
                radius: 0.2,
                ..default()
            },
            Transform::from_translation(path(stage, i, 1.0)),
            Bolt(i, Part::Bang),
        ));
        let at = path(stage, i, 1.0);
        let burst = commands.spawn((Burst { at, age: -1.0 }, Bolt(i, Part::Bang))).id();
        let back = (stage.hero() - stage.bark()).normalize() + Vec3::Y * 0.6;
        sparks(&mut commands, burst, &streak, &spark, &mut rng, 30, back, 0.9, (3.0, 9.0), 0.026);
        let shell = materials.add(glow(LinearRgba::rgb(1.2, 2.6, 8.0), Some(ring.clone())));
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(shell.clone()),
            off,
            bevy::light::NotShadowCaster,
            Bolt(i, Part::Shell(shell)),
        ));
        let ground = stage.ground(land.as_deref(), at.xz());
        commands.spawn((
            PointLight {
                color: Color::srgb(0.35, 0.6, 1.0),
                intensity: 0.0,
                range: 3.5,
                radius: 0.1,
                ..default()
            },
            Transform::from_translation(ground + Vec3::Y * POOL_UP),
            Bolt(i, Part::Glint),
        ));
        let m = mark(
            &mut commands,
            &mut decals,
            &frost,
            ground,
            MARK,
            MARK_LIFE,
            LinearRgba::rgb(0.9, 2.2, 6.5),
            LinearRgba::new(0.03, 0.045, 0.07, 0.75),
        );
        commands.entity(m).insert(Bolt(i, Part::Mark));
    }
}

/// The impact's shell as a white texture, its brightness in the alpha: a shockwave as the
/// ARPGs' impact flipbooks draw one (an eroded shell, not a drawn circle: a perfect ring
/// read as "UI circles", the lead's judges, 2026-11-06), its rim wandering in and out,
/// thick and thin, broken into arcs by noise round it, with streaks thrown inward.
fn shell_image() -> Image {
    let (round, arcs) = (Noise::new(8, 0x5e11), Noise::new(16, 0x5e12));
    let n = 128;
    image(n, n, |x, y| {
        let p = (Vec2::new(x as f32, y as f32) + 0.5) / n as f32 * 2.0 - 1.0;
        let a = p.y.atan2(p.x) / std::f32::consts::TAU + 0.5;
        let (wander, broken) = (round.at(Vec2::new(a * 8.0, 0.5)), arcs.at(Vec2::new(a * 16.0, p.length() * 3.0)));
        [1.0, 1.0, 1.0, shell(p.length(), wander, broken)]
    })
}

/// The shell's brightness at `r` of its radius, its rim moved and thickened by `wander`
/// and broken by `broken` (both 0 to 1, noise round it).
fn shell(r: f32, wander: f32, broken: f32) -> f32 {
    let at = 0.8 + 0.1 * (wander - 0.5) * 2.0;
    let width = 0.035 + 0.06 * wander;
    let gaps = smooth(0.3, 0.62, broken);
    let rim = (-((r - at) / width).powi(2)).exp() * gaps;
    let inside = 0.3 * (r / at).powi(5) * (1.0 - smooth(at, at + 0.04, r)) * smooth(0.45, 0.8, broken);
    (rim + inside).min(1.0) * (1.0 - smooth(0.93, 0.99, r))
}

/// A ribbon `width` wide at the bolt through `points` (tail first) facing the camera at
/// `eye`, widening and brightening from nothing at the tail to the bolt.
fn ribbon(points: &[Vec3], eye: Vec3, width: f32) -> Mesh {
    let n = points.len();
    let mut pos = Vec::with_capacity(n * 2);
    let mut col = Vec::with_capacity(n * 2);
    let mut idx = Vec::new();
    for (j, &p) in points.iter().enumerate() {
        let next = points[(j + 1).min(n - 1)];
        let prev = points[j.saturating_sub(1)];
        let along = (next - prev).normalize_or(Vec3::X);
        let side = along.cross(eye - p).normalize_or(Vec3::Y);
        let k = j as f32 / (n - 1).max(1) as f32;
        let w = width * k.powf(0.7);
        pos.push((p + side * w * 0.5).to_array());
        pos.push((p - side * w * 0.5).to_array());
        // k cubed left the trail's back half invisible: fading slower, it reads as a trail.
        let a = k.powf(1.6);
        col.extend([[1.0, 1.0, 1.0, a]; 2]);
        if j + 1 < n {
            let b = (j * 2) as u32;
            idx.extend([b, b + 1, b + 2, b + 1, b + 3, b + 2]);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; pos.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx))
}

fn fly(
    clock: Res<Clock>,
    cam: Res<Cam>,
    mut parts: Query<(
        &Bolt,
        Option<&mut Transform>,
        Option<&mut PointLight>,
        Option<&mut Burst>,
        Option<&mut Mark>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stage: Res<Stage>,
) {
    let stage = &*stage;
    let p = if clock.still { STILL } else { clock.t.rem_euclid(PERIOD) };
    for (Bolt(i, part), tr, light, burst, mark) in &mut parts {
        let (s, struck) = flight(*i, p);
        let flying = (0.0..1.0).contains(&s);
        let head = path(stage, *i,s.clamp(0.0, 1.0));
        match part {
            Part::Core => {
                // Drawn out along the flight, as a fast thing reads.
                let mut tr = tr.unwrap();
                let ahead = path(stage, *i, (s + 0.02).min(1.0)) - path(stage, *i, (s - 0.02).max(0.0));
                tr.translation = head;
                tr.rotation = Quat::from_rotation_arc(Vec3::Z, ahead.normalize_or(Vec3::Z));
                tr.scale = if flying { Vec3::new(CORE, CORE, CORE_LENGTH) } else { Vec3::ZERO };
            }
            Part::Glow => {
                let mut tr = tr.unwrap();
                tr.translation = head;
                tr.rotation = cam.rot;
                tr.scale = Vec3::splat(if flying { HALO } else { 0.0 });
            }
            Part::Light => {
                tr.unwrap().translation = head;
                light.unwrap().intensity = if flying { BOLT_LM } else { 0.0 };
            }
            Part::Pool => {
                tr.unwrap().translation = Vec3::new(head.x, stage.floor + POOL_UP, head.z);
                light.unwrap().intensity = if flying { POOL_LM } else { 0.0 };
            }
            Part::Trail(mesh, width) => {
                let tail = (s - TAIL).max(0.0);
                let tip = s.min(1.0);
                let points: Vec<Vec3> = if s > 0.0 && tail < tip {
                    (0..TRAIL_POINTS)
                        .map(|j| path(stage, *i,tail + (tip - tail) * j as f32 / (TRAIL_POINTS - 1) as f32))
                        .collect()
                } else {
                    Vec::new()
                };
                if let Some(mut m) = meshes.get_mut(mesh) {
                    *m = ribbon(&points, cam.pos, *width);
                }
            }
            Part::Flash(mat) => {
                // A hard star at full size at once, shrinking a little as it dies; each
                // bolt's turned its own way so the three are not one stamp.
                let mut tr = tr.unwrap();
                let on = (0.0..STAR_LIFE).contains(&struck);
                let k = (struck / STAR_LIFE).clamp(0.0, 1.0);
                tr.translation = path(stage, *i, 1.0);
                tr.rotation = cam.rot * Quat::from_rotation_z(0.35 + *i as f32 * 0.5);
                tr.scale = Vec3::splat(if on { STAR * (1.0 - 0.35 * k) } else { 0.0 });
                if let Some(mut m) = materials.get_mut(mat) {
                    let f = (1.0 - k).powf(1.5);
                    m.base_color = LinearRgba::rgb(4.0 * f, 7.0 * f, 18.0 * f).into();
                }
            }
            Part::Shell(mat) => {
                let mut tr = tr.unwrap();
                let k = (struck / SHELL_LIFE).clamp(0.0, 1.0);
                let on = (0.0..SHELL_LIFE).contains(&struck);
                tr.translation = path(stage, *i, 1.0);
                // Each bolt's shell turned its own way and turning as it swells, so its
                // gaps are not one stamp.
                tr.rotation = cam.rot * Quat::from_rotation_z(*i as f32 * 2.1 + 0.8 * k);
                let grow = 1.0 - (1.0 - k) * (1.0 - k);
                tr.scale = Vec3::splat(if on { SHELL.0 + (SHELL.1 - SHELL.0) * grow } else { 0.0 });
                if let Some(mut m) = materials.get_mut(mat) {
                    let f = (1.0 - k).powf(1.3);
                    m.base_color = LinearRgba::rgb(1.2 * f, 2.6 * f, 8.0 * f).into();
                }
            }
            Part::Glint => {
                light.unwrap().intensity =
                    if struck >= 0.0 { GLINT_LM * (-struck / GLINT_FALL).exp() } else { 0.0 };
            }
            Part::Mark => mark.unwrap().age = struck,
            Part::Bang => {
                if let Some(mut light) = light {
                    light.intensity = if struck >= 0.0 {
                        FLASH_LM * (-struck / FLASH_FALL).exp()
                    } else {
                        0.0
                    };
                }
                if let Some(mut b) = burst {
                    b.age = struck;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In the still, one bolt has struck and the others are on their way, nearer the target
    /// the earlier they left.
    #[test]
    fn the_still_shows_a_strike_and_bolts_in_flight() {
        let f: Vec<_> = (0..BOLTS).map(|i| flight(i, STILL)).collect();
        assert!((0.0..0.2).contains(&f[0].1), "bolt 0 struck {} s ago", f[0].1);
        assert!(f[1].0 > f[2].0 && f[2].0 > 0.1 && f[1].0 < 1.0);
    }

    /// The pool under a bolt outshines the bolt's own light on the floor by far, and falls
    /// to under a quarter a metre out, so it reads as a pool, not a wash.
    #[test]
    fn the_pool_under_a_bolt_is_tight_and_bright() {
        let st = Stage::default();
        let high = (0..=10).map(|j| path(&st, 1, j as f32 / 10.0).y - st.floor).fold(9.0, f32::min);
        let under = POOL_LM / (POOL_UP * POOL_UP);
        assert!(under > 4.0 * BOLT_LM / (high * high), "pool {under} vs bolt at {high} m");
        let out = POOL_LM * POOL_UP / (1.0 + POOL_UP * POOL_UP).powf(1.5);
        assert!(out < 0.25 * under, "a metre out the pool is {out} of {under}");
    }

    /// The shell is broken: round it the rim's brightest is under a third of its peak in
    /// places (gaps) and whole in others, and nothing reaches the card's border.
    #[test]
    fn the_shell_is_broken_not_a_circle() {
        let img = shell_image();
        let d = img.data.as_ref().unwrap();
        let alpha = |x: usize, y: usize| d[(y * 128 + x) * 4 + 3];
        let best: Vec<u8> = (0..90)
            .map(|j| {
                let a = j as f32 / 90.0 * std::f32::consts::TAU;
                (40..64)
                    .map(|s| alpha((64.0 + a.cos() * s as f32) as usize, (64.0 + a.sin() * s as f32) as usize))
                    .max()
                    .unwrap()
            })
            .collect();
        let peak = *best.iter().max().unwrap();
        let gaps = best.iter().filter(|&&b| (b as f32) < peak as f32 / 3.0).count();
        let whole = best.iter().filter(|&&b| (b as f32) > peak as f32 * 0.8).count();
        assert!(peak > 200 && gaps > 9 && whole > 9, "peak {peak}, {gaps} gaps, {whole} whole of 90");
        assert!((0..128).all(|k| alpha(k, 0) == 0 && alpha(0, k) == 0 && alpha(127, k) == 0));
    }

    /// Each bolt leaves the hero and ends at the target, above the floor all the way.
    #[test]
    fn bolts_fly_hero_to_target() {
        let st = Stage::default();
        for i in 0..BOLTS {
            assert!(path(&st, i, 0.0).distance(st.hero()) < 0.6);
            assert!(path(&st, i, 1.0).distance(st.bark()) < 0.5);
            assert!((0..=10).all(|j| path(&st, i, j as f32 / 10.0).y > 1.0));
        }
    }
}
