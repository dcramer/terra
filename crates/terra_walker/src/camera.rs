//! The player's view: looking about with the mouse and keys, the mouse taken for the
//! game and given back, and the camera behind and a little to the side of the figure,
//! zoomed with the wheel; with a weapon drawn, framing the fight instead (`framing`, D180).

pub mod framing;

use crate::controls::{self, Action, Controls};
use crate::stepping::Stepping;
use crate::walker::Hands;
use crate::{IsLocalPlayer, View, options};
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use framing::{FightView, Framing};
use terra_bases::{self as build, Site};
use terra_core::Solid;
use terra_world::parting::CameraSubject;
use terra_world::planet::{MainCamera, Planet};
use terra_world::shell::Options;
use terra_world::start::Start;

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
/// Where the walk's boom hangs from: so far right of the head, and up.
const SHOULDER_M: [f32; 2] = [0.6, 0.25];
/// The figure's middle, which the camera keeps clear of foliage.
const CHEST_M: f32 = 1.2;
/// How round the lens is: it never comes nearer anything solid than this, a trunk included
/// (the probe of Unreal's spring arm). The near plane's corners are 0.29 m from the lens (0.2 m
/// off, 55 degrees high, 16:9).
const PROBE_M: f32 = 0.3;
/// A wider sweep, which the boom eases in to ahead of the hard one before what is broad, so
/// it slides in front of a rock or wall swinging into the way rather than jumping.
const SOFT_PROBE_M: f32 = 0.6;
/// Trees, and anything else narrower than this across, do not pull the boom in when they
/// cross the line to the figure: they hide it for a moment as they pass, and only the lens
/// keeps out of them (as shipped games put trees on a collision channel the boom's sweep
/// ignores). Walls, the ground and anything broader, as a boulder, pull it in.
const THIN_M: f32 = 2.5;
/// How fast the lens comes back to the boom's end once a trunk it slid round is past, as a
/// rate a second.
const LENS_BACK: f32 = 2.0;
/// A lens further than this from the boom's end, as when the player is moved, starts afresh
/// there.
const MOST_ASIDE_M: f32 = 2.0;
/// How long something thin may hide the figure before the boom comes in front of it, how long
/// the figure must then be in sight from where the boom would be before it goes back out, and
/// how fast it comes in, a rate a second: slowly, so it never punches in (Cinemachine's
/// minimum occlusion time and occluded damping).
const HIDDEN_S: f32 = 0.6;
const SEEN_S: f32 = 0.5;
const UNHIDE_IN: f32 = 2.5;
/// The boom comes no shorter than this to come in front of something hiding the figure;
/// nearer the figure than that, it is let hide it.
const UNHIDE_NEAREST_M: f32 = 2.0;
/// The boom shortens over the ground behind no shorter than this.
const GROUND_BOOM_M: f32 = 3.0;
/// How fast the boom comes in before what is broad and in the way, and goes back out once it
/// is clear, as rates a second: in quickly, out slowly, as shipped third-person cameras do.
const BOOM_IN: f32 = 14.0;
const BOOM_OUT: f32 = 2.5;
/// How high over the ground the lens is kept.
const OVER_GROUND_M: f32 = 0.35;
/// How far the lens keeps from the line up the middle of the figure, feet to crown.
const BODY_CLEAR_M: f32 = 0.55;

/// The camera's boom over the right shoulder: where it hangs from, which way it goes and how
/// long it would be, and where its narrow and wide sweeps against what is broad stop.
struct Arm {
    pivot: Vec3,
    along: Vec3,
    length: f32,
    hard: f32,
    soft: f32,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Zoom>()
        .init_resource::<Framing>()
        .add_systems(
            PostUpdate,
            follow
                .before(TransformSystems::Propagate)
                .before(terra_world::remote::CameraHold),
        );
}

/// The player as their figure is shown, its step smoothing's offset under the body
/// (stepping.rs), so a step up is followed as the figure rises, not as the capsule pops.
fn as_shown(player: &Transform, stepping: Option<&Stepping>) -> Transform {
    let below = stepping.map_or(0.0, |stepping| stepping.below);
    player.with_translation(player.translation - player.translation.normalize() * below)
}

