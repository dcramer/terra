//! A player at an outpost by the water, with a builder tool.
//!
//! You are a body in the world: an animated figure you steer in third person, which
//! walks on the planet, is held up by a base's floors, stopped by its walls and windows,
//! and let through its doors. The figure carries one tool, a builder: pick a part from
//! its dial, and build it where you aim.
//!
//!   Mouse         look (click to capture the mouse)
//!   W A S D       move          Shift   sprint       Space   jump
//!   Hold right    the dial of parts to build; the rest of the builder is in `tool.rs`
//!   Esc           options, where every control can be set to another key
//!   F3            more frame timings
//!
//! The scene around the player is in the modules here: `scene` is the light and air,
//! `flora` what grows, `props` what has been placed, `figure` the player's body, `tool`
//! the builder, `dial` its menu and prompts, and `catalogue` what it can build. See docs/player.md for the design this is a step toward.

mod bag;
mod bodies;
mod build_menu;
mod catalogue;
mod claim;
mod combat;
mod controls;
mod crafting;
mod creatures;
mod death;
mod dial;
mod figure;
mod flora;
mod harvest;
mod held;
mod interact;
mod inventory;
mod map;
mod models;
mod options;
mod people;
mod pooling;
mod props;
mod save;
mod scene;
mod settlement;
mod skills;
mod tool;
mod vitals;
mod work;

use super::build::{self, DrivenByPlayer, Site};
use crate::planet::Planet;
use crate::shell::Options;
use bevy::{
    camera_controller::free_camera::FreeCamera,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use controls::{Action, Controls};
use std::ops::Sub;
use terra_build::Walker;

const JOG_SPEED: f32 = 4.3;
const SPRINT_SPEED: f32 = 7.4;
const JUMP_SPEED: f32 = 6.0;
const GRAVITY: f32 = 16.0;
/// What a jump costs in stamina, out of 100.
const JUMP_STAMINA: f32 = 8.0;
/// A roll: what it costs, how long it lasts and how fast it goes.
const DODGE_STAMINA: f32 = 18.0;
const DODGE_S: f32 = 0.45;
const DODGE_SPEED: f32 = 8.5;
const LOOK_SPEED: f32 = 0.0025;
/// How fast the arrow keys turn the view, in radians a second.
const KEY_LOOK_SPEED: f32 = 1.6;
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
    /// How long the player has lain where they fell, while they are down.
    down: Option<f32>,
    /// Seconds left of a swing, a roll, and a flinch from a blow, for the figure.
    attacking: f32,
    dodging: f32,
    flinch: f32,
    /// Which way a roll goes.
    dodge_direction: Vec3,
    /// Holding a guard up.
    guarding: bool,
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
    dial::plugin(app);
    bag::plugin(app);
    build_menu::plugin(app);
    crafting::plugin(app);
    interact::plugin(app);
    claim::plugin(app);
    vitals::plugin(app);
    death::plugin(app);
    skills::plugin(app);
    creatures::plugin(app);
    bodies::plugin(app);
    combat::plugin(app);
    people::plugin(app);
    pooling::plugin(app);
    settlement::plugin(app);
    map::plugin(app);
    held::plugin(app);
    save::plugin(app);
    harvest::plugin(app);
    options::plugin(app);
    app.insert_resource(DrivenByPlayer)
        .add_systems(PostStartup, spawn)
        .add_systems(
            Update,
            (
                options::toggle,
                capture_mouse,
                dial::steer,
                warp,
                look,
                walk,
            )
                .chain()
                .before(tool::Acting),
        )
        .add_systems(PostUpdate, follow.before(TransformSystems::Propagate));
}

fn spawn(
    mut commands: Commands,
    mut items: ResMut<inventory::Items>,
    options: Res<Options>,
    site: Res<Site>,
    planet: Res<Planet>,
    camera: Single<Entity, With<Camera3d>>,
) {
    // The player steers the camera now.
    commands.entity(*camera).remove::<FreeCamera>();

    // Stand a little way out from the door, facing it.
    let (approach, facing) = site.approach();
    // For testing, a run can start somewhere else (--at).
    let outside = match options.start_at {
        Some((east, south)) => site.frame.transform_point(Vec3::new(east, 0.0, south)),
        None => site.frame.transform_point(approach - facing * 5.0),
    };
    let feet = planet.drawn(outside.normalize());
    let up = feet.normalize();
    // For testing, a run can start facing a given spot (--face).
    let looking_at = match options.start_facing {
        Some((east, south)) => Vec3::new(east, 0.0, south),
        None => approach + facing,
    };
    let toward_door = site
        .frame
        .transform_point(looking_at)
        .sub(outside)
        .reject_from(up)
        .normalize();
    commands.insert_resource(View {
        forward: toward_door,
        pitch: -0.08,
    });
    commands.insert_resource(death::Arrival {
        at: feet,
        facing: toward_door,
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
            down: None,
            attacking: 0.0,
            dodging: 0.0,
            flinch: 0.0,
            dodge_direction: Vec3::ZERO,
            guarding: false,
        },
        {
            // For testing, --give can also set how the body starts.
            let mut body = vitals::Vitals::default();
            for (name, value) in &options.gives {
                match name.as_str() {
                    "health" => body.health = *value as f32,
                    "fed" => body.fed = *value as f32,
                    "stamina" => body.stamina = *value as f32,
                    _ => {}
                }
            }
            body
        },
        skills::Skills::default(),
        {
            let mut kit = inventory::starting_kit(&mut items);
            inventory::give_for_testing(
                &mut items,
                &mut kit.0.0,
                &mut kit.1.0,
                &mut kit.2.0,
                &options.gives,
            );
            kit
        },
        Transform::from_translation(feet).looking_to(toward_door, up),
        Visibility::default(),
    ));
}

