//! The light, air and sky of the scene: a low warm sun, cool shade, haze, and the
//! camera: its exposure, its film response and the effects that seat things in the world,
//! shade in corners and contact shadows (D212).

use crate::planet::MainCamera;
use crate::planet::Tweaks;
use crate::sky::{Sky, SkyPlugin};
use bevy::light::{FogVolume, ShadowFilteringMethod, VolumetricFog, VolumetricLight};
use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    camera::{Exposure, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap},
    pbr::{ContactShadows, ScreenSpaceAmbientOcclusion},
    post_process::bloom::Bloom,
    prelude::*,
    render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection},
};
use bevy::{
    curve::cubic_splines::LinearSpline,
    post_process::auto_exposure::{
        AutoExposure, AutoExposureCompensationCurve, AutoExposurePlugin,
    },
};

/// Direction from the scene toward the sun: low over the water.
pub const SUN_DIRECTION: Vec3 = Vec3::new(0.74, 0.33, 0.58);
const HORIZON: Color = Color::srgb(0.98, 0.85, 0.7);
const ZENITH: Color = Color::srgb(0.24, 0.46, 0.84);
/// The camera's exposure, as a photographer's EV at ISO 100: set for sunlit ground, as a
/// camera on a fine day is, so that a mid-grey surface in the sun comes out mid-grey.
pub const EXPOSURE_EV100: f32 = 10.8;

pub fn plugin(app: &mut App) {
    app.add_plugins((SkyPlugin, AutoExposurePlugin))
        .insert_resource(Sky {
            // The sky's colour is the air's (`air`); this draws the clouds over it.
            over_air: true,
            ..Sky::by_day(HORIZON, ZENITH, Color::srgb(1.0, 0.74, 0.46), 0.55, 1.6)
        })
        // Shade is lit by the sky (`skylight`), so it comes out cool against the warm sun.
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .add_systems(Startup, preset)
        .add_systems(PostStartup, (style_camera, style_sun, haze))
        .add_systems(Update, haze_follows);
}

fn preset(mut tweaks: ResMut<Tweaks>) {
    tweaks.sun_direction = SUN_DIRECTION;
    tweaks.sun_color = Color::srgb(1.0, 0.96, 0.9);
    tweaks.sun_illuminance = 30_000.0;
    // Brown earth showing through grass, in the colours real ground has (D14): soil and
    // grass are darker and less saturated than a painted palette would make them.
    tweaks.ground_low = Color::srgb(0.42, 0.32, 0.24);
    tweaks.ground_high = Color::srgb(0.5, 0.4, 0.3);
    tweaks.ground_steep = Color::srgb(0.38, 0.34, 0.3);
    tweaks.ground_cover = 1.0;
    tweaks.grass_dry = Color::srgb(0.46, 0.43, 0.27);
    tweaks.grass_lush = Color::srgb(0.27, 0.36, 0.15);
    tweaks.sand = Color::srgb(0.62, 0.56, 0.45);
    tweaks.sea_shallow = Color::srgb(0.08, 0.42, 0.5);
    tweaks.sea_deep = Color::srgb(0.03, 0.14, 0.27);
}

/// A thin haze round the camera that the sun lights in shafts where it comes through the
/// leaves, as the forests of Red Dead Redemption 2 and Sons of the Forest have (D217): Bevy's
/// volumetric fog, a box this wide and tall that follows the camera.
#[derive(Component)]
struct Haze;
const HAZE_M: Vec3 = Vec3::new(240.0, 60.0, 240.0);

fn haze(mut commands: Commands) {
    commands.spawn((
        Name::new("Haze"),
        Haze,
        FogVolume {
            fog_color: Color::srgb(0.92, 0.95, 1.0),
            density_factor: 0.008,
            absorption: 0.05,
            scattering: 0.5,
            // Light scattered on forward, so a shaft shows best looking toward the sun.
            scattering_asymmetry: 0.7,
            ..default()
        },
        Transform::from_scale(HAZE_M),
    ));
}

/// The haze stands round the camera, upright on the planet, its foot below the ground.
fn haze_follows(
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut haze: Query<&mut Transform, With<Haze>>,
) {
    let (Some(camera), Ok(mut haze)) = (camera.iter().next(), haze.single_mut()) else {
        return;
    };
    let at = camera.translation();
    let up = at.normalize_or(Vec3::Y);
    haze.translation = at + up * (HAZE_M.y * 0.5 - 10.0);
    haze.rotation = Quat::from_rotation_arc(Vec3::Y, up);
}

