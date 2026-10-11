//! The inventory on screen: the action bar along the bottom, a count of raw items, and
//! the panel that Tab opens.
//!
//!   1 to 6        pick a slot of the action bar; whatever is in it is in hand
//!   Tab           open and close the inventory (I does the same; either can be changed)
//!   In the inventory, point at a tool and press 1 to 6 to put it in that slot.

use super::controls::{Action, Settings};
use super::dial::Dial;
use super::grab_mouse;
use super::inventory::{BAR_SLOTS, Inventory, Raw, Tool};
use super::options::OptionsMenu;
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

const PLAIN: Color = Color::srgba(0.07, 0.08, 0.1, 0.6);
const PICKED: Color = Color::srgba(0.9, 0.46, 0.17, 0.95);
const POINTED: Color = Color::srgba(0.35, 0.75, 0.85, 0.95);
const NUMBERS: [KeyCode; BAR_SLOTS] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
];

#[derive(Component)]
struct Slot(usize);

#[derive(Component)]
struct SlotLabel(usize);

#[derive(Component)]
struct RawCount(Raw);

#[derive(Component)]
struct Panel;

/// A tool listed in the inventory panel.
#[derive(Component)]
struct Listed(Tool);

#[derive(Component)]
struct ListedAbout;

/// Everything here that is not the panel, hidden when taking pictures.
#[derive(Component)]
struct Hud;

pub fn plugin(app: &mut App) {
    app.init_resource::<Inventory>()
        .add_systems(Startup, setup)
        .add_systems(Update, (use_keys, draw).chain());
}

fn setup(mut commands: Commands, inventory: Res<Inventory>) {
    let small = TextFont::from_font_size(12.0);
    let normal = TextFont::from_font_size(15.0);
    let white = TextColor(Color::WHITE);

    // The action bar.
    commands
        .spawn((
            Hud,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(20),
                width: percent(100),
                justify_content: JustifyContent::Center,
                column_gap: px(8),
                ..default()
            },
        ))
        .with_children(|bar| {
            for slot in 0..BAR_SLOTS {
                bar.spawn((
                    Slot(slot),
                    Node {
                        width: px(78),
                        height: px(58),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::axes(px(8), px(6)),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(8)),
                        ..default()
                    },
                    BackgroundColor(PLAIN),
                    BorderColor::all(Color::NONE),
                ))
                .with_children(|slot_box| {
                    slot_box.spawn((
                        Text::new((slot + 1).to_string()),
                        small.clone(),
                        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.7)),
                    ));
                    slot_box.spawn((SlotLabel(slot), Text::new(""), small.clone(), white));
                });
            }
        });

    // What is carried, down the right-hand side.
    commands
        .spawn((
            Hud,
            Node {
                position_type: PositionType::Absolute,
                top: px(20),
                right: px(20),
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                padding: UiRect::axes(px(14), px(10)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(PLAIN),
        ))
        .with_children(|list| {
            for raw in Raw::ALL {
                list.spawn((RawCount(raw), Text::new(""), normal.clone(), white));
            }
        });

    // The inventory itself, hidden until opened.
    commands
        .spawn((
            Panel,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Node {
                        width: px(520),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(10),
                        padding: UiRect::all(px(22)),
                        border_radius: BorderRadius::all(px(12)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.88)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("Inventory"),
                        TextFont::from_font_size(20.0),
                        white,
                    ));
                    panel.spawn((
                        Text::new("Tools.  Point at one and press 1 to 6 to put it on the bar."),
                        small.clone(),
                        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
                    ));
                    for &tool in &inventory.tools {
                        panel
                            .spawn((
                                Listed(tool),
                                Button,
                                Node {
                                    padding: UiRect::axes(px(12), px(8)),
                                    border_radius: BorderRadius::all(px(7)),
                                    ..default()
                                },
                                BackgroundColor(PLAIN),
                            ))
                            .with_child((Text::new(tool.name()), normal.clone(), white));
                    }
                    panel.spawn((
                        ListedAbout,
                        Text::new(""),
                        small.clone(),
                        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
                    ));
                    panel.spawn((
                        Text::new(
                            "Raw items are listed at the top right. The builder spends them, and \
                             taking a thing apart gives them back. The cutter gathers more.",
                        ),
                        small.clone(),
                        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
                    ));
                });
        });
}

