//! The player's view: looking about with the mouse and keys, the mouse taken for the
//! game and given back, and the camera behind and a little to the side of the figure,
//! zoomed with the wheel.

use super::controls::{self, Action, Controls};
use super::walker::Hands;
use super::{IsLocalPlayer, View, options};
use crate::build::{self, Site};
use crate::planet::{MainCamera, Planet};
use crate::shell::Options;
use crate::start::Start;
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

const LOOK_SPEED: f32 = 0.0025;
/// How fast the arrow keys turn the view, in radians a second.
const KEY_LOOK_SPEED: f32 = 1.6;
/// How far behind the player the camera sits at the start, and how near and far the
/// wheel takes it.
const CAMERA_BACK_M: f32 = 4.6;
const NEAREST_M: f32 = 1.4;
const FURTHEST_M: f32 = 9.0;
/// How far one notch of the wheel moves the camera, and holding a zoom key a second.
const ZOOM_NOTCH_M: f32 = 0.6;
const ZOOM_KEY_M_S: f32 = 5.0;
const HEAD_M: f32 = 1.62;

pub fn plugin(app: &mut App) {
    app.init_resource::<Zoom>().add_systems(
        PostUpdate,
        follow
            .before(TransformSystems::Propagate)
            .before(crate::remote::CameraHold),
    );
}

