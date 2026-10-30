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

pub use flora::{FloraSystems, Growth, Obstacles};
pub use flora_kinds::Flora;
pub use footprint::Footprint;
mod map;
pub mod music;
mod noises;
mod probe;
pub use props::{
    Look as PropLook, Making as PropMaking, glowing as prop_glowing, lit as prop_lit,
    looks as prop_looks,
};
mod save;
mod sounds;
mod soundscape;
mod title;
pub mod worlds;

// Building play, its own crate since D407, by its old names here.
use terra_building::{build_menu, dial, tool};
// Creatures and fighting, their own crate since D407, by their old names here.
use terra_creatures::{
    archery, coats, combat, creatures, fliers, hits, melee, sighted, species, staged, wildlife,
    zones,
};
// Townsfolk, their own crate since D407, by their old names here.
pub use terra_folk::town;
use terra_folk::{people, pooling, town_animals, town_sound, townsfolk};
pub(crate) use terra_folk::{rising, settlement};
// What a player carries, makes and needs, its own crate since D407, by its old names here.
use super::build::{self, DrivenByPlayer, Site};
#[cfg(test)]
use terra_survival::test_world;
use terra_survival::{
    bag, breath, crafting, death, dig, gains, harvest, noise, pile, registries, skills,
    station_screen, storage_screen, torchlight, vitals, warmth, wear, worn,
};
pub(crate) use terra_survival::{catalogue, claim, held, interact, props, storage, told};
pub use terra_survival::{contract, inventory};
// The walker layer (D198), its own crate since D407, by its old names here.
pub(crate) use terra_walker::bodies;
use terra_walker::{button, camera, controls, options, shake, walker};
// The world layer's modules, by their old names here, for the modules above them.
use crate::shell::Options;
use bevy::{app::PluginGroupBuilder, prelude::*, window::CursorOptions};
use options::MenuSystems;
use terra_world::instance::{Building, InWorld};
use terra_world::{daylight, flora, flora_kinds, footprint, lod, world};
pub use tool::Builder;
pub use walker::{ArriveAt, Hands, PlayerInput, Stamina, WalkerPlugin, WarpTo};
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
        build::building::plugin(app);
        build::building::player_plugin(app);
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
        storage::plugin(app);
        storage_screen::plugin(app);
        interact::plugin(app);
        claim::plugin(app);
        vitals::plugin(app);
        breath::plugin(app);
        warmth::plugin(app);
        death::plugin(app);
        skills::plugin(app);
        creatures::plugin(app);
        wildlife::plugin(app);
        sighted::plugin(app);
        coats::plugin(app);
        fliers::plugin(app);
        staged::plugin(app);
        melee::plugin(app);
        bodies::plugin(app);
        combat::plugin(app);
        shake::plugin(app);
        hits::plugin(app);
        archery::plugin(app);
        people::plugin(app);
        pooling::plugin(app);
        settlement::plugin(app);
        town::plugin(app);
        rising::plugin(app);
        townsfolk::plugin(app);
        town_sound::plugin(app);
        town_animals::plugin(app);
        map::plugin(app);
        held::plugin(app);
        worn::plugin(app);
        save::plugin(app);
        title::plugin(app);
        // A crash capsule says which save the run keeps (terra_world::crash).
        app.add_systems(PostStartup, |path: Option<Res<save::SavePath>>| {
            if let Some(path) = path {
                terra_world::crash::note("save", path.0.display());
            }
        });
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
            .add_systems(
                OnEnter(InWorld),
                arrive_by_the_door
                    .run_if(not(resource_exists::<ArriveAt>))
                    .in_set(Building::Arrive)
                    .before(walker::spawn),
            )
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

/// Stands the player a little way out from the base's door, facing it, unless a scene has
/// already said where they arrive (a sandbox's dungeon, at its entrance stair).
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
    let mut stamina = Stamina::default();
    for (name, value) in &options.gives {
        match name.as_str() {
            "health" => body.health = *value as f32,
            "fed" => body.fed = *value as f32,
            "stamina" => stamina.left = *value as f32,
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
        stamina,
        warmth::BodyHeat::default(),
        skills::Skills::default(),
        kit,
    ));
}

/// Tells the walker what the screens and the builder have of the player's hands. The title
/// screen has all of them.
fn claim_hands(
    screen: Res<inventory::InventoryScreen>,
    build_menu: Res<build_menu::BuildMenu>,
    dial: Res<dial::Dial>,
    builds: Res<catalogue::Builds>,
    builder: Option<Single<&tool::Builder, IsLocalPlayer>>,
    mut hands: ResMut<Hands>,
    shown: Option<Res<State<terra_world::loading::Screen>>>,
) {
    let wheel = builder.is_some_and(|builder| builder.wants_wheel(&builds));
    let title = shown.is_some_and(|shown| title::up(&shown));
    hands.set_if_neq(Hands {
        busy: screen.open || title,
        choosing: dial.open || build_menu.open,
        pointer: screen.open || build_menu.open || title,
        arrows: build_menu.open || title,
        mouse: dial.open || title,
        wheel: wheel || screen.open || title,
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn the_hands_are_choosing_while_the_dial_or_the_build_menu_is_open() {
        let mut world = test_world();
        let builds = {
            let items = world.resource::<inventory::Items>();
            catalogue::Builds::read(items, &crafting::Stations::read())
        };
        world.insert_resource(builds);
        world.init_resource::<inventory::InventoryScreen>();
        world.init_resource::<build_menu::BuildMenu>();
        world.init_resource::<dial::Dial>();
        world.init_resource::<Hands>();
        for (dial_open, menu_open) in [(false, false), (true, false), (false, true), (true, true)] {
            world.resource_mut::<dial::Dial>().open = dial_open;
            world.resource_mut::<build_menu::BuildMenu>().open = menu_open;
            world.run_system_once(claim_hands).unwrap();
            let choosing = world.resource::<Hands>().choosing;
            assert_eq!(
                choosing,
                dial_open || menu_open,
                "dial {dial_open}, menu {menu_open}"
            );
        }
    }
}
