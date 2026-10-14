//! The light, air and sky of the scene: a low warm sun, cool shade, haze, and the
//! camera: its exposure, its film response and the effects that seat things in the world,
//! shade in corners and contact shadows (D212).

use crate::planet::MainCamera;
use crate::planet::Tweaks;
use crate::quality::Quality;
use crate::sky::{Sky, SkyPlugin};
use bevy::light::{FogVolume, ShadowFilteringMethod, VolumetricFog, VolumetricLight};
use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    camera::{Exposure, visibility::RenderLayers},
    core_pipeline::prepass::NormalPrepass,
    core_pipeline::tonemapping::Tonemapping,
    light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap},
    pbr::{ContactShadows, DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion},
    post_process::bloom::Bloom,
    prelude::*,
    render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection},
};
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
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
        .init_resource::<Indoors>()
        .add_systems(Update, (haze_follows, follow_quality, adapt_indoors));
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

/// The haze's density through its box, as a picture 64 by 48 by 64 (D244): a thin haze all
/// through it, and in its lowest dozen metres, round the ground, a mist up to seven times as
/// thick, in drifts that wander, so hollows fill and the light shafts have something to
/// cross. Tiling, so it can be laid fixed to the world as the box follows the camera.
fn haze_density() -> Image {
    const W: u32 = 64;
    const H: u32 = 48;
    const D: u32 = 64;
    const HAZE: f32 = 1.0 / 7.0;
    // A noise that repeats every `period` texels: values on a lattice, smoothly blended.
    let lattice = |i: i32, k: i32, period: i32, salt: u32| -> f32 {
        let (i, k) = (i.rem_euclid(period) as u32, k.rem_euclid(period) as u32);
        let mut x = i.wrapping_mul(0x9E37_79B9)
            ^ k.wrapping_mul(0x85EB_CA6B)
            ^ salt.wrapping_mul(0xC2B2_AE35);
        x ^= x >> 16;
        x = x.wrapping_mul(0x7FEB_352D);
        x ^= x >> 15;
        (x & 0xFFFF) as f32 / 65_535.0
    };
    let noise = |a: f32, c: f32, cells: i32, salt: u32| {
        let (i, k) = (a.floor() as i32, c.floor() as i32);
        let (u, v) = (a.fract(), c.fract());
        let (u, v) = (u * u * (3.0 - 2.0 * u), v * v * (3.0 - 2.0 * v));
        let at = |di, dk| lattice(i + di, k + dk, cells, salt);
        let top = at(0, 0) * (1.0 - u) + at(1, 0) * u;
        let bottom = at(0, 1) * (1.0 - u) + at(1, 1) * u;
        top * (1.0 - v) + bottom * v
    };
    let mut data = Vec::with_capacity((W * H * D) as usize);
    for z in 0..D {
        for y in 0..H {
            // Metres above the box's foot, which stands 10 m below the camera.
            let up = (y as f32 + 0.5) / H as f32 * HAZE_M.y;
            let low = 1.0 - smoothstep(9.0, 15.0, up);
            for x in 0..W {
                let (a, c) = (x as f32 / W as f32, z as f32 / D as f32);
                let drift =
                    0.6 * noise(a * 6.0, c * 6.0, 6, 1) + 0.4 * noise(a * 16.0, c * 16.0, 16, 2);
                let mist = low * smoothstep(0.3, 0.75, drift);
                data.push(((HAZE + (1.0 - HAZE) * mist).clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: D,
        },
        TextureDimension::D3,
        data,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    let mut sampler = ImageSamplerDescriptor::linear();
    sampler.set_address_mode(ImageAddressMode::Repeat);
    image.sampler = ImageSampler::Descriptor(sampler);
    image
}

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How thick the haze is at its thinnest; the mist is seven times this (D244). The day
/// thickens it in the morning (`daylight`).
pub const HAZE_DENSITY: f32 = 0.028;

fn haze(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn((
        Name::new("Haze"),
        Haze,
        FogVolume {
            fog_color: Color::srgb(0.95, 0.94, 0.88),
            density_factor: HAZE_DENSITY,
            density_texture: Some(images.add(haze_density())),
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
    mut haze: Query<(&mut Transform, &mut FogVolume), With<Haze>>,
    time: Res<Time>,
) {
    let (Some(camera), Ok((mut haze, mut fog))) = (camera.iter().next(), haze.single_mut()) else {
        return;
    };
    let at = camera.translation();
    let up = at.normalize_or(Vec3::Y);
    haze.translation = at + up * (HAZE_M.y * 0.5 - 10.0);
    haze.rotation = Quat::from_rotation_arc(Vec3::Y, up);
    // The mist's drifts stay where they are in the world as the box follows the camera, and
    // creep slowly downwind.
    let local = haze.rotation.inverse() * at / HAZE_M;
    let drift = Vec3::new(0.0006, 0.0, 0.0002) * time.elapsed_secs();
    fog.density_texture_offset = Vec3::new(local.x, 0.0, local.z) + drift;
}

fn style_sun(mut commands: Commands, sun: Single<(Entity, &mut DirectionalLight)>) {
    let (sun, mut light) = sun.into_inner();
    // The sun lights the haze, in shafts through the canopy (D217).
    commands.entity(sun).insert(VolumetricLight);
    // A low sun rakes across level surfaces, which need more leeway before they are
    // taken to shade themselves.
    light.shadow_depth_bias = 0.06;
    light.shadow_normal_bias = 2.6;
    // The sun sees the stand-ins plants cast their shadows from as well as everything the
    // camera sees. Its cascades and contact shadows follow the quality preset
    // (`follow_quality`).
    commands
        .entity(sun)
        .insert(RenderLayers::from_layers(&[0, crate::grown::SUN_ONLY]));
}

/// How far one sees between the trees before the haze swallows what is there.
const HAZE_VISIBLE_M: f32 = 240.0;

/// What the eye adapts round: the log brightness a scene meters at when it is as bright as
/// the day's exposure for the hour expects (D214).
const METERED_AS_SET: f32 = -3.6;
/// How far the eye adapts from the day's exposure: up to two and a half stops brighter in deep shade,
/// as under a canopy, and one darker in glare, as every game with a forest clamps it.
const ADAPT_BRIGHTER: f32 = 2.5;
const ADAPT_DARKER: f32 = 1.0;
/// How far it adapts brighter with the camera in a room closed and covered (D240): a room
/// lit through its doors and windows is four to six stops darker than the day outside,
/// and games with interiors let the eye open that far once inside, as Unreal's auto
/// exposure does over its range.
const ADAPT_BRIGHTER_INDOORS: f32 = 5.0;

/// Whether the camera is in a room closed and covered, which the game says (D155).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Indoors(pub bool);

/// The curve the eye adapts by, holding its correction to so many stops brighter and
/// darker: Bevy's auto exposure meters the frame and corrects by `curve(metered) -
/// metered`, so a curve flat at the expected brightness over the band, and following the
/// metering beyond it, holds the correction in the band.
fn compensation(brighter: f32, darker: f32) -> AutoExposureCompensationCurve {
    let at = METERED_AS_SET;
    AutoExposureCompensationCurve::from_curve(LinearSpline::new([
        Vec2::new(at - 8.0, at - 8.0 + brighter),
        Vec2::new(at - brighter, at),
        Vec2::new(at + darker, at),
        Vec2::new(at + 8.0, at + 8.0 - darker),
    ]))
    .expect("a valid curve")
}

/// Eye adaptation clamped round the day's exposure.
fn adaptation(curves: &mut Assets<AutoExposureCompensationCurve>) -> AutoExposure {
    AutoExposure {
        compensation_curve: curves.add(compensation(ADAPT_BRIGHTER, ADAPT_DARKER)),
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
    // Haze between the trees, thickening with distance: a forest's own air, at the scale of
    // its sightlines, where the physical air (`air`) works at the scale of hills. Its colour
    // follows the hour (`daylight`). The Witcher 3 fades its distance the same way, so its
    // far trees change detail in air that has already flattened them (D224).
    commands.entity(camera).insert(DistanceFog {
        color: Color::srgb(0.5, 0.56, 0.55),
        directional_light_color: Color::srgba(1.0, 0.85, 0.65, 0.5),
        directional_light_exponent: 16.0,
        falloff: FogFalloff::from_visibility_squared(HAZE_VISIBLE_M),
    });
    commands.entity(camera).insert((
        Exposure {
            ev100: EXPOSURE_EV100,
        },
        // Ambient occlusion works on single samples, so edges are smoothed afterwards.
        Msaa::Off,
        // Each frame jittered and blended with the last, as every game with dense foliage
        // does: it resolves the dithered crossfades between levels of detail, which
        // otherwise crawl, and stops leaves and blades shimmering (D214). The wind shaders
        // give it last frame's positions.
        TemporalAntiAliasing::default(),
        // Khronos PBR Neutral: colours as they are and the full range from deep shade to
        // bright sun, which a forest needs. AgX squeezed it into mid-greys, so the shade under
        // the canopy and the sun flecks in it read alike: the scene's middle 90% spanned
        // the same 106 levels with the mist, haze and adaptation each turned off (D254).
        Tonemapping::KhronosPbrNeutral,
        // AgX alone is soft, as a film's negative is before it is printed: a little more
        // contrast through the middle and shadows, as a print's look adds.
        ColorGrading {
            global: ColorGradingGlobal {
                // Neutral keeps saturation where AgX drained it; a little less, toward the
                // muted greens of the forests we look to.
                post_saturation: 0.82,
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

/// Puts the quality preset into effect (D239), at the start and whenever it changes: the
/// sun's shadow map and cascades, shade in corners and contact shadows, and the haze's
/// shafts of light.
fn follow_quality(
    quality: Res<Quality>,
    mut applied: Local<Option<Quality>>,
    mut commands: Commands,
    camera: Query<Entity, With<MainCamera>>,
    mut sun: Query<(Entity, &mut DirectionalLight)>,
    mut map: ResMut<DirectionalLightShadowMap>,
) {
    let (Some(camera), Ok((sun, mut light))) = (camera.iter().next(), sun.single_mut()) else {
        return;
    };
    if *applied == Some(*quality) {
        return;
    }
    *applied = Some(*quality);
    let shadows = quality.shadows();
    if map.size != shadows.map {
        map.size = shadows.map;
    }
    // Sharp shadows close up, and shadows at all out to the far trees.
    commands.entity(sun).insert(
        CascadeShadowConfigBuilder {
            num_cascades: shadows.cascades,
            first_cascade_far_bound: shadows.first_m,
            maximum_distance: shadows.most_m,
            ..default()
        }
        .build(),
    );
    // Where things meet the ground, finer than the shadow map can draw (D204).
    light.contact_shadows_enabled = quality.occlusion();
    let mut camera = commands.entity(camera);
    if quality.occlusion() {
        camera.insert((
            // Shade in corners, under eaves and where things stand on the ground, over a
            // reach the size of a building's details.
            ScreenSpaceAmbientOcclusion {
                radius: 1.2,
                ..default()
            },
            // Short shadows marched through the depth buffer, where a foot, a root or a
            // blade meets the ground.
            ContactShadows {
                length: 0.4,
                ..default()
            },
        ));
    } else {
        // Without them nothing reads the prepass's normals, so they are not drawn.
        camera.remove::<(ScreenSpaceAmbientOcclusion, ContactShadows, NormalPrepass)>();
    }
    // Soft shadow edges that TAA smooths over frames, or the hardware's own.
    camera.insert(if quality.soft_shadows() {
        ShadowFilteringMethod::Temporal
    } else {
        ShadowFilteringMethod::Hardware2x2
    });
    if quality.bloom() {
        // A soft glow round the sun and bright sky, not a haze over everything.
        // An energy-conserving bloom blends this much of a wide blur into every pixel,
        // which at 0.1 washed the middle distance flat.
        camera.insert(Bloom {
            intensity: 0.03,
            ..Bloom::NATURAL
        });
    } else {
        camera.remove::<Bloom>();
    }
    match quality.haze_steps() {
        Some(steps) => {
            camera.insert(VolumetricFog {
                // The sky's light in the haze: under trees it comes through leaves, greener and
                // far dimmer than the open sky's, which laid a grey-blue veil over the spruces
                // (D266). The haze glows where the sun lights it, in shafts.
                ambient_color: Color::srgb(0.55, 0.62, 0.5),
                ambient_intensity: 0.015,
                // Each frame's march starts a little differently, for TAA to smooth.
                jitter: 0.5,
                step_count: steps,
            });
        }
        None => {
            camera.remove::<VolumetricFog>();
        }
    }
}

/// Lets the eye open further with the camera indoors, and back to the outdoor band when it
/// leaves; it adapts there at its own speed, as an eye does stepping through a door.
fn adapt_indoors(
    indoors: Res<Indoors>,
    camera: Query<&AutoExposure, With<MainCamera>>,
    mut curves: ResMut<Assets<AutoExposureCompensationCurve>>,
) {
    let Some(adapting) = camera.iter().next() else {
        return;
    };
    // Changed, or the camera's adaptation newly set up.
    if !indoors.is_changed() {
        return;
    }
    let brighter = if indoors.0 {
        ADAPT_BRIGHTER_INDOORS
    } else {
        ADAPT_BRIGHTER
    };
    if let Some(mut curve) = curves.get_mut(&adapting.compensation_curve) {
        *curve = compensation(brighter, ADAPT_DARKER);
    }
}
