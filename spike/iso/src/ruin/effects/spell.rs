//! The hero's spell: a volley of three arcane bolts from his off hand at the tree monster,
//! as Path of Exile's and Diablo IV's are drawn: each bolt a small white-hot core drawn out
//! along its flight inside a thin blue halo (the bloom does the rest), a sharp streak behind
//! it (a white inner ribbon in a blue outer one) fading to nothing, its own light casting a
//! blue pool on the stones under it; and where it lands, a hard star flash a few frames
//! long, blue-white sparks thrown back off the bark, and a short bright light (Last Epoch's
//! "a number of lights" in a skill). Every part is a function of the clock: in a still the
//! first bolt has just struck and the other two are in the air.

use super::Stage;
use super::kit::{Burst, Cam, Clock, glow, sparks, star_image};
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
const TAIL: f32 = 0.35;
const TRAIL_POINTS: usize = 16;
/// The core's radius and how far it is drawn out along the flight, the halo's size (m).
const CORE: f32 = 0.04;
const CORE_LENGTH: f32 = 0.17;
const HALO: f32 = 0.3;
/// The bolt's own light, in lumens: enough for a blue pool on the floor 1.5 m under it
/// beside the braziers' 60k to 140k (a quarter of their illuminance).
const BOLT_LM: f32 = 30_000.0;
/// The trails' widths at the bolt (m): the outer blue streak and the white one inside it.
const OUTER: f32 = 0.11;
const INNER: f32 = 0.035;
/// The impact's light at its height, in lumens, and how fast it dies (s).
const FLASH_LM: f32 = 40_000.0;
const FLASH_FALL: f32 = 0.09;
/// The impact's star: its size at its height (m) and how long it lasts (s).
const STAR: f32 = 1.2;
const STAR_LIFE: f32 = 0.2;

#[derive(Component)]
enum Part {
    Core,
    Glow,
    Light,
    Trail(Handle<Mesh>, f32),
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
    stage: Res<Stage>,
) {
    let stage = &*stage;
    let mut rng = Rng::new(0x5be11);
    let soft = images.add(crate::motes::soft_image());
    let star = images.add(star_image());
    let ball = meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap());
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    let streak = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let core = materials.add(glow(LinearRgba::rgb(24.0, 26.0, 30.0), None));
    let halo = materials.add(glow(LinearRgba::rgb(0.25, 0.6, 2.4), Some(soft.clone())));
    let ribbon_of = |c: LinearRgba| StandardMaterial {
        double_sided: true,
        cull_mode: None,
        ..glow(c, None)
    };
    let outer = materials.add(ribbon_of(LinearRgba::rgb(0.5, 1.3, 4.5)));
    let inner = materials.add(ribbon_of(LinearRgba::rgb(4.0, 6.0, 11.0)));
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
        sparks(&mut commands, burst, &streak, &spark, &mut rng, 20, back, 0.9, (3.0, 8.0), 0.014);
    }
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
        let a = k * k * k;
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
    stage: Res<Stage>,
) {
    let stage = &*stage;
    let p = if clock.still { STILL } else { clock.t.rem_euclid(PERIOD) };
    for (Bolt(i, part), tr, light, burst) in &mut parts {
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
                    m.base_color = LinearRgba::rgb(6.0 * f, 8.0 * f, 14.0 * f).into();
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
        let st = Stage::default();
        for i in 0..BOLTS {
            assert!(path(&st, i, 0.0).distance(st.hero()) < 0.6);
            assert!(path(&st, i, 1.0).distance(st.bark()) < 0.5);
            assert!((0..=10).all(|j| path(&st, i, j as f32 / 10.0).y > 1.0));
        }
    }
}
