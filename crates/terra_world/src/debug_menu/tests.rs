//! The menu drawn by egui alone, without a GPU or a window: the frames a person's pointer
//! makes are given to it as egui's own input, and what it set is read from the tunables.

use super::tunables::{Drawn, TunablesPage};
use super::*;
use bevy_inspector_egui::bevy_egui::egui::{
    Event, PointerButton, Pos2, RawInput, Rect, pos2, vec2,
};
use terra_gen::tune::{self, SetBy, Tunable};

// Each test its own group: the tunables are the process's, shared with tests on other threads.
static CLICK: Tunable = Tunable::new(
    "test.menu_click.level",
    2.0,
    0.0..=8.0,
    "for the tests: a level",
);
static SEEN: Tunable = Tunable::new(
    "test.menu_seen.shade",
    0.5,
    0.0..=1.0,
    "for the tests: a shade",
);
static GROUP_A: Tunable = Tunable::new("test.menu_reset.a", 1.0, 0.0..=4.0, "for the tests");
static GROUP_B: Tunable = Tunable::new("test.menu_reset.b", 3.0, 0.0..=4.0, "for the tests");
static FOUND: Tunable = Tunable::new(
    "test.menu_find.width",
    1.0,
    0.0..=2.0,
    "how wide the ribbon is",
);

/// A world with the menu open on its tunables page, searched for `search`.
fn opened(search: &str) -> (World, egui::Context) {
    let mut world = World::new();
    world.insert_resource(TunablesPage {
        search: search.into(),
        ..default()
    });
    let mut menu = DebugMenu::with_tunables();
    menu.open = true;
    world.insert_resource(menu);
    let ctx = egui::Context::default();
    // A window's first frame only measures it (egui's sizing pass); it is laid out after.
    frame(&mut world, &ctx, vec![]);
    (world, ctx)
}

/// One frame of the menu, given these events; what the page drew.
fn frame(world: &mut World, ctx: &egui::Context, events: Vec<Event>) -> Drawn {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0))),
        events,
        ..default()
    };
    // Nothing is painted, so the font atlas egui made is let go unapplied.
    ctx.run_ui(input, |ui| {
        show(ui.ctx(), world);
    })
    .drop_without_applying_deltas();
    world.resource::<TunablesPage>().drawn.clone()
}

/// A press and a let-go at a spot, over two frames, as a click is.
fn click(world: &mut World, ctx: &egui::Context, at: Pos2) -> Drawn {
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(world, ctx, vec![Event::PointerMoved(at), button(true)]);
    frame(world, ctx, vec![button(false)]);
    frame(world, ctx, vec![])
}

fn row<'a>(drawn: &'a Drawn, name: &str) -> &'a tunables::TunableDrawn {
    drawn
        .tunables
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("{name} is on the page"))
}

#[test]
fn a_click_on_a_slider_sets_its_tunable_as_the_remote_would_read_it() {
    tune::register(&[&CLICK]);
    let (mut world, ctx) = opened("test.menu_click");
    let drawn = frame(&mut world, &ctx, vec![]);
    let slider = row(&drawn, CLICK.name);
    let [least, most] = slider.track;
    let middle = slider.slider.0[1] + slider.slider.0[3] / 2.0;
    // Three quarters of the way along its track: three quarters of its range, 0 to 8.
    let drawn = click(
        &mut world,
        &ctx,
        pos2(least + 0.75 * (most - least), middle),
    );
    assert!((CLICK.get() - 6.0).abs() < 0.05, "set to {}", CLICK.get());
    assert_eq!(CLICK.set_by(), SetBy::Menu);
    assert_eq!(row(&drawn, CLICK.name).set_by, "debug menu");
    // Its reset puts it back, set by its default again.
    let reset = row(&drawn, CLICK.name).reset.0;
    click(
        &mut world,
        &ctx,
        pos2(reset[0] + reset[2] / 2.0, reset[1] + reset[3] / 2.0),
    );
    assert_eq!(CLICK.get(), CLICK.default);
    assert_eq!(CLICK.set_by(), SetBy::Default);
}

#[test]
fn what_the_remote_set_is_shown_on_the_next_frame() {
    tune::register(&[&SEEN]);
    let (mut world, ctx) = opened("test.menu_seen");
    frame(&mut world, &ctx, vec![]);
    crate::tune::set_logged(&SEEN, Some(0.125), SetBy::Remote).unwrap();
    let drawn = frame(&mut world, &ctx, vec![]);
    assert_eq!(row(&drawn, SEEN.name).value, 0.125);
    assert_eq!(row(&drawn, SEEN.name).set_by, "remote");
    SEEN.reset();
}

#[test]
fn a_groups_reset_puts_back_every_one_in_it() {
    tune::register(&[&GROUP_A, &GROUP_B]);
    GROUP_A.set(0.5, SetBy::Remote).unwrap();
    GROUP_B.set(0.5, SetBy::CommandLine).unwrap();
    let (mut world, ctx) = opened("test.menu_reset");
    let drawn = frame(&mut world, &ctx, vec![]);
    let group = drawn
        .groups
        .iter()
        .find(|g| g.prefix == "test.menu_reset")
        .expect("its group is shown");
    assert_eq!(group.shown, 2);
    let r = group.reset.0;
    click(&mut world, &ctx, pos2(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0));
    assert_eq!((GROUP_A.get(), GROUP_B.get()), (1.0, 3.0));
}

#[test]
fn the_search_looks_in_names_and_help_and_says_when_nothing_matches() {
    tune::register(&[&FOUND, &CLICK]);
    let (mut world, ctx) = opened("ribbon wide");
    let drawn = frame(&mut world, &ctx, vec![]);
    let names: Vec<_> = drawn.tunables.iter().map(|t| t.name).collect();
    assert_eq!(names, [FOUND.name], "the help's words find it");
    assert!(drawn.note.is_none());
    world.resource_mut::<TunablesPage>().search = "no.such.thing".into();
    let drawn = frame(&mut world, &ctx, vec![]);
    assert!(drawn.tunables.is_empty());
    let note = drawn.note.expect("it says why the list is empty");
    assert!(
        note.contains("no.such.thing") && note.contains("Clear the search"),
        "{note}"
    );
}

#[test]
fn tunables_are_grouped_by_every_word_but_their_last() {
    assert_eq!(tunables::group("sky.cloud.heaped_cover"), "sky.cloud");
    tune::register(&[&GROUP_A, &GROUP_B]);
    let groups = tunables::grouped("test.menu_reset");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].0, "test.menu_reset");
    assert_eq!(groups[0].1.len(), 2);
}
