//! A player on the ground with a builder tool.
//!
//! The first build where you are a body in the world: a figure you steer in third person
//! that walks on the planet, is held up by a base's floors, stopped by its walls and
//! windows, and let through its doors. The figure holds one tool, a builder, whose main
//! and second actions depend on its mode.
//!
//!   Mouse         look (click to capture the mouse, Esc to release it)
//!   W A S D       walk          Shift   run          Space   jump
//!   1 2 3         builder mode: shape, openings, room
//!   Left click    main action   (E does the same)
//!   Right click   second action (R does the same)
//!
//! | Mode     | Main action                         | Second action          |
//! |----------|-------------------------------------|------------------------|
//! | Shape    | Push the wall you aim at out        | Pull it in             |
//! | Openings | Change the tile: window, door, ...  | Make it solid again    |
//! | Room     | Add a room on the ground you aim at |                        |
//!
//! Everything is a placeholder: the figure is a few simple shapes with no animation, and
//! the planet is the stand-in one. See docs/player.md for the design this is a first
//! step toward.

use super::build::{self, DrivenByPlayer, Site};
use crate::planet::Planet;
use crate::shell::Options;
use bevy::{
    camera_controller::free_camera::FreeCamera,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use std::ops::Sub;
use terra_build::{Base, BuildOp, Face, Panel, TILE_M, Walker};
use terra_core::Ground;

const WALK_SPEED: f32 = 4.5;
const RUN_SPEED: f32 = 9.0;
const JUMP_SPEED: f32 = 6.0;
const GRAVITY: f32 = 16.0;
const LOOK_SPEED: f32 = 0.0025;
/// How far from the player the builder can act.
const TOOL_REACH_M: f32 = 18.0;
const CAMERA_BACK_M: f32 = 5.0;
const HEAD_M: f32 = 1.65;

#[derive(Component)]
struct Player {
    /// Speed away from the planet's centre, in metres a second.
    rising: f32,
    on_ground: bool,
    indoors: bool,
}

/// Where the player is looking, which is also where the figure faces.
#[derive(Resource)]
struct View {
    /// Along the ground.
    forward: Vec3,
    /// Up or down from level, in radians.
    pitch: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Shape,
    Openings,
    Room,
}

#[derive(Resource)]
struct Builder {
    mode: Mode,
}

#[derive(Component)]
struct Readout;

pub fn plugin(app: &mut App) {
    // A player needs something to build on, and takes over from the build keys.
    build::plugin(app);
    app.insert_resource(DrivenByPlayer)
        .insert_resource(Builder { mode: Mode::Shape })
        .add_systems(PostStartup, spawn)
        .add_systems(
            Update,
            (capture_mouse, look, walk, use_builder, update_readout).chain(),
        )
        .add_systems(PostUpdate, follow.before(TransformSystems::Propagate));
}

fn spawn(
    mut commands: Commands,
    site: Res<Site>,
    planet: Res<Planet>,
    camera: Single<Entity, With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // The player steers the camera now.
    commands.entity(*camera).remove::<FreeCamera>();

    // Stand at the foot of the ramp to the base's door, facing up it.
    let (approach, facing) = site.approach();
    let outside = site.frame.transform_point(approach);
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
        pitch: -0.12,
    });

    let mut flat = |colour: Color| {
        materials.add(StandardMaterial {
            base_color: colour,
            perceptual_roughness: 0.9,
            ..default()
        })
    };
    let suit = flat(Color::srgb(0.92, 0.5, 0.2));
    let pale = flat(Color::srgb(0.93, 0.92, 0.88));
    let dark = flat(Color::srgb(0.2, 0.22, 0.27));
    let glow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.9, 1.0),
        emissive: LinearRgba::rgb(0.4, 2.5, 3.0),
        ..default()
    });
    let mut part = |mesh: Mesh, material: &Handle<StandardMaterial>, at: Vec3| {
        (
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(at),
        )
    };
    // A placeholder figure: body, head, visor, pack, and the builder held out in front.
    commands
        .spawn((
            Name::new("Player"),
            Player {
                rising: 0.0,
                on_ground: false,
                indoors: false,
            },
            Transform::from_translation(feet).looking_to(toward_door, up),
            Visibility::default(),
        ))
        .with_children(|figure| {
            figure.spawn(part(
                Capsule3d::new(0.27, 0.75).into(),
                &suit,
                Vec3::new(0.0, 0.85, 0.0),
            ));
            figure.spawn(part(
                Sphere::new(0.21).mesh().ico(2).unwrap(),
                &pale,
                Vec3::new(0.0, 1.62, 0.0),
            ));
            figure.spawn(part(
                Cuboid::new(0.3, 0.14, 0.12).into(),
                &dark,
                Vec3::new(0.0, 1.64, -0.17),
            ));
            figure.spawn(part(
                Cuboid::new(0.4, 0.55, 0.2).into(),
                &pale,
                Vec3::new(0.0, 1.1, 0.27),
            ));
            figure.spawn(part(
                Cuboid::new(0.09, 0.12, 0.5).into(),
                &dark,
                Vec3::new(0.34, 1.15, -0.38),
            ));
            figure.spawn(part(
                Cuboid::new(0.07, 0.07, 0.08).into(),
                &glow,
                Vec3::new(0.34, 1.15, -0.66),
            ));
        });

    commands.spawn((
        Readout,
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            ..default()
        },
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
    player: Single<(&mut Transform, &mut Player)>,
) {
    let (mut transform, mut player) = player.into_inner();
    let walker = Walker::default();
    let dt = time.delta_secs().min(0.05);
    let feet = transform.translation;
    let up = feet.normalize();
    let right = view.forward.cross(up);

    let pressed = |key: KeyCode| f32::from(keys.pressed(key));
    let wish = view.forward * (pressed(KeyCode::KeyW) - pressed(KeyCode::KeyS))
        + right * (pressed(KeyCode::KeyD) - pressed(KeyCode::KeyA));
    let speed = if keys.pressed(KeyCode::ShiftLeft) {
        RUN_SPEED
    } else {
        WALK_SPEED
    };
    let mut target = feet + wish.normalize_or_zero() * speed * dt;

    // The base decides what blocks the way; it works in its own space.
    let to_base = site.frame.compute_affine().inverse();
    let from_base = site.frame.compute_affine();
    let walked = site.base.walk(
        to_base.transform_point3(feet),
        to_base.transform_point3(target),
        &walker,
    );
    target = from_base.transform_point3(walked);

    player.rising -= GRAVITY * dt;
    if player.on_ground && keys.just_pressed(KeyCode::Space) {
        player.rising = JUMP_SPEED;
    }
    target += up * player.rising * dt;

    // What holds the player up: indoors, the base's floor; outdoors, whichever is higher of
    // the ground and anything the base offers there, such as a ramp or a roof.
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

    let up = target.normalize();
    *transform = Transform::from_translation(target).looking_to(view.forward.reject_from(up), up);
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
    mut camera: Single<&mut Transform, With<Camera3d>>,
) {
    let (head, looking) = eye(&player, &view);
    let up = head.normalize();
    let right = view.forward.cross(up);
    let pivot = head + right * 0.55 + up * 0.25;

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
    let ground = planet.surface(at.normalize()).length() + 0.3;
    if at.length() < ground {
        at = at.normalize() * ground;
    }
    **camera = Transform::from_translation(at).looking_to(looking, up);
}

