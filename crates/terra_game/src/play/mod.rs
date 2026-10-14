//! The game a player plays, in three layers of plugins (D198, docs/tech.md):
//!
//! - `world`: the world around a player, from the data files to what grows.
//! - `walker`: a body this machine steers through it, its camera, controls and options.
//! - `GameplayPlugin`, here: everything a player does there, from building to sound.
//!
//! `GamePlugins` is all three, as the `play` experiment runs them; `walk` runs the first
//! two alone. A module belongs to the lowest layer that can hold it, and never needs a
//! system or resource above it to run. The player's own components (`Player`, `Motion`,
//! `Stance`, `View`, `Downed`) are the walker's, for every layer to read. See docs/player.md and docs/building.md for
//! the design.

mod archery;
mod bag;
pub(crate) mod bodies;
mod build_menu;
mod button;
mod camera;
mod catalogue;
pub(crate) mod claim;
mod combat;
mod controls;
mod crafting;
mod creatures;
mod death;
mod dial;
mod dig;
mod figure;
pub use flora::{FloraSystems, Growth, Obstacles};
pub use flora_kinds::Flora;
pub use footprint::Footprint;
mod gains;
mod harvest;
pub(crate) mod held;
mod hits;
mod intent;
pub(crate) mod interact;
pub mod inventory;
mod kinds;
mod map;
mod movement;
mod music;
mod noise;
mod noises;
mod options;
mod people;
mod pile;
mod pooling;
mod probe;
pub(crate) mod props;
mod registries;
pub use props::{Look as PropLook, glowing as prop_glowing, lit as prop_lit, looks as prop_looks};
mod save;
mod settlement;
mod skills;
mod sounds;
mod soundscape;
mod species;
mod station_screen;
mod tiles;
pub(crate) mod told;
mod tool;
mod torchlight;
mod vitals;
mod walker;
mod wear;
mod work;
mod worn;
mod zones;

use super::build::{self, DrivenByPlayer, Site};
// The world layer's modules, by their old names here, for the modules above them.
use crate::shell::Options;
use bevy::{app::PluginGroupBuilder, prelude::*, window::CursorOptions};
use options::MenuSystems;
use terra_world::{data, daylight, flora, flora_kinds, footprint, lod, models, tick, world};
pub use tool::Builder;
pub use walker::{ArriveAt, Hands, PlayerInput, WalkerPlugin, WarpTo};
// The player's own components are the walker's; every layer reads them by these names.
use walker::{Downed, IsLocalPlayer, Motion, Stance, View};
pub(crate) use walker::{LocalPlayer, Player};
pub use world::WorldPlugin;

/// The whole game, in three layers, each standing only on those before it:
///
/// - [`WorldPlugin`]: the world around a player, from the data files to what grows.
/// - [`WalkerPlugin`]: a body this machine steers through it, and the camera.
/// - [`GameplayPlugin`]: everything a player does there, from building to combat and sound.
///
/// A run can leave a layer out, `GamePlugins.build().disable::<GameplayPlugin>()`, to try
/// the ones below it alone; the `walk` experiment does.
pub struct GamePlugins;

impl PluginGroup for GamePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(WorldPlugin)
            .add(WalkerPlugin)
            .add(GameplayPlugin)
    }
}

/// The whole game: [`GamePlugins`].
pub fn plugin(app: &mut App) {
    app.add_plugins(GamePlugins);
}

