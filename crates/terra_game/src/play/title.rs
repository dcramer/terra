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
        .add_systems(OnExit(Screen::Title), view::hide)
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

/// Nothing happens in the world while it waits behind the screen.
fn hold(mut time: ResMut<Time<Virtual>>) {
    time.pause();
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
            yard();
            exit.write(AppExit::Success);
        }
        Act::Settings => settings.open = true,
        Act::Quit => {
            exit.write(AppExit::Success);
        }
    }
}

/// The yard is another level, made when the game starts, so the game starts again on it,
/// with the same command line but for the title screen.
fn yard() {
    let args = std::env::args().skip(1).filter(|arg| arg != "--title");
    let started = std::env::current_exe().and_then(|exe| {
        std::process::Command::new(exe)
            .args(args)
            .args(["--level", "yard"])
            .spawn()
    });
    if let Err(problem) = started {
        warn!("Could not start the yard: {problem}");
    }
}
