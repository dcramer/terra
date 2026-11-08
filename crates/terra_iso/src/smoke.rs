//! The camp fire's smoke: a thin column of soft puffs rising off the flames, swelling and
//! thinning as they climb and lean off on the wind, warm where the fire lights them from
//! below and grey above. Unlit cards facing the camera, each with its own fading colour.

use crate::forest::Rng;
use crate::scene::FIRE;
use crate::terrain::Land;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn.after(crate::scene::build))
        .add_systems(Update, rise);
}

const PUFFS: usize = 14;
/// Seconds a puff takes from the flames to gone.
const LIFE: f32 = 7.0;

#[derive(Component)]
struct Puff {
    phase: f32,
    seed: f32,
    material: Handle<StandardMaterial>,
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut rng = Rng::new(0x5a0c_e011);
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    let soft = images.add(crate::motes::soft_image());
    for i in 0..PUFFS {
        let material = materials.add(StandardMaterial {
            base_color: Color::NONE,
            base_color_texture: Some(soft.clone()),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(FIRE.x, -100.0, FIRE.y),
            bevy::light::NotShadowCaster,
            Puff {
                phase: i as f32 / PUFFS as f32,
                seed: rng.f() * 100.0,
                material,
            },
        ));
    }
}

fn rise(
    time: Res<Time>,
    land: Res<Land>,
    camera: Query<&GlobalTransform, With<crate::camera::MainCamera>>,
    mut puffs: Query<(&Puff, &mut Transform)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    let face = camera.single().map_or(Quat::IDENTITY, |c| c.rotation());
    let base = Vec3::new(FIRE.x, land.height(FIRE) + 0.7, FIRE.y);
    for (p, mut tr) in &mut puffs {
        let age = ((t / LIFE + p.phase).fract()) * LIFE;
        let k = age / LIFE;
        let s = p.seed;
        // Up, slowing as it cools; off on the wind, with a little curl.
        let up = 4.2 * (1.0 - (1.0 - k) * (1.0 - k));
        let wind = Vec3::new(0.9, 0.0, -0.4) * k * k * 2.2;
        let curl =
            Vec3::new(libm::sinf(t * 0.7 + s), 0.0, libm::cosf(t * 0.5 + s * 1.3)) * 0.18 * k;
        tr.translation = base + Vec3::Y * up + wind + curl;
        tr.rotation = face * Quat::from_rotation_z(s + t * 0.15);
        tr.scale = Vec3::splat(0.45 + 1.9 * k);
        // In over the first tenth, out slowly; fire-warm low, cool grey high.
        let alpha = (k / 0.1).min(1.0) * libm::powf(1.0 - k, 1.3) * 0.3;
        let warm = (1.0 - k / 0.35).clamp(0.0, 1.0);
        // (sRGB, as it reaches the screen: unlit colour is not exposed.)
        let colour = Vec3::new(0.3, 0.32, 0.36).lerp(Vec3::new(0.8, 0.48, 0.25), warm);
        if let Some(mut m) = materials.get_mut(&p.material) {
            m.base_color = Color::srgba(colour.x, colour.y, colour.z, alpha);
        }
    }
}
