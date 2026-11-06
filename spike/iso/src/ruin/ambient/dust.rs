//! Dust in the firelight and fireflies in the dark, as the ARPGs hang specks in their light
//! pools (Diablo IV's motes by a brazier, V Rising's fireflies at night). Each mote is a soft
//! additive dot facing the camera that wanders slowly about a fire, bright near the flame
//! and gone a couple of metres out, so the specks show only where the fire lights the air.
//! The fireflies wander the dead grass and the trees' feet, each blinking on its own beat.

use super::WIND;
use crate::forest::Rng;
use crate::ruin::effects::kit::{Cam, Clock, Fires, Found, glow};
use bevy::prelude::*;

/// Motes a fire of size 1 lights (a brazier has more, a torch fewer), how far from the
/// flame they wander, and the glow of one beside the flame.
const MOTES: f32 = 40.0;
const REACH: f32 = 2.0;
const DUST: LinearRgba = LinearRgba::rgb(2.2, 1.5, 0.9);
/// The fireflies, where they gather (the dead trees, the grass at the frame's edges), and
/// their light at the height of a blink.
const FLIES: usize = 18;
const SWARMS: [Vec2; 6] = [
    Vec2::new(-10.0, -7.4),
    Vec2::new(9.6, -5.4),
    Vec2::new(5.6, 4.4),
    Vec2::new(-9.5, 5.5),
    Vec2::new(-12.5, -1.5),
    Vec2::new(12.0, 1.0),
];
const FLY: LinearRgba = LinearRgba::rgb(2.6, 4.2, 0.7);

#[derive(Component)]
struct Mote {
    fire: Vec3,
    home: Vec3,
    phase: f32,
    rate: f32,
    size: f32,
}

#[derive(Component)]
struct Fly {
    home: Vec3,
    phase: f32,
    beat: f32,
}

pub fn plugin(app: &mut App) {
    app.add_systems(PostStartup, spawn.after(Found))
        .add_systems(Update, (motes, flies));
}

fn spawn(
    mut commands: Commands,
    fires: Res<Fires>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut rng = Rng::new(0xd057);
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    let soft = images.add(crate::motes::soft_image());
    for &(bed, size) in &fires.0 {
        let fire = bed + Vec3::Y * size * 0.5;
        for _ in 0..(MOTES * size).ceil() as usize {
            // Spread through the lit air about the flame, thicker near it.
            let r = REACH * rng.f().sqrt();
            let a = rng.f() * std::f32::consts::TAU;
            let home = Vec3::new(a.cos() * r, rng.range(-0.6, 1.6), a.sin() * r);
            commands.spawn((
                Mesh3d(card.clone()),
                MeshMaterial3d(materials.add(glow(DUST, Some(soft.clone())))),
                Transform::default(),
                bevy::light::NotShadowCaster,
                Mote {
                    fire,
                    home,
                    phase: rng.f() * 100.0,
                    rate: rng.range(0.6, 1.4),
                    size: rng.range(0.06, 0.11),
                },
            ));
        }
    }
    let fly = materials.add(glow(FLY, Some(soft)));
    for i in 0..FLIES {
        let at = SWARMS[i % SWARMS.len()];
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(fly.clone()),
            Transform::default(),
            bevy::light::NotShadowCaster,
            Fly {
                home: Vec3::new(
                    at.x + rng.range(-1.6, 1.6),
                    rng.range(0.3, 1.5),
                    at.y + rng.range(-1.6, 1.6),
                ),
                phase: rng.f() * 100.0,
                beat: rng.range(0.25, 0.5),
            },
        ));
    }
}

/// How bright a mote is `d` metres from the flame's middle: lit as the fire lights the air,
/// full beside it and gone at the reach.
fn lit(d: f32) -> f32 {
    let k = (1.0 - d / REACH).clamp(0.0, 1.0);
    k * k / (1.0 + d * d)
}

/// A slow wander, the same for the same phase and time: a sum of unrelated sines in each
/// axis, so no loop shows.
fn wander(p: f32, t: f32) -> Vec3 {
    Vec3::new(
        (t * 0.31 + p).sin() + 0.5 * (t * 0.73 + p * 1.7).sin(),
        (t * 0.23 + p * 2.3).sin() + 0.4 * (t * 0.61 + p * 0.3).sin(),
        (t * 0.27 + p * 3.1).sin() + 0.5 * (t * 0.67 + p * 2.9).sin(),
    )
}

/// A firefly's light at time `t`: dark most of its beat, then a short soft blink.
fn blink(t: f32, beat: f32, phase: f32) -> f32 {
    (std::f32::consts::TAU * (t * beat + phase)).sin().max(0.0).powi(4)
}

fn motes(
    clock: Res<Clock>,
    cam: Res<Cam>,
    mut motes: Query<(&Mote, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = clock.t;
    let drift = Vec3::new(WIND.x, 0.0, WIND.y) * 0.12;
    for (m, mut tr, mat) in &mut motes {
        let s = t * m.rate;
        // Wander about home, the warm air lifting them and the wind leaning them.
        let rise = ((s * 0.08 + m.phase).fract() - 0.5) * 0.8;
        let p = m.fire + m.home + wander(m.phase, s) * 0.35 + Vec3::Y * rise + drift * rise;
        let d = p.distance(m.fire);
        // A speck turns and catches the light, then not.
        let glint = 0.55 + 0.45 * (s * 2.1 + m.phase * 5.0).sin();
        let k = lit(d) * glint;
        tr.translation = p;
        tr.rotation = cam.rot;
        tr.scale = Vec3::splat(m.size * (0.6 + 0.4 * k.sqrt()));
        if let Some(mut mat) = materials.get_mut(&mat.0) {
            mat.base_color = (DUST * k).into();
        }
    }
}

fn flies(clock: Res<Clock>, cam: Res<Cam>, mut flies: Query<(&Fly, &mut Transform)>) {
    let t = clock.t;
    for (f, mut tr) in &mut flies {
        let b = blink(t, f.beat, f.phase);
        tr.translation = f.home + wander(f.phase, t * 0.8) * Vec3::new(0.7, 0.25, 0.7);
        tr.rotation = cam.rot;
        // A blink swells the dot; between, a faint glow stays.
        tr.scale = Vec3::splat(0.05 + 0.13 * b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dust shows only where the fire lights it: bright beside the flame, nothing at the reach.
    #[test]
    fn dust_lit_by_the_fire_alone() {
        assert!(lit(0.1) > 0.8);
        assert!(lit(1.0) < lit(0.4));
        assert_eq!(lit(REACH), 0.0);
    }

    /// A firefly is dark most of its beat and full at its height.
    #[test]
    fn flies_blink() {
        let beat = 0.4;
        let dark = (0..100)
            .filter(|i| blink(*i as f32 * 0.025, beat, 0.0) < 0.1)
            .count();
        assert!(dark > 60, "dark {dark} of 100");
        assert!((blink(0.25 / beat, beat, 0.0) - 1.0).abs() < 1e-4);
    }
}
