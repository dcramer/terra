//! An art-direction test, aiming at the look of Forever Ago (docs/art.md).
//!
//! What that look is made of, as far as rendering goes: simple faceted shapes in flat
//! colours with no textures, a warm sun against cool shadows, haze that turns distance
//! into soft layers, and a little bloom. This switches each of those on over the stand-in
//! planet so it can be judged from a screenshot. The models, the composition and the
//! foliage that make the reference what it is are not something a shader provides.

use super::scatter::placements;
use crate::planet::{Planet, Tweaks};
use bevy::{
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    mesh::VertexAttributeValues,
    post_process::bloom::Bloom,
    prelude::*,
    reflect::TypePath,
    render::{
        render_resource::AsBindGroup,
        view::{ColorGrading, ColorGradingGlobal},
    },
    shader::ShaderRef,
};
use terra_gen::{Noise, Seed};

const HORIZON: Color = Color::srgb(0.98, 0.84, 0.74);
const ZENITH: Color = Color::srgb(0.36, 0.58, 0.86);
const SUN_DIRECTION: Vec3 = Vec3::new(0.62, 0.42, 0.5);

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct SkyMaterial {
    #[uniform(0)]
    horizon: LinearRgba,
    #[uniform(0)]
    zenith: LinearRgba,
    #[uniform(0)]
    sun_direction: Vec3,
    #[uniform(0)]
    radius_m: f32,
}

impl Material for SkyMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/sky.wgsl".into()
    }
}

#[derive(Component)]
struct SkyDome;

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<SkyMaterial>::default())
        // Shadows are lit by the sky, so they come out cool against the warm sun.
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.62, 0.68, 1.0),
            brightness: 700.0,
            ..default()
        })
        .add_systems(Startup, (preset, dress))
        .add_systems(PostStartup, style_camera)
        .add_systems(Update, keep_sky_around_camera);
}

fn preset(mut tweaks: ResMut<Tweaks>) {
    tweaks.sun_direction = SUN_DIRECTION;
    tweaks.sun_color = Color::srgb(1.0, 0.9, 0.76);
    tweaks.sun_illuminance = 16_000.0;
    tweaks.ground_low = Color::srgb(0.78, 0.44, 0.33);
    tweaks.ground_high = Color::srgb(0.93, 0.7, 0.5);
    tweaks.ground_steep = Color::srgb(0.55, 0.3, 0.26);
    tweaks.ground_faceted = true;
}

fn style_camera(mut commands: Commands, camera: Single<(Entity, &mut Transform), With<Camera3d>>) {
    let (camera, mut transform) = camera.into_inner();
    // Lift the view so the sky is in it.
    transform.rotate_local_x(0.22);
    commands.entity(camera).insert((
        Bloom::NATURAL,
        ColorGrading {
            global: ColorGradingGlobal {
                post_saturation: 1.15,
                ..default()
            },
            ..default()
        },
        DistanceFog {
            color: HORIZON,
            directional_light_color: Color::srgba(1.0, 0.88, 0.7, 0.4),
            directional_light_exponent: 12.0,
            // Distant ground fades toward the horizon's colour, which is what turns far
            // hills into soft layers.
            falloff: FogFalloff::from_visibility_colors(
                1800.0,
                Color::srgb(0.55, 0.42, 0.5),
                Color::srgb(0.98, 0.86, 0.78),
            ),
        },
    ));
}

fn dress(
    mut commands: Commands,
    planet: Res<Planet>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut skies: ResMut<Assets<SkyMaterial>>,
) {
    let radius_m = planet.ground.spec().radius_m;
    let seed = planet.ground.spec().seed.derive("look");

    // The sky is the inside of a large sphere that follows the camera.
    commands.spawn((
        Name::new("Sky"),
        SkyDome,
        Mesh3d(
            meshes.add(
                Sphere::new(1.0)
                    .mesh()
                    .ico(6)
                    .unwrap()
                    .with_inverted_winding()
                    .unwrap(),
            ),
        ),
        MeshMaterial3d(skies.add(SkyMaterial {
            horizon: HORIZON.into(),
            zenith: ZENITH.into(),
            sun_direction: SUN_DIRECTION,
            radius_m,
        })),
        Transform::from_scale(Vec3::splat(radius_m * 20.0)),
        NotShadowCaster,
        NoFrustumCulling,
    ));

    // Boulders and shrubs: a few generated shapes each, reused many times.
    let flat = |colour: Color| StandardMaterial {
        base_color: colour,
        perceptual_roughness: 1.0,
        ..default()
    };
    let boulder_look = materials.add(flat(Color::srgb(0.6, 0.42, 0.38)));
    let shrub_look = materials.add(flat(Color::srgb(0.45, 0.55, 0.3)));
    let boulder_shapes: Vec<_> = (0..4)
        .map(|i| meshes.add(faceted_blob(seed.derive("boulder").index(i), 0.45)))
        .collect();
    let shrub_shapes: Vec<_> = (0..3)
        .map(|i| meshes.add(faceted_blob(seed.derive("shrub").index(i), 0.25)))
        .collect();

    for (index, place) in placements(&planet, seed.derive("boulders"), 500)
        .into_iter()
        .enumerate()
    {
        commands.spawn((
            Mesh3d(boulder_shapes[index % boulder_shapes.len()].clone()),
            MeshMaterial3d(boulder_look.clone()),
            Transform {
                translation: place.position,
                rotation: place.rotation,
                scale: place.scale * 2.2,
            },
        ));
    }
    for (index, place) in placements(&planet, seed.derive("shrubs"), 900)
        .into_iter()
        .enumerate()
    {
        commands.spawn((
            Mesh3d(shrub_shapes[index % shrub_shapes.len()].clone()),
            MeshMaterial3d(shrub_look.clone()),
            Transform {
                translation: place.position,
                rotation: place.rotation,
                scale: place.scale * 0.9,
            },
        ));
    }
}

fn keep_sky_around_camera(
    camera: Single<&Transform, (With<Camera3d>, Without<SkyDome>)>,
    mut sky: Single<&mut Transform, With<SkyDome>>,
) {
    sky.translation = camera.translation;
}

/// A lumpy low-polygon shape with flat faces: a coarse sphere with every corner pushed in
/// or out by noise, then lit one face at a time.
fn faceted_blob(seed: Seed, lumpiness: f32) -> Mesh {
    let noise = Noise::new(seed);
    let mut mesh = Sphere::new(1.0).mesh().ico(1).unwrap();
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for position in positions {
            // Sampling by position moves corners that share a place by the same amount,
            // so the shape stays closed.
            let p = Vec3::from_array(*position);
            *position = (p * (1.0 + lumpiness * noise.sample(p * 1.7 + 3.1))).to_array();
        }
    }
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();
    mesh
}
