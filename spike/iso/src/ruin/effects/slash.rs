//! The knight's slash: a ribbon swept along the blade's own path through the chop, as the
//! shipped ARPGs draw a melee swing. Unreal's weapon trail (the `AnimNotifyState_Trail`
//! between two sword sockets, sampled every frame) and Diablo IV's and Path of Exile 2's
//! slash meshes (a ribbon with a gradient texture, its hot edge outside, fading from the
//! newest edge back) are the same thing: a strip between two points on the blade, kept over
//! the last fifth of a second, additive over bloom, with a light riding the edge.
//!
//! A still holds the clip, so there is no past to keep: the blade's path through the swing
//! is baked from the clip itself at startup, as a trail system samples its sockets, by
//! seeking the knight through it a sixtieth of a second a frame and reading the sword (held
//! on the hand bone, held.rs) after each pose is applied; then the clip goes back where it
//! stood. The trail at any clip time is the baked path over the 0.2 s before it.

use super::super::held::Held;
use super::Stage;
use super::kit::{Noise, image, smooth};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// The part of the attack clip that leaves a trail (s): from the top of the wind-up to the
/// blade's stop after the chop (fight.rs: arms up at 1.1, the blow at 1.25, bent over at 1.4).
const FROM: f32 = 1.0;
const TO: f32 = 1.42;
/// The bake's step through it (s).
const STEP: f32 = 1.0 / 60.0;
/// How long a point of the trail lives (s).
const AGE: f32 = 0.2;
const POINTS: usize = 28;
/// The strip's inner and outer edges, in shares of the blade from the pommel (0.95 m).
const BLADE: f32 = 0.95;
const INNER: f32 = 0.3;
const OUTER: f32 = 1.12;
/// The light riding the edge, in lumens, a third of the way in from the tip.
const EDGE_LM: f32 = 45_000.0;

#[derive(Default, Clone, Copy)]
enum Bake {
    #[default]
    Wait,
    Run { next: usize, back: f32, speed: f32 },
    Done,
}

/// The blade's path through the swing: (clip time, inner point, outer point), in the world.
#[derive(Resource, Default)]
struct Blade {
    player: Option<Entity>,
    sword: Option<Entity>,
    waited: u32,
    bake: Bake,
    path: Vec<(f32, Vec3, Vec3)>,
    shown: f32,
}

#[derive(Component)]
enum Part {
    Trail(Handle<Mesh>),
    Light,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Blade>()
        .add_systems(Startup, spawn)
        .add_systems(Update, (find, bake, show).chain());
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut img = gradient_image();
    img.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    let material = materials.add(StandardMaterial {
        base_color: LinearRgba::rgb(7.0, 7.0, 7.0).into(),
        base_color_texture: Some(images.add(img)),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let mesh = meshes.add(strip(&[]));
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material),
        Transform::default(),
        NoFrustumCulling,
        bevy::light::NotShadowCaster,
        Part::Trail(mesh),
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.62, 0.3),
            intensity: 0.0,
            range: 5.0,
            radius: 0.15,
            ..default()
        },
        Transform::default(),
        Part::Light,
    ));
}

/// The strip's colour across it (v, 0 inner to 1 outer) and along it (u, the age): a hot,
/// near-white edge over a saturated orange body that thins to nothing toward the hilt, with
/// streaks along the swing (the slash meshes' panning noise), in linear light.
fn gradient_image() -> Image {
    let noise = Noise::new(8, 0x51a5);
    let (w, h) = (128, 64);
    image(w, h, |x, y| {
        let u = (x as f32 + 0.5) / w as f32;
        let v = (y as f32 + 0.5) / h as f32;
        let edge = smooth(0.62, 0.9, v) * (1.0 - smooth(0.94, 1.0, v));
        let body = smooth(0.0, 0.75, v) * 0.5;
        let streak = 0.55 + 0.45 * noise.at(Vec2::new(u * 4.0, v * 8.0));
        let hot = edge * edge;
        let c = Vec3::new(1.0, 0.24, 0.035).lerp(Vec3::new(1.0, 0.78, 0.45), hot);
        [c.x, c.y, c.z, edge.max(body) * streak]
    })
}

