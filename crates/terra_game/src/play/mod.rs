//! A player in a meadow by the water, with a builder tool and nothing built yet.
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
//! the builder, `dial` its menu and prompts, and `catalogue` what it can build. See docs/player.md and docs/building.md for the design.

mod archery;
mod bag;
mod bodies;
mod build_menu;
mod camera;
mod catalogue;
mod claim;
mod combat;
mod controls;
mod crafting;
mod creatures;
mod data;
mod daylight;
mod death;
mod dial;
mod dig;
mod figure;
mod flora;
pub use flora::{Growth, Obstacles};
mod flora_kinds;
pub use flora_kinds::Flora;
mod gains;
mod harvest;
pub(crate) mod held;
mod hits;
mod intent;
mod interact;
pub mod inventory;
mod kinds;
mod lod;
mod map;
mod models;
mod movement;
mod music;
mod noise;
mod noises;
mod options;
mod people;
mod pile;
mod pooling;
mod probe;
mod props;
pub use props::{Look as PropLook, glowing as prop_glowing, lit as prop_lit, looks as prop_looks};
mod save;
mod scene;
mod settlement;
mod skills;
mod sounds;
mod soundscape;
mod species;
mod station_screen;
mod tick;
mod tiles;
mod told;
mod tool;
mod torchlight;
mod vitals;
mod wear;
mod work;
mod worn;
mod zones;

use super::build::{self, DrivenByPlayer, Site};
use crate::planet::MainCamera;
use crate::planet::Planet;
use crate::shell::Options;
use bevy::{camera_controller::free_camera::FreeCamera, prelude::*};
use std::ops::Sub;
use tick::Smoothed;

/// Someone playing. What they are doing is in the components beside this one: how they
/// move (`Motion`), what their hands and feet are up to (`Stance`), and whether they are
/// down (`Downed`), along with what they carry, know and feel.
#[derive(Component)]
#[require(Smoothed)]
struct Player;

/// The player this machine steers: its input moves them, and the camera, the screen and
/// what is heard follow them. Systems driven by input or drawing the screen act on this
/// one; the world's own systems, such as creatures and stations, act on every [`Player`].
#[derive(Component)]
struct LocalPlayer;

/// The query filter for this machine's player. It names [`Player`] as well, so Bevy can
/// tell a query for them is apart from one that leaves players out (`Without<Player>`).
type IsLocalPlayer = (With<Player>, With<LocalPlayer>);

/// How a body moves over the planet.
#[derive(Component)]
struct Motion {
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
}

