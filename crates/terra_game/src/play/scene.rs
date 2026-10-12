//! The light, air and sky of the scene: a low warm sun, cool shade, haze, and the
//! camera effects that make flat-coloured shapes read as solid.

use crate::planet::MainCamera;
use crate::planet::Tweaks;
use crate::sky::{Sky, SkyPlugin};
use bevy::{
    anti_alias::smaa::Smaa,
    core_pipeline::tonemapping::Tonemapping,
    light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap},
    pbr::ScreenSpaceAmbientOcclusion,
    post_process::bloom::Bloom,
    prelude::*,
    render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection},
};

/// Direction from the scene toward the sun: low over the water.
pub const SUN_DIRECTION: Vec3 = Vec3::new(0.74, 0.33, 0.58);
const HORIZON: Color = Color::srgb(0.98, 0.85, 0.7);
const ZENITH: Color = Color::srgb(0.24, 0.46, 0.84);

pub fn plugin(app: &mut App) {
    app.add_plugins(SkyPlugin)
        .insert_resource(Sky::by_day(
            HORIZON,
            ZENITH,
            Color::srgb(1.0, 0.74, 0.46),
            0.55,
            2.4,
        ))
        // Shade is lit by the sky, so it comes out cool against the warm sun.
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.66, 0.76, 1.0),
            brightness: 1800.0,
            ..default()
        })
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .add_systems(Startup, preset)
        .add_systems(PostStartup, (style_camera, style_sun));
}

fn preset(mut tweaks: ResMut<Tweaks>) {
    tweaks.sun_direction = SUN_DIRECTION;
    tweaks.sun_color = Color::srgb(1.0, 0.87, 0.7);
    tweaks.sun_illuminance = 21_000.0;
    // Red soil showing through young growth.
    tweaks.ground_low = Color::srgb(0.7, 0.4, 0.3);
    tweaks.ground_high = Color::srgb(0.82, 0.58, 0.42);
    tweaks.ground_steep = Color::srgb(0.5, 0.3, 0.27);
    tweaks.ground_cover = 1.0;
    tweaks.grass_dry = Color::srgb(0.6, 0.55, 0.28);
    tweaks.grass_lush = Color::srgb(0.36, 0.49, 0.21);
    tweaks.sand = Color::srgb(0.9, 0.76, 0.56);
    tweaks.sea_shallow = Color::srgb(0.08, 0.42, 0.5);
    tweaks.sea_deep = Color::srgb(0.03, 0.14, 0.27);
}

fn style_sun(mut commands: Commands, sun: Single<(Entity, &mut DirectionalLight)>) {
    let (sun, mut light) = sun.into_inner();
    // A low sun rakes across level surfaces, which need more leeway before they are
    // taken to shade themselves.
    light.shadow_depth_bias = 0.06;
    light.shadow_normal_bias = 2.6;
    // Sharp shadows close up, and shadows at all out to the far trees.
    commands.entity(sun).insert(
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            first_cascade_far_bound: 24.0,
            maximum_distance: 300.0,
            ..default()
        }
        .build(),
    );
}

/// The field of view is a setting, put into effect by `options::apply`.
fn style_camera(mut commands: Commands, camera: Single<Entity, With<MainCamera>>) {
    let camera = *camera;
    commands.entity(camera).insert((
        // Ambient occlusion works on single samples, so edges are smoothed afterwards.
        Msaa::Off,
        ScreenSpaceAmbientOcclusion::default(),
        Smaa::default(),
        Bloom {
            intensity: 0.2,
            ..Bloom::NATURAL
        },
        Tonemapping::TonyMcMapface,
        ColorGrading {
            global: ColorGradingGlobal {
                post_saturation: 1.15,
                ..default()
            },
            shadows: ColorGradingSection {
                contrast: 1.04,
                ..default()
            },
            ..default()
        },
        DistanceFog {
            color: HORIZON,
            directional_light_color: Color::srgba(1.0, 0.86, 0.66, 0.3),
            directional_light_exponent: 10.0,
            // Distant ground fades toward the horizon's colour, which is what turns far
            // hills into soft layers.
            falloff: FogFalloff::from_visibility_colors(
                2600.0,
                Color::srgb(0.5, 0.5, 0.62),
                Color::srgb(0.95, 0.88, 0.82),
            ),
        },
    ));
}