/// Finds the knight's player and the sword in his hand.
fn find(
    mut blade: ResMut<Blade>,
    stage: Res<Stage>,
    bodies: Query<(Entity, &Transform), (With<WorldAssetRoot>, Without<ChildOf>)>,
    children: Query<&Children>,
    players: Query<&AnimationPlayer>,
    held: Query<(), With<Held>>,
) {
    if blade.player.is_some() && blade.sword.is_some() {
        return;
    }
    let Some((knight, _)) = bodies
        .iter()
        .find(|(_, t)| t.translation.xz().distance(stage.knight) < 0.3)
    else {
        return;
    };
    blade.player = children
        .iter_descendants(knight)
        .find(|e| players.get(*e).is_ok_and(|p| p.playing_animations().next().is_some()));
    blade.sword = children.iter_descendants(knight).find(|e| held.contains(*e));
}

/// Seeks the knight through the swing a step a frame, reading the blade after each pose,
/// then puts the clip back.
fn bake(
    mut blade: ResMut<Blade>,
    mut players: Query<&mut AnimationPlayer>,
    swords: Query<&GlobalTransform>,
) {
    let (Some(player), Some(sword)) = (blade.player, blade.sword) else {
        return;
    };
    let Ok(mut p) = players.get_mut(player) else { return };
    let Some((_, a)) = p.playing_animations_mut().next() else { return };
    let steps = ((TO - FROM) / STEP).ceil() as usize + 1;
    match blade.bake {
        Bake::Wait => {
            // A few frames for the sword's transform to be propagated under the hand.
            blade.waited += 1;
            if blade.waited > 4 {
                let (back, speed) = (a.seek_time(), a.speed());
                a.set_speed(0.0).seek_to(FROM);
                blade.bake = Bake::Run { next: 1, back, speed };
            }
        }
        Bake::Run { next, back, speed } => {
            if let Ok(g) = swords.get(sword) {
                let at = |s: f32| g.transform_point(Vec3::Y * BLADE * s);
                let t = a.seek_time();
                blade.path.push((t, at(INNER), at(OUTER)));
            }
            if next < steps {
                a.seek_to((FROM + next as f32 * STEP).min(TO));
                blade.bake = Bake::Run { next: next + 1, back, speed };
            } else {
                a.seek_to(back).set_speed(speed);
                blade.path.sort_by(|x, y| x.0.total_cmp(&y.0));
                blade.bake = Bake::Done;
            }
        }
        Bake::Done => {}
    }
}

/// The blade's inner and outer points at clip time `t`, between the baked ones.
fn blade_at(path: &[(f32, Vec3, Vec3)], t: f32) -> (Vec3, Vec3) {
    let j = path.partition_point(|s| s.0 < t).clamp(1, path.len() - 1);
    let (a, b) = (path[j - 1], path[j]);
    let k = ((t - a.0) / (b.0 - a.0).max(1e-6)).clamp(0.0, 1.0);
    (a.1.lerp(b.1, k), a.2.lerp(b.2, k))
}

/// The trail at clip time `t`: the blade's points over the `AGE` before it within the
/// swing, newest last, each with its age's share (0 new, 1 gone).
fn trail(path: &[(f32, Vec3, Vec3)], t: f32) -> Vec<(Vec3, Vec3, f32)> {
    let (from, to) = ((t - AGE).max(FROM), t.min(TO));
    if path.len() < 2 || to <= from {
        return Vec::new();
    }
    (0..POINTS)
        .map(|j| {
            let s = from + (to - from) * j as f32 / (POINTS - 1) as f32;
            let (i, o) = blade_at(path, s);
            (i, o, ((t - s) / AGE).clamp(0.0, 1.0))
        })
        .collect()
}

