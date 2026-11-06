//! Will-o'-wisps and midges: cold orbs drifting low over the channels, each a hot core, a
//! soft glow facing the camera and its own light mirrored in the water (`pool::Glint`),
//! swelling and fading on its own slow beat with a flicker through it; and clouds of midges
//! dancing over the reeds, catching the light.

use super::LEVEL;
use super::land::{far, near};
use crate::forest::Rng;
use crate::motes::soft_image;
use crate::pool::Glint;
use crate::ruin::effects::kit::{Cam, Clock, glow};
use bevy::prelude::*;

/// The wisps' homes (x, z), height over the water and how far they drift.
const HOMES: [Vec2; 4] = [
    Vec2::new(-5.6, 3.4),
    Vec2::new(4.6, 4.3),
    Vec2::new(-3.0, -5.6),
    Vec2::new(7.4, 1.6),
];
const HIGH: f32 = 0.7;
const DRIFT: f32 = 0.6;
/// A wisp's colour (linear, over 1 drives the bloom), light (lumens) and beat (s).
const COLD: LinearRgba = LinearRgba::rgb(2.2, 6.0, 5.0);
const WISP_LM: f32 = 900.0;
const BEAT: f32 = 6.0;
/// Midge clouds, the specks in each, and their size (m: under 0.05 they vanish at 960 px).
const CLOUDS: usize = 5;
const MIDGES: usize = 26;
const MIDGE: f32 = 0.05;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn)
        .add_systems(Update, (float, dance));
}

/// One wisp's parts: the glow card and the light, moved with the core.
#[derive(Component)]
struct Wisp {
    home: Vec3,
    phase: f32,
    part: Part,
}

#[derive(Clone, Copy, PartialEq)]
enum Part {
    Core,
    Glow,
    Light,
}

#[derive(Component)]
struct Midge {
    home: Vec3,
    phase: Vec3,
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let soft = images.add(soft_image());
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    let core = meshes.add(Sphere::new(0.06));
    let hot = materials.add(glow(COLD * 3.0, None));
    let halo = materials.add(glow(COLD * 0.12, Some(soft.clone())));
    for (i, h) in HOMES.iter().enumerate() {
        let home = h.extend(LEVEL + HIGH).xzy();
        let phase = i as f32 * 1.7;
        commands.spawn((
            Mesh3d(core.clone()),
            MeshMaterial3d(hot.clone()),
            Transform::from_translation(home),
            bevy::light::NotShadowCaster,
            Wisp { home, phase, part: Part::Core },
        ));
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(halo.clone()),
            Transform::from_translation(home),
            bevy::light::NotShadowCaster,
            Wisp { home, phase, part: Part::Glow },
        ));
        commands.spawn((
            PointLight {
                color: Color::LinearRgba(COLD),
                intensity: WISP_LM,
                range: 3.5,
                radius: 0.05,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(home),
            Wisp { home, phase, part: Part::Light },
            Glint,
        ));
    }
    let mut rng = Rng::new(0x31d9e);
    let speck = materials.add(glow(LinearRgba::rgb(0.5, 0.48, 0.38), Some(soft)));
    for c in 0..CLOUDS {
        let x = -7.0 + 3.4 * c as f32;
        // Over the water's edges, a cloud to each channel by turns.
        let z = if c % 2 == 0 { near(x) - 1.4 } else { far(x) + 1.3 };
        for _ in 0..MIDGES {
            let home = Vec3::new(
                x + rng.range(-0.5, 0.5),
                LEVEL + rng.range(0.5, 1.4),
                z + rng.range(-0.5, 0.5),
            );
            commands.spawn((
                Mesh3d(card.clone()),
                MeshMaterial3d(speck.clone()),
                Transform::from_translation(home).with_scale(Vec3::splat(MIDGE)),
                bevy::light::NotShadowCaster,
                Midge {
                    home,
                    phase: Vec3::new(rng.range(0.0, 6.3), rng.range(0.0, 6.3), rng.range(0.0, 6.3)),
                },
            ));
        }
    }
}

/// How bright a wisp is at time `t`: swelling and dying on its beat, a flicker through it.
pub fn bright(t: f32, phase: f32) -> f32 {
    let k = ((t / BEAT + phase) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    let flick = 0.85 + 0.15 * (t * 17.0 + phase * 5.0).sin() * (t * 7.3 + phase).sin();
    (0.25 + 0.75 * k * k) * flick
}

fn float(clock: Res<Clock>, cam: Res<Cam>, mut wisps: Query<(&Wisp, &mut Transform, Option<&mut PointLight>)>) {
    let t = clock.t;
    for (w, mut tr, light) in &mut wisps {
        let p = w.phase;
        let at = w.home
            + Vec3::new(
                (t * 0.21 + p).sin() * DRIFT,
                (t * 0.9 + p * 2.0).sin() * 0.12,
                (t * 0.17 + p * 1.3).sin() * DRIFT * 0.6,
            );
        let k = bright(t, p);
        tr.translation = at;
        match w.part {
            Part::Core => tr.scale = Vec3::splat(0.4 + 0.6 * k),
            Part::Glow => {
                tr.rotation = cam.rot;
                tr.scale = Vec3::splat(0.35 + 0.5 * k);
            }
            Part::Light => {
                if let Some(mut l) = light {
                    l.intensity = WISP_LM * k;
                }
            }
        }
    }
}

/// The midges' tight dance about their homes, each speck facing the camera.
fn dance(clock: Res<Clock>, cam: Res<Cam>, mut midges: Query<(&Midge, &mut Transform)>) {
    let t = clock.t;
    for (m, mut tr) in &mut midges {
        let p = m.phase;
        tr.translation = m.home
            + Vec3::new(
                (t * 3.1 + p.x).sin() + 0.5 * (t * 7.7 + p.y).sin(),
                (t * 2.3 + p.y).sin() * 0.6,
                (t * 2.9 + p.z).sin() + 0.5 * (t * 6.1 + p.x).sin(),
            ) * 0.22;
        tr.rotation = cam.rot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wisp never quite goes out and never passes its full light.
    #[test]
    fn a_wisp_fades_but_never_dies() {
        let ks: Vec<f32> = (0..600).map(|i| bright(i as f32 * 0.05, 0.3)).collect();
        let (lo, hi) = ks.iter().fold((9.0f32, 0.0f32), |(a, b), k| (a.min(*k), b.max(*k)));
        assert!(lo > 0.1 && lo < 0.35, "dimmest {lo}");
        assert!(hi > 0.8 && hi <= 1.0, "brightest {hi}");
    }
}
