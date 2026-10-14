//! The walker: a body this machine steers over the planet, the camera behind it, and the
//! controls and options menu that steer it. It needs only the world beneath it
//! (`WorldPlugin`), so a run with no gameplay on, such as `--experiment walk`, can go for a
//! walk through the forest with nothing else of the game in it.
//!
//! What lies above it, such as the inventory or the builder, talks to it through the small
//! types here: it says what the player's hands are on (`Hands`), where a player arrives
//! (`ArriveAt`) and asks for a warp (`WarpTo`), and orders its systems around
//! [`PlayerInput`]. The walker never names a system above it.

use super::controls::Action;
use super::intent::Intent;
use super::{camera, figure, intent, movement, options};
use crate::planet::{MainCamera, Planet};
use crate::shell::{Options, ScriptFrame};
use crate::start::Start;
use bevy::{camera_controller::free_camera::FreeCamera, prelude::*};
use terra_world::{lod::Viewpoint, parting::PartsPlants, tick::Smoothed};

/// Someone playing. What they are doing is in the components beside this one: how they
/// move (`Motion`), what their hands and feet are up to (`Stance`), and whether they are
/// down (`Downed`), along with what they carry, know and feel, which the layers above add.
#[derive(Component)]
#[require(Smoothed, Viewpoint, PartsPlants)]
pub(crate) struct Player;

/// The player this machine steers: its input moves them, and the camera, the screen and
/// what is heard follow them. Systems driven by input or drawing the screen act on this
/// one; the world's own systems, such as creatures and stations, act on every [`Player`].
#[derive(Component)]
pub(crate) struct LocalPlayer;

/// The query filter for this machine's player. It names [`Player`] as well, so Bevy can
/// tell a query for them is apart from one that leaves players out (`Without<Player>`).
pub(super) type IsLocalPlayer = (With<Player>, With<LocalPlayer>);

/// How a body moves over the planet.
#[derive(Component)]
pub(super) struct Motion {
    /// Speed over the ground, as a direction and metres a second.
    pub(super) velocity: Vec3,
    /// Speed away from the planet's centre, in metres a second.
    pub(super) rising: f32,
    pub(super) on_ground: bool,
    /// Seconds since the feet last touched something.
    pub(super) airborne: f32,
    pub(super) indoors: bool,
    /// The level direction the figure faces.
    pub(super) facing: Vec3,
}

impl Motion {
    pub(super) fn facing(facing: Vec3) -> Self {
        Self {
            velocity: Vec3::ZERO,
            rising: 0.0,
            on_ground: false,
            airborne: 0.0,
            indoors: false,
            facing,
        }
    }
}

/// What a player's hands and feet are doing besides walking: using a tool, swinging,
/// rolling, flinching from a blow, or guarding.
#[derive(Component)]
pub(super) struct Stance {
    /// Seconds since the tool was last used.
    pub(super) since_acting: f32,
    /// Seconds left of a swing, a roll, and a flinch from a blow, for the figure.
    pub(super) attacking: f32,
    pub(super) dodging: f32,
    pub(super) flinch: f32,
    /// Which way a roll goes.
    pub(super) dodge_direction: Vec3,
    /// Holding a guard up.
    pub(super) guarding: bool,
}

impl Default for Stance {
    fn default() -> Self {
        Self {
            since_acting: f32::MAX,
            attacking: 0.0,
            dodging: 0.0,
            flinch: 0.0,
            dodge_direction: Vec3::ZERO,
            guarding: false,
        }
    }
}

/// Lying where they fell, and for how long so far. Taken away when they wake.
#[derive(Component)]
pub(super) struct Downed {
    pub(super) lying_s: f32,
}

/// Where a player is looking.
#[derive(Component)]
pub(super) struct View {
    /// Along the ground.
    pub(super) forward: Vec3,
    /// Up or down from level, in radians.
    pub(super) pitch: f32,
}

pub struct WalkerPlugin;

impl Plugin for WalkerPlugin {
    fn build(&self, app: &mut App) {
        options::plugin(app);
        intent::plugin(app);
        figure::plugin(app);
        camera::plugin(app);
        movement::plugin(app);
        app.init_resource::<Hands>()
            .init_resource::<WarpTo>()
            .add_systems(PostStartup, spawn)
            .add_systems(
                Update,
                (options::toggle, camera::capture_mouse, warp, camera::zoom)
                    .chain()
                    .in_set(PlayerInput),
            )
            // What this machine's player means to do, read before the frame's ticks.
            .add_systems(
                RunFixedMainLoop,
                (camera::look, intent::read, follow_route)
                    .chain()
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            );
    }
}

