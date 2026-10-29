//! How the title screen looks: the world behind it, darkened on the left under a column
//! of choices, in the loading screen's lettering and colours (D135), and its clicks.

use super::super::button::{button, pointer_on};
use super::super::worlds;
use super::menu::{Item, Menu, Page};
use super::{Chosen, Title};
use bevy::{picking::hover::Hovered, prelude::*, text::LetterSpacing, ui::Pressed};

const TITLE: Color = Color::srgb(0.88, 0.86, 0.80);
const QUIET: Color = Color::srgba(0.88, 0.86, 0.80, 0.6);
const GREYED: Color = Color::srgba(0.88, 0.86, 0.80, 0.3);
const SHADE: Color = Color::srgba(0.02, 0.025, 0.03, 0.86);
const PLAIN: Color = Color::srgba(1.0, 1.0, 1.0, 0.0);
const PICKED: Color = Color::srgba(0.85, 0.62, 0.32, 0.30);
const EDGE: Color = Color::srgb(0.85, 0.62, 0.32);
const WIDTH: f32 = 380.0;

/// The screen's root.
#[derive(Component)]
pub(super) struct Root;

/// Where the page's choices are drawn.
#[derive(Component)]
pub(super) struct Column;

/// A choice, by its place on the page.
#[derive(Component, Clone, Copy)]
pub(super) struct Pick(usize);

/// A world's delete button, by its place in the list.
#[derive(Component, Clone, Copy)]
pub(super) struct Cross(usize);

/// The view darkened at its edges and deeply down its left, under the choices, as
/// Enshrouded's and Valheim's menus lie over their worlds: the choices read over the
/// brightest ground, and the world shows clear on the right.
fn shading() -> BackgroundGradient {
    let shade = |alpha: f32| SHADE.with_alpha(alpha);
    BackgroundGradient(vec![
        RadialGradient::new(
            UiPosition::CENTER,
            RadialGradientShape::FarthestCorner,
            vec![
                ColorStop::new(shade(0.0), percent(50)),
                ColorStop::new(shade(0.55), percent(100)),
            ],
        )
        .into(),
        LinearGradient::to_right(vec![
            ColorStop::new(shade(0.88), percent(0)),
            ColorStop::new(shade(0.80), percent(22)),
            ColorStop::new(shade(0.45), percent(36)),
            ColorStop::new(shade(0.0), percent(56)),
        ])
        .into(),
    ])
}

/// Letters lifted off the world behind by a soft dark shadow.
fn lifted() -> TextShadow {
    TextShadow {
        offset: Vec2::new(0.0, 2.0),
        color: Color::srgba(0.0, 0.0, 0.0, 0.7),
    }
}

pub(super) fn show(mut commands: Commands) {
    commands
        .spawn((
            Root,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            shading(),
            // Over the game's screens, under the options menu (1000) for Settings.
            GlobalZIndex(900),
        ))
        .with_children(|root| {
            root.spawn((Node {
                width: px(WIDTH + 260.0),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect::new(px(96), px(0), percent(14), px(48)),
                ..default()
            },))
                .with_children(|side| {
                    side.spawn((
                        Text::new("TERRA"),
                        TextFont::from_font_size(72.0),
                        LetterSpacing::Px(22.0),
                        TextColor(TITLE),
                        TextShadow {
                            offset: Vec2::new(0.0, 3.0),
                            color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                        },
                        Node {
                            margin: UiRect::bottom(px(56)),
                            ..default()
                        },
                    ));
                    side.spawn((
                        Column,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(6),
                            flex_grow: 1.0,
                            ..default()
                        },
                    ));
                    side.spawn((
                        Text::new("Up and down to pick, Enter to choose, Esc to go back"),
                        TextFont::from_font_size(14.0),
                        TextColor(QUIET),
                        lifted(),
                    ));
                });
        });
}

pub(super) fn hide(mut commands: Commands, root: Query<Entity, With<Root>>) {
    for entity in &root {
        commands.entity(entity).despawn();
    }
}

/// The pointer on a choice picks it; a click chooses it, and a click on a world's Delete
/// asks before deleting it.
pub(super) fn point(
    mut title: ResMut<Title>,
    mut chosen: ResMut<Chosen>,
    picks: Query<(&Pick, &Hovered, Has<Pressed>)>,
    clicked: Query<&Pick, Added<Pressed>>,
    crosses: Query<&Cross, Added<Pressed>>,
) {
    let menu = &mut title.0;
    let items = menu.items();
    for (&Pick(at), hovered, pressed) in &picks {
        let usable = items.get(at).is_some_and(|&item| menu.enabled(item));
        if usable && pointer_on(hovered, pressed) && menu.at != at {
            menu.at = at;
        }
    }
    if let Some(&Cross(at)) = crosses.iter().next() {
        menu.ask_delete(at);
    } else if let Some(&Pick(at)) = clicked.iter().next() {
        menu.at = at;
        chosen.0 = menu.choose();
    }
}

/// What a drawn page shows, so it is drawn again only when that changes.
#[derive(PartialEq, Default)]
pub(super) struct Drawn(Option<(Page, usize, String, bool, bool)>);

