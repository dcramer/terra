//! Small lights that make the clearing breathe: fireflies along the wood's edge and over the
//! pond's reeds at dusk and by night, each wandering and glowing on and off in its own
//! rhythm, and embers rising off the camp fire on the wind, at every hour. Unlit emissive
//! specks, the bloom making their glow.

use crate::forest::Rng;
use crate::scene::{FIRE, Hour};
use crate::terrain::{self, Land, POND, WATER};
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn.after(crate::scene::build))
        .add_systems(Update, (wander, rise, lick, show, face_halo));
}

#[derive(Component)]
struct Firefly {
    home: Vec3,
    /// Each one's own rhythm: the wander's speeds and the glow's period and phase.
    seed: f32,
    period: f32,
}

/// A card that turns to face the camera.
#[derive(Component)]
pub struct Halo;

#[derive(Component)]
pub struct Tongue {
    pub foot: Vec3,
    pub seed: f32,
    pub size: f32,
}

#[derive(Component)]
struct Ember {
    born: f32,
    life: f32,
    drift: Vec2,
    seed: f32,
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    land: Res<Land>,
) {
    let mut rng = Rng::new(0x0f1e_f1e5);
    let speck = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
    // Unlit, so the colour reaches the screen unexposed and past white, for the bloom.
    let lit = |c: LinearRgba| StandardMaterial {
        base_color: c.into(),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..default()
    };
    let glow = materials.add(lit(LinearRgba::rgb(5.0, 8.0, 1.0)));
    let ember = materials.add(lit(LinearRgba::rgb(4.0, 1.1, 0.08)));
    // The tongues' colour is in their image (gold at the root, orange, red at the tips); kept
    // below where the tone curve bleaches a colour to white, as overlapping tongues add up.
    let flame = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(tongue_image())),
        alpha_mode: AlphaMode::Blend,
        ..lit(LinearRgba::rgb(1.8, 1.8, 1.8))
    });
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    let soft = images.add(soft_image());
    // The coals: a bed of red-orange glow among the logs, lying on the ground.
    commands.spawn((
        Mesh3d(card.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(soft.clone()),
            ..lit(LinearRgba::rgb(1.1, 0.22, 0.02))
        })),
        Transform::from_xyz(FIRE.x, land.height(FIRE) + 0.06, FIRE.y)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::splat(1.0)),
        bevy::light::NotShadowCaster,
    ));
    // The air about the flames aglow, a faint orange halo round them (Glow faces the camera).
    commands.spawn((
        Mesh3d(card.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(soft.clone()),
            ..lit(LinearRgba::rgb(0.16, 0.06, 0.012))
        })),
        Transform::from_xyz(FIRE.x, land.height(FIRE) + 0.45, FIRE.y).with_scale(Vec3::splat(2.4)),
        bevy::light::NotShadowCaster,
        Halo,
    ));
    // The fire's flames: tongues over the logs, licking up and falling back.
    let at = Vec3::new(FIRE.x, land.height(FIRE) + 0.12, FIRE.y);
    for i in 0..12 {
        let a = i as f32 * 2.4;
        let r = if i < 3 { 0.05 } else { rng.range(0.12, 0.32) };
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(flame.clone()),
            Transform::from_translation(at + Vec3::new(libm::cosf(a) * r, 0.0, libm::sinf(a) * r)),
            bevy::light::NotShadowCaster,
            Tongue {
                foot: at + Vec3::new(libm::cosf(a) * r, 0.0, libm::sinf(a) * r),
                seed: rng.f() * 50.0,
                size: if i < 3 { 1.6 } else { rng.range(0.7, 1.15) },
            },
        ));
    }
    // Fireflies: where the clearing meets the wood, and over the reeds.
    let mut placed = 0;
    let mut tries = 0;
    while placed < 70 && tries < 5000 {
        tries += 1;
        let near_pond = placed % 4 == 0;
        let p = if near_pond {
            POND + Vec2::from_angle(rng.f() * std::f32::consts::TAU) * rng.range(3.5, 7.0)
        } else {
            Vec2::new(rng.range(-22.0, 22.0), rng.range(-20.0, 24.0))
        };
        let woods = terrain::woods(p);
        // Over the long grass toward the wood's edge, not in the firelight.
        if !near_pond && (woods > 0.6 || p.distance(FIRE) < 5.0 || woods < 0.02 && rng.f() < 0.6) {
            continue;
        }
        let ground = land.height(p).max(WATER);
        let home = Vec3::new(p.x, ground + rng.range(0.35, 1.6), p.y);
        commands.spawn((
            Mesh3d(speck.clone()),
            MeshMaterial3d(glow.clone()),
            Transform::from_translation(home).with_scale(Vec3::splat(0.0)),
            bevy::light::NotShadowCaster,
            Firefly {
                home,
                seed: rng.f() * 100.0,
                period: rng.range(1.6, 3.4),
            },
        ));
        placed += 1;
    }
    for i in 0..40 {
        commands.spawn((
            Mesh3d(speck.clone()),
            MeshMaterial3d(ember.clone()),
            Transform::from_xyz(FIRE.x, -100.0, FIRE.y),
            bevy::light::NotShadowCaster,
            Ember {
                born: -(i as f32) * 0.07,
                life: rng.range(1.4, 2.6),
                drift: Vec2::new(rng.range(-0.25, 0.25), rng.range(-0.25, 0.25)),
                seed: rng.f() * 100.0,
            },
        ));
    }
}