/// Takes the mouse for the game, or gives it back.
///
/// Windows has no way to lock the pointer in place, only to fence it into the window; see
/// `capture_mouse` for what that costs.
pub fn grab_mouse(cursor: &mut CursorOptions, take: bool) {
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
pub fn capture_mouse(
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

pub fn look(
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

/// How far behind the player the camera is, where the wheel is taking it, and how far in
/// from that the boom is pulled by what is broad and in the way; where the lens was last frame
/// and how far it had slid aside from the boom's end round something thin; and how long the
/// figure has been hidden from the boom's end, or in sight from it, how far in the boom has
/// come to see past what hides it, and whether it is coming in; and how far it has come to a
/// fight's framing.
#[derive(Resource)]
pub struct Zoom {
    now: f32,
    wanted: f32,
    pulled: f32,
    lens: Option<Vec3>,
    aside: Vec3,
    hidden_s: f32,
    seen_s: f32,
    unhide: f32,
    unhiding: bool,
    framing: framing::Eased,
}

impl Default for Zoom {
    fn default() -> Self {
        Self {
            now: CAMERA_BACK_M,
            wanted: CAMERA_BACK_M,
            pulled: 0.0,
            lens: None,
            aside: Vec3::ZERO,
            hidden_s: 0.0,
            seen_s: 0.0,
            unhide: 0.0,
            unhiding: false,
            framing: framing::Eased::default(),
        }
    }
}

/// Where the camera is wanted after so much asked of the wheel and keys.
fn zoomed(wanted: f32, closer_m: f32) -> f32 {
    (wanted - closer_m).clamp(NEAREST_M, FURTHEST_M)
}

/// The mouse wheel, or the zoom keys, bring the camera nearer or take it further, unless
/// the wheel is turning a part in the builder. It eases to where it is wanted.
pub fn zoom(
    time: Res<Time>,
    controls: Controls,
    scroll: Res<AccumulatedMouseScroll>,
    menu: Res<crate::options::OptionsMenu>,
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

/// Third person: behind and a little to the right of the figure, on a boom, as Unreal's
/// spring arm and most third-person games carry their camera (D274, D285). The boom is swept
/// as a sphere against build walls, the ground and what is broad and solid, as a boulder
/// (`Obstacles`, D267), and pulls in before them: quickly, ahead of them, on a wider sweep,
/// and never through them; then out again slowly once they are clear. Trees and other thin
/// things do not pull it in as they cross the line to the figure, as a forest's trunks do
/// every second or so: they hide it for a moment. The lens itself never goes into any of them:
/// it slides round a trunk in its way, turning to keep the figure where it was in the picture,
/// and comes back to the boom's end as it passes. A trunk that hides the figure for more than
/// a moment has the boom come slowly in front of it. Shrubs, young trees and leaves are not
/// solid; they fade instead (`terra_world::parting`). Where the boom would go into the ground
/// behind, as looking up, the lens slides up over the ground rather than in toward the
/// figure, and it keeps out of the figure itself. With a weapon drawn the boom hangs and
/// looks as `framing` has it, and all of that holds the same.
/// The local player as the camera follows them.
type Followed<'a> = (
    &'a Transform,
    &'a View,
    Option<&'a FightView>,
    Option<&'a Stepping>,
);

pub fn follow(
    time: Res<Time>,
    framing: Res<Framing>,
    player: Single<Followed, (IsLocalPlayer, Without<MainCamera>)>,
    others: Query<&Transform, Without<MainCamera>>,
    mut zoom: ResMut<Zoom>,
    start: Res<Start>,
    site: Option<Res<Site>>,
    builds: Query<&build::Built>,
    planet: Res<Planet>,
    options: Res<Options>,
    obstacles: Option<Res<terra_world::flora::Obstacles>>,
    camera: Single<(&mut Transform, &mut CameraSubject, Option<&Projection>), With<MainCamera>>,
) {
    let (mut camera, mut subject, lens) = camera.into_inner();
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
    let (player, view, fight) = (&as_shown(player.0, player.3), player.1, player.2);
    let (head, looking) = eye(player, view);
    let up = head.normalize();
    let dt = time.delta_secs();
    let target = fight.and_then(|fight| others.get(fight.target?).ok());
    let frame = framing::framed(
        &mut zoom, &framing, dt, view, head, looking, fight, target, lens,
    );
    let looking = frame.looking;
    let obstacles = obstacles.as_deref();
    let broad = |solid: &Solid| !thin(solid);
    // How far a sphere of `radius` goes from `from` along `along` within `length` before
    // something solid and broad stops it: the ground, a boulder, or a build's wall (theirs,
    // or a house they are in). Walls and the ground are met by a ray, kept a little short.
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
            .and_then(|obstacles| obstacles.sweep_where(from, along, length, radius, broad))
            .into_iter()
            .chain(walls)
            .fold(length, f32::min)
            .max(0.0)
    };
    // A sphere that starts touching something would stop at once: one standing against a
    // boulder sweeps a sphere as round as the room it has.
    let room = |at: Vec3, radius: f32| {
        obstacles.map_or(radius, |obstacles| {
            (obstacles.clearance_where(at, radius + 0.02, broad) - 0.02).clamp(0.05, radius)
        })
    };
    // The boom hangs over the right shoulder, or for a fight over the head, unless something
    // is in the way there, as a narrow pit's wall.
    let (shoulder, zoomed_m) = (frame.offset.normalize_or(up), frame.boom_m);
    let room_m = room(head, PROBE_M * 0.5);
    let pivot = head + shoulder * swept(head, shoulder, frame.offset.length(), room_m);
    let arm = {
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
    // Something thin that has hidden the figure from the boom's end for more than a moment,
    // as a trunk right behind a player standing still, has the boom come slowly in front of
    // it, and go out again once the figure would be in sight from the end (the hysteresis).
    let chest = player.translation + up * CHEST_M;
    let hidden_from = |at: Vec3| {
        [chest, head].iter().all(|&to| {
            let gap = to - at;
            obstacles.is_some_and(|obstacles| {
                obstacles
                    .sweep(at, gap.normalize_or(up), gap.length(), 0.05)
                    .is_some()
            })
        })
    };
    if hidden_from(arm.pivot + arm.along * boom) {
        zoom.hidden_s += dt;
        zoom.seen_s = 0.0;
    } else {
        zoom.seen_s += dt;
        zoom.hidden_s = 0.0;
    }
    if zoom.hidden_s > HIDDEN_S {
        zoom.unhiding = true;
    } else if zoom.seen_s > SEEN_S {
        zoom.unhiding = false;
    }
    let unhide = if zoom.unhiding {
        obstacles
            .and_then(|obstacles| obstacles.sweep_where(arm.pivot, arm.along, boom, PROBE_M, thin))
            .filter(|&before| before >= UNHIDE_NEAREST_M)
            .map_or(0.0, |before| boom - before)
    } else {
        0.0
    };
    let rate = if unhide > zoom.unhide {
        UNHIDE_IN
    } else {
        BOOM_OUT
    };
    zoom.unhide += (unhide - zoom.unhide) * (1.0 - libm::expf(-dt * rate));
    let mut end = arm.pivot + arm.along * (boom - zoom.unhide).max(0.0);
    let ground = planet.ground_at(end).length() + OVER_GROUND_M;
    if end.length() < ground {
        end = end.normalize() * ground;
    }
    // The lens itself never goes into anything solid: from where it was, it moves toward the
    // boom's end and slides round a trunk in the way, as a character controller slides along
    // a wall, and having slid aside, comes back to the end over half a second or so. A lens
    // that jumped, as the player was moved, starts afresh, pushed out of what it is in.
    let gap = |at: Vec3| obstacles.map_or(f32::INFINITY, |o| o.clearance(at, PROBE_M + 0.5));
    let toward = end + zoom.aside * libm::expf(-dt * LENS_BACK);
    let from = zoom
        .lens
        .filter(|lens| lens.distance(end) < MOST_ASIDE_M)
        .unwrap_or_else(|| push_clear(toward, PROBE_M, gap));
    let lens = slide(from, toward, PROBE_M, up, gap, |from, along, length| {
        obstacles.and_then(|obstacles| obstacles.sweep(from, along, length, PROBE_M))
    });
    zoom.lens = Some(lens);
    zoom.aside = lens - end;
    // Slid aside, it turns to keep the figure where it was in the picture, looking at the
    // point it would have looked through from the boom's end.
    let focus = end + looking * (end.distance(arm.pivot) + 2.0);
    let looking = (focus - lens).normalize_or(looking);
    // Keep the camera out of the ground.
    let mut at = lens;
    let ground = planet.ground_at(at).length() + OVER_GROUND_M;
    if at.length() < ground {
        at = at.normalize() * ground;
    }
    at = clear_of_body(at, player.translation, up, |from, to| {
        let gap = to - from;
        let along = gap.normalize_or(up);
        swept(from, along, gap.length(), PROBE_M) >= gap.length() - 1e-3
            && obstacles
                .and_then(|obstacles| obstacles.sweep(from, along, gap.length(), PROBE_M))
                .is_none()
    });
    *camera = Transform::from_translation(at).looking_to(looking, up);
    // Foliage between the camera and the figure fades (D271).
    subject.0 = Some(chest);
}

/// Which way is out of what is solid at a point, by how its distance from it (`gap`) grows:
/// none where it is the same all round.
fn outward(at: Vec3, gap: &impl Fn(Vec3) -> f32) -> Vec3 {
    const E: f32 = 0.01;
    Vec3::new(
        gap(at + Vec3::X * E) - gap(at - Vec3::X * E),
        gap(at + Vec3::Y * E) - gap(at - Vec3::Y * E),
        gap(at + Vec3::Z * E) - gap(at - Vec3::Z * E),
    )
    .normalize_or_zero()
}

/// A sphere of `radius` at a point moved out of whatever solid it is in or touching, `gap`
/// saying how far a point is from anything solid.
fn push_clear(at: Vec3, radius: f32, gap: impl Fn(Vec3) -> f32) -> Vec3 {
    let mut at = at;
    for _ in 0..4 {
        let short = radius - gap(at);
        if short <= 0.0 {
            break;
        }
        let out = outward(at, &gap);
        if out == Vec3::ZERO {
            break;
        }
        at += out * (short + 0.005);
    }
    at
}

/// A sphere of `radius` moved from `from` toward `to`, stopping where it would touch something
/// solid (`sweep`: how far it goes from a point along a direction, within a length, before it
/// touches) and sliding on along it with what is left, a few times: a character controller's
/// collide-and-slide. Met head on by something `to` is clear of, as a trunk the lens walks
/// into, it goes round it in the level (`up` is up), toward the side `to` is on, rather than
/// stopping against it.
fn slide(
    from: Vec3,
    to: Vec3,
    radius: f32,
    up: Vec3,
    gap: impl Fn(Vec3) -> f32,
    sweep: impl Fn(Vec3, Vec3, f32) -> Option<f32>,
) -> Vec3 {
    // A sweep stops within a few millimetres of what it touches, and one starting there
    // would stop at once whichever way it went: so the sphere is kept a skin further off.
    const SKIN_M: f32 = 0.02;
    let mut at = push_clear(from, radius + SKIN_M, &gap);
    let mut left = to - at;
    let past = gap(to) >= radius;
    for _ in 0..4 {
        let length = left.length();
        if length < 1e-4 {
            break;
        }
        let along = left / length;
        let Some(went) = sweep(at, along, length) else {
            return at + left;
        };
        at = push_clear(at + along * went, radius + SKIN_M, &gap);
        let out = outward(at, &gap);
        let rest = length - went;
        // What is left, less what goes into it.
        left = along * rest;
        left -= out * left.dot(out).min(0.0);
        if past && left.length() < rest * 0.5 {
            let round = up.cross(out).normalize_or_zero();
            let side = if round.dot(to - at) < 0.0 { -1.0 } else { 1.0 };
            left = round * side * rest;
        }
    }
    at
}

/// Whether the boom passes behind something, letting it hide the figure for a moment: a tree,
/// whatever its girth, its trunk standing up through any height (nobody stands on a trunk, so
/// it has no top), or anything narrower than THIN_M.
fn thin(solid: &Solid) -> bool {
    solid.top().is_some_and(f32::is_infinite) || solid.breadth() < THIN_M
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
    use crate::{LocalPlayer, Player};
    use bevy::ecs::system::RunSystemOnce;
    use terra_core::{Piece, PlanetSpec, Seed};
    use terra_world::flora::Obstacles;

    /// A player standing on flat ground looking along x, up or down by `pitch`, with the
    /// camera following; and their feet.
    fn scene(pitch: f32) -> (World, Vec3) {
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
        world.init_resource::<Framing>();
        world.init_resource::<Time>();
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
        let (mut world, feet) = scene(pitch);
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
    fn drawn_the_camera_frames_the_fight_and_sheathed_it_goes_back() {
        // A player with a weapon drawn, looking a little down, at 55 degrees.
        let (mut world, feet) = scene(-0.2);
        let fov = 55_f32.to_radians();
        let mut camera = world.query_filtered::<Entity, With<MainCamera>>();
        let camera = camera.single(&world).unwrap();
        let lens = PerspectiveProjection { fov, ..default() };
        world
            .entity_mut(camera)
            .insert(Projection::Perspective(lens));
        let mut player = world.query_filtered::<Entity, With<Player>>();
        let player = player.single(&world).unwrap();
        let walk = frame(&mut world, 1.0 / 60.0);
        world.entity_mut(player).insert(FightView::default());
        let drawn = (0..60)
            .map(|_| frame(&mut world, 1.0 / 60.0))
            .last()
            .unwrap();
        let up = feet.normalize();
        let body = world.resource::<Framing>().fight.body_m;
        let filled = framing::share(drawn.translation, *drawn.forward(), up, fov, feet, body);
        assert!(
            (0.20..=0.27).contains(&filled),
            "the player fills {filled} of the frame"
        );
        let middle = feet + up * body * 0.5;
        let across = (middle - drawn.translation).dot(*drawn.right()).abs();
        assert!(across < 0.05, "the player is {across} m off the middle");
        world.entity_mut(player).remove::<FightView>();
        let back = (0..60)
            .map(|_| frame(&mut world, 1.0 / 60.0))
            .last()
            .unwrap();
        assert!(
            back.translation.distance(walk.translation) < 1e-3,
            "{back:?} {walk:?}"
        );
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
        let (mut world, feet) = scene(1.2);
        let camera = frame(&mut world, 0.0).translation;
        assert!(from_body(camera, feet) > 1.0, "{}", from_body(camera, feet));
    }

    /// Solid things round a player's feet, each as how far behind them and to their right its
    /// middle stands, how round it is and how tall: a trunk has no top.
    fn solids(world: &mut World, feet: Vec3, things: &[(f32, f32, f32, f32)]) {
        let planet = world.resource::<Planet>().clone();
        let mut obstacles = Obstacles::default();
        for &(back, right, radius, top) in things {
            let at = planet.ground_at(feet + Vec3::new(-back, 0.0, right));
            let piece = Piece::round(Vec2::ZERO, radius, top);
            obstacles.add_solid(at, Solid::of_pieces(at, vec![piece]));
        }
        world.insert_resource(obstacles);
    }

    /// How far the lens is from anything solid.
    fn clearance(world: &World, at: Vec3) -> f32 {
        world
            .get_resource::<Obstacles>()
            .map_or(f32::INFINITY, |obstacles| obstacles.clearance(at, 5.0))
    }

    /// Whether a point is in the camera's picture, 16:9 at the default field of view.
    fn in_picture(camera: &Transform, at: Vec3) -> bool {
        let to = at - camera.translation;
        let ahead = to.dot(*camera.forward());
        let high = libm::tanf(55f32.to_radians() * 0.5);
        ahead > 0.0
            && (to.dot(*camera.right()) / ahead).abs() <= high * 16.0 / 9.0
            && (to.dot(*camera.up()) / ahead).abs() <= high
    }

    #[test]
    fn a_boulder_behind_pulls_the_camera_in_quickly_and_it_goes_out_slowly() {
        // A boulder 3 m across and 3 m high, its middle 4 m behind the player, on the boom.
        let (mut world, feet) = scene(0.0);
        solids(&mut world, feet, &[(4.0, 0.6, 1.5, 3.0)]);
        // Never inside it, from the first frame on, and settled in front of it within a
        // fifth of a second.
        let mut camera = frame(&mut world, 1.0 / 60.0);
        let clear = clearance(&world, camera.translation);
        assert!(clear >= PROBE_M - 0.01, "{clear}");
        for _ in 0..12 {
            camera = frame(&mut world, 1.0 / 60.0);
            assert!(clearance(&world, camera.translation) >= PROBE_M - 0.01);
        }
        let clear = clearance(&world, camera.translation);
        assert!(
            clear < SOFT_PROBE_M + 0.1,
            "it stands {clear} m off the rock"
        );
        let head = feet + feet.normalize() * HEAD_M;
        let pulled = camera.translation.distance(head);
        assert!(pulled < 2.5, "{pulled}");
        // The boulder gone, it eases back out, not at once.
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
    fn a_trunk_between_the_camera_and_the_player_does_not_pull_the_boom_in() {
        // A trunk 0.7 m thick 2 m behind, across the boom over the right shoulder (right is
        // +z, looking along x), and one right behind that shoulder: the camera stays out
        // behind them, over the same shoulder, and out of both.
        let (mut world, feet) = scene(0.0);
        let trunk = f32::INFINITY;
        solids(
            &mut world,
            feet,
            &[(2.0, 0.45, 0.35, trunk), (1.0, 0.7, 0.3, trunk)],
        );
        let head = feet + feet.normalize() * HEAD_M;
        for _ in 0..90 {
            let camera = frame(&mut world, 1.0 / 60.0).translation;
            assert!(camera.distance(head) > CAMERA_BACK_M - 0.05);
            assert!((camera - feet).z > 0.5, "over the right shoulder");
            assert!(clearance(&world, camera) >= PROBE_M - 0.01);
        }
    }

    #[test]
    fn running_through_a_stand_of_trunks_the_camera_keeps_out_of_them_and_does_not_punch_in() {
        // Trunks 0.5 to 0.9 m thick every 2 m along the way, either side of it and some across
        // the boom over the right shoulder, the player running through them at 4.5 m a second.
        let (mut world, feet) = scene(-0.15);
        let sides = [0.75, -0.9, 1.2, 0.7, 2.0, -1.5, 0.9, 1.6];
        let trunks: Vec<_> = (0..24)
            .map(|i| {
                let radius = 0.25 + 0.05 * (i % 5) as f32;
                (
                    -2.0 * i as f32,
                    sides[i % sides.len()],
                    radius,
                    f32::INFINITY,
                )
            })
            .collect();
        solids(&mut world, feet, &trunks);
        let planet = world.resource::<Planet>().clone();
        let mut booms = Vec::new();
        for step in 0..600 {
            let at = planet.ground_at(feet + Vec3::X * (4.5 * step as f32 / 60.0));
            let player = world
                .query_filtered::<&mut Transform, With<Player>>()
                .single_mut(&mut world);
            *player.unwrap() = Transform::from_translation(at);
            let camera = frame(&mut world, 1.0 / 60.0);
            let up = at.normalize();
            let clear = clearance(&world, camera.translation);
            assert!(
                clear >= PROBE_M - 0.01,
                "the lens {clear} m off bark at {step}"
            );
            assert!(
                in_picture(&camera, at + up * CHEST_M) && in_picture(&camera, at + up * HEAD_M),
                "the player out of the picture at {step}"
            );
            booms.push(camera.translation.distance(at + up * HEAD_M));
            // Never more than 30% shorter within 0.3 s.
            let before = booms[booms.len().saturating_sub(18)..]
                .iter()
                .copied()
                .fold(0.0, f32::max);
            let now = booms[booms.len() - 1];
            assert!(
                now > before * 0.7,
                "the boom punched in, {before} to {now}, at {step}"
            );
        }
    }

    #[test]
    fn a_trunk_hiding_the_player_for_long_brings_the_camera_slowly_in_front_of_it() {
        // A trunk 0.7 m thick 3.2 m behind a player standing still, between them and the
        // camera.
        let (mut world, feet) = scene(0.0);
        solids(&mut world, feet, &[(3.2, 0.42, 0.35, f32::INFINITY)]);
        let up = feet.normalize();
        let (head, chest) = (feet + up * HEAD_M, feet + up * CHEST_M);
        let hidden = |world: &World, from: Vec3| {
            let to = chest - from;
            world
                .resource::<Obstacles>()
                .sweep(from, to.normalize(), to.length(), 0.05)
                .is_some()
        };
        let mut booms = Vec::new();
        for step in 0..240 {
            let camera = frame(&mut world, 1.0 / 60.0).translation;
            assert!(clearance(&world, camera) >= PROBE_M - 0.01);
            booms.push(camera.distance(head));
            // Hidden for a moment, it stays out behind the trunk.
            if step < 30 {
                assert!(hidden(&world, camera), "the trunk hides the player");
                assert!(camera.distance(head) > CAMERA_BACK_M - 0.05);
            }
            let before = booms[booms.len().saturating_sub(18)..]
                .iter()
                .copied()
                .fold(0.0, f32::max);
            assert!(booms[step] > before * 0.7, "punched in at {step}");
        }
        // Then it comes in front of it, and the player is in sight.
        let camera = frame(&mut world, 1.0 / 60.0).translation;
        assert!(!hidden(&world, camera), "{}", camera.distance(head));
        assert!(camera.distance(head) < 3.0, "{}", camera.distance(head));
        // The trunk felled, it goes back out.
        world.insert_resource(Obstacles::default());
        for _ in 0..240 {
            frame(&mut world, 1.0 / 60.0);
        }
        let out = frame(&mut world, 1.0 / 60.0).translation.distance(head);
        assert!(out > CAMERA_BACK_M - 0.05, "{out}");
    }

    #[test]
    fn a_lens_slides_round_a_trunk_in_its_way_and_rests_against_one_it_is_sent_into() {
        // A round trunk of radius 1 at the middle, and a lens 0.3 round.
        let gap = |at: Vec3| Vec2::new(at.x, at.z).length() - 1.0;
        let sweep = |from: Vec3, along: Vec3, length: f32| {
            let mut t = 0.0;
            while t < length {
                let left = gap(from + along * t) - 0.3;
                if left < 0.005 {
                    return Some(t);
                }
                t += left;
            }
            None
        };
        // Sent past it, a tenth of a metre at a time, glancing or dead on: it goes round,
        // never into it, and gets where it was sent.
        for across in [0.5, 0.0] {
            let mut at = Vec3::new(-3.0, 0.0, across);
            for step in 1..=60 {
                let to = Vec3::new(-3.0 + 0.1 * step as f32, 0.0, across);
                at = slide(at, to, 0.3, Vec3::Y, gap, sweep);
                assert!(gap(at) >= 0.3 - 1e-3, "{at}");
            }
            for _ in 0..20 {
                at = slide(at, Vec3::new(3.0, 0.0, across), 0.3, Vec3::Y, gap, sweep);
            }
            assert!(at.distance(Vec3::new(3.0, 0.0, across)) < 0.05, "{at}");
        }
        // Sent into it, it rests against it and stays there.
        let mut at = Vec3::new(-3.0, 0.0, 0.2);
        for _ in 0..100 {
            at = slide(at, Vec3::new(-0.5, 0.0, 0.2), 0.3, Vec3::Y, gap, sweep);
        }
        let rest = at;
        for _ in 0..100 {
            at = slide(at, Vec3::new(-0.5, 0.0, 0.2), 0.3, Vec3::Y, gap, sweep);
        }
        assert!(gap(at) >= 0.3 - 1e-3 && gap(at) < 0.35, "{at}");
        assert!(at.distance(rest) < 0.01, "it crept from {rest} to {at}");
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