fn style_sun(mut commands: Commands, sun: Single<(Entity, &mut DirectionalLight)>) {
    let (sun, mut light) = sun.into_inner();
    // The sun lights the haze, in shafts through the canopy (D217).
    commands.entity(sun).insert(VolumetricLight);
    // A low sun rakes across level surfaces, which need more leeway before they are
    // taken to shade themselves.
    light.shadow_depth_bias = 0.06;
    light.shadow_normal_bias = 2.6;
    // Where things meet the ground, finer than the shadow map can draw (D204).
    light.contact_shadows_enabled = true;
    // Sharp shadows close up, and shadows at all out to the far trees. The sun sees the
    // stand-ins plants cast their shadows from as well as everything the camera sees.
    commands.entity(sun).insert((
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            first_cascade_far_bound: 24.0,
            maximum_distance: 300.0,
            ..default()
        }
        .build(),
        RenderLayers::from_layers(&[0, crate::grown::SUN_ONLY]),
    ));
}

/// What the eye adapts round: the log brightness a scene meters at when it is as bright as
/// the day's exposure for the hour expects (D214).
const METERED_AS_SET: f32 = -3.0;
/// How far the eye adapts from the day's exposure: up to two stops brighter in deep shade,
/// as under a canopy, and one darker in glare, as every game with a forest clamps it.
const ADAPT_BRIGHTER: f32 = 2.0;
const ADAPT_DARKER: f32 = 1.0;

/// Eye adaptation clamped round the day's exposure: Bevy's auto exposure meters the frame
/// and corrects by `curve(metered) - metered`, so a curve flat at the expected brightness
/// over the band, and following the metering beyond it, holds the correction in the band.
fn adaptation(curves: &mut Assets<AutoExposureCompensationCurve>) -> AutoExposure {
    let at = METERED_AS_SET;
    let curve = AutoExposureCompensationCurve::from_curve(LinearSpline::new([
        Vec2::new(at - 8.0, at - 8.0 + ADAPT_BRIGHTER),
        Vec2::new(at - ADAPT_BRIGHTER, at),
        Vec2::new(at + ADAPT_DARKER, at),
        Vec2::new(at + 8.0, at + 8.0 - ADAPT_DARKER),
    ]))
    .expect("a valid curve");
    AutoExposure {
        compensation_curve: curves.add(curve),
        // An eye takes a few seconds to open up in the shade, and less to close in the sun.
        speed_brighten: 1.2,
        speed_darken: 2.0,
        ..default()
    }
}

/// The field of view is a setting, put into effect by `options::apply`.
fn style_camera(
    mut commands: Commands,
    camera: Single<Entity, With<MainCamera>>,
    mut curves: ResMut<Assets<AutoExposureCompensationCurve>>,
) {
    let camera = *camera;
    commands.entity(camera).insert(adaptation(&mut curves));
    commands.entity(camera).insert(VolumetricFog {
        // The sky's light in the haze, roughly its colour.
        ambient_color: Color::srgb(0.7, 0.8, 1.0),
        ambient_intensity: 0.05,
        // Each frame's march starts a little differently, for TAA to smooth.
        jitter: 0.5,
        step_count: 48,
    });
    commands.entity(camera).insert((
        Exposure {
            ev100: EXPOSURE_EV100,
        },
        // Ambient occlusion works on single samples, so edges are smoothed afterwards.
        Msaa::Off,
        // Shade in corners, under eaves and where things stand on the ground, over a reach
        // the size of a building's details.
        ScreenSpaceAmbientOcclusion {
            radius: 1.2,
            ..default()
        },
        // Short shadows marched through the depth buffer, where a foot, a root or a blade
        // meets the ground.
        ContactShadows {
            length: 0.4,
            ..default()
        },
        // Each frame jittered and blended with the last, as every game with dense foliage
        // does: it resolves the dithered crossfades between levels of detail, which
        // otherwise crawl, and stops leaves and blades shimmering (D214). The wind shaders
        // give it last frame's positions.
        TemporalAntiAliasing::default(),
        // Soft shadow edges that TAA smooths over frames.
        ShadowFilteringMethod::Temporal,
        // A soft glow round the sun and bright sky, not a haze over everything.
        Bloom {
            intensity: 0.1,
            ..Bloom::NATURAL
        },
        // A film's response: highlights roll off and lose their colour as they brighten,
        // as on film, rather than turning saturated and flat (ACES does both).
        Tonemapping::AgX,
        // AgX alone is soft, as a film's negative is before it is printed: a little more
        // contrast through the middle and shadows, as a print's look adds.
        ColorGrading {
            global: ColorGradingGlobal {
                post_saturation: 1.12,
                ..default()
            },
            // No extra contrast in the shadows, which crushed the shade under a canopy to
            // black (D216); a little in the midtones.
            midtones: ColorGradingSection {
                contrast: 1.1,
                ..default()
            },
            ..default()
        },
    ));
}
