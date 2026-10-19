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
use terra_world::{climate::ClimateSample, lod::Viewpoint, parting::PartsPlants, tick::Smoothed};

/// Someone playing. What they are doing is in the components beside this one: how they
/// move (`Motion`), what their hands and feet are up to (`Stance`), and whether they are
/// down (`Downed`), the climate where they stand (`ClimateSample`), along with what they
/// carry, know and feel, which the layers above add.
#[derive(Component)]
#[require(Smoothed, Viewpoint, PartsPlants, ClimateSample)]
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
    /// What a layer above asks of the walk this tick, as the sword stance does (D405).
    pub(super) steer: Steer,
}

/// How a layer above steers a walk: which way the body faces, how fast it walks, a move's
/// own drive in place of what the keys ask, and whether Ctrl's roll is left to it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Steer {
    pub(super) face: Option<Vec3>,
    pub(super) pace: Option<f32>,
    pub(super) drive: Option<Vec3>,
    pub(super) no_roll: bool,
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
            steer: Steer::default(),
        }
    }
}

/// Lying where they fell, and for how long so far. Taken away when they wake.
#[derive(Component)]
pub(super) struct Downed {
    pub(super) lying_s: f32,
}

/// What a body has left to sprint, roll and jump on, out of [`Stamina::FULL`]. The walk
/// spends it; what lies above, the body's needs (`vitals`), fills it. A body without it,
/// as in the walker on its own, never tires.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct Stamina {
    pub left: f32,
    /// Seconds since it was last spent; it waits a moment before coming back.
    since_spent: f32,
}

impl Default for Stamina {
    fn default() -> Self {
        Self {
            left: Self::FULL,
            since_spent: 10.0,
        }
    }
}

impl Stamina {
    pub const FULL: f32 = 100.0;

    /// Spends stamina, if there is any to spend. Returns whether there was.
    pub fn spend(&mut self, amount: f32) -> bool {
        if self.left <= 0.0 {
            return false;
        }
        self.left = (self.left - amount).max(0.0);
        self.since_spent = 0.0;
        true
    }

    pub fn can_sprint(&self) -> bool {
        self.left > 0.0
    }

    /// Lets a moment pass without spending any.
    pub fn wait(&mut self, dt: f32) {
        self.since_spent += dt;
    }

    /// Seconds since it was last spent.
    pub fn rested_s(&self) -> f32 {
        self.since_spent
    }

