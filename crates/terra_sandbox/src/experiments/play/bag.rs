//! The inventory on screen: the action bar along the bottom, a count of raw items, a
//! notice above the bar when something could not be done, and the inventory itself,
//! which Tab opens: the character sheet of what is worn (D64), and the backpack's grid,
//! with each item filling its cells.
//!
//!   1 to 6        pick a slot of the action bar; whatever it points at is in hand
//!   Tab           open and close the inventory (I does the same; either can be changed)
//!   In the inventory, point at a tool and press 1 to 6 to put it in that slot; click
//!   gear in the backpack to wear it, and click a slot of the sheet to take it off.

use super::Player;
use super::controls::{Action, Settings};
use super::dial::Dial;
use super::grab_mouse;
use super::inventory::{self, BAR_SLOTS, Backpack, Belongings, InventoryScreen, Items, Raw, SLOTS};
use super::options::OptionsMenu;
use super::vitals::{Vitals, nourishment};
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use terra_items::{GearSlot, ItemId, ItemKind};

const PLAIN: Color = Color::srgba(0.07, 0.08, 0.1, 0.6);
const PICKED: Color = Color::srgba(0.9, 0.46, 0.17, 0.95);
const POINTED: Color = Color::srgba(0.35, 0.75, 0.85, 0.95);
const EMPTY_CELL: Color = Color::srgba(1.0, 1.0, 1.0, 0.06);
const NUMBERS: [KeyCode; BAR_SLOTS] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
];
/// The size of one cell of the backpack on screen.
const CELL_PX: f32 = 48.0;
/// How long a notice stays up.
const NOTICE_SECONDS: f32 = 3.0;

/// A line above the bar saying why something was not done, such as a full backpack.
#[derive(Resource, Default)]
pub struct Notice {
    text: String,
    age: f32,
}

impl Notice {
    pub fn say(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.age = 0.0;
    }
}

#[derive(Component)]
struct Slot(usize);

#[derive(Component)]
struct SlotLabel(usize);

#[derive(Component)]
struct RawCount(Raw);

#[derive(Component)]
struct NoticeText;

#[derive(Component)]
struct Panel;

/// Where the backpack's grid is drawn.
#[derive(Component)]
struct GridArea;

/// An item drawn in the backpack's grid.
#[derive(Component)]
struct Shown(ItemId, ItemKind);

#[derive(Component)]
struct About;

/// How big the backpack is and how much of it is free.
#[derive(Component)]
struct Capacity;

/// A slot of the character sheet.
#[derive(Component)]
struct GearBox(GearSlot);

/// The name of what is worn in a slot of the character sheet.
#[derive(Component)]
struct GearName(GearSlot);

/// Everything here that is not the panel, hidden when taking pictures.
#[derive(Component)]
struct Hud;

pub fn plugin(app: &mut App) {
    app.init_resource::<Items>()
        .init_resource::<InventoryScreen>()
        .init_resource::<Notice>()
        .add_systems(Startup, setup)
        .add_systems(Update, (use_keys, click, lay_out, draw).chain());
}

