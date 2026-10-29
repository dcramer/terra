//! The options menu built as the game builds it, in an app with no window or GPU: its
//! systems are made here as there, so two queries in one system that could reach the same
//! component (Bevy's B0001) fail this test, not the game on the GPU (D579's quit to title
//! was such a crash).

use super::*;
use bevy::input::mouse::AccumulatedMouseScroll;
use terra_world::quality::{Asked, Forced};

/// The game's menu: its plugin and Esc, with what they read from the rest of the game.
fn menu_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // Scripted, so the developer's own settings are neither read nor written.
    app.insert_resource(Options::from_args(["--bench".to_string()].into_iter()).unwrap())
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseScroll>()
        .init_resource::<ScreenRate>()
        .init_resource::<Asked>()
        .insert_resource(Forced(false))
        .init_resource::<FrameLimit>()
        .add_plugins(plugin)
        .add_systems(Update, toggle.before(MenuSystems));
    app.world_mut()
        .spawn((Window::default(), PrimaryWindow, CursorOptions::default()));
    app.world_mut()
        .spawn((MainCamera, Projection::Perspective(default())));
    app
}

/// The menu's screen, and the quit to title button's own visibility.
fn shown(app: &mut App) -> (Visibility, Visibility) {
    let world = app.world_mut();
    let screen = *world
        .query_filtered::<&Visibility, With<Screen>>()
        .single(world)
        .unwrap();
    let title = world
        .query::<(&Control, &Visibility)>()
        .iter(world)
        .find(|(control, _)| **control == Control::Title)
        .map(|(_, visible)| *visible)
        .unwrap();
    (screen, title)
}

/// A frame with Esc pressed, and let go after it (there is no input plugin here to clear it).
fn press_esc(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release_all();
    keys.clear();
}

#[test]
fn esc_opens_the_menu_and_quit_to_title_shows_only_when_the_game_offers_it() {
    let mut app = menu_app();
    app.update();
    assert_eq!(shown(&mut app).0, Visibility::Hidden, "closed at the start");

    press_esc(&mut app);
    assert!(app.world().resource::<OptionsMenu>().open);
    assert_eq!(
        shown(&mut app),
        (Visibility::Visible, Visibility::Hidden),
        "open, and no quit to title where the game does not offer it"
    );

    app.world_mut().resource_mut::<OptionsMenu>().offers_title = true;
    app.update();
    assert_eq!(
        shown(&mut app).1,
        Visibility::Inherited,
        "offered, it shows"
    );

    press_esc(&mut app);
    assert!(!app.world().resource::<OptionsMenu>().open);
    assert_eq!(shown(&mut app).0, Visibility::Hidden, "Esc closes it again");
}