    /// Puts some back, up to full.
    pub fn fill(&mut self, amount: f32) {
        self.left = (self.left + amount).min(Self::FULL);
    }
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
        // What a shot films as the player (`--cine`).
        terra_world::cine::Subject,
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
/// Each try round the same thing is half as long again as the last, up to this long, and after
/// so many tries to one side it tries the other: always to the left and for as long, a walk met
/// a boulder wider than its detour at the same face again and again, and stood there for the
/// rest of the film; turning the other way each try, each undid the last and it stood eight
/// seconds at a rock.
const DETOUR_LONGEST_S: f32 = 8.0;
const TRIES_A_SIDE: u32 = 3;
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
                self.side = if (self.tries / TRIES_A_SIDE).is_multiple_of(2) {
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

/// For scripted walks, such as benchmarks (`--route`): the player walks the route through
/// its spots in turn, east and south of the start, turning no faster than the arrow keys turn
/// the view, as someone playing would. Once at the last, they stand. Stopped by a trunk or
/// a rock in the way, they head off to one side for a moment and on again, the other side
/// and longer each try, and give a spot up for the next if it cannot be reached, so the walk
/// keeps going however the world it crosses is laid out (D276).
///
/// The walk heads for the point of the route a few metres ahead of it (`pursued`), as a
/// game's path following and a vehicle's pure pursuit steer, not for the next spot itself:
/// made for each spot in turn, a walk a little off its line swung toward the spot as it came
/// near and turned back at it, and a trailer's camera jumped sideways at every spot of a
/// smoothed route, once a second.
fn follow_route(
    time: Res<Time>,
    options: Res<Options>,
    start: Res<Start>,
    mut next: Local<usize>,
    mut detour: Local<Detour>,
    player: Single<(&Transform, &mut View, &mut Intent), IsLocalPlayer>,
    cine: Option<Res<terra_world::cine::Cine>>,
) {
    // In a shot, the walk starts as it rolls (`--cine`).
    if cine.is_some_and(|cine| !cine.rolling()) {
        return;
    }
    let (transform, mut view, mut intent) = player.into_inner();
    let feet = transform.translation;
    let up = feet.normalize();
    let spot = |i: usize| {
        let (east, south) = options.route[i];
        start.at(east, south)
    };
    let away = |i: usize| (spot(i) - feet).reject_from(up).length();
    // Spots inside the look-ahead are passed, all but the last, which is reached by standing
    // on it; a spot given up is passed too.
    while let Some(&(east, south)) = options.route.get(*next) {
        let last = *next + 1 == options.route.len();
        let reached = away(*next) < if last { ROUTE_REACHED_M } else { ROUTE_AHEAD_M };
        if !reached && !detour.given_up() {
            break;
        }
        if reached {
            info!("Route: reached {east:.0},{south:.0}.");
        } else {
            info!(
                "Route: gave up {east:.0},{south:.0}, {:.0} m short.",
                away(*next)
            );
        }
        *next += 1;
        *detour = Detour::default();
    }
    if *next >= options.route.len() {
        return;
    }
    let from = if *next > 0 { spot(*next - 1) } else { feet };
    let to = pursued(feet, up, from, spot(*next), ROUTE_AHEAD_M);
    let wanted =
        Quat::from_axis_angle(up, detour.turn(away(*next), time.delta_secs())) * to.normalize();
    view.forward = turned_toward(view.forward, wanted, up, ROUTE_TURN * time.delta_secs());
    intent.keep_held(Action::Forward);
}

/// How far ahead along its route a route walk heads for, in metres: a few strides, past the
/// next spot of a smoothed route (one every 4 m).
const ROUTE_AHEAD_M: f32 = 6.0;

/// The way, across the ground at `feet`, to the point `ahead` metres away on the leg of a
/// route from `from` to `to`, the far one where the leg crosses that distance; to `to` if
/// the leg is nearer or never comes that near.
fn pursued(feet: Vec3, up: Vec3, from: Vec3, to: Vec3, ahead: f32) -> Vec3 {
    let a = (from - feet).reject_from(up);
    let b = (to - feet).reject_from(up);
    let d = b - a;
    // |a + t d| = ahead, the larger t in 0 to 1.
    let (qa, qb, qc) = (d.dot(d), 2.0 * a.dot(d), a.dot(a) - ahead * ahead);
    let disc = qb * qb - 4.0 * qa * qc;
    if qa < 1e-6 || disc < 0.0 || b.length() <= ahead {
        return b;
    }
    let t = ((-qb + libm::sqrtf(disc)) / (2.0 * qa)).clamp(0.0, 1.0);
    a + d * t
}

/// `forward` turned about `up` toward `wanted`, by at most `most` radians.
fn turned_toward(forward: Vec3, wanted: Vec3, up: Vec3, most: f32) -> Vec3 {
    let turn = libm::atan2f(up.dot(forward.cross(wanted)), forward.dot(wanted));
    Quat::from_axis_angle(up, turn.clamp(-most, most)) * forward
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

    /// Walked at a jog down a smoothed route, a spot every 4 m, wandering a little either
    /// side of a straight line, as a path's laid points do, the walk turns smoothly: never
    /// more than a little a frame once it is on its way, and nowhere more at one spot than
    /// between them. Heading for each spot in turn, it turned hardest as it came on each, and
    /// the camera behind it jumped once a second.
    #[test]
    fn a_route_walk_turns_smoothly_past_its_spots() {
        let up = Vec3::Y;
        let spots: Vec<Vec3> = (1..60)
            .map(|i| Vec3::new(0.35 * libm::sinf(i as f32 * 1.7), 0.0, -4.0 * i as f32))
            .collect();
        let dt = 1.0 / 60.0;
        let mut feet = Vec3::new(0.8, 0.0, 0.0);
        let mut forward = Vec3::NEG_Z;
        let mut next = 0;
        let mut turns = Vec::new();
        for _ in 0..(60 * 40) {
            while next + 1 < spots.len() && (spots[next] - feet).length() < ROUTE_AHEAD_M {
                next += 1;
            }
            let from = if next > 0 { spots[next - 1] } else { feet };
            let to = pursued(feet, up, from, spots[next], ROUTE_AHEAD_M);
            let turned = turned_toward(forward, to.normalize(), up, ROUTE_TURN * dt);
            turns.push(forward.angle_between(turned));
            forward = turned;
            feet += forward * 4.3 * dt;
        }
        let settled = &turns[120..];
        let most = settled.iter().cloned().fold(0.0, f32::max);
        assert!(
            most < 0.15_f32.to_radians(),
            "turned {:.2} degrees in a frame",
            most.to_degrees()
        );
        // And it went the way, staying near its line.
        assert!(feet.z < -150.0 && feet.x.abs() < 1.0, "ended at {feet}");
    }

    /// The point pursued is that far off on the route's leg, ahead of the walk, and the leg's
    /// end when it is nearer than that.
    #[test]
    fn a_route_walk_heads_for_the_point_a_few_metres_on() {
        let up = Vec3::Y;
        let to = pursued(
            Vec3::ZERO,
            up,
            Vec3::new(1.0, 0.0, 10.0),
            Vec3::new(1.0, 0.0, -10.0),
            6.0,
        );
        assert!((to.length() - 6.0).abs() < 1e-4 && to.z < 0.0, "{to}");
        let near = pursued(
            Vec3::ZERO,
            up,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, -3.0),
            6.0,
        );
        assert!((near - Vec3::new(0.0, 0.0, -3.0)).length() < 1e-5, "{near}");
    }

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

    /// Stopped again and again at the same thing, a walk tries longer each time, a few times
    /// to one side and then the other, so it gets round something wider than one try; and
    /// once past, it starts again from a short try to the left.
    #[test]
    fn a_walk_stopped_again_tries_the_other_side_for_longer() {
        let mut detour = Detour::default();
        let dt = 1.0 / 60.0;
        let distance = 20.0;
        // Never getting nearer: record each try's side and length.
        let mut tries: Vec<(f32, f32)> = Vec::new();
        let mut current: Option<(f32, f32)> = None;
        for _ in 0..(60 * 30) {
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
        let a_side = TRIES_A_SIDE as usize;
        assert!(tries.len() > a_side, "{tries:?}");
        assert!(
            tries[..a_side].iter().all(|t| t.0 > 0.0),
            "not to the left first: {tries:?}"
        );
        assert!(
            tries[a_side].0 < 0.0,
            "never tried the other side: {tries:?}"
        );
        for pair in tries.windows(2) {
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
