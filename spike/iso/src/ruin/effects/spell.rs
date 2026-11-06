//! The hero's spell: a volley of three arcane bolts from his off hand at the tree monster,
//! as Path of Exile's and Diablo IV's are drawn: each bolt a white-hot core over a blue glow
//! (the bloom does the rest), a ribbon trail behind it fading to nothing, its own small
//! light moving over the stones; and where it lands, a burst: a flash swelling and fading,
//! blue-white sparks thrown back off the bark, and a short bright light (Last Epoch's "a
//! number of lights" in a skill). Every part is a function of the clock: in a still the
//! first bolt has just struck and the other two are in the air.

use super::kit::{Burst, Cam, Clock, glow, sparks};
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
/// The caster's chest and the target's (the tree monster's, fight.rs's places).
const HERO: Vec3 = Vec3::new(0.0, 1.35, 0.0);
const TARGET: Vec3 = Vec3::new(-2.8, 1.5, -3.4);
/// How far behind a bolt its trail reaches, in the flight's share.
const TAIL: f32 = 0.4;
const TRAIL_POINTS: usize = 14;
/// The impact's light at its height, in lumens, and how fast it dies (s).
const FLASH_LM: f32 = 40_000.0;
const FLASH_FALL: f32 = 0.09;

#[derive(Component)]
enum Part {
    Core,
    Glow,
    Light,
    Trail(Handle<Mesh>),
    Flash(Handle<StandardMaterial>),
    Bang,
}

#[derive(Component)]
struct Bolt(usize, Part);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, fly);
}

/// Where bolt `i` is at `s` of its flight: from the hero's off hand to the bark, rising in
/// an arc and swinging to its own side so the three do not lie in one line.
fn path(i: usize, s: f32) -> Vec3 {
    let dir = (TARGET - HERO).normalize();
    let from = HERO + dir * 0.45;
    let to = TARGET - dir * 0.35;
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
) {
    let mut rng = Rng::new(0x5be11);
    let soft = images.add(crate::motes::soft_image());
    let ball = meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap());
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    let streak = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let core = materials.add(glow(LinearRgba::rgb(14.0, 16.0, 24.0), None));
    let halo = materials.add(glow(LinearRgba::rgb(0.6, 1.4, 4.5), Some(soft.clone())));
    let trail = materials.add(StandardMaterial {
        double_sided: true,
        cull_mode: None,
        ..glow(LinearRgba::rgb(1.6, 3.2, 9.0), None)
    });
    let spark = materials.add(glow(LinearRgba::rgb(5.0, 8.0, 16.0), None));
    let off = Transform::from_scale(Vec3::ZERO);
    for i in 0..BOLTS {
        commands.spawn((Mesh3d(ball.clone()), MeshMaterial3d(core.clone()), off, Bolt(i, Part::Core)));
        commands.spawn((Mesh3d(card.clone()), MeshMaterial3d(halo.clone()), off, Bolt(i, Part::Glow)));
        commands.spawn((
            PointLight {
                color: Color::srgb(0.45, 0.65, 1.0),
                intensity: 0.0,
                range: 5.0,
                radius: 0.05,
                ..default()
            },
            Transform::default(),
            Bolt(i, Part::Light),
        ));
        let mesh = meshes.add(ribbon(&[], Vec3::ZERO));
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(trail.clone()),
            Transform::default(),
            NoFrustumCulling,
            bevy::light::NotShadowCaster,
            Bolt(i, Part::Trail(mesh)),
        ));
        let flash = materials.add(glow(LinearRgba::rgb(2.0, 3.5, 8.0), Some(soft.clone())));
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
            Transform::from_translation(path(i, 1.0)),
            Bolt(i, Part::Bang),
        ));
        let at = path(i, 1.0);
        let burst = commands.spawn((Burst { at, age: -1.0 }, Bolt(i, Part::Bang))).id();
        let back = (HERO - TARGET).normalize() + Vec3::Y * 0.6;
        sparks(&mut commands, burst, &streak, &spark, &mut rng, 14, back, 0.9, (3.0, 7.5), 0.02);
    }
}

/// A ribbon through `points` (tail first) facing the camera at `eye`, widening and
/// brightening from nothing at the tail to the bolt.
fn ribbon(points: &[Vec3], eye: Vec3) -> Mesh {
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
        let w = 0.15 * k.powf(0.7);
        pos.push((p + side * w * 0.5).to_array());
        pos.push((p - side * w * 0.5).to_array());
        let a = k * k;
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
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let p = if clock.still { STILL } else { clock.t.rem_euclid(PERIOD) };
    for (Bolt(i, part), tr, light, burst) in &mut parts {
        let (s, struck) = flight(*i, p);
        let flying = (0.0..1.0).contains(&s);
        let head = path(*i, s.clamp(0.0, 1.0));
        match part {
            Part::Core | Part::Glow => {
                let mut tr = tr.unwrap();
                let size = if matches!(part, Part::Core) { 0.065 } else { 0.7 };
                tr.translation = head;
                tr.rotation = cam.rot;
                tr.scale = Vec3::splat(if flying { size } else { 0.0 });
            }
            Part::Light => {
                tr.unwrap().translation = head;
                light.unwrap().intensity = if flying { 4_000.0 } else { 0.0 };
            }
            Part::Trail(mesh) => {
                let tail = (s - TAIL).max(0.0);
                let tip = s.min(1.0);
                let points: Vec<Vec3> = if s > 0.0 && tail < tip {
                    (0..TRAIL_POINTS)
                        .map(|j| path(*i, tail + (tip - tail) * j as f32 / (TRAIL_POINTS - 1) as f32))
                        .collect()
                } else {
                    Vec::new()
                };
                if let Some(mut m) = meshes.get_mut(mesh) {
                    *m = ribbon(&points, cam.pos);
                }
            }
            Part::Flash(mat) => {
                let mut tr = tr.unwrap();
                let on = (0.0..0.35).contains(&struck);
                let k = (struck / 0.35).clamp(0.0, 1.0);
                tr.translation = path(*i, 1.0);
                tr.rotation = cam.rot;
                tr.scale = Vec3::splat(if on { 0.5 + 1.6 * (1.0 - (1.0 - k).powi(3)) } else { 0.0 });
                if let Some(mut m) = materials.get_mut(mat) {
                    let f = (1.0 - k).powi(2);
                    m.base_color = LinearRgba::rgb(2.0 * f, 3.5 * f, 8.0 * f).into();
                }
            }
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

    /// Each bolt leaves the hero and ends at the target, above the floor all the way.
    #[test]
    fn bolts_fly_hero_to_target() {
        for i in 0..BOLTS {
            assert!(path(i, 0.0).distance(HERO) < 0.6);
            assert!(path(i, 1.0).distance(TARGET) < 0.5);
            assert!((0..=10).all(|j| path(i, j as f32 / 10.0).y > 1.0));
        }
    }
}