/// The frame's reading of the keys and mouse into the player and the camera: Esc and the
/// options menu, taking the mouse, warps, and the camera's zoom. A system above the walker
/// that acts on what the player means runs after this set.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PlayerInput;

/// What has this machine's player's pointer, mouse, wheel and hands this frame, besides
/// the options menu, which the walker knows itself. Screens and tools above the walker
/// fill it in, before `camera::capture_mouse` and `camera::look`, so the walker never
/// needs to know which screens there are. With nothing above it, all of it is free.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Hands {
    /// The hands are on a screen, such as the inventory, not on the world.
    pub busy: bool,
    /// The player is choosing something, such as what to build.
    pub choosing: bool,
    /// A screen wants the pointer, so the mouse is not taken.
    pub pointer: bool,
    /// The arrow keys move about a screen rather than look.
    pub arrows: bool,
    /// The mouse steers something other than the view.
    pub mouse: bool,
    /// The wheel turns something other than the camera's distance.
    pub wheel: bool,
}

/// Where this machine's player first stands, and the point they face. Something above the
/// walker can set it before `PostStartup`, as building does to stand them outside the
/// base's door. Without it they stand at the start, facing south. `--at` and `--face`
/// override either.
#[derive(Resource, Clone, Copy, Debug)]
pub struct ArriveAt {
    pub at: Vec3,
    pub toward: Vec3,
}

/// A warp asked for from outside the game, such as over the remote protocol: so many
/// metres east and south of the start, and the spot to face, if any. Taken on the next
/// frame.
#[derive(Resource, Default)]
pub struct WarpTo(pub Option<((f32, f32), Option<(f32, f32)>)>);

/// This machine's player: a body standing where `ArriveAt` says, with the camera theirs.
/// Whatever the layers above give a player (what they carry, know and feel) they add when
/// a [`LocalPlayer`] appears.
pub(super) fn spawn(
    mut commands: Commands,
    options: Res<Options>,
    start: Res<Start>,
    arrive: Option<Res<ArriveAt>>,
    planet: Res<Planet>,
    camera: Single<Entity, With<MainCamera>>,
) {
    // The player steers the camera now.
    commands.entity(*camera).remove::<FreeCamera>();

    let arrive = arrive.map_or(
        ArriveAt {
            at: start.at(0.0, 0.0),
            toward: start.at(0.0, 10.0),
        },
        |arrive| *arrive,
    );
    // For testing, a run can start somewhere else (--at), facing a given spot (--face).
    let outside = options
        .start_at
        .map_or(arrive.at, |(east, south)| start.at(east, south));
    let toward = options
        .start_facing
        .map_or(arrive.toward, |(east, south)| start.at(east, south));
    let feet = planet.ground_at(outside);
    let up = feet.normalize();
    let facing = (toward - outside).reject_from(up).normalize();
    commands.spawn((
        Name::new("Player"),
        Player,
        LocalPlayer,
        View {
            forward: facing,
            pitch: -0.08,
        },
        Motion::facing(facing),
        Stance::default(),
        Transform::from_translation(feet).looking_to(facing, up),
        Visibility::default(),
    ));
}

/// How fast a route turns the player, in radians a second: as fast as the arrow keys do.
const ROUTE_TURN: f32 = 1.6;
/// A spot of a route is reached this near it.
const ROUTE_REACHED_M: f32 = 1.5;

/// How long a route walk may go without getting nearer its spot before it steps round
/// what is in its way, in seconds; how long it heads off to one side the first time; and how
/// far it turns from its spot to do it, in radians.
const DETOUR_AFTER_S: f32 = 0.5;
const DETOUR_S: f32 = 2.0;
const DETOUR_TURN: f32 = 1.4;
/// Each try round the same thing heads off the other way, half as long again as the last,
/// up to this long: always to the left and for as long, a walk met a boulder wider than its
/// detour at the same face again and again, and stood there for the rest of the film.
const DETOUR_LONGEST_S: f32 = 8.0;
/// After this long at one spot without reaching it, the walk gives it up for the next.
const GIVE_UP_S: f32 = 20.0;
/// Getting this much nearer its spot means what was in the way is passed.
const PASSED_M: f32 = 2.0;