/// A firefly drifts about its home on slow loops and glows for a moment each period.
fn wander(time: Res<Time>, mut flies: Query<(&Firefly, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (f, mut tr) in &mut flies {
        let s = f.seed;
        let off = Vec3::new(
            libm::sinf(t * 0.31 + s) * 0.9 + libm::sinf(t * 0.73 + s * 1.7) * 0.3,
            libm::sinf(t * 0.47 + s * 0.6) * 0.25,
            libm::cosf(t * 0.27 + s * 2.3) * 0.9 + libm::cosf(t * 0.61 + s * 0.4) * 0.3,
        );
        tr.translation = f.home + off;
        // On for a third of its period, swelling and fading.
        let phase = ((t + s) / f.period).fract();
        let on = (1.0 - (phase / 0.35 - 0.5).abs() * 2.0).clamp(0.0, 1.0);
        tr.scale = Vec3::splat(0.035 * libm::powf(on, 0.7));
    }
}

/// Embers rise off the fire, drift on the wind, shrink and die, and are born again.
fn rise(time: Res<Time>, land: Res<Land>, mut embers: Query<(&mut Ember, &mut Transform)>) {
    let t = time.elapsed_secs();
    let base = Vec3::new(FIRE.x, land.height(FIRE) + 0.25, FIRE.y);
    for (mut e, mut tr) in &mut embers {
        let mut age = t - e.born;
        if age > e.life {
            e.born = t;
            e.seed += 1.37;
            age = 0.0;
        }
        if age < 0.0 {
            continue;
        }
        let k = age / e.life;
        let swirl = Vec3::new(
            libm::sinf(age * 4.0 + e.seed),
            0.0,
            libm::cosf(age * 3.3 + e.seed),
        ) * 0.12
            * k;
        let start = Vec3::new(libm::sinf(e.seed * 7.1), 0.0, libm::cosf(e.seed * 3.7)) * 0.3;
        tr.translation = base
            + start
            + Vec3::new(e.drift.x + 0.35, 0.0, e.drift.y + 0.2) * age
            + Vec3::Y * (1.1 * age - 0.12 * age * age)
            + swirl;
        tr.scale = Vec3::splat(0.022 * libm::powf(1.0 - k, 0.8));
    }
}

/// A flame's tongue, a card turned upright to the camera, swells and leans and sinks,
/// never twice the same.
pub fn lick(
    time: Res<Time>,
    camera: Query<&GlobalTransform, With<crate::camera::MainCamera>>,
    mut tongues: Query<(&Tongue, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    let face = camera.single().map_or(Quat::IDENTITY, |c| {
        let b = c.back();
        Quat::from_rotation_y(libm::atan2f(b.x, b.z))
    });
    for (f, mut tr) in &mut tongues {
        let s = f.seed;
        let h = 0.75 + 0.15 * libm::sinf(t * 9.0 + s) + 0.1 * libm::sinf(t * 15.0 + s * 1.3);
        let tall = 0.8 * f.size * h;
        let wide = 0.3 * f.size * (0.85 + 0.15 * libm::sinf(t * 11.0 + s * 0.7));
        let lean = Vec3::new(
            libm::sinf(t * 5.0 + s) * 0.03,
            0.0,
            libm::cosf(t * 4.1 + s) * 0.03,
        );
        tr.translation = f.foot + Vec3::Y * tall * 0.45 + lean;
        tr.rotation = face * Quat::from_rotation_z(libm::sinf(t * 3.0 + s) * 0.12);
        tr.scale = Vec3::new(wide, tall, 1.0);
    }
}

/// A flame's tongue: a teardrop, white-gold at its root, orange, then red at its edge and
/// tip, fading to nothing.
pub fn tongue_image() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let (w, h) = (48u32, 96u32);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        // v up from the root.
        let v = 1.0 - (y as f32 + 0.5) / h as f32;
        let r = 0.46 * libm::powf(1.0 - v, 0.9) * (v * 5.0).min(1.0).sqrt();
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32 - 0.5;
            let d = if r > 0.0 { u.abs() / r } else { 2.0 };
            let i = ((1.0 - d * d).max(0.0) * libm::powf(1.0 - v, 0.6)).clamp(0.0, 1.0);
            // Gold in the hot root, orange through the body, red at the edge and tip; its
            // opacity the teardrop's.
            let hot = (i * 1.5 - 0.5).clamp(0.0, 1.0);
            let c = [
                0.85 + 0.15 * hot,
                0.15 + 0.25 * hot + 0.25 * hot * hot,
                0.01 + 0.05 * hot + 0.16 * hot.powi(3),
            ];
            data.extend(c.iter().map(|c| (c * 255.0) as u8));
            data.push((i.sqrt() * 255.0) as u8);
        }
    }
    Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Fireflies only at dusk and by night.
fn show(hour: Option<Res<Hour>>, mut flies: Query<&mut Visibility, With<Firefly>>) {
    let Some(hour) = hour else { return };
    if !hour.is_changed() {
        return;
    }
    let vis = if *hour == Hour::Day {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut v in &mut flies {
        *v = vis;
    }
}

/// The fire's halo faces the camera.
pub fn face_halo(
    camera: Query<&GlobalTransform, With<crate::camera::MainCamera>>,
    mut halos: Query<&mut Transform, With<Halo>>,
) {
    let Ok(c) = camera.single() else { return };
    for mut tr in &mut halos {
        tr.rotation = c.rotation();
    }
}

/// A round soft glow: white, its alpha falling smoothly from the middle to nothing at the rim.
pub fn soft_image() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let n = 32u32;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let p = (Vec2::new(x as f32, y as f32) + 0.5) / n as f32 * 2.0 - 1.0;
            let a = (1.0 - p.length()).clamp(0.0, 1.0);
            let a = a * a * (3.0 - 2.0 * a);
            data.extend([255, 255, 255, (a * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}
