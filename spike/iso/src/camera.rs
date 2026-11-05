//! The view: one fixed angle from above and to the south, orthographic as a painted
//! background is, panned by the keys, the screen's edge or the middle button, and zoomed
//! by the wheel. Never turned: the Infinity Engine never turns its camera either.

use crate::Args;
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::camera::{Exposure, RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::light::ShadowFilteringMethod;
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// How far above the horizon the camera looks down, and how far it is turned from due north.
const ELEVATION: f32 = 42.0;
const YAW: f32 = 28.0;
const DISTANCE: f32 = 60.0;

#[derive(Resource)]
pub struct View {
    pub focus: Vec2,
    /// How many metres the screen is tall.
    pub zoom: f32,
}

#[derive(Component)]
pub struct MainCamera;

/// What the main camera draws to when no window is open (a scripted still).
#[derive(Resource)]
pub struct Offscreen(pub Handle<Image>);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn)
        .add_systems(Update, (pan, place).chain());
}

pub fn offset() -> Vec3 {
    let (el, yaw) = (ELEVATION.to_radians(), YAW.to_radians());
    Vec3::new(el.cos() * yaw.sin(), el.sin(), el.cos() * yaw.cos()) * DISTANCE
}

fn spawn(mut commands: Commands, args: Res<Args>, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(View {
        focus: args.focus,
        zoom: args.zoom,
    });
    let mut camera = commands.spawn((
        Camera3d::default(),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: args.zoom,
            },
            near: 1.0,
            far: 200.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(offset()).looking_at(Vec3::ZERO, Vec3::Y),
        bevy::camera::Hdr,
        Msaa::Off,
        TemporalAntiAliasing::default(),
        ScreenSpaceAmbientOcclusion::default(),
        ShadowFilteringMethod::Temporal,
        Bloom::NATURAL,
        Tonemapping::AgX,
        Exposure { ev100: 9.0 },
        // A little haze: the land farther off (up the screen) a shade paler and bluer than
        // the crowns near, its colour the hour's (scene.rs).
        bevy::pbr::DistanceFog {
            color: Color::NONE,
            directional_light_color: Color::NONE,
            falloff: bevy::pbr::FogFalloff::Linear {
                start: DISTANCE - 2.0,
                end: DISTANCE + 70.0,
            },
            ..default()
        },
        MainCamera,
    ));
    if args.shot.is_some() {
        let (w, h) = args.size;
        let image = Image::new_target_texture(
            w,
            h,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        );
        let image = images.add(image);
        camera.insert(RenderTarget::Image(image.clone().into()));
        commands.insert_resource(Offscreen(image));
    }
}

fn pan(
    mut view: ResMut<View>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    windows: Query<&Window>,
    time: Res<Time<Real>>,
) {
    let dt = time.delta_secs();
    let (yaw_s, yaw_c) = YAW.to_radians().sin_cos();
    // Screen right and screen up, laid on the ground.
    let right = Vec2::new(yaw_c, -yaw_s);
    let up = Vec2::new(-yaw_s, -yaw_c);
    let mut push = Vec2::ZERO;
    for (key, d) in [
        (KeyCode::KeyW, Vec2::Y),
        (KeyCode::ArrowUp, Vec2::Y),
        (KeyCode::KeyS, -Vec2::Y),
        (KeyCode::ArrowDown, -Vec2::Y),
        (KeyCode::KeyD, Vec2::X),
        (KeyCode::ArrowRight, Vec2::X),
        (KeyCode::KeyA, -Vec2::X),
        (KeyCode::ArrowLeft, -Vec2::X),
    ] {
        if keys.pressed(key) {
            push += d;
        }
    }
    let mut height = 900.0;
    if let Ok(window) = windows.single() {
        height = window.height();
        if window.focused
            && let Some(c) = window.cursor_position()
        {
            let edge = 6.0;
            if c.x < edge {
                push.x -= 1.0;
            }
            if c.x > window.width() - edge {
                push.x += 1.0;
            }
            if c.y < edge {
                push.y += 1.0;
            }
            if c.y > window.height() - edge {
                push.y -= 1.0;
            }
        }
    }
    let speed = view.zoom * 0.9;
    let push = push.clamp_length_max(1.0) * speed * dt;
    view.focus += right * push.x + up * push.y / ELEVATION.to_radians().sin();
    if buttons.pressed(MouseButton::Middle) {
        let per_pixel = view.zoom / height;
        let d = motion.delta * per_pixel;
        view.focus -= right * d.x - up * d.y / ELEVATION.to_radians().sin();
    }
    if scroll.delta.y != 0.0 {
        view.zoom = (view.zoom * (1.0 - scroll.delta.y.signum() * 0.1)).clamp(7.0, 40.0);
    }
    view.focus = view.focus.clamp(Vec2::splat(-24.0), Vec2::splat(24.0));
}

fn place(
    view: Res<View>,
    land: Res<crate::terrain::Land>,
    mut camera: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    let Ok((mut tr, mut projection)) = camera.single_mut() else {
        return;
    };
    let focus = Vec3::new(view.focus.x, land.height(view.focus), view.focus.y);
    *tr = Transform::from_translation(focus + offset()).looking_at(focus, Vec3::Y);
    if let Projection::Orthographic(ortho) = projection.as_mut() {
        ortho.scaling_mode = ScalingMode::FixedVertical {
            viewport_height: view.zoom,
        };
    }
}