/// A route walk's way round something in its way: the nearest it has been to its spot, how
/// long since it got nearer, which try round it this is and how long it has still to head
/// off, and how long it has been making for this spot.
#[derive(Default)]
struct Detour {
    nearest: Option<f32>,
    since: f32,
    tries: u32,
    off: f32,
    side: f32,
    /// The nearest it was before its first try round, to know when it is past.
    before: Option<f32>,
    at_spot: f32,
}

impl Detour {
    /// How far to turn from the way to the spot, `distance` away, `dt` seconds on: to the
    /// left positive.
    fn turn(&mut self, distance: f32, dt: f32) -> f32 {
        self.at_spot += dt;
        if self.off > 0.0 {
            self.off -= dt;
            return DETOUR_TURN * self.side;
        }
        if self
            .before
            .is_some_and(|before| distance < before - PASSED_M)
        {
            self.tries = 0;
            self.before = None;
        }
        if self.nearest.is_none_or(|nearest| distance < nearest - 0.05) {
            self.nearest = Some(distance);
            self.since = 0.0;
        } else {
            self.since += dt;
            if self.since > DETOUR_AFTER_S {
                self.before.get_or_insert(distance);
                self.side = if self.tries.is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                };
                self.off = (DETOUR_S * 1.5f32.powi(self.tries as i32)).min(DETOUR_LONGEST_S);
                self.tries += 1;
                self.since = 0.0;
                self.nearest = None;
            }
        }
        0.0
    }

    /// Whether the walk has made for its spot so long it should give it up.
    fn given_up(&self) -> bool {
        self.at_spot > GIVE_UP_S
    }
}

/// For scripted walks, such as benchmarks (`--route`): the player walks to each spot in
/// turn, east and south of the start, turning toward it no faster than the arrow keys turn
/// the view, as someone playing would. Once at the last, they stand. Stopped by a trunk or
/// a rock in the way, they head off to one side for a moment and on again, the other side
/// and longer each try, and give a spot up for the next if it cannot be reached, so the walk
/// keeps going however the world it crosses is laid out (D276).
fn follow_route(
    time: Res<Time>,
    options: Res<Options>,
    start: Res<Start>,
    mut next: Local<usize>,
    mut detour: Local<Detour>,
    player: Single<(&Transform, &mut View, &mut Intent), IsLocalPlayer>,
) {
    let Some(&(east, south)) = options.route.get(*next) else {
        return;
    };
    let (transform, mut view, mut intent) = player.into_inner();
    let feet = transform.translation;
    let up = feet.normalize();
    let to = (start.at(east, south) - feet).reject_from(up);
    if to.length() < ROUTE_REACHED_M || detour.given_up() {
        *next += 1;
        if detour.given_up() {
            info!(
                "Route: gave up {east:.0},{south:.0}, {:.0} m short.",
                to.length()
            );
        } else {
            info!("Route: reached {east:.0},{south:.0}.");
        }
        *detour = Detour::default();
        return;
    }
    let wanted =
        Quat::from_axis_angle(up, detour.turn(to.length(), time.delta_secs())) * to.normalize();
    let turn = libm::atan2f(up.dot(view.forward.cross(wanted)), view.forward.dot(wanted));
    let most = ROUTE_TURN * time.delta_secs();
    view.forward = Quat::from_axis_angle(up, turn.clamp(-most, most)) * view.forward;
    intent.keep_held(Action::Forward);
}