/// Everything a player does in the world: building, what they carry, make and wear,
/// creatures and people, fighting, staying alive, the map, saving, the remote probe, and
/// sound. It stands on the world and the walker, and tells the walker what the player's
/// hands are on.
pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        // The kinds of thing gameplay runs on, after the world's.
        registries::plugin(app);
        // A player needs something to build on, and takes over from the build keys.
        build::plugin(app);
        build::player_plugin(app);
        torchlight::plugin(app);
        wear::plugin(app);
        pile::plugin(app);
        zones::plugin(app);
        props::plugin(app);
        tool::plugin(app);
        dial::plugin(app);
        told::plugin(app);
        bag::plugin(app);
        gains::plugin(app);
        build_menu::plugin(app);
        crafting::plugin(app);
        station_screen::plugin(app);
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
        probe::plugin(app);
        // Sound: noises from what happens, what they sound like, the place, and music.
        noise::plugin(app);
        noises::plugin(app);
        sounds::plugin(app);
        soundscape::plugin(app);
        music::plugin(app);
        app.insert_resource(DrivenByPlayer)
            .add_observer(equip)
            .add_systems(PostStartup, arrive_by_the_door.before(walker::spawn))
            .configure_sets(Update, tool::Acting.after(PlayerInput).before(MenuSystems))
            .add_systems(
                Update,
                (
                    close_screens.before(options::toggle),
                    claim_hands
                        .after(options::toggle)
                        .before(camera::capture_mouse),
                    dial::steer
                        .after(camera::capture_mouse)
                        .before(camera::zoom),
                )
                    .in_set(PlayerInput),
            )
            .add_systems(
                RunFixedMainLoop,
                claim_hands
                    .before(camera::look)
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            );
    }
}

/// Stands the player a little way out from the base's door, facing it.
fn arrive_by_the_door(mut commands: Commands, site: Res<Site>) {
    let (approach, facing) = site.approach();
    commands.insert_resource(ArriveAt {
        at: site.frame.transform_point(approach - facing * 5.0),
        toward: site.frame.transform_point(approach + facing),
    });
}

/// What a player brings to the game besides their body: where they come back to, how they
/// are, what they know, and what they carry, from the items file and, for testing,
/// `--give`.
fn equip(
    arrived: On<Add<LocalPlayer>>,
    mut commands: Commands,
    mut items: ResMut<inventory::Items>,
    options: Res<Options>,
    players: Query<(&Transform, &Motion)>,
) {
    let Ok((transform, motion)) = players.get(arrived.entity) else {
        return;
    };
    let mut body = vitals::Vitals::default();
    for (name, value) in &options.gives {
        match name.as_str() {
            "health" => body.health = *value as f32,
            "fed" => body.fed = *value as f32,
            "stamina" => body.stamina = *value as f32,
            _ => {}
        }
    }
    let mut kit = inventory::starting_kit(&mut items);
    inventory::give_for_testing(
        &mut items,
        &mut kit.0.0,
        &mut kit.1.0,
        &mut kit.2.0,
        &options.gives,
    );
    commands.entity(arrived.entity).insert((
        death::Arrival {
            at: transform.translation,
            facing: motion.facing,
        },
        body,
        skills::Skills::default(),
        kit,
    ));
}

/// Tells the walker what the screens and the builder have of the player's hands.
fn claim_hands(
    screen: Res<inventory::InventoryScreen>,
    build_menu: Res<build_menu::BuildMenu>,
    dial: Res<dial::Dial>,
    builds: Res<catalogue::Builds>,
    builder: Option<Single<&tool::Builder, IsLocalPlayer>>,
    mut hands: ResMut<Hands>,
) {
    let wheel = builder.is_some_and(|builder| builder.wants_wheel(&builds));
    hands.set_if_neq(Hands {
        busy: screen.open,
        choosing: dial.open || build_menu.open,
        pointer: screen.open || build_menu.open,
        arrows: build_menu.open,
        mouse: dial.open,
        wheel: wheel || screen.open,
    });
}

/// Esc closes the build menu, else the inventory, before it reaches the options menu,
/// and takes the mouse back.
fn close_screens(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    menu: Res<options::OptionsMenu>,
    mut inventory: ResMut<inventory::InventoryScreen>,
    mut build_menu: ResMut<build_menu::BuildMenu>,
    window: Single<(&Window, &mut CursorOptions)>,
) {
    if menu.open || !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if build_menu.open {
        build_menu.open = false;
    } else if inventory.open {
        inventory.open = false;
        inventory.at = None;
    } else {
        return;
    }
    keys.clear_just_pressed(KeyCode::Escape);
    let (window, mut cursor) = window.into_inner();
    camera::grab_mouse(&mut cursor, window.focused);
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
