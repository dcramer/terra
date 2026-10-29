//! The title screen (D579): what the game opens on when asked (`--title`, which
//! `cargo xtask play` passes when nothing was chosen in a terminal), over the world it has
//! just loaded, the clock held: Continue, Worlds, New world, The yard, Settings and Quit, as
//! Minecraft, Valheim and Enshrouded open. Mouse and keyboard; the game has no gamepad yet.
//!
//!   Up, Down, W, S   pick           Enter, Space   choose
//!   Esc              back           Delete         delete the picked world, after asking
//!   Left, Right      the new world's mode, Survival or Creative; typing names it
//!
//! A world's save is read once it is chosen (`save`), and the game loads again round where
//! its player stands. The yard is another level, so the game starts again on it. The
//! choices' logic is in `menu`, tested without a screen.

mod menu;
mod view;

use super::daylight::Clock;
use super::options::OptionsMenu;
use super::worlds::{self, Root};
use crate::shell::Options;
use bevy::{
    input::{ButtonState, keyboard::Key, keyboard::KeyboardInput},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use menu::{Act, Item, Menu, Page};
use terra_world::choice::Choice;
use terra_world::loading::{Screen, Then};

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
fn open(mut commands: Commands, root: Option<Res<Root>>) {
    let worlds = root.map_or_else(Vec::new, |root| worlds::list(&root.0));
    commands.insert_resource(Then(Screen::Title));
    commands.insert_resource(Title(Menu::new(worlds)));
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
    mut clock: ResMut<Clock>,
) {
    if let Some(before) = before {
        options.vantage = before.vantage;
        options.vantage_ground = before.ground;
        clock.hour = before.hour;
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
    let naming = menu.page == Page::New && menu.picked() == Some(Item::Name);
    for input in typed.read() {
        if let (true, ButtonState::Pressed, Key::Character(text)) =
            (naming, input.state, &input.logical_key)
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
    } else if pressed(&[KeyCode::Delete])
        && let Some(Item::World(at)) = menu.picked()
    {
        menu.ask_delete(at);
    }
    keys.reset_all();
    mouse.reset_all();
}

/// Does what was chosen.
#[allow(clippy::too_many_arguments)]
fn act(
    mut commands: Commands,
    mut chosen: ResMut<Chosen>,
    mut title: ResMut<Title>,
    mut options: ResMut<Options>,
    root: Option<Res<Root>>,
    mut next: ResMut<NextState<Screen>>,
    mut settings: ResMut<OptionsMenu>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(act) = chosen.0.take() else {
        return;
    };
    let mut play = |commands: &mut Commands, options: &mut Options, world: Option<_>| {
        if let Some(world) = world {
            worlds::play(commands, options, world);
        }
        commands.insert_resource(Then(Screen::Playing));
        next.set(Screen::Loading);
    };
    match act {
        Act::Play(mut world) => {
            if let Err(problem) = world.played(worlds::now()) {
                warn!("Could not mark {} played: {problem}", world.meta.name);
            }
            play(&mut commands, &mut options, Some(world));
        }
        Act::Make { name, creative } => {
            // A run keeping no worlds (a script's, without --worlds) plays one unsaved.
            let made = root.map(|root| worlds::make(&root.0, &name, creative, worlds::now()));
            match made {
                Some(Err(problem)) => warn!("Could not make the world {name}: {problem}"),
                Some(Ok(world)) => play(&mut commands, &mut options, Some(world)),
                None => {
                    options.free_build |= creative;
                    play(&mut commands, &mut options, None);
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

/// The exit codes asking `cargo xtask play` to start the game again (its launcher's `again`
/// reads the same numbers), on the title screen or on the yard. The yard is another level,
/// made as the game starts, and a world played cannot yet be emptied in place for another,
/// so each needs a fresh game; the launcher starts it in the same terminal, with the same
/// build and profiling, so one session (and its reports) runs on (D579).
pub const AGAIN_ON_TITLE: u8 = 75;
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

/// Quit to title: the game is saved as it ends (`save`'s `save_on_exit`) and starts again on
/// the title screen.
fn quit_to_title(mut menu: ResMut<OptionsMenu>, mut exit: MessageWriter<AppExit>) {
    if std::mem::take(&mut menu.quit_to_title) {
        info!("{}", again("the title screen", AGAIN_ON_TITLE));
        exit.write(AppExit::from_code(AGAIN_ON_TITLE));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the game does in a frame where quit to title was or was not chosen: how it
    /// ends, and whether the request is still there.
    fn quitting(asked: bool) -> (Vec<AppExit>, bool) {
        let mut world = World::new();
        world.init_resource::<Messages<AppExit>>();
        let mut menu = OptionsMenu::default();
        menu.offers_title = true;
        menu.quit_to_title = asked;
        world.insert_resource(menu);
        world.run_system_cached(quit_to_title).unwrap();
        let exits = world.resource_mut::<Messages<AppExit>>().drain().collect();
        (exits, world.resource::<OptionsMenu>().quit_to_title)
    }

    #[test]
    fn quit_to_title_ends_the_game_asking_to_start_again_on_the_title_screen() {
        let (exits, still_asked) = quitting(true);
        assert_eq!(exits, vec![AppExit::from_code(AGAIN_ON_TITLE)]);
        assert!(!still_asked, "the request is taken, so it is done once");
        assert_eq!(quitting(false).0, vec![]);
    }
}