/// For testing, moves the player where a scripted run says, on the frame it says (--warp),
/// or where something outside the game asks (`WarpTo`).
fn warp(
    options: Res<Options>,
    frame: Res<ScriptFrame>,
    mut asked: ResMut<WarpTo>,
    start: Res<Start>,
    planet: Res<Planet>,
    player: Single<(&mut Transform, &mut Motion, &mut View), IsLocalPlayer>,
) {
    let scripted = options
        .warps
        .iter()
        .find(|warp| warp.frame == frame.0)
        .map(|warp| (warp.at, Some(warp.facing)));
    let Some((at, facing)) = scripted.or_else(|| asked.0.take()) else {
        return;
    };
    let (mut transform, mut motion, mut view) = player.into_inner();
    let place = |(east, south): (f32, f32)| start.at(east, south);
    let feet = planet.ground_at(place(at));
    let up = feet.normalize();
    let ahead = facing.map_or(view.forward, |facing| place(facing) - feet);
    let facing = ahead.reject_from(up).normalize_or(view.forward);
    transform.translation = feet;
    motion.velocity = Vec3::ZERO;
    motion.facing = facing;
    view.forward = facing;
    info!("Warped to {:.1} m east and {:.1} m south.", at.0, at.1);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A walk getting nearer goes straight; one stopped turns off to its left for a while,
    /// then heads for its spot again.
    #[test]
    fn a_stopped_route_walk_steps_round_and_on() {
        let mut detour = Detour::default();
        let dt = 1.0 / 60.0;
        let mut distance = 20.0;
        for _ in 0..120 {
            distance -= 0.08;
            assert_eq!(
                detour.turn(distance, dt),
                0.0,
                "turned while getting nearer"
            );
        }
        let turned: Vec<f32> = (0..360).map(|_| detour.turn(distance, dt)).collect();
        let first = turned
            .iter()
            .position(|&t| t != 0.0)
            .expect("never turned off");
        assert!(
            (first as f32 * dt) < DETOUR_AFTER_S + 0.1,
            "waited {first} frames"
        );
        let off = turned[first..].iter().take_while(|&&t| t != 0.0).count();
        assert!(
            (off as f32 * dt - DETOUR_S).abs() < 0.05,
            "turned off for {off} frames"
        );
        assert_eq!(turned[first + off], 0.0, "never turned back");
    }

    /// Stopped again and again at the same thing, a walk tries the other side each time,
    /// longer each time, so it gets round something wider than one try; and once past, it
    /// starts again from a short try to the left.
    #[test]
    fn a_walk_stopped_again_tries_the_other_side_for_longer() {
        let mut detour = Detour::default();
        let dt = 1.0 / 60.0;
        let distance = 20.0;
        // Never getting nearer: record each try's side and length.
        let mut tries: Vec<(f32, f32)> = Vec::new();
        let mut current: Option<(f32, f32)> = None;
        for _ in 0..(60 * 18) {
            let t = detour.turn(distance, dt);
            match (current.as_mut(), t != 0.0) {
                (Some(c), true) => c.1 += dt,
                (Some(c), false) => {
                    tries.push(*c);
                    current = None;
                }
                (None, true) => current = Some((t.signum(), dt)),
                (None, false) => {}
            }
        }
        assert!(tries.len() >= 3, "{tries:?}");
        for pair in tries.windows(2) {
            assert_eq!(pair[0].0, -pair[1].0, "the same side twice: {tries:?}");
            assert!(
                pair[1].1 > pair[0].1 * 1.3 || pair[1].1 >= DETOUR_LONGEST_S - 0.05,
                "not longer: {tries:?}"
            );
        }
        // Let the try under way finish.
        while detour.turn(distance, dt) != 0.0 {}
        // Now past it: much nearer; the next stop starts over to the left, short.
        let mut near = distance - PASSED_M - 1.0;
        for _ in 0..60 {
            near -= 0.08;
            detour.turn(near, dt);
        }
        let turned: Vec<f32> = (0..300).map(|_| detour.turn(near, dt)).collect();
        let off: Vec<&f32> = turned
            .iter()
            .skip_while(|t| **t == 0.0)
            .take_while(|t| **t != 0.0)
            .collect();
        assert!(
            off.first().is_some_and(|t| **t > 0.0),
            "did not start again to the left"
        );
        assert!(
            (off.len() as f32 * dt - DETOUR_S).abs() < 0.05,
            "did not start again short"
        );
    }

    /// A spot that cannot be reached is given up after a while, so the walk goes on.
    #[test]
    fn a_spot_that_cannot_be_reached_is_given_up() {
        let mut detour = Detour::default();
        let dt = 1.0 / 60.0;
        for _ in 0..(60.0 * (GIVE_UP_S - 1.0)) as usize {
            detour.turn(20.0, dt);
        }
        assert!(!detour.given_up());
        for _ in 0..120 {
            detour.turn(20.0, dt);
        }
        assert!(detour.given_up());
    }
}