impl Motion {
    fn facing(facing: Vec3) -> Self {
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
struct Stance {
    /// Seconds since the tool was last used.
    since_acting: f32,
    /// Seconds left of a swing, a roll, and a flinch from a blow, for the figure.
    attacking: f32,
    dodging: f32,
    flinch: f32,
    /// Which way a roll goes.
    dodge_direction: Vec3,
    /// Holding a guard up.
    guarding: bool,
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
struct Downed {
    lying_s: f32,
}

/// Where a player is looking.
#[derive(Component)]
struct View {
    /// Along the ground.
    forward: Vec3,
    /// Up or down from level, in radians.
    pitch: f32,
}

pub fn plugin(app: &mut App) {
    // A player needs something to build on, and takes over from the build keys.
    build::plugin(app);
    data::plugin(app);
    scene::plugin(app);
    daylight::plugin(app);
    torchlight::plugin(app);
    wear::plugin(app);
    pile::plugin(app);
    zones::plugin(app);
    flora::plugin(app);
    props::plugin(app);
    figure::plugin(app);
    tool::plugin(app);
    dial::plugin(app);
    told::plugin(app);
    bag::plugin(app);
    gains::plugin(app);
    build_menu::plugin(app);
    crafting::plugin(app);
    station_screen::plugin(app);
    intent::plugin(app);
    interact::plugin(app);
    claim::plugin(app);
    vitals::plugin(app);
    death::plugin(app);
    skills::plugin(app);
    creatures::plugin(app);
    bodies::plugin(app);
    combat::plugin(app);
    hits::plugin(app);
    archery::plugin(app);
    people::plugin(app);
    pooling::plugin(app);
    settlement::plugin(app);
    map::plugin(app);
    held::plugin(app);
    worn::plugin(app);
    save::plugin(app);
    harvest::plugin(app);
    dig::plugin(app);
    options::plugin(app);
    probe::plugin(app);
    // Sound: noises from what happens, what they sound like, the place, and music.
    noise::plugin(app);
    noises::plugin(app);
    sounds::plugin(app);
    soundscape::plugin(app);
    music::plugin(app);
    tick::plugin(app);
    lod::plugin(app);
    camera::plugin(app);
    movement::plugin(app);
    app.insert_resource(DrivenByPlayer)
        .add_systems(PostStartup, spawn)
        .add_systems(
            Update,
            (
                options::toggle,
                camera::capture_mouse,
                dial::steer,
                warp,
                camera::zoom,
            )
                .chain()
                .before(tool::Acting),
        )
        // What this machine's player means to do, read before the frame's ticks.
        .add_systems(
            RunFixedMainLoop,
            (camera::look, intent::read)
                .chain()
                .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
        );
}

fn spawn(
    mut commands: Commands,
    mut items: ResMut<inventory::Items>,
    options: Res<Options>,
    site: Res<Site>,
    planet: Res<Planet>,
    camera: Single<Entity, With<MainCamera>>,
) {
    // The player steers the camera now.
    commands.entity(*camera).remove::<FreeCamera>();

    // Stand a little way out from the door, facing it.
    let (approach, facing) = site.approach();
    // For testing, a run can start somewhere else (--at).
    let outside = match options.start_at {
        Some((east, south)) => site.start.transform_point(Vec3::new(east, 0.0, south)),
        None => site.frame.transform_point(approach - facing * 5.0),
    };
    let feet = planet.ground_at(outside);
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
    commands.spawn((
        Name::new("Player"),
        Player,
        LocalPlayer,
        View {
            forward: toward_door,
            pitch: -0.08,
        },
        death::Arrival {
            at: feet,
            facing: toward_door,
        },
        Motion::facing(toward_door),
        Stance::default(),
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

/// For testing, moves the player where a scripted run says, on the frame it says (--warp),
/// or where a script asks over the remote protocol (`probe`).
fn warp(
    options: Res<Options>,
    frame: Res<crate::shell::ScriptFrame>,
    mut asked: ResMut<probe::Asked>,
    site: Res<Site>,
    planet: Res<Planet>,
    player: Single<(&mut Transform, &mut Motion, &mut View), IsLocalPlayer>,
) {
    let scripted = options
        .warps
        .iter()
        .find(|warp| warp.frame == frame.0)
        .map(|warp| (warp.at, Some(warp.facing)));
    let Some((at, facing)) = scripted.or_else(|| asked.warp.take()) else {
        return;
    };
    let (mut transform, mut motion, mut view) = player.into_inner();
    let place = |(east, south): (f32, f32)| site.start.transform_point(Vec3::new(east, 0.0, south));
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

/// A world with what the play systems read and write that is not of any one entity: the
/// kinds of item, and the messages about what players carry and are told. For tests.
#[cfg(test)]
fn test_world() -> World {
    let mut world = World::new();
    world.init_resource::<inventory::Items>();
    world.init_resource::<Messages<inventory::Gained>>();
    world.init_resource::<Messages<inventory::Broke>>();
    world.init_resource::<Messages<inventory::Spilled>>();
    world.init_resource::<Messages<told::Told>>();
    world.init_resource::<Messages<interact::Interacted>>();
    world
}

/// A player standing at a place, with what the world's systems read of one, for tests
/// that put several in one world.
#[cfg(test)]
fn test_player(world: &mut World, at: Vec3) -> Entity {
    let mut items = world.get_resource_or_init::<inventory::Items>();
    let kit = inventory::starting_kit(&mut items);
    world
        .spawn((
            Player,
            Motion::facing(Vec3::X),
            Stance::default(),
            vitals::Vitals::default(),
            skills::Skills::default(),
            kit,
            intent::Intent::default(),
            Transform::from_translation(at),
        ))
        .id()
}