fn setup(mut commands: Commands) {
    let small = TextFont::from_font_size(12.0);
    let normal = TextFont::from_font_size(15.0);
    let white = TextColor(Color::WHITE);
    let dim = TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75));

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

    // A notice just above the bar.
    commands
        .spawn((
            Hud,
            Node {
                position_type: PositionType::Absolute,
                top: percent(12),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_child((NoticeText, Text::new(""), normal.clone(), white));

    // How many of each raw item are belongings, down the right-hand side.
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
                        flex_direction: FlexDirection::Column,
                        row_gap: px(10),
                        padding: UiRect::all(px(22)),
                        border_radius: BorderRadius::all(px(12)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.88)),
                ))
                .with_children(|panel| {
                    panel
                        .spawn(Node {
                            column_gap: px(24),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: px(8),
                                ..default()
                            })
                            .with_children(|sheet| {
                                sheet.spawn((
                                    Text::new("Equipment"),
                                    TextFont::from_font_size(20.0),
                                    white,
                                ));
                                for (slot, name) in SLOTS {
                                    sheet
                                        .spawn((
                                            GearBox(slot),
                                            Button,
                                            Node {
                                                width: px(150),
                                                flex_direction: FlexDirection::Column,
                                                row_gap: px(2),
                                                padding: UiRect::axes(px(10), px(7)),
                                                border: UiRect::all(px(2)),
                                                border_radius: BorderRadius::all(px(7)),
                                                ..default()
                                            },
                                            BackgroundColor(PLAIN),
                                            BorderColor::all(Color::NONE),
                                        ))
                                        .with_children(|gear_box| {
                                            gear_box.spawn((Text::new(name), small.clone(), dim));
                                            gear_box.spawn((
                                                GearName(slot),
                                                Text::new(""),
                                                normal.clone(),
                                                white,
                                            ));
                                        });
                                }
                            });
                            row.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: px(8),
                                ..default()
                            })
                            .with_children(|pack| {
                                pack.spawn((
                                    Text::new("Backpack"),
                                    TextFont::from_font_size(20.0),
                                    white,
                                ));
                                pack.spawn((Capacity, Text::new(""), small.clone(), dim));
                                pack.spawn((
                                    GridArea,
                                    Node {
                                        position_type: PositionType::Relative,
                                        ..default()
                                    },
                                ));
                            });
                        });
                    panel.spawn((About, Text::new(" "), small.clone(), dim));
                    panel.spawn((
                        Text::new(
                            "Point at a tool and press 1 to 6 to put it on the bar. Click gear \
                             to wear it, and a slot of the equipment to take it off. The \
                             builder spends raw items and taking a thing apart gives them \
                             back, if there is room; your tools gather more.",
                        ),
                        small.clone(),
                        dim,
                        Node {
                            max_width: px(470),
                            ..default()
                        },
                    ));
                });
        });
}

/// Opens and closes the inventory, picks slots of the bar, and puts tools on it.
#[allow(clippy::too_many_arguments)]
fn use_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    settings: Res<Settings>,
    menu: Res<OptionsMenu>,
    dial: Res<Dial>,
    shown: Query<(&Interaction, &Shown)>,
    window: Single<(&Window, &mut CursorOptions)>,
    mut screen: ResMut<InventoryScreen>,
    mut belongings: Belongings,
) {
    if menu.open {
        return;
    }
    let (window, mut cursor) = window.into_inner();
    let captured = cursor.grab_mode != CursorGrabMode::None;
    if settings.went_down(Action::Inventory, &keys, &mouse, captured) {
        screen.open = !screen.open;
        screen.at = None;
        // The pointer is needed to point at things in the inventory, and goes back to
        // steering the view when it closes.
        grab_mouse(&mut cursor, !screen.open && window.focused);
    }
    // While the dial is open the numbers pick parts from it instead.
    if dial.open || dial.closed_by_key {
        return;
    }
    let pointed = shown
        .iter()
        .find(|(interaction, _)| **interaction != Interaction::None)
        .map(|(_, shown)| shown.0);
    for (slot, key) in NUMBERS.into_iter().enumerate() {
        if !keys.just_pressed(key) {
            continue;
        }
        match (screen.open, pointed) {
            (true, Some(id)) => belongings.put_on_bar(slot, id),
            (true, None) => {}
            (false, _) => belongings.pick(slot),
        }
    }
}

/// Wears gear clicked in the backpack, and takes off what is in a slot of the sheet
/// when it is clicked. What cannot be done is said above the bar.
fn click(
    screen: Res<InventoryScreen>,
    shown: Query<(&Interaction, &Shown), Changed<Interaction>>,
    boxes: Query<(&Interaction, &GearBox), Changed<Interaction>>,
    mut belongings: Belongings,
    mut vitals: Single<&mut Vitals, With<Player>>,
    mut notice: ResMut<Notice>,
) {
    if !screen.open {
        return;
    }
    let mut result = None;
    for (interaction, shown) in &shown {
        // Food is eaten.
        let food = belongings
            .items()
            .raw(shown.1)
            .filter(|&raw| nourishment(raw) > 0.0);
        if *interaction == Interaction::Pressed
            && let Some(food) = food
            && belongings.eat(food)
        {
            vitals.eat(food);
            notice.say(format!("Ate {}", food.name().to_lowercase()));
            continue;
        }
        let is_gear = belongings
            .items()
            .catalogue
            .get(shown.1)
            .is_ok_and(|def| def.gear.is_some());
        if *interaction == Interaction::Pressed && is_gear {
            result = Some(belongings.wear(shown.0));
        }
    }
    for (interaction, gear_box) in &boxes {
        if *interaction == Interaction::Pressed && belongings.gear().worn(gear_box.0).is_some() {
            result = Some(belongings.take_off(gear_box.0));
        }
    }
    if let Some(Err(refused)) = result {
        notice.say(inventory::reason(refused));
    }
}