fn use_builder(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions>,
    planet: Res<Planet>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    player: Single<&Transform, With<Player>>,
    mut builder: ResMut<Builder>,
    mut site: ResMut<Site>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut gizmos: Gizmos,
) {
    for (key, mode) in [
        (KeyCode::Digit1, Mode::Shape),
        (KeyCode::Digit2, Mode::Openings),
        (KeyCode::Digit3, Mode::Room),
    ] {
        if keys.just_pressed(key) {
            builder.mode = mode;
        }
    }
    // A click that only captured the mouse should not also build something.
    let captured = cursor.grab_mode != CursorGrabMode::None;
    let main =
        keys.just_pressed(KeyCode::KeyE) || (captured && mouse.just_pressed(MouseButton::Left));
    let second =
        keys.just_pressed(KeyCode::KeyR) || (captured && mouse.just_pressed(MouseButton::Right));

    // Aim along the middle of the view, and act only on what the player could reach.
    let site = &mut *site;
    let to_base = site.frame.compute_affine().inverse();
    let origin = camera.translation();
    let direction = *camera.forward();
    let within_reach = |point: Vec3| point.distance(player.translation) <= TOOL_REACH_M;
    let aimed: Option<Face> = site
        .base
        .raycast(
            to_base.transform_point3(origin),
            to_base.transform_vector3(direction).normalize(),
            200.0,
        )
        .filter(|hit| within_reach(origin + direction * hit.distance))
        .map(|hit| hit.face);

    let outline = |gizmos: &mut Gizmos, face: Face, colour: Color| {
        let corners = Base::corners(face).map(|corner| site.frame.transform_point(corner));
        gizmos.linestrip(
            [corners[0], corners[1], corners[2], corners[3], corners[0]],
            colour,
        );
    };
    let op = match (builder.mode, aimed) {
        (Mode::Shape, Some(face)) => {
            for part in site.base.wall(face) {
                outline(&mut gizmos, part, Color::srgb(0.3, 0.9, 1.0));
            }
            if main {
                Some(BuildOp::Grow(face))
            } else if second {
                Some(BuildOp::Shrink(face))
            } else {
                None
            }
        }
        (Mode::Openings, Some(face)) => {
            outline(&mut gizmos, face, Color::srgb(1.0, 0.85, 0.2));
            if main {
                Some(BuildOp::SetPanel(face, site.base.panel(face).next()))
            } else if second {
                Some(BuildOp::SetPanel(face, Panel::Solid))
            } else {
                None
            }
        }
        (Mode::Room, None) if main => planet
            .ground
            .raycast(origin, direction, 200.0)
            .filter(|ground| within_reach(ground.point))
            .map(|ground| {
                // Three tiles square, centred on the ground aimed at.
                let local = to_base.transform_point3(ground.point);
                let (x, z) = (
                    (local.x / TILE_M).floor() as i32,
                    (local.z / TILE_M).floor() as i32,
                );
                BuildOp::AddRoom {
                    min: [x - 1, 0, z - 1],
                    max: [x + 1, 0, z + 1],
                }
            }),
        _ => None,
    };
    if let Some(op) = op {
        site.apply(op, &mut commands, &planet, &mut meshes);
    }
}

fn update_readout(
    builder: Res<Builder>,
    player: Single<&Player>,
    options: Res<Options>,
    mut readout: Single<&mut Text, With<Readout>>,
) {
    let (mode, main, second) = match builder.mode {
        Mode::Shape => ("Shape", "push the wall out", "pull it in"),
        Mode::Openings => ("Openings", "change window or door", "make solid"),
        Mode::Room => ("Room", "add a room on the ground", "nothing"),
    };
    let hint = if options.scripted() {
        ""
    } else {
        "Click to capture the mouse. "
    };
    readout.0 = format!(
        "Builder: {mode}   (1 shape, 2 openings, 3 room)\n\
         Left click or E: {main}.   Right click or R: {second}.\n\
         {hint}W A S D walk, Shift run, Space jump.   You are {}.",
        if player.indoors {
            "indoors"
        } else {
            "outdoors"
        }
    );
}
