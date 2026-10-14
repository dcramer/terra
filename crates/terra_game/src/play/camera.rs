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
use terra_world::parting::CameraSubject;

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
/// The figure's middle, which the camera keeps clear of foliage.
const CHEST_M: f32 = 1.2;
/// How round the camera's boom is swept against trunks, rocks and logs: the lens comes no
/// nearer them than this (the probe of Unreal's spring arm).
/// The near plane's corners are 0.21 m from the lens (0.2 m off at 55 degrees).
const PROBE_M: f32 = 0.3;
/// A wider sweep, which the boom eases in to ahead of the hard one, so it slides in front of
/// a trunk swinging into the way rather than jumping.
const SOFT_PROBE_M: f32 = 0.6;
/// The boom shortens over the ground behind no shorter than this.
const GROUND_BOOM_M: f32 = 3.0;
/// How fast the boom comes in before what is in the way, and goes back out once it is clear,
/// as rates a second: in quickly, out slowly, as shipped third-person cameras do.
const BOOM_IN: f32 = 14.0;
const BOOM_OUT: f32 = 2.5;
/// How high over the ground the lens is kept.
const OVER_GROUND_M: f32 = 0.35;
/// How far the lens keeps from the line up the middle of the figure, feet to crown.
const BODY_CLEAR_M: f32 = 0.55;
/// The boom swings to the left shoulder when the right one's is pulled in shorter than this
/// and the left's would be this much longer, and back once the right's reaches the other.
const SWAP_M: f32 = 1.6;
const SWAP_GAIN_M: f32 = 1.0;
const SWAP_BACK_M: f32 = 2.6;
/// How fast it swings across, in shoulders a second (2 is right to left).
const SWING: f32 = 2.5;

/// The camera's boom over one shoulder: where it hangs from, which way it goes and how long
/// it would be, and where its narrow and wide sweeps stop.
struct Arm {
    pivot: Vec3,
    along: Vec3,
    length: f32,
    hard: f32,
    soft: f32,
}

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

/// How far behind the player the camera is, where the wheel is taking it, how far in from
/// that the boom is pulled by what is in the way, and over which shoulder it hangs (1 the
/// right, -1 the left) and is swinging to.
#[derive(Resource)]
pub(super) struct Zoom {
    now: f32,
    wanted: f32,
    pulled: f32,
    side: f32,
    shoulder: f32,
}