/// The colour an item is drawn in: one per raw item, and one for tools and gear.
fn colour(items: &Items, kind: ItemKind) -> Color {
    match items.raw(kind) {
        Some(Raw::Wood) => Color::srgb(0.5, 0.36, 0.22),
        Some(Raw::Stone) => Color::srgb(0.5, 0.5, 0.52),
        Some(Raw::Fibre) => Color::srgb(0.55, 0.6, 0.35),
        Some(Raw::Hide) => Color::srgb(0.62, 0.48, 0.36),
        Some(Raw::Ore) => Color::srgb(0.56, 0.34, 0.26),
        Some(Raw::Iron) => Color::srgb(0.42, 0.45, 0.5),
        Some(Raw::Meat) => Color::srgb(0.7, 0.3, 0.3),
        Some(Raw::Berries) => Color::srgb(0.7, 0.2, 0.35),
        Some(Raw::CookedMeat) => Color::srgb(0.6, 0.36, 0.2),
        None => Color::srgb(0.2, 0.48, 0.45),
    }
}

/// Draws the backpack's grid again whenever what is in it changes.
fn lay_out(
    mut commands: Commands,
    items: Res<Items>,
    backpack: Single<Ref<Backpack>>,
    area: Single<(Entity, &mut Node), With<GridArea>>,
    mut drawn: Local<bool>,
) {
    if *drawn && !backpack.is_changed() {
        return;
    }
    *drawn = true;
    let grid = &backpack.0;
    let (area, mut node) = area.into_inner();
    node.width = px(grid.size().w as f32 * CELL_PX);
    node.height = px(grid.size().h as f32 * CELL_PX);
    let cell = |x: f32, y: f32, w: f32, h: f32| Node {
        position_type: PositionType::Absolute,
        left: px(x * CELL_PX + 2.0),
        top: px(y * CELL_PX + 2.0),
        width: px(w * CELL_PX - 4.0),
        height: px(h * CELL_PX - 4.0),
        padding: UiRect::all(px(3)),
        border: UiRect::all(px(2)),
        border_radius: BorderRadius::all(px(5)),
        ..default()
    };
    let mut entity = commands.entity(area);
    entity.despawn_children();
    entity.with_children(|area| {
        for y in 0..grid.size().h {
            for x in 0..grid.size().w {
                area.spawn((
                    cell(x as f32, y as f32, 1.0, 1.0),
                    BackgroundColor(EMPTY_CELL),
                ));
            }
        }
        for placed in grid.iter() {
            let kind = placed.item.kind;
            area.spawn((
                Shown(placed.item.id, kind),
                Button,
                cell(
                    placed.at.x as f32,
                    placed.at.y as f32,
                    placed.size.w as f32,
                    placed.size.h as f32,
                ),
                BackgroundColor(colour(&items, kind)),
                BorderColor::all(Color::NONE),
            ))
            .with_child((
                Text::new(items.name(kind)),
                TextFont::from_font_size(9.0),
                TextColor(Color::WHITE),
            ));
        }
    });
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn draw(
    time: Res<Time>,
    screen: Res<InventoryScreen>,
    options: Res<crate::shell::Options>,
    belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut panel: Single<&mut Visibility, (With<Panel>, Without<Hud>)>,
    mut hud: Query<&mut Visibility, With<Hud>>,
    mut slots: Query<
        (&Slot, &mut BackgroundColor, &mut BorderColor),
        (Without<Shown>, Without<GearBox>),
    >,
    mut shown: Query<(&Interaction, &Shown, &mut BorderColor), (Without<Slot>, Without<GearBox>)>,
    mut boxes: Query<(&Interaction, &GearBox, &mut BorderColor), (Without<Slot>, Without<Shown>)>,
    mut texts: ParamSet<(
        Query<(&SlotLabel, &mut Text)>,
        Query<(&RawCount, &mut Text)>,
        Single<&mut Text, With<About>>,
        Single<&mut Text, With<Capacity>>,
        Single<(&mut Text, &mut TextColor), With<NoticeText>>,
        Query<(&GearName, &mut Text)>,
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
    let outline = |border: &mut BorderColor, pointed: bool| {
        let wanted = BorderColor::all(if pointed { POINTED } else { Color::NONE });
        if *border != wanted {
            *border = wanted;
        }
    };
    let items = belongings.items();
    let (backpack, bar, gear) = (belongings.backpack(), belongings.bar(), belongings.gear());
    show(&mut panel, screen.open);
    for mut hud in &mut hud {
        show(&mut hud, options.vantage.is_none());
    }
    for (slot, mut background, mut border) in &mut slots {
        let held = slot.0 == bar.picked();
        let colour = if held { PICKED } else { PLAIN };
        if background.0 != colour {
            background.0 = colour;
            *border = BorderColor::all(if held { Color::WHITE } else { Color::NONE });
        }
    }
    for (label, mut text) in &mut texts.p0() {
        let name = bar
            .get(label.0)
            .and_then(|id| backpack.get(id))
            .map_or("", |placed| items.name(placed.item.kind));
        set(&mut text, name.to_string());
    }
    for (count, mut text) in &mut texts.p1() {
        set(
            &mut text,
            format!("{:<6} {:>3}", count.0.name(), belongings.count(count.0)),
        );
    }
    for (name, mut text) in &mut texts.p5() {
        let worn = gear
            .worn(name.0)
            .map_or("empty", |item| items.name(item.kind));
        set(&mut text, worn.to_string());
    }

    let mut about = " ".to_string();
    for (interaction, shown, mut border) in &mut shown {
        let pointed = *interaction != Interaction::None;
        outline(&mut border, pointed);
        if !pointed {
            continue;
        }
        let name = items.name(shown.1);
        let worn_in = items
            .catalogue
            .get(shown.1)
            .ok()
            .and_then(|def| def.gear)
            .and_then(|slot| SLOTS.iter().find(|(each, _)| *each == slot))
            .map(|(_, slot_name)| *slot_name);
        about = match (items.tool(shown.1), worn_in) {
            (Some(tool), _) => {
                let slot = (0..bar.len()).find(|&slot| bar.get(slot) == Some(shown.0));
                format!(
                    "{name}.  {}.  {}",
                    tool.about(),
                    slot.map_or("Not on the bar.".to_string(), |slot| format!(
                        "On the bar at {}.",
                        slot + 1
                    ))
                )
            }
            (None, Some(slot_name)) => {
                format!("{name}.  Worn in the {slot_name} slot.  Click to wear it.")
            }
            (None, None) => format!("{name}.  A raw item, for building."),
        };
    }
    for (interaction, gear_box, mut border) in &mut boxes {
        let pointed = *interaction != Interaction::None;
        outline(&mut border, pointed);
        if pointed && let Some(item) = gear.worn(gear_box.0) {
            about = format!("{}.  Click to take it off.", items.name(item.kind));
        }
    }
    set(&mut texts.p2(), about);

    let size = backpack.size();
    let cells = size.w as usize * size.h as usize;
    let used: usize = backpack
        .iter()
        .map(|placed| placed.size.w as usize * placed.size.h as usize)
        .sum();
    set(
        &mut texts.p3(),
        format!(
            "{} by {}, {} of {} cells free.",
            size.w,
            size.h,
            cells - used,
            cells
        ),
    );

    notice.age += time.delta_secs();
    // Longer things said stay up longer, so they can be read.
    let lasts = NOTICE_SECONDS + notice.text.len() as f32 / 22.0;
    let fade = (lasts - notice.age).clamp(0.0, 1.0);
    let (mut text, mut text_colour) = texts.p4().into_inner();
    let wanted = if fade > 0.0 {
        notice.text.clone()
    } else {
        String::new()
    };
    set(&mut text, wanted);
    let wanted = TextColor(Color::srgba(1.0, 0.85, 0.6, fade));
    if *text_colour != wanted {
        *text_colour = wanted;
    }
}