/// The strip through the trail's points: u its age, v across it, alpha fading with age.
fn strip(points: &[(Vec3, Vec3, f32)]) -> Mesh {
    let mut pos = Vec::new();
    let mut uv = Vec::new();
    let mut col = Vec::new();
    let mut idx = Vec::new();
    for (j, &(i, o, age)) in points.iter().enumerate() {
        pos.extend([i.to_array(), o.to_array()]);
        uv.extend([[age, 0.0], [age, 1.0]]);
        let a = (1.0 - age).powf(1.4);
        col.extend([[1.0, 1.0, 1.0, a]; 2]);
        if j + 1 < points.len() {
            let b = (j * 2) as u32;
            idx.extend([b, b + 1, b + 2, b + 1, b + 3, b + 2]);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; pos.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx))
}

fn show(
    mut blade: ResMut<Blade>,
    players: Query<&AnimationPlayer>,
    mut parts: Query<(&Part, Option<&mut Transform>, Option<&mut PointLight>)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let t = match (&blade.bake, blade.player) {
        (Bake::Done, Some(p)) => players
            .get(p)
            .ok()
            .and_then(|p| p.playing_animations().next().map(|(_, a)| a.seek_time()))
            .unwrap_or(-1.0),
        _ => -1.0,
    };
    if t == blade.shown {
        return;
    }
    blade.shown = t;
    let points = trail(&blade.path, t);
    for (part, tr, light) in &mut parts {
        match part {
            Part::Trail(mesh) => {
                if let Some(mut m) = meshes.get_mut(mesh) {
                    *m = strip(&points);
                }
            }
            Part::Light => {
                let (Some(mut tr), Some(mut light)) = (tr, light) else { continue };
                match points.last() {
                    Some(&(i, o, age)) => {
                        tr.translation = i.lerp(o, 0.7);
                        light.intensity = EDGE_LM * (1.0 - age);
                    }
                    None => light.intensity = 0.0,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A blade turning about the shoulder through the swing.
    fn swing() -> Vec<(f32, Vec3, Vec3)> {
        (0..=30)
            .map(|j| {
                let t = FROM + (TO - FROM) * j as f32 / 30.0;
                let a = (t - FROM) / (TO - FROM) * 2.0;
                let d = Vec3::new(a.cos(), a.sin(), 0.0);
                (t, d * 0.4, d * 1.2)
            })
            .collect()
    }

    /// The trail lies on the blade's path, newest at the blade and full, oldest an `AGE`
    /// back and faded, and none before the swing or long after it.
    #[test]
    fn the_trail_follows_the_blade_and_fades() {
        let path = swing();
        let t = 1.3;
        let pts = trail(&path, t);
        let (i, o, age) = *pts.last().unwrap();
        let (bi, bo) = blade_at(&path, t);
        assert!(i.distance(bi) < 1e-4 && o.distance(bo) < 1e-4 && age < 1e-4);
        let first = pts[0];
        assert!((first.2 - 1.0).abs() < 1e-3, "oldest point's age {}", first.2);
        assert!(first.1.distance(blade_at(&path, t - AGE).1) < 1e-4);
        assert!(trail(&path, FROM - 0.1).is_empty());
        assert!(trail(&path, TO + AGE + 0.01).iter().all(|p| p.2 >= 1.0 - 1e-4));
    }

    /// The strip is hot at its outer edge and nothing at the hilt.
    #[test]
    fn the_strip_is_hot_outside() {
        let img = gradient_image();
        let data = img.data.as_ref().unwrap();
        let px = |x: usize, y: usize| &data[(y * 128 + x) * 4..(y * 128 + x) * 4 + 4];
        let (inner, edge) = (px(10, 1), px(10, 52));
        assert!(inner[3] < 10 && edge[3] > 90, "inner {inner:?} edge {edge:?}");
        assert!(edge[1] > 100, "the edge is near white: {edge:?}");
    }
}
