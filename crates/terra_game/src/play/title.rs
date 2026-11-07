//! The title screen (D579): what the game opens on when asked (`--title`, which
//! `cargo xtask play` passes when nothing was chosen in a terminal), over the world it has
//! just loaded, the clock held: Continue, Worlds, New world, The yard, Settings and Quit, as
//! Minecraft, Valheim and Enshrouded open. Mouse and keyboard; the game has no gamepad yet.
//!
//!   Up, Down, W, S   pick           Enter, Space   choose
//!   Esc              back           Delete         delete the picked world, after asking
//!   Left, Right      the new world's mode, Survival or Creative; typing names it
//!   Enter on Seed    rolls the new world's seed again; typing digits gives one's own (D596)
//!
//! A world's save is read once it is chosen (`save`), and the game loads again round where
//! its player stands. The yard is another level, so the game starts again on it. The
//! choices' logic is in `menu`, tested without a screen.

mod menu;
mod view;

pub use leaving::Leaving;
mod character;
mod joining;
mod leaving;
mod profile;
mod remote;

use super::daylight::Clock;
use super::options::OptionsMenu;
use super::worlds::{self, Root};
use crate::shell::Options;
use bevy::{
    input::{ButtonState, keyboard::Key, keyboard::KeyboardInput},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use menu::{Act, Item, Menu};
use terra_world::choice::Choice;
use terra_world::instance::{Cross, WorldSpec};
use terra_world::loading::{Screen, Then};
use terra_world::planet::Planet;

/// The title screen's state while it is up.
#[derive(Resource)]
struct Title(Menu);

/// What was chosen this frame, by key or click, for `act` to do.
#[derive(Resource, Default)]
struct Chosen(Option<Act>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Chosen>()
        .add_systems(PostStartup, open.run_if(|choice: Res<Choice>| choice.title))
        .add_systems(OnEnter(Screen::Title), (view::show, hold))
        .add_systems(OnExit(Screen::Title), (view::hide, release))
        .add_systems(OnEnter(Screen::Playing), offer)
        .add_plugins((
            leaving::plugin,
            joining::plugin,
            character::plugin,
            remote::plugin,
        ))
        .add_systems(
            Update,
            quit_to_title
                .after(super::options::MenuSystems)
                .run_if(in_state(Screen::Playing)),
        )
        .add_systems(
            PreUpdate,
            keys.after(crate::shell::scripted_presses)
                .run_if(in_state(Screen::Title)),
        )
        .add_systems(
            Update,
            (view::point, act, view::draw)
                .chain()
                .run_if(in_state(Screen::Title)),
        );
}

/// Whether the title screen is up, for what reads the player's hands (`claim_hands`).
pub fn up(screen: &State<Screen>) -> bool {
    *screen.get() == Screen::Title
}

/// Asked for: the first load ends on the title screen, with the worlds there are.
/// New world's seeds are rolled from the dice the land behind was chosen with
/// (`worlds::Dice`), so with no world kept the first it offers is the land already loaded.
fn open(mut commands: Commands, root: Option<Res<Root>>, dice: Option<Res<worlds::Dice>>) {
    let worlds = root
        .as_deref()
        .map_or_else(Vec::new, |root| worlds::list(&root.0));
    commands.insert_resource(Then(Screen::Title));
    let profile = profile::Profile::load(root.as_deref().map(|root| root.0.as_path()));
    let mut menu = Menu::new(worlds);
    menu.dice = dice.map_or_else(worlds::Dice::from_clock, |dice| *dice).0;
    (menu.player, menu.address) = (profile.name.clone(), profile.address.clone());
    menu.features = profile.features.clone();
    commands.insert_resource(profile);
    commands.insert_resource(Title(menu));
}

/// Where the title screen's camera stands and what it looks at, in metres east, up and
/// south of the start, its heights above the ground under each (`--vantage-ground`'s terms):
/// out over the start's lake to its wooded shore, as Valheim and Enshrouded open on a still
/// view of their world rather than over the player's shoulder.
const VISTA: (Vec3, Vec3) = (Vec3::new(0.0, 8.0, 0.0), Vec3::new(-4.0, 10.0, 90.0));

/// The hour the title screen shows, late afternoon, the shore lit warm across the water; a
/// world chosen gets its own hour back.
const HOUR: f32 = 17.0;

/// The camera and hour the run had before the title screen took them, given back when it
/// goes.
#[derive(Resource)]
struct Before {
    vantage: Option<(Vec3, Vec3)>,
    ground: bool,
    hour: f32,
}

/// Nothing happens in the world while it waits behind the screen, and a fixed camera looks
/// over it at its hour, which also puts the game's interface away
/// (`Options::shows_interface`). A run given its own fixed camera or hour keeps it.
fn hold(
    mut commands: Commands,
    mut time: ResMut<Time<Virtual>>,
    mut options: ResMut<Options>,
    mut clock: ResMut<Clock>,
) {
    time.pause();
    commands.insert_resource(Before {
        vantage: options.vantage,
        ground: options.vantage_ground,
        hour: clock.hour,
    });
    if options.vantage.is_none() {
        options.vantage = Some(VISTA);
        options.vantage_ground = true;
    }
    if options.time.is_none() {
        clock.hour = HOUR;
    }
}

/// The camera goes back to following the player, and the world to its hour.
fn release(
    mut commands: Commands,
    before: Option<Res<Before>>,
    mut options: ResMut<Options>,
    clock: Option<ResMut<Clock>>,
) {
    if let Some(before) = before {
        options.vantage = before.vantage;
        options.vantage_ground = before.ground;
        // A world crossed to from the screen has its own clock, made as it is entered.
        if let Some(mut clock) = clock {
            clock.hour = before.hour;
        }
        commands.remove_resource::<Before>();
    }
}

/// Keys move about the title screen and choose; then the game sees none of them, unless the
/// options menu (Settings) is open over it, which reads its own.
#[allow(clippy::too_many_arguments)]
fn keys(
    mut title: ResMut<Title>,
    mut chosen: ResMut<Chosen>,
    settings: Res<OptionsMenu>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut typed: MessageReader<KeyboardInput>,
    mut cursor: Single<&mut CursorOptions>,
) {
    // The pointer is the screen's, even after Settings closes and takes it.
    if cursor.grab_mode != CursorGrabMode::None || !cursor.visible {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
    if settings.open {
        typed.clear();
        return;
    }
    let menu = &mut title.0;
    // Where letters are text (a name, an address), W, S and Space are not moves.
    let (typing, naming) = menu.typing();
    for input in typed.read() {
        if let (true, ButtonState::Pressed, Key::Character(text)) =
            (typing, input.state, &input.logical_key)
        {
            menu.type_text(text);
        }
    }
    let pressed = |codes: &[KeyCode]| keys.any_just_pressed(codes.iter().copied());
    if pressed(&[KeyCode::ArrowUp]) || (!naming && pressed(&[KeyCode::KeyW])) {
        menu.step(false);
    } else if pressed(&[KeyCode::ArrowDown]) || (!naming && pressed(&[KeyCode::KeyS])) {
        menu.step(true);
    } else if pressed(&[KeyCode::Enter, KeyCode::NumpadEnter])
        || (!naming && pressed(&[KeyCode::Space]))
    {
        chosen.0 = menu.choose();
    } else if pressed(&[KeyCode::Escape]) {
        menu.back();
    } else if pressed(&[KeyCode::Backspace]) {
        menu.erase();
    } else if pressed(&[KeyCode::ArrowLeft, KeyCode::ArrowRight])
        && menu.picked() == Some(Item::Mode)
    {
        menu.turn();
    } else if let (Some(Item::Feature(feature)), true) = (
        menu.picked(),
        pressed(&[KeyCode::ArrowLeft, KeyCode::ArrowRight]),
    ) {
        (menu.features).turn(feature, pressed(&[KeyCode::ArrowRight]));
    } else if pressed(&[KeyCode::Delete])
        && let Some(Item::World(at)) = menu.picked()
    {
        menu.ask_delete(at);
    }
    keys.reset_all();
    mouse.reset_all();
}

/// Plays the land of a seed, of the size the options now say. The land behind the screen, if
/// it is that land, is played as it stands; another is crossed to first, under the loading
/// screen, as a portal crosses (D596), with nobody standing on the land left (`nobody`).
fn go(world: &mut World, seed: u64) {
    world.insert_resource(Then(Screen::Playing));
    let radius_m = world.resource::<Options>().radius_m;
    let resized = world
        .get_resource::<Planet>()
        .is_some_and(|planet| planet.radius_m() != radius_m);
    match world.get_resource::<WorldSpec>().cloned() {
        Some(loaded) if loaded.seed != seed || resized => {
            info!(
                "Crossing from the land of seed {} to seed {seed}, {} km round.",
                loaded.seed,
                radius_m / 1000.0
            );
            leaving::nobody(world);
            Command::apply(Cross(WorldSpec { seed, ..loaded }), world);
        }
        _ => world
            .resource_mut::<NextState<Screen>>()
            .set(Screen::Loading),
    }
}

/// Does what was chosen.
fn act(
    mut commands: Commands,
    mut chosen: ResMut<Chosen>,
    mut title: ResMut<Title>,
    mut options: ResMut<Options>,
    root: Option<Res<Root>>,
    mut settings: ResMut<OptionsMenu>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(act) = chosen.0.take() else {
        return;
    };
    let play = |commands: &mut Commands, options: &mut Options, world: Option<_>, seed| {
        options.seed = Some(seed);
        match world {
            Some(world) => worlds::play(commands, options, world),
            None => options.radius_m = worlds::new_radius_m(options),
        }
        commands.queue(move |world: &mut World| go(world, seed));
    };
    match act {
        Act::Play(mut world) => {
            if let Err(problem) = world.played(worlds::now()) {
                warn!("Could not mark {} played: {problem}", world.meta.name);
            }
            let seed = world.meta.seed;
            play(&mut commands, &mut options, Some(world), seed);
        }
        Act::Host(mut world) => {
            if let Err(problem) = world.played(worlds::now()) {
                warn!("Could not mark {} played: {problem}", world.meta.name);
            }
            let seed = world.meta.seed;
            commands.queue(joining::remember);
            commands.insert_resource(joining::Hosting);
            play(&mut commands, &mut options, Some(world), seed);
        }
        Act::Join { address } => {
            commands.queue(move |world: &mut World| joining::begin(world, &address));
        }
        Act::Cancel => commands.queue(joining::cancel),
        Act::Make {
            name,
            creative,
            seed,
        } => {
            // A run keeping no worlds (a script's, without --worlds) plays one unsaved.
            let radius_m = worlds::new_radius_m(&options);
            let made = root
                .map(|root| worlds::make(&root.0, &name, creative, seed, radius_m, worlds::now()));
            match made {
                Some(Err(problem)) => warn!("Could not make the world {name}: {problem}"),
                Some(Ok(world)) => play(&mut commands, &mut options, Some(world), seed),
                None => {
                    options.free_build |= creative;
                    play(&mut commands, &mut options, None, seed);
                }
            }
        }
        Act::Delete(world) => {
            match world.delete() {
                Ok(()) => info!("Deleted the world {}.", world.meta.name),
                Err(problem) => warn!("Could not delete {}: {problem}", world.meta.name),
            }
            let left = root.map_or_else(Vec::new, |root| worlds::list(&root.0));
            title.0.deleted(left);
        }
        Act::Yard => {
            info!("{}", again("the yard", AGAIN_ON_YARD));
            exit.write(AppExit::from_code(AGAIN_ON_YARD));
        }
        Act::Settings => settings.open = true,
        Act::Quit => {
            exit.write(AppExit::Success);
        }
    }
}

/// The exit code asking `cargo xtask play` to start the game again on the yard (its
/// launcher's `again` reads the same number). The yard is another level, made as the game
/// starts, so it needs a fresh game; the launcher starts it in the same terminal, with the
/// same build and profiling, so one session (and its reports) runs on (D579). A world is
/// left for the title screen in place (`leaving`).
pub const AGAIN_ON_YARD: u8 = 76;

/// What the log says as the game ends to start again.
fn again(on: &str, code: u8) -> String {
    format!(
        "Ending to start again on {on} (exit code {code}): `cargo xtask play` starts it; a game started otherwise just ends (D579)."
    )
}

/// While a world is played, the options menu offers to quit to the title screen.
fn offer(mut menu: ResMut<OptionsMenu>) {
    menu.offers_title = true;
}

/// Quit to title: the world is left for the title screen (`leaving`), saved as it goes.
fn quit_to_title(mut commands: Commands, mut menu: ResMut<OptionsMenu>) {
    if std::mem::take(&mut menu.quit_to_title) {
        menu.open = false;
        commands.insert_resource(Leaving::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quit_to_title_asks_once_to_leave_the_world_and_closes_the_menu() {
        let mut world = World::new();
        let mut menu = OptionsMenu::default();
        (menu.open, menu.offers_title, menu.quit_to_title) = (true, true, true);
        world.insert_resource(menu);
        world.run_system_cached(quit_to_title).unwrap();
        assert!(world.contains_resource::<Leaving>());
        let menu = world.resource::<OptionsMenu>();
        assert!(!menu.quit_to_title && !menu.open);
        world.remove_resource::<Leaving>();
        world.run_system_cached(quit_to_title).unwrap();
        assert!(!world.contains_resource::<Leaving>(), "asked once");
    }
}