/// Opens and closes the inventory, picks slots of the bar, and puts tools on it.
fn use_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    settings: Res<Settings>,
    menu: Res<OptionsMenu>,
    dial: Res<Dial>,
    listed: Query<(&Interaction, &Listed)>,
    window: Single<(&Window, &mut CursorOptions)>,
    mut inventory: ResMut<Inventory>,
) {
    if menu.open {
        return;
    }
    let (window, mut cursor) = window.into_inner();
    let captured = cursor.grab_mode != CursorGrabMode::None;
    if settings.went_down(Action::Inventory, &keys, &mouse, captured) {
        inventory.open = !inventory.open;
        // The pointer is needed to point at things in the inventory, and goes back to
        // steering the view when it closes.
        grab_mouse(&mut cursor, !inventory.open && window.focused);
    }
    // While the dial is open the numbers pick parts from it instead.
    if dial.open || dial.closed_by_key {
        return;
    }
    let pointed = listed
        .iter()
        .find(|(interaction, _)| **interaction != Interaction::None)
        .map(|(_, listed)| listed.0);
    for (slot, key) in NUMBERS.into_iter().enumerate() {
        if !keys.just_pressed(key) {
            continue;
        }
        match (inventory.open, pointed) {
            (true, Some(tool)) => inventory.put_on_bar(tool, slot),
            (true, None) => {}
            (false, _) => inventory.held = slot,
        }
    }
}

fn draw(
    inventory: Res<Inventory>,
    options: Res<crate::shell::Options>,
    mut panel: Single<&mut Visibility, (With<Panel>, Without<Hud>)>,
    mut hud: Query<&mut Visibility, With<Hud>>,
    mut slots: Query<(&Slot, &mut BackgroundColor, &mut BorderColor), Without<Listed>>,
    mut listed: Query<(&Interaction, &Listed, &mut BackgroundColor), Without<Slot>>,
    mut texts: ParamSet<(
        Query<(&SlotLabel, &mut Text)>,
        Query<(&RawCount, &mut Text)>,
        Single<&mut Text, With<ListedAbout>>,
    )>,
) {
    let set = |text: &mut Text, to: String| {
        if text.0 != to {
            text.0 = to;
        }
    };
    let show = |visibility: &mut Visibility, shown: bool| {
        let wanted = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    };
    show(&mut panel, inventory.open);
    for mut hud in &mut hud {
        show(&mut hud, options.vantage.is_none());
    }
    for (slot, mut background, mut border) in &mut slots {
        let held = slot.0 == inventory.held;
        let colour = if held { PICKED } else { PLAIN };
        if background.0 != colour {
            background.0 = colour;
            *border = BorderColor::all(if held { Color::WHITE } else { Color::NONE });
        }
    }
    for (label, mut text) in &mut texts.p0() {
        let name = inventory.bar[label.0].map_or("", Tool::name);
        set(&mut text, name.to_string());
    }
    for (count, mut text) in &mut texts.p1() {
        set(
            &mut text,
            format!("{:<6} {:>3}", count.0.name(), inventory.count(count.0)),
        );
    }
    let mut about = String::new();
    for (interaction, tool, mut background) in &mut listed {
        let pointed = *interaction != Interaction::None;
        let colour = if pointed { POINTED } else { PLAIN };
        if background.0 != colour {
            background.0 = colour;
        }
        if pointed {
            let slot = inventory.bar.iter().position(|held| *held == Some(tool.0));
            about = format!(
                "{}.  {}",
                tool.0.about(),
                slot.map_or("Not on the bar.".to_string(), |slot| format!(
                    "On the bar at {}.",
                    slot + 1
                ))
            );
        }
    }
    set(&mut texts.p2(), about);
}