/// Takes the mouse for the game, or gives it back.
///
/// Windows has no way to lock the pointer in place, only to fence it into the window; see
/// `capture_mouse` for what that costs.
pub(super) fn grab_mouse(cursor: &mut CursorOptions, take: bool) {
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
pub(super) fn capture_mouse(
    mouse: Res<ButtonInput<MouseButton>>,
    menu: Res<options::OptionsMenu>,
    hands: Res<Hands>,
    window: Single<(&mut Window, &mut CursorOptions)>,
) {
    // While a menu or the inventory is open, the pointer is for pointing at it.
    if menu.open || hands.pointer {
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

pub(super) fn look(
    time: Res<Time>,
    motion: Res<AccumulatedMouseMotion>,
    settings: Res<controls::Settings>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions>,
    menu: Res<options::OptionsMenu>,
    hands: Res<Hands>,
    player: Single<(&Transform, &mut View), IsLocalPlayer>,
) {
    let (player, mut view) = player.into_inner();
    let up = player.translation.normalize();
    // The arrow keys look about too, for keyboards and for scripted runs, except while
    // the build menu uses them.
    if !menu.open && !hands.arrows {
        let held = |action| f32::from(settings.held(action, &keys, &mouse, true));
        let turn = KEY_LOOK_SPEED * time.delta_secs();
        let across = held(Action::LookRight) - held(Action::LookLeft);
        let rise = held(Action::LookUp) - held(Action::LookDown);
        view.forward = Quat::from_axis_angle(up, -across * turn) * view.forward;
        view.pitch = (view.pitch + rise * turn).clamp(-1.2, 1.2);
    }
    // While the dial is open the mouse steers it, not the view.
    if cursor.grab_mode != CursorGrabMode::None && !hands.mouse {
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

/// The eye, and the direction it looks, for the player's view.
fn eye(player: &Transform, view: &View) -> (Vec3, Vec3) {
    let up = player.translation.normalize();
    let right = view.forward.cross(up);
    let looking = Quat::from_axis_angle(right, view.pitch) * view.forward;
    (player.translation + up * HEAD_M, looking)
}

/// Third person: behind and a little to the side of the figure, pulled in when a wall of
/// the base would otherwise come between the camera and the player.
/// How far behind the player the camera is, and where the wheel is taking it.
#[derive(Resource)]
pub(super) struct Zoom {
    now: f32,
    wanted: f32,
}

impl Default for Zoom {
    fn default() -> Self {
        Self {
            now: CAMERA_BACK_M,
            wanted: CAMERA_BACK_M,
        }
    }
}

/// Where the camera is wanted after so much asked of the wheel and keys.
fn zoomed(wanted: f32, closer_m: f32) -> f32 {
    (wanted - closer_m).clamp(NEAREST_M, FURTHEST_M)
}

/// The mouse wheel, or the zoom keys, bring the camera nearer or take it further, unless
/// the wheel is turning a part in the builder. It eases to where it is wanted.
pub(super) fn zoom(
    time: Res<Time>,
    controls: Controls,
    scroll: Res<AccumulatedMouseScroll>,
    menu: Res<super::options::OptionsMenu>,
    hands: Res<Hands>,
    mut zoom: ResMut<Zoom>,
) {
    let dt = time.delta_secs();
    // With a panel open the wheel scrolls it instead.
    let wheel = if hands.wheel || menu.open {
        0.0
    } else {
        match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y * ZOOM_NOTCH_M,
            MouseScrollUnit::Pixel => scroll.delta.y * 0.01,
        }
    };
    let keys = (f32::from(controls.held(Action::ZoomIn))
        - f32::from(controls.held(Action::ZoomOut)))
        * ZOOM_KEY_M_S
        * dt;
    zoom.wanted = zoomed(zoom.wanted, wheel + keys);
    let ease = 1.0 - libm::expf(-dt * 12.0);
    zoom.now += (zoom.wanted - zoom.now) * ease;
}

pub(super) fn follow(
    player: Single<(&Transform, &View), (IsLocalPlayer, Without<MainCamera>)>,
    zoom: Res<Zoom>,
    start: Res<Start>,
    site: Option<Res<Site>>,
    builds: Query<&build::Built>,
    planet: Res<Planet>,
    options: Res<Options>,
    mut camera: Single<&mut Transform, With<MainCamera>>,
) {
    // A fixed camera, for taking pictures.
    if let Some((from, toward)) = options.vantage {
        let from = start.transform_point(from);
        **camera = Transform::from_translation(from)
            .looking_at(start.transform_point(toward), from.normalize());
        return;
    }
    let (player, view) = *player;
    let (head, looking) = eye(player, view);
    let up = head.normalize();
    let right = view.forward.cross(up);
    // Over the shoulder, unless the ground is in the way there, as in a narrow pit.
    let shoulder = right * 0.6 + up * 0.25;
    let pivot = head
        + shoulder
            * planet
                .raycast(head, shoulder.normalize(), shoulder.length() + 0.2)
                .map_or(1.0, |hit| {
                    ((hit.distance - 0.2) / shoulder.length()).max(0.0)
                });

    // The camera comes in front of any build's wall behind the player: theirs, or a
    // house they are in.
    let back = -looking;
    let blocked = site
        .as_deref()
        .map(|site| &site.build)
        .into_iter()
        .chain(builds.iter().map(|built| &built.0))
        .filter_map(|build| {
            let to_base = build.frame.compute_affine().inverse();
            build
                .base
                .raycast(
                    to_base.transform_point3(pivot),
                    to_base.transform_vector3(back).normalize(),
                    zoom.now,
                )
                .map(|hit| hit.distance)
        })
        // The ground behind comes in front of the camera too, such as a pit's wall.
        .chain(
            planet
                .raycast(pivot, back, zoom.now)
                .map(|hit| hit.distance),
        )
        .map(|distance| (distance - 0.25).max(0.3))
        .fold(zoom.now, f32::min);
    let mut at = pivot + back * blocked;
    // Keep the camera out of the ground.
    let ground = planet.ground_at(at).length() + 0.35;
    if at.length() < ground {
        at = at.normalize() * ground;
    }
    **camera = Transform::from_translation(at).looking_to(looking, up);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::{LocalPlayer, Player};
    use bevy::ecs::system::RunSystemOnce;
    use terra_core::{PlanetSpec, Seed};

    /// Where the camera follows a player standing on flat ground, looking up or down by
    /// `pitch`, and where the player's head is, and the ground.
    fn follow_one(pitch: f32) -> (Vec3, Vec3, Planet) {
        let planet = Planet::smooth(PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        });
        let feet = planet.ground_at(Vec3::Y * 2100.0);
        let mut world = World::new();
        world.insert_resource(Options::from_args(std::iter::empty()).unwrap());
        world.insert_resource(Start(Transform::from_translation(feet)));
        world.insert_resource(planet.clone());
        world.init_resource::<Zoom>();
        world.spawn((
            Player,
            LocalPlayer,
            Transform::from_translation(feet),
            View {
                forward: Vec3::X,
                pitch,
            },
        ));
        let camera = world.spawn((MainCamera, Transform::default())).id();
        world.run_system_once(follow).unwrap();
        let camera = *world.get::<Transform>(camera).unwrap();
        let head = feet + feet.normalize() * HEAD_M;
        // The player is in the middle of the view: in front, and near the line it looks
        // along.
        let to_head = head - camera.translation;
        let along = to_head.dot(*camera.forward());
        assert!(along > 0.0, "the player is behind the camera");
        let off = (to_head - *camera.forward() * along).length();
        assert!(
            off < 1.0,
            "the player is {off} m off the middle of the view"
        );
        (camera.translation, head, planet)
    }

    #[test]
    fn the_camera_sits_back_as_far_as_it_is_zoomed() {
        let (camera, head, _) = follow_one(0.0);
        assert!(camera.distance(head) >= CAMERA_BACK_M);
    }

    #[test]
    fn looking_up_brings_the_camera_in_over_the_ground_behind() {
        // Behind and below the player, the ground is nearer than the camera would sit.
        let (camera, head, planet) = follow_one(1.0);
        assert!(camera.distance(head) < CAMERA_BACK_M);
        assert!(
            camera.length() > planet.ground_at(camera).length(),
            "the camera is in the ground"
        );
    }
}