impl Default for Zoom {
    fn default() -> Self {
        Self {
            now: CAMERA_BACK_M,
            wanted: CAMERA_BACK_M,
            pulled: 0.0,
            side: 1.0,
            shoulder: 1.0,
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

/// Third person: behind and a little to the side of the figure, on a boom, as Unreal's
/// spring arm and most third-person games carry their camera (D274). The boom is swept as a
/// sphere against what is solid, build walls, trunks, rocks, stumps and logs (`Obstacles`,
/// D267), and pulls in before them: quickly, ahead of them, on a wider sweep, and never
/// through them; then out again slowly once they are clear. Hemmed in behind one shoulder, it
/// swings to the other. Shrubs, young trees and leaves are not solid; they fade instead
/// (`terra_world::parting`). Where the boom would go into the ground behind, as looking up,
/// the lens slides up over the ground rather than in toward the figure, and it keeps out of
/// the figure itself.
pub(super) fn follow(
    time: Res<Time>,
    player: Single<(&Transform, &View), (IsLocalPlayer, Without<MainCamera>)>,
    mut zoom: ResMut<Zoom>,
    start: Res<Start>,
    site: Option<Res<Site>>,
    builds: Query<&build::Built>,
    planet: Res<Planet>,
    options: Res<Options>,
    obstacles: Option<Res<super::Obstacles>>,
    camera: Single<(&mut Transform, &mut CameraSubject), With<MainCamera>>,
) {
    let (mut camera, mut subject) = camera.into_inner();
    // A fixed camera, for taking pictures.
    if let Some((from, toward)) = options.vantage {
        subject.0 = None;
        // Heights above the start, or with --vantage-ground above the ground there.
        let place = |at: Vec3| {
            if options.vantage_ground {
                let ground = planet.ground_at(start.transform_point(Vec3::new(at.x, 0.0, at.z)));
                ground + ground.normalize() * at.y
            } else {
                start.transform_point(at)
            }
        };
        let toward = place(toward);
        let mut from = place(from);
        // Standing where a walker could, out of any trunk or rock.
        if options.vantage_ground
            && let Some(obstacles) = &obstacles
        {
            // As high as a walker's feet would be under it, so anything taller than a step
            // counts, whatever the eye's height.
            let feet = planet.ground_at(from);
            let moved = obstacles.push_out(feet, 0.3, 0.4) - feet;
            from += moved;
        }
        *camera = Transform::from_translation(from).looking_at(toward, from.normalize());
        return;
    }
    let (player, view) = *player;
    let (head, looking) = eye(player, view);
    let up = head.normalize();
    let right = view.forward.cross(up);
    // How far a sphere of `radius` goes from `from` along `along` within `length` before
    // something solid stops it: the ground, a trunk or rock, or a build's wall (theirs, or
    // a house they are in). Walls and the ground are met by a ray, kept a little short.
    let swept = |from: Vec3, along: Vec3, length: f32, radius: f32| {
        let walls = site
            .as_deref()
            .map(|site| &site.build)
            .into_iter()
            .chain(builds.iter().map(|built| &built.0))
            .filter_map(|build| {
                let to_base = build.frame.compute_affine().inverse();
                build
                    .base
                    .raycast(
                        to_base.transform_point3(from),
                        to_base.transform_vector3(along).normalize(),
                        length,
                    )
                    .map(|hit| hit.distance - radius)
            })
            .chain(
                planet
                    .raycast(from, along, length)
                    .map(|hit| hit.distance - radius),
            );
        obstacles
            .as_deref()
            .and_then(|obstacles| obstacles.sweep(from, along, length, radius))
            .into_iter()
            .chain(walls)
            .fold(length, f32::min)
            .max(0.0)
    };
    // A sphere that starts touching something would stop at once: one standing against a
    // trunk sweeps a sphere as round as the room it has.
    let room = |at: Vec3, radius: f32| {
        obstacles.as_deref().map_or(radius, |obstacles| {
            (obstacles.clearance(at, radius + 0.02) - 0.02).clamp(0.05, radius)
        })
    };
    // The boom over one shoulder, `side` 1 for the right and -1 for the left, and between
    // them while it swings across.
    let zoomed_m = zoom.now;
    let arm = |side: f32| {
        // Over the shoulder, unless something is in the way there, as a trunk at the player's
        // side or a narrow pit's wall.
        let shoulder = right * (0.6 * side) + up * 0.25;
        let pivot = head
            + shoulder.normalize()
                * swept(
                    head,
                    shoulder.normalize(),
                    shoulder.length(),
                    room(head, PROBE_M * 0.5),
                );
        // Where the boom would put the lens. Where the ground behind is in the way, as
        // looking up, the boom shortens over it, down to GROUND_BOOM_M, and then the lens
        // slides up over the ground instead, so it stays back from the figure rather than
        // coming in to its shoulder.
        let reach = planet
            .raycast(pivot, -looking, zoomed_m)
            .map_or(zoomed_m, |hit| hit.distance.max(GROUND_BOOM_M));
        let mut wanted = pivot - looking * zoomed_m.min(reach);
        let floor = planet.ground_at(wanted).length() + OVER_GROUND_M;
        if wanted.length() < floor {
            wanted = wanted.normalize() * floor;
        }
        let length = pivot.distance(wanted);
        let along = (wanted - pivot).normalize_or(-looking);
        let hard = swept(pivot, along, length, room(pivot, PROBE_M));
        let soft = swept(pivot, along, length, room(pivot, SOFT_PROBE_M)).min(hard);
        Arm {
            pivot,
            along,
            length,
            hard,
            soft,
        }
    };
    let dt = time.delta_secs();
    // Hemmed in behind the right shoulder, as by a trunk at the player's back, the boom swings
    // over to the left shoulder if that is clear, as shooters swap shoulders, and back once
    // the right is clear again.
    let (right_arm, left_arm) = (arm(1.0), arm(-1.0));
    if right_arm.soft >= SWAP_BACK_M.min(right_arm.length - 0.1) {
        zoom.shoulder = 1.0;
    } else if right_arm.soft < SWAP_M && left_arm.soft > right_arm.soft + SWAP_GAIN_M {
        zoom.shoulder = -1.0;
    }
    zoom.side += (zoom.shoulder - zoom.side).clamp(-SWING * dt, SWING * dt);
    let arm = arm(zoom.side);
    // How far in it is pulled eases to where the wider sweep stops it, quickly in and slowly
    // out, but never lets the lens past where the narrow one does.
    let pull = arm.length - arm.soft;
    let rate = if pull > zoom.pulled {
        BOOM_IN
    } else {
        BOOM_OUT
    };
    zoom.pulled += (pull - zoom.pulled) * (1.0 - libm::expf(-dt * rate));
    zoom.pulled = zoom.pulled.clamp(0.0, arm.length);
    let boom = (arm.length - zoom.pulled).min(arm.hard);
    let mut at = arm.pivot + arm.along * boom;
    // Keep the camera out of the ground.
    let ground = planet.ground_at(at).length() + OVER_GROUND_M;
    if at.length() < ground {
        at = at.normalize() * ground;
    }
    at = clear_of_body(at, player.translation, up, |from, to| {
        let gap = to - from;
        swept(from, gap.normalize_or(up), gap.length(), PROBE_M) >= gap.length() - 1e-3
    });
    *camera = Transform::from_translation(at).looking_to(looking, up);
    // Foliage between the camera and the figure fades (D271).
    subject.0 = Some(player.translation + up * CHEST_M);
}

/// The lens moved out of the figure standing at `feet`: away from the line up its middle in
/// the level, or, where that is blocked (`clear`, whether a lens may move from one point to
/// another), up over its head.
fn clear_of_body(at: Vec3, feet: Vec3, up: Vec3, clear: impl Fn(Vec3, Vec3) -> bool) -> Vec3 {
    let crown = HEAD_M + 0.25;
    let rise = (at - feet).dot(up).clamp(0.0, crown);
    let off = at - (feet + up * rise);
    let apart = off.length();
    if apart >= BODY_CLEAR_M {
        return at;
    }
    // Straight above the crown, it is out of it already when far enough up.
    let level = off - up * off.dot(up);
    if level.length() > 1e-3 {
        let out = at + level.normalize() * (BODY_CLEAR_M - apart);
        if clear(at, out) {
            return out;
        }
    }
    feet + up * (crown + BODY_CLEAR_M) + level
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::{LocalPlayer, Obstacles, Player};
    use bevy::ecs::system::RunSystemOnce;
    use terra_core::{PlanetSpec, Seed};

    /// A player standing on flat ground looking along x, up or down by `pitch`, with the
    /// camera following, and a trunk 0.4 m round `trunk_m` behind them if any; and their
    /// feet.
    fn scene(pitch: f32, trunk_m: Option<f32>) -> (World, Vec3) {
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
        world.init_resource::<Time>();
        if let Some(back) = trunk_m {
            let mut obstacles = Obstacles::default();
            obstacles.add(planet.ground_at(feet - Vec3::X * back), 0.4);
            world.insert_resource(obstacles);
        }
        world.spawn((
            Player,
            LocalPlayer,
            Transform::from_translation(feet),
            View {
                forward: Vec3::X,
                pitch,
            },
        ));
        world.spawn((MainCamera, Transform::default()));
        (world, feet)
    }

    /// Runs the camera for a frame of `dt` seconds, and says where it is.
    fn frame(world: &mut World, dt: f32) -> Transform {
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(dt));
        world.run_system_once(follow).unwrap();
        *world
            .query_filtered::<&Transform, With<MainCamera>>()
            .single(world)
            .unwrap()
    }

    /// Where the camera follows a player standing on flat ground, looking up or down by
    /// `pitch`, and where the player's head is, and the ground.
    fn follow_one(pitch: f32) -> (Vec3, Vec3, Planet) {
        let (mut world, feet) = scene(pitch, None);
        let camera = frame(&mut world, 0.0);
        let planet = world.resource::<Planet>().clone();
        let head = feet + feet.normalize() * HEAD_M;
        // The player is in the view: in front, and near the line it looks along.
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

    /// How far a point is from the line up the middle of a figure standing at `feet`.
    fn from_body(at: Vec3, feet: Vec3) -> f32 {
        let up = feet.normalize();
        let rise = (at - feet).dot(up).clamp(0.0, HEAD_M + 0.25);
        at.distance(feet + up * rise)
    }

    #[test]
    fn the_camera_sits_back_as_far_as_it_is_zoomed() {
        let (camera, head, _) = follow_one(0.0);
        assert!(camera.distance(head) >= CAMERA_BACK_M);
    }

    #[test]
    fn looking_up_the_camera_slides_over_the_ground_behind_and_keeps_off_the_figure() {
        // Behind and below the player, the ground is nearer than the camera would sit.
        let (camera, head, planet) = follow_one(1.0);
        assert!(camera.distance(head) < CAMERA_BACK_M);
        assert!(
            camera.length() > planet.ground_at(camera).length() + 0.3,
            "the camera is in the ground"
        );
        let feet = head - head.normalize() * HEAD_M;
        assert!(
            from_body(camera, feet) > 1.0,
            "the camera is {} m from the figure",
            from_body(camera, feet)
        );
        // As far up as the view goes, it is still off the figure.
        let (mut world, feet) = scene(1.2, None);
        let camera = frame(&mut world, 0.0).translation;
        assert!(from_body(camera, feet) > 1.0, "{}", from_body(camera, feet));
    }

    #[test]
    fn a_trunk_behind_pulls_the_camera_in_quickly_and_it_goes_out_slowly() {
        // A trunk 0.4 m round, its middle 3 m behind the player.
        let (mut world, feet) = scene(0.0, Some(3.0));
        let trunk = world.resource::<Planet>().ground_at(feet - Vec3::X * 3.0);
        let bark = |at: Vec3| {
            let up = trunk.normalize();
            let level = (at - trunk) - up * (at - trunk).dot(up);
            level.length() - 0.4
        };
        // Never inside it, from the first frame on, and settled in front of it within a
        // fifth of a second.
        let mut camera = frame(&mut world, 1.0 / 60.0);
        assert!(
            bark(camera.translation) >= PROBE_M - 0.01,
            "{} at {:?}, trunk {:?}",
            bark(camera.translation),
            camera.translation - feet,
            trunk - feet
        );
        for _ in 0..12 {
            camera = frame(&mut world, 1.0 / 60.0);
            assert!(bark(camera.translation) >= PROBE_M - 0.01);
        }
        assert!(
            bark(camera.translation) < SOFT_PROBE_M + 0.1,
            "it stands {} m off the bark",
            bark(camera.translation)
        );
        let head = feet + feet.normalize() * HEAD_M;
        let pulled = camera.translation.distance(head);
        assert!(pulled < 3.0);
        // The trunk felled, it eases back out, not at once.
        world.insert_resource(Obstacles::default());
        let next = frame(&mut world, 1.0 / 60.0).translation.distance(head);
        assert!(next > pulled && next < pulled + 0.2, "{pulled} then {next}");
        for _ in 0..180 {
            frame(&mut world, 1.0 / 60.0);
        }
        let out = frame(&mut world, 1.0 / 60.0).translation.distance(head);
        assert!(out > CAMERA_BACK_M - 0.05, "{out}");
    }

    #[test]
    fn hemmed_in_behind_the_right_shoulder_the_camera_swings_to_the_left() {
        // A trunk 0.4 m round just behind the right shoulder (right is +z, looking along x).
        let (mut world, feet) = scene(0.0, None);
        let planet = world.resource::<Planet>().clone();
        let mut obstacles = Obstacles::default();
        obstacles.add(planet.ground_at(feet + Vec3::new(-1.2, 0.0, 0.6)), 0.4);
        world.insert_resource(obstacles);
        let head = feet + feet.normalize() * HEAD_M;
        let first = frame(&mut world, 1.0 / 60.0).translation;
        assert!(
            first.distance(head) < 1.5,
            "the right shoulder is hemmed in"
        );
        for _ in 0..90 {
            frame(&mut world, 1.0 / 60.0);
        }
        let camera = frame(&mut world, 1.0 / 60.0).translation;
        assert!(
            (camera - feet).z < -0.3,
            "over the left shoulder: {:?}",
            camera - feet
        );
        assert!(camera.distance(head) > 3.0, "{}", camera.distance(head));
    }

    #[test]
    fn a_lens_in_the_figure_is_moved_out_of_it_or_over_its_head() {
        let feet = Vec3::Y * 1000.0;
        let up = Vec3::Y;
        let inside = feet + Vec3::new(0.2, 1.4, 0.0);
        let out = clear_of_body(inside, feet, up, |_, _| true);
        assert!((from_body(out, feet) - BODY_CLEAR_M).abs() < 1e-3);
        assert!((out - inside).dot(up).abs() < 1e-4, "moved in the level");
        // Blocked to the side, it goes over the head instead.
        let over = clear_of_body(inside, feet, up, |_, _| false);
        assert!(from_body(over, feet) >= BODY_CLEAR_M - 1e-3);
        assert!((over - feet).dot(up) > HEAD_M);
        // Clear of it, it stays where it is.
        let clear = feet + Vec3::new(2.0, 1.0, 0.0);
        assert_eq!(clear_of_body(clear, feet, up, |_, _| true), clear);
    }
}