/// For testing, moves the player where a scripted run says, on the frame it says (--warp).
fn warp(
    options: Res<Options>,
    frame: Res<crate::shell::ScriptFrame>,
    site: Res<Site>,
    planet: Res<Planet>,
    mut view: ResMut<View>,
    player: Single<(&mut Transform, &mut Player)>,
) {
    let Some(warp) = options.warps.iter().find(|warp| warp.frame == frame.0) else {
        return;
    };
    let (mut transform, mut player) = player.into_inner();
    let place = |(east, south): (f32, f32)| site.frame.transform_point(Vec3::new(east, 0.0, south));
    let feet = planet.drawn(place(warp.at).normalize());
    let up = feet.normalize();
    let facing = (place(warp.facing) - feet)
        .reject_from(up)
        .normalize_or(view.forward);
    transform.translation = feet;
    player.velocity = Vec3::ZERO;
    player.facing = facing;
    view.forward = facing;
    info!(
        "Warped to {:.1} m east and {:.1} m south.",
        warp.at.0, warp.at.1
    );
}

/// Takes the mouse for the game, or gives it back.
///
/// Windows has no way to lock the pointer in place, only to fence it into the window; see
/// `capture_mouse` for what that costs.
fn grab_mouse(cursor: &mut CursorOptions, take: bool) {
    cursor.grab_mode = match take {
        false => CursorGrabMode::None,
        true if cfg!(target_os = "windows") => CursorGrabMode::Confined,
        true => CursorGrabMode::Locked,
    };
    cursor.visible = !take;
}

/// Takes the mouse on a click and gives it back when the window is no longer the one in
/// front. Esc gives it back too, by opening the options menu (`options::toggle`).
///
/// Windows drops its fence round the pointer whenever another window takes over. So while
/// the game has the mouse it also puts the pointer back in the middle of the window every
/// frame, and it lets go properly the moment the window loses focus instead of believing
/// a fence that is gone.
///
/// macOS does lock the pointer, and moving the pointer there undoes the lock, so on a Mac
/// it is left where it is.
fn capture_mouse(
    mouse: Res<ButtonInput<MouseButton>>,
    screen: Res<inventory::InventoryScreen>,
    menu: Res<options::OptionsMenu>,
    build_menu: Res<build_menu::BuildMenu>,
    window: Single<(&mut Window, &mut CursorOptions)>,
) {
    // While a menu or the inventory is open, the pointer is for pointing at it.
    if screen.open || menu.open || build_menu.open {
        return;
    }
    let (mut window, mut cursor) = window.into_inner();
    let captured = cursor.grab_mode != CursorGrabMode::None;
    if captured && !window.focused {
        grab_mouse(&mut cursor, false);
    } else if !captured && window.focused && mouse.just_pressed(MouseButton::Left) {
        grab_mouse(&mut cursor, true);
    } else if captured && !cfg!(target_os = "macos") {
        let middle = window.size() * 0.5;
        window.set_cursor_position(Some(middle));
    }
}