/// Draws the page when it changes, and marks the picked choice every frame.
pub(super) fn draw(
    mut commands: Commands,
    title: Res<Title>,
    column: Single<(Entity, Option<&Children>), With<Column>>,
    mut picks: Query<(&Pick, &mut BackgroundColor, &mut BorderColor)>,
    mut drawn: Local<Drawn>,
) {
    let menu = &title.0;
    // The name's caret shows only while it is picked.
    let naming = menu.picked() == Some(Item::Name);
    let page = Some((
        menu.page,
        menu.worlds.len(),
        menu.name.clone(),
        menu.creative,
        naming,
    ));
    if drawn.0 != page {
        drawn.0 = page;
        let (column, children) = *column;
        for &child in children.into_iter().flatten() {
            commands.entity(child).despawn();
        }
        commands.entity(column).with_children(|column| {
            if let Some(heading) = heading(menu) {
                column.spawn((
                    Text::new(heading),
                    TextFont::from_font_size(22.0),
                    TextColor(QUIET),
                    lifted(),
                    Node {
                        margin: UiRect::bottom(px(14)),
                        ..default()
                    },
                ));
            }
            for (at, item) in menu.items().into_iter().enumerate() {
                choice(column, menu, at, item);
            }
        });
        return;
    }
    for (&Pick(at), mut background, mut border) in &mut picks {
        let (fill, edge) = if at == menu.at {
            (PICKED, EDGE)
        } else {
            (PLAIN, PLAIN)
        };
        background.set_if_neq(BackgroundColor(fill));
        border.set_if_neq(BorderColor::all(edge));
    }
}

fn heading(menu: &Menu) -> Option<String> {
    match menu.page {
        Page::Main => None,
        Page::Worlds => Some("Your worlds".into()),
        Page::New => Some("A new world".into()),
        Page::Delete(at) => menu
            .worlds
            .get(at)
            .map(|world| format!("Delete {} for good? Its save goes too.", world.meta.name)),
    }
}

/// A choice's words, and the quieter line under it.
fn words(menu: &Menu, item: Item) -> (String, Option<String>) {
    let mode = |creative: bool| if creative { "Creative" } else { "Survival" };
    match item {
        Item::Continue => (
            "Continue".into(),
            Some(
                menu.worlds
                    .first()
                    .map_or("No world yet".into(), |w| w.meta.name.clone()),
            ),
        ),
        Item::Worlds => ("Worlds".into(), Some(count(menu.worlds.len()))),
        Item::NewWorld => ("New world".into(), None),
        Item::Yard => (
            "The yard".into(),
            Some("A flat level for building, nothing saved".into()),
        ),
        Item::Settings => ("Settings".into(), None),
        Item::Quit => ("Quit".into(), None),
        Item::World(at) => {
            let world = &menu.worlds[at];
            let ago = ago(worlds::now().saturating_sub(world.meta.played));
            (
                world.meta.name.clone(),
                Some(format!("{}, played {ago}", mode(world.meta.creative))),
            )
        }
        Item::Name => {
            let caret = if menu.picked() == Some(item) { "_" } else { "" };
            (format!("Name   {}{caret}", menu.name), None)
        }
        Item::Mode => (
            format!("Mode   <  {}  >", mode(menu.creative)),
            Some(if menu.creative {
                "Building costs nothing".into()
            } else {
                "Every cost paid from what you gather".into()
            }),
        ),
        Item::Create => ("Create and play".into(), None),
        Item::ConfirmDelete => ("Delete it".into(), None),
        Item::Back if matches!(menu.page, Page::Delete(_)) => ("Keep it".into(), None),
        Item::Back => ("Back".into(), None),
    }
}

fn count(worlds: usize) -> String {
    match worlds {
        0 => "None yet".into(),
        1 => "1 world".into(),
        n => format!("{n} worlds"),
    }
}

/// How long ago, in words.
fn ago(seconds: u64) -> String {
    match seconds {
        0..120 => "just now".into(),
        120..7_200 => format!("{} minutes ago", seconds / 60),
        7_200..172_800 => format!("{} hours ago", seconds / 3_600),
        _ => format!("{} days ago", seconds / 86_400),
    }
}

fn choice(column: &mut ChildSpawnerCommands, menu: &Menu, at: usize, item: Item) {
    let usable = menu.enabled(item);
    let (main, under) = words(menu, item);
    let colour = if usable { TITLE } else { GREYED };
    let mut row = column.spawn((
        Pick(at),
        button(),
        Node {
            width: px(WIDTH),
            min_height: px(46),
            padding: UiRect::axes(px(16), px(7)),
            border: UiRect::left(px(3)),
            border_radius: BorderRadius::all(px(4)),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        },
        BackgroundColor(PLAIN),
        BorderColor::all(PLAIN),
    ));
    row.with_children(|row| {
        row.spawn(Node {
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|words| {
            words.spawn((
                Text::new(main),
                TextFont::from_font_size(22.0),
                TextColor(colour),
                lifted(),
            ));
            if let Some(under) = under {
                words.spawn((
                    Text::new(under),
                    TextFont::from_font_size(14.0),
                    TextColor(if usable { QUIET } else { GREYED }),
                    lifted(),
                ));
            }
        });
        if let Item::World(world) = item {
            row.spawn((
                Cross(world),
                button(),
                Node {
                    padding: UiRect::axes(px(10), px(4)),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            ))
            .with_child((
                Text::new("Delete"),
                TextFont::from_font_size(13.0),
                TextColor(QUIET),
            ));
        }
    });
}
