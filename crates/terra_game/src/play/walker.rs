//! The walker: a body this machine steers over the planet, the camera behind it, and the
//! controls and options menu that steer it. It needs only the world beneath it
//! (`WorldPlugin`), so a run with no gameplay on, such as `--experiment walk`, can go for a
//! walk through the forest with nothing else of the game in it.
//!
//! What lies above it, such as the inventory or the builder, talks to it through the small
//! types here: it says what the player's hands are on (`Hands`), where a player arrives
//! (`ArriveAt`) and asks for a warp (`WarpTo`), and orders its systems around
//! [`PlayerInput`]. The walker never names a system above it.

use super::{IsLocalPlayer, LocalPlayer, Motion, Player, Stance, View};
use super::{camera, figure, intent, movement, options};
use crate::planet::{MainCamera, Planet};
use crate::shell::{Options, ScriptFrame};
use crate::start::Start;
use bevy::{camera_controller::free_camera::FreeCamera, prelude::*};

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
                (camera::look, intent::read)
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
