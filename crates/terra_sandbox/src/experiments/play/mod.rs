//! A player at an outpost by the water, with a builder tool.
//!
//! You are a body in the world: an animated figure you steer in third person, which
//! walks on the planet, is held up by a base's floors, stopped by its walls and windows,
//! and let through its doors. The figure carries one tool, a builder, whose two actions
//! depend on its mode.
//!
//!   Mouse         look (click to capture the mouse, Esc to release it)
//!   W A S D       move          Shift   sprint       Space   jump
//!   1 2 3         builder mode: shape, openings, room
//!   Left click    main action   (E does the same)
//!   Right click   second action (R does the same)
//!   F3            frame rate
//!
//! | Mode     | Main action                         | Second action          |
//! |----------|-------------------------------------|------------------------|
//! | Shape    | Push the wall you aim at out        | Pull it in             |
//! | Openings | Change the tile: window, door, ...  | Make it solid again    |
//! | Room     | Add a room on the ground you aim at |                        |
//!
//! The scene around the player is in the modules here: `scene` is the light and air,
//! `flora` what grows, `figure` the player's body, and `tool` the builder and its
//! on-screen prompts. See docs/player.md for the design this is a step toward.

mod figure;
mod flora;
mod models;
mod props;
mod scene;
mod tool;

use super::build::{self, DrivenByPlayer, Site};
use crate::planet::Planet;
use crate::shell::Options;
use bevy::{
    camera_controller::free_camera::FreeCamera,
    dev_tools::fps_overlay::FpsOverlayConfig,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use std::ops::Sub;
use terra_build::Walker;

const JOG_SPEED: f32 = 4.3;
const SPRINT_SPEED: f32 = 7.4;
const JUMP_SPEED: f32 = 6.0;
const GRAVITY: f32 = 16.0;
const LOOK_SPEED: f32 = 0.0025;
const CAMERA_BACK_M: f32 = 4.6;
const HEAD_M: f32 = 1.62;
/// The player can wade this deep and no deeper.
const WADE_DEPTH_M: f32 = 0.7;

#[derive(Component)]
struct Player {
    /// Speed over the ground, as a direction and metres a second.
    velocity: Vec3,
    /// Speed away from the planet's centre, in metres a second.
    rising: f32,
    on_ground: bool,
    /// Seconds since the feet last touched something.
    airborne: f32,
    indoors: bool,
    /// The level direction the figure faces.
    facing: Vec3,
    /// Seconds since the tool was last used.
    since_acting: f32,
}

/// Where the player is looking.
#[derive(Resource)]
struct View {
    /// Along the ground.
    forward: Vec3,
    /// Up or down from level, in radians.
    pitch: f32,
}

pub fn plugin(app: &mut App) {
    // A player needs something to build on, and takes over from the build keys.
    build::plugin(app);
    scene::plugin(app);
    flora::plugin(app);
    props::plugin(app);
    figure::plugin(app);
    tool::plugin(app);
    app.insert_resource(DrivenByPlayer)
        .add_systems(Startup, hide_frame_rate)
        .add_systems(PostStartup, spawn)
        .add_systems(
            Update,
            (capture_mouse, look, walk, toggle_frame_rate)
                .chain()
                .before(tool::Acting),
        )
        .add_systems(PostUpdate, follow.before(TransformSystems::Propagate));
}

/// The frame counter is a tool for us, not part of the scene; F3 brings it back.
fn hide_frame_rate(mut overlay: ResMut<FpsOverlayConfig>) {
    overlay.enabled = false;
    overlay.frame_time_graph_config.enabled = false;
}

fn toggle_frame_rate(keys: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<FpsOverlayConfig>) {
    if keys.just_pressed(KeyCode::F3) {
        overlay.enabled = !overlay.enabled;
        overlay.frame_time_graph_config.enabled = overlay.enabled;
    }
}

fn spawn(
    mut commands: Commands,
    site: Res<Site>,
    planet: Res<Planet>,
    camera: Single<Entity, With<Camera3d>>,
) {
    // The player steers the camera now.
    commands.entity(*camera).remove::<FreeCamera>();

    // Stand a little way out from the door, facing it.
    let (approach, facing) = site.approach();
    let outside = site.frame.transform_point(approach - facing * 5.0);
    let feet = planet.surface(outside.normalize());
    let up = feet.normalize();
    let toward_door = site
        .frame
        .transform_point(approach + facing)
        .sub(outside)
        .reject_from(up)
        .normalize();
    commands.insert_resource(View {
        forward: toward_door,
        pitch: -0.08,
    });
    commands.spawn((
        Name::new("Player"),
        Player {
            velocity: Vec3::ZERO,
            rising: 0.0,
            on_ground: false,
            airborne: 0.0,
            indoors: false,
            facing: toward_door,
            since_acting: f32::MAX,
        },
        Transform::from_translation(feet).looking_to(toward_door, up),
        Visibility::default(),
    ));
}

fn capture_mouse(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut cursor: Single<&mut CursorOptions>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    } else if mouse.just_pressed(MouseButton::Left) && cursor.grab_mode == CursorGrabMode::None {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

fn look(
    motion: Res<AccumulatedMouseMotion>,
    cursor: Single<&CursorOptions>,
    player: Single<&Transform, With<Player>>,
    mut view: ResMut<View>,
) {
    let up = player.translation.normalize();
    if cursor.grab_mode != CursorGrabMode::None {
        view.forward = Quat::from_axis_angle(up, -motion.delta.x * LOOK_SPEED) * view.forward;
        view.pitch = (view.pitch - motion.delta.y * LOOK_SPEED).clamp(-1.2, 1.2);
    }
    // "Level" changes as the player walks round the planet.
    view.forward = view
        .forward
        .reject_from(up)
        .normalize_or(up.any_orthonormal_vector());
}

fn walk(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    planet: Res<Planet>,
    site: Res<Site>,
    view: Res<View>,
    obstacles: Res<flora::Obstacles>,
    player: Single<(&mut Transform, &mut Player)>,
) {
    let (mut transform, mut player) = player.into_inner();
    let walker = Walker::default();
    let dt = time.delta_secs().min(0.05);
    let feet = transform.translation;
    let up = feet.normalize();
    let right = view.forward.cross(up);

    let pressed = |key: KeyCode| f32::from(keys.pressed(key));
    let wish = (view.forward * (pressed(KeyCode::KeyW) - pressed(KeyCode::KeyS))
        + right * (pressed(KeyCode::KeyD) - pressed(KeyCode::KeyA)))
    .normalize_or_zero();
    let top_speed = if keys.pressed(KeyCode::ShiftLeft) {
        SPRINT_SPEED
    } else {
        JOG_SPEED
    };
    // Ease into and out of a run rather than starting and stopping dead.
    let ease = 1.0 - libm::expf(-dt * if player.on_ground { 11.0 } else { 2.5 });
    player.velocity = player.velocity.lerp(wish * top_speed, ease).reject_from(up);
    let mut target = feet + player.velocity * dt;

    // The base decides what blocks the way; it works in its own space.
    let to_base = site.frame.compute_affine().inverse();
    let from_base = site.frame.compute_affine();
    let walked = site.base.walk(
        to_base.transform_point3(feet),
        to_base.transform_point3(target),
        &walker,
    );
    target = from_base.transform_point3(walked);
    // Trees and boulders push the player round them.
    for &(at, radius) in &obstacles.0 {
        let apart = (target - at).reject_from(up);
        let reach = radius + walker.radius;
        if apart.length_squared() < reach * reach && (target - at).dot(up).abs() < 3.0 {
            target += apart.normalize_or(view.forward) * (reach - apart.length());
        }
    }
    // The sea is for looking at: wade in a little way and no further.
    let ground_ahead = planet.surface(target.normalize()).length();
    if ground_ahead < planet.sea_radius_m() - WADE_DEPTH_M
        && ground_ahead < planet.surface(up).length()
    {
        target = feet;
    }

    player.rising -= GRAVITY * dt;
    if player.on_ground && keys.just_pressed(KeyCode::Space) {
        player.rising = JUMP_SPEED;
    }
    target += up * player.rising * dt;

    // What holds the player up: indoors, the base's floor; outdoors, whichever is higher of
    // the ground and anything the base offers there, such as its steps or a roof.
    let local = to_base.transform_point3(target);
    let before = to_base.transform_point3(feet);
    let offered = site.support(Vec3::new(local.x, before.y.max(local.y), local.z), &walker);
    let offered = offered.map(|height| {
        from_base
            .transform_point3(Vec3::new(local.x, height, local.z))
            .length()
    });
    let ground = planet.surface(target.normalize()).length();
    let held_at = match offered {
        Some(floor) if site.base.ceiling(before).is_some() => floor,
        Some(surface) => surface.max(ground),
        None => ground,
    };
    player.on_ground = target.length() <= held_at + 0.02 && player.rising <= 0.0;
    if player.on_ground {
        target = target.normalize() * held_at;
        player.rising = 0.0;
        player.airborne = 0.0;
    } else {
        player.airborne += dt;
    }
    // A ceiling stops a jump.
    if let Some(ceiling) = site.base.ceiling(before)
        && player.rising > 0.0
        && to_base.transform_point3(target).y + walker.height > ceiling
    {
        target = from_base.transform_point3(Vec3::new(local.x, ceiling - walker.height, local.z));
        player.rising = 0.0;
    }

    let indoors = site
        .base
        .ceiling(to_base.transform_point3(target))
        .is_some();
    if indoors != player.indoors {
        player.indoors = indoors;
        info!(
            "Player is now {}.",
            if indoors {
                "inside the base"
            } else {
                "outside"
            }
        );
    }

    // What was actually covered, which is less than what was asked when something is in
    // the way. The figure's stride follows this.
    let up = target.normalize();
    player.velocity = ((target - feet) / dt.max(1e-4)).reject_from(up);
    // Turn to face the way the figure is going, or the way it is working.
    player.since_acting += dt;
    let toward = if player.since_acting < 0.8 {
        view.forward
    } else {
        wish
    };
    let turn = 1.0 - libm::expf(-dt * 12.0);
    player.facing = player
        .facing
        .lerp(toward, if toward == Vec3::ZERO { 0.0 } else { turn })
        .reject_from(up)
        .normalize_or(view.forward);
    *transform = Transform::from_translation(target).looking_to(player.facing, up);
}

/// The eye, and the direction it looks, for the player's view.
fn eye(player: &Transform, view: &View) -> (Vec3, Vec3) {
    let up = player.translation.normalize();
    let right = view.forward.cross(up);
    let looking = Quat::from_axis_angle(right, view.pitch) * view.forward;
    (player.translation + up * HEAD_M, looking)
}

/// Third person: behind and a little to the side of the figure, pulled in when a wall of
/// the base would otherwise come between the camera and the player.
fn follow(
    player: Single<&Transform, (With<Player>, Without<Camera3d>)>,
    view: Res<View>,
    site: Res<Site>,
    planet: Res<Planet>,
    options: Res<Options>,
    mut camera: Single<&mut Transform, With<Camera3d>>,
) {
    // A fixed camera, for taking pictures.
    if let Some((from, toward)) = options.vantage {
        let from = site.frame.transform_point(from);
        **camera = Transform::from_translation(from)
            .looking_at(site.frame.transform_point(toward), from.normalize());
        return;
    }
    let (head, looking) = eye(&player, &view);
    let up = head.normalize();
    let right = view.forward.cross(up);
    let pivot = head + right * 0.6 + up * 0.25;

    let to_base = site.frame.compute_affine().inverse();
    let back = -looking;
    let blocked = site
        .base
        .raycast(
            to_base.transform_point3(pivot),
            to_base.transform_vector3(back).normalize(),
            CAMERA_BACK_M,
        )
        .map_or(CAMERA_BACK_M, |hit| (hit.distance - 0.25).max(0.3));
    let mut at = pivot + back * blocked;
    // Keep the camera out of the ground.
    let ground = planet.surface(at.normalize()).length() + 0.35;
    if at.length() < ground {
        at = at.normalize() * ground;
    }
    **camera = Transform::from_translation(at).looking_to(looking, up);
}