fn look(
    time: Res<Time>,
    motion: Res<AccumulatedMouseMotion>,
    settings: Res<controls::Settings>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions>,
    dial: Res<dial::Dial>,
    menu: Res<options::OptionsMenu>,
    build_menu: Res<build_menu::BuildMenu>,
    player: Single<&Transform, With<Player>>,
    mut view: ResMut<View>,
) {
    let up = player.translation.normalize();
    // The arrow keys look about too, for keyboards and for scripted runs, except while
    // the build menu uses them.
    if !menu.open && !build_menu.open {
        let held = |action| f32::from(settings.held(action, &keys, &mouse, true));
        let turn = KEY_LOOK_SPEED * time.delta_secs();
        let across = held(Action::LookRight) - held(Action::LookLeft);
        let rise = held(Action::LookUp) - held(Action::LookDown);
        view.forward = Quat::from_axis_angle(up, -across * turn) * view.forward;
        view.pitch = (view.pitch + rise * turn).clamp(-1.2, 1.2);
    }
    // While the dial is open the mouse steers it, not the view.
    if cursor.grab_mode != CursorGrabMode::None && !dial.open {
        let speed = LOOK_SPEED * settings.look_speed;
        let rise = if settings.invert_look { -1.0 } else { 1.0 };
        view.forward = Quat::from_axis_angle(up, -motion.delta.x * speed) * view.forward;
        view.pitch = (view.pitch - rise * motion.delta.y * speed).clamp(-1.2, 1.2);
    }
    // "Level" changes as the player walks round the planet.
    view.forward = view
        .forward
        .reject_from(up)
        .normalize_or(up.any_orthonormal_vector());
}

fn walk(
    time: Res<Time>,
    controls: Controls,
    planet: Res<Planet>,
    site: Res<Site>,
    view: Res<View>,
    obstacles: Res<flora::Obstacles>,
    items: Query<(&props::Prop, &Transform, &Visibility), Without<Player>>,
    others: Query<(&bodies::Body, &Transform), Without<Player>>,
    player: Single<(&mut Transform, &mut Player, &mut vitals::Vitals)>,
) {
    let (mut transform, mut player, mut vitals) = player.into_inner();
    // Lying where you fell, you do not move.
    if player.down.is_some() {
        player.velocity = Vec3::ZERO;
        return;
    }
    let walker = Walker::default();
    let dt = time.delta_secs().min(0.05);
    let feet = transform.translation;
    let up = feet.normalize();
    let right = view.forward.cross(up);

    let held = |action| f32::from(controls.held(action));
    let wish = (view.forward * (held(Action::Forward) - held(Action::Back))
        + right * (held(Action::Right) - held(Action::Left)))
    .normalize_or_zero();
    // A roll carries you a short way fast, clear of blows; a guard slows you.
    player.attacking = (player.attacking - dt).max(0.0);
    player.flinch = (player.flinch - dt).max(0.0);
    player.dodging = (player.dodging - dt).max(0.0);
    if player.dodging == 0.0
        && player.on_ground
        && controls.went_down(Action::Dodge)
        && vitals.spend(DODGE_STAMINA)
    {
        player.dodging = DODGE_S;
        player.dodge_direction = if wish == Vec3::ZERO {
            -view.forward
        } else {
            wish
        };
    }
    let top_speed = if player.dodging > 0.0 {
        DODGE_SPEED
    } else if player.guarding {
        JOG_SPEED * 0.5
    } else if controls.held(Action::Sprint) && vitals.can_sprint() {
        SPRINT_SPEED
    } else {
        JOG_SPEED
    };
    // Ease into and out of a run rather than starting and stopping dead.
    let ease = 1.0 - libm::expf(-dt * if player.on_ground { 11.0 } else { 2.5 });
    let wish = if player.dodging > 0.0 {
        player.dodge_direction
    } else {
        wish
    };
    player.velocity = player.velocity.lerp(wish * top_speed, ease).reject_from(up);
    let mut target = feet + player.velocity * dt;
    // Furniture and kit are solid. Walls come after, so they have the last word.
    target = props::push_out(target, walker.radius, items.iter());
    // Creatures and people take up room.
    target = bodies::push_out(target, walker.radius, others.iter());

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
    let ground_ahead = planet.drawn(target.normalize()).length();
    if ground_ahead < planet.sea_radius_m() - WADE_DEPTH_M
        && ground_ahead < planet.drawn(up).length()
    {
        target = feet;
    }

    player.rising -= GRAVITY * dt;
    if player.on_ground && controls.went_down(Action::Jump) && vitals.spend(JUMP_STAMINA) {
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
    // The ground as it is drawn, so the player stands on what they see. Near the start
    // that is the true ground; further off the planet's mesh is coarser, and walking on
    // the true ground there sinks the player into the drawn one or lifts them off it.
    let ground = planet.drawn(target.normalize()).length();
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
    // Turning right round has no shorter way to go, so it is given one.
    let about_face = toward != Vec3::ZERO && player.facing.dot(toward) < -0.98;
    let from = if about_face {
        player.facing + player.facing.cross(up) * 0.3
    } else {
        player.facing
    };
    player.facing = from
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
    let ground = planet.drawn(at.normalize()).length() + 0.35;
    if at.length() < ground {
        at = at.normalize() * ground;
    }
    **camera = Transform::from_translation(at).looking_to(looking, up);
}
