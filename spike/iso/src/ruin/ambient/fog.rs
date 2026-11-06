//! Low ground fog in drifts: fog cards, as shipped games lay them, lying a hand to a knee
//! above the flagstones, each a patch of soft noise that drifts on the wind, turns slowly,
//! and fades in and out over its way, so no card's edge or loop shows. Lit like the stone
//! (the moon blue, the braziers warm) and translucent, so the fight's feet stand in mist.

use super::WIND;
use crate::forest::Rng;
use crate::ruin::effects::kit::{Clock, Noise, image, smooth};
use bevy::prelude::*;

/// How many drifts, how thick at their thickest, and how far each drifts before it fades.
const CARDS: usize = 16;
const THICK: f32 = 0.34;
const SPAN: f32 = 5.0;

/// A scene's fog: how thick its drifts are at their thickest, their colour (linear, lit
/// like the stone), and how high they lie (m over the ground). The default is the marsh's
/// and the first ruin's; a scene inserts its own before Startup.
#[derive(Resource, Clone, Copy, Debug)]
pub struct FogLook {
    pub thick: f32,
    pub colour: Vec3,
    pub height: (f32, f32),
}

impl Default for FogLook {
    fn default() -> Self {
        Self {
            thick: THICK,
            colour: Vec3::new(0.42, 0.47, 0.56),
            height: (0.1, 0.45),
        }
    }
}

#[derive(Component)]
struct Drift {
    from: Vec3,
    size: f32,
    speed: f32,
    phase: f32,
    turn: f32,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<FogLook>()
        .add_systems(Startup, spawn)
        .add_systems(Update, drift);
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    look: Res<FogLook>,
) {
    let mut rng = Rng::new(0xf06);
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    let blobs: Vec<_> = (0..3).map(|i| images.add(blob(0x0f0 + i))).collect();
    for i in 0..CARDS {
        let from = Vec3::new(
            rng.range(-12.0, 8.0),
            rng.range(look.height.0, look.height.1),
            rng.range(-8.5, 3.5),
        ) - WIND.extend(0.0).xzy() * SPAN * 0.5;
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::NONE,
                base_color_texture: Some(blobs[i % 3].clone()),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                ..default()
            })),
            Transform::default(),
            bevy::light::NotShadowCaster,
            Drift {
                from,
                size: rng.range(4.0, 7.5),
                speed: rng.range(0.1, 0.18),
                phase: rng.f(),
                turn: rng.range(-0.04, 0.04),
            },
        ));
    }
}

/// A drift's patch: noise thinned to tatters, fading to nothing well inside the card's edge.
fn blob(seed: u32) -> Image {
    let big = Noise::new(5, seed);
    let fine = Noise::new(10, seed * 7 + 1);
    let s = 128;
    image(s, s, |x, y| {
        let p = (Vec2::new(x as f32, y as f32) + 0.5) / s as f32;
        let r = (p * 2.0 - 1.0).length();
        let n = 0.65 * big.at(p * 5.0) + 0.35 * fine.at(p * 10.0);
        let a = smooth(0.95, 0.35, r) * smooth(0.3, 0.7, n);
        [1.0, 1.0, 1.0, a]
    })
}

/// How thick a drift is `k` of the way along its span: in from nothing, out to nothing.
fn fade(k: f32) -> f32 {
    (std::f32::consts::PI * k).sin().powf(1.5)
}

fn drift(
    clock: Res<Clock>,
    mut cards: Query<(&Drift, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    look: Res<FogLook>,
) {
    let t = clock.t;
    for (d, mut tr, mat) in &mut cards {
        let life = SPAN / d.speed;
        let k = (t / life + d.phase).rem_euclid(1.0);
        tr.translation = d.from + WIND.extend(0.0).xzy() * SPAN * k;
        tr.rotation = Quat::from_rotation_y(d.turn * t + d.phase * 6.0)
            * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        tr.scale = Vec3::new(d.size, d.size * 0.7, 1.0);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            let c = look.colour;
            m.base_color = Color::linear_rgba(c.x, c.y, c.z, look.thick * fade(k));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drift comes and goes from nothing: no card pops in or out at its span's ends.
    #[test]
    fn drifts_fade_at_their_ends() {
        assert!(fade(0.0) < 1e-3 && fade(0.999) < 0.01);
        assert!((fade(0.5) - 1.0).abs() < 1e-5);
    }
}
