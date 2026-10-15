//! The inventory on screen: the action bar along the bottom, a count of raw items, a
//! notice above the bar when something could not be done, and the inventory itself,
//! which Tab opens: the character sheet of what is worn (D64), and the backpack's grid,
//! with each item filling its cells. Items show their icon, baked by `cargo xtask icons`
//! (D129), or their name if they have none yet.
//!
//!   1 to 8        pick a slot of the action bar; whatever it points at is in hand
//!   Tab           open and close the inventory (I does the same; either can be changed)
//!   In the inventory, point at a tool and press 1 to 8 to put it in that slot; click
//!   gear in the backpack to wear it, and click a slot of the sheet to take it off.

use super::IsLocalPlayer;
use super::build_menu::BuildMenu;
use super::button::{button, left_click, pointer_on};
use super::camera::grab_mouse;
use super::controls::{Action, Settings};
use super::dial::Dial;
use super::inventory::{self, BAR_SLOTS, Backpack, Belongings, InventoryScreen, Items, SLOTS};
use super::options::OptionsMenu;
use super::told::Notice;
use super::vitals::Vitals;
use crate::{character, icons};
use bevy::{
    picking::hover::Hovered,
    prelude::*,
    ui::Pressed,
    window::{CursorGrabMode, CursorOptions},
};
use std::collections::HashMap;
use terra_items::{GearSlot, ItemId, ItemKind};

const PLAIN: Color = Color::srgba(0.07, 0.08, 0.1, 0.6);
const PICKED: Color = Color::srgba(0.9, 0.46, 0.17, 0.95);
pub(super) const POINTED: Color = Color::srgba(0.35, 0.75, 0.85, 0.95);
const EMPTY_CELL: Color = Color::srgba(1.0, 1.0, 1.0, 0.06);
/// Behind an item shown by its icon.
const FULL_CELL: Color = Color::srgba(1.0, 1.0, 1.0, 0.11);
const NUMBERS: [KeyCode; BAR_SLOTS] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
];
/// The size of one cell of the backpack on screen.
const CELL_PX: f32 = 48.0;
/// How long a notice stays up.
const NOTICE_SECONDS: f32 = 3.0;

#[derive(Component)]
struct Slot(usize);

#[derive(Component)]
struct SlotLabel(usize);

#[derive(Component)]
struct NoticeText;

#[derive(Component)]
struct Panel;

/// Where the backpack's grid is drawn.
#[derive(Component)]
struct GridArea;

/// A column of the inventory beside the backpack, for what was opened with it, such as a
/// chest: another screen fills it and shows it.
#[derive(Component)]
pub(super) struct Beside;

/// An item drawn in the backpack's grid. Other screens read it to know when the pointer
/// is on the backpack.
#[derive(Component)]
pub struct Shown(pub(super) ItemId, pub(super) ItemKind);

#[derive(Component)]
struct About;

/// How big the backpack is and how much of it is free.
#[derive(Component)]
struct Capacity;

/// Where an icon is shown outside the backpack's grid: a slot of the bar, or of the
/// character sheet. Its node is sized to the icon's shape within the room it has.
#[derive(Component)]
enum IconAt {
    Bar(usize),
    Gear(GearSlot),
}

impl IconAt {
    /// The most room the icon has, in pixels.
    fn room(&self) -> Vec2 {
        match self {
            IconAt::Bar(_) => Vec2::new(70.0, 46.0),
            IconAt::Gear(_) => Vec2::new(40.0, 40.0),
        }
    }
}

/// A slot of the character sheet.
#[derive(Component)]
struct GearBox(GearSlot);

/// The name of what is worn in a slot of the character sheet.
#[derive(Component)]
struct GearName(GearSlot);

/// Everything here that is not the panel, hidden when taking pictures.
#[derive(Component)]
struct Hud;

/// The line along the foot of a bar slot saying how worn its tool is (D99).
#[derive(Component)]
struct BarWear(usize);

/// A wear line, as long as the use left, or all the way across in red when broken.
fn wear_node(fraction: f32) -> Node {
    let shown = if fraction <= 0.0 {
        1.0
    } else {
        fraction.min(1.0)
    };
    Node {
        position_type: PositionType::Absolute,
        left: px(4),
        bottom: px(2),
        width: percent(shown.max(0.04) * 92.0),
        height: px(3),
        border_radius: BorderRadius::all(px(2)),
        ..default()
    }
}

/// The colour of a wear line: green while there is plenty of use left, amber when
/// little, red when broken.
fn wear_colour(condition: f32) -> Color {
    if condition <= 0.0 {
        Color::srgb(0.9, 0.2, 0.15)
    } else if condition < 0.25 {
        Color::srgb(0.95, 0.65, 0.2)
    } else {
        Color::srgba(0.55, 0.85, 0.45, 0.9)
    }
}

/// The wear of an item, if it is of a kind that wears and has been used at all.
fn worn(items: &Items, item: terra_items::Item) -> Option<f32> {
    let def = items.catalogue.get(item.kind).ok()?;
    item.condition(def).filter(|condition| *condition < 1.0)
}

/// Draws each bar slot's wear line, for a tool that has been used.
fn draw_wear(
    items: Res<Items>,
    player: Single<(&Backpack, &inventory::Bar)>,
    mut lines: Query<(&BarWear, &mut Node, &mut BackgroundColor)>,
) {
    let (backpack, bar) = *player;
    for (BarWear(slot), mut node, mut colour) in &mut lines {
        let condition = bar
            .0
            .get(*slot)
            .and_then(|id| backpack.0.get(id))
            .and_then(|placed| worn(&items, placed.item));
        let wanted = condition.map_or(Color::NONE, wear_colour);
        if colour.0 != wanted {
            colour.0 = wanted;
        }
        let width = wear_node(condition.unwrap_or(0.0)).width;
        if node.width != width {
            node.width = width;
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ItemIcons>()
        .init_resource::<InventoryScreen>()
        .add_systems(Startup, setup)
        .add_systems(Update, (use_keys, click, lay_out, draw, draw_wear).chain());
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
                    slot_box
                        .spawn(Node {
                            position_type: PositionType::Absolute,
                            left: px(0),
                            right: px(0),
                            top: px(0),
                            bottom: px(0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_child((
                            IconAt::Bar(slot),
                            ImageNode::default(),
                            Node {
                                display: Display::None,
                                ..default()
                            },
                        ));
                    slot_box.spawn((
                        Text::new((slot + 1).to_string()),
                        small.clone(),
                        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.7)),
                    ));
                    slot_box.spawn((SlotLabel(slot), Text::new(""), small.clone(), white));
                    slot_box.spawn((BarWear(slot), wear_node(0.0), BackgroundColor(Color::NONE)));
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
        .with_child((
            NoticeText,
            Text::new(""),
            normal.clone(),
            white,
            // A dark backing, so it reads against a bright sky.
            Node {
                max_width: px(1100),
                padding: UiRect::axes(px(12), px(6)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ));

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
                                            button(),
                                            Node {
                                                width: px(170),
                                                min_height: px(54),
                                                column_gap: px(8),
                                                align_items: AlignItems::Center,
                                                padding: UiRect::axes(px(10), px(7)),
                                                border: UiRect::all(px(2)),
                                                border_radius: BorderRadius::all(px(7)),
                                                ..default()
                                            },
                                            BackgroundColor(PLAIN),
                                            BorderColor::all(Color::NONE),
                                        ))
                                        .with_children(|gear_box| {
                                            gear_box.spawn((
                                                IconAt::Gear(slot),
                                                ImageNode::default(),
                                                Node {
                                                    display: Display::None,
                                                    ..default()
                                                },
                                            ));
                                            gear_box
                                                .spawn(Node {
                                                    flex_direction: FlexDirection::Column,
                                                    row_gap: px(2),
                                                    ..default()
                                                })
                                                .with_children(|names| {
                                                    names.spawn((
                                                        Text::new(name),
                                                        small.clone(),
                                                        dim,
                                                    ));
                                                    names.spawn((
                                                        GearName(slot),
                                                        Text::new(""),
                                                        normal.clone(),
                                                        white,
                                                    ));
                                                });
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
                            // Beside the backpack, what was opened with it, such as a
                            // chest (`storage_screen`); hidden otherwise.
                            row.spawn((
                                Beside,
                                Node {
                                    display: Display::None,
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(8),
                                    ..default()
                                },
                            ));
                        });
                    panel.spawn((About, Text::new(" "), small.clone(), dim));
                    panel.spawn((
                        Text::new(
                            "Point at a tool and press 1 to 8 to put it on the bar. Click gear \
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
    build_menu: Res<BuildMenu>,
    shown: Query<(&Hovered, Has<Pressed>, &Shown)>,
    window: Single<(&Window, &mut CursorOptions)>,
    mut screen: ResMut<InventoryScreen>,
    mut belongings: Belongings,
) {
    let Some(mut belongings) = belongings.mine() else {
        return;
    };
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
    // While the build menu is open the numbers pick its tabs instead.
    if dial.open || build_menu.open {
        return;
    }
    let pointed = shown
        .iter()
        .find(|(hovered, pressed, _)| pointer_on(hovered, *pressed))
        .map(|(_, _, shown)| shown.0);
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
    mouse: Res<ButtonInput<MouseButton>>,
    shown: Query<&Shown, Added<Pressed>>,
    boxes: Query<&GearBox, Added<Pressed>>,
    mut belongings: Belongings,
    mut vitals: Single<&mut Vitals, IsLocalPlayer>,
    mut notice: ResMut<Notice>,
) {
    let Some(mut belongings) = belongings.mine() else {
        return;
    };
    // With storage open, a click moves things to it instead (`storage_screen`).
    if !screen.open || screen.storage.is_some() || !left_click(&mouse) {
        return;
    }
    let mut result = None;
    for shown in &shown {
        // Food is eaten.
        let filling = belongings.items().food(shown.1);
        if filling > 0.0 && belongings.eat(shown.1) {
            vitals.eat(filling);
            let name = belongings.items().name(shown.1).to_lowercase();
            notice.say(format!("Ate {name}"));
            continue;
        }
        let is_gear = belongings
            .items()
            .catalogue
            .get(shown.1)
            .is_ok_and(|def| def.gear.is_some());
        if is_gear {
            result = Some(belongings.wear(shown.0));
        }
    }
    for gear_box in &boxes {
        if belongings.gear().worn(gear_box.0).is_some() {
            result = Some(belongings.take_off(gear_box.0));
        }
    }
    if let Some(Err(refused)) = result {
        notice.say(inventory::reason(refused));
    }
}

/// The colour an item is drawn in while it has no icon: its kind's own, or one for tools
/// and gear.
fn colour(items: &Items, kind: ItemKind) -> Color {
    match items.def(kind).colour {
        Some((red, green, blue)) => Color::srgb(red, green, blue),
        None => Color::srgb(0.2, 0.48, 0.45),
    }
}

/// Each kind's baked icon (D129), loaded the first time it is shown, or none if it has
/// not been baked: an item without a look yet.
#[derive(Resource, Default)]
pub(super) struct ItemIcons(HashMap<ItemKind, Option<(Handle<Image>, (u8, u8))>>);

impl ItemIcons {
    /// The icon and the cells it was baked for.
    fn get(
        &mut self,
        items: &Items,
        assets: &AssetServer,
        kind: ItemKind,
    ) -> Option<(Handle<Image>, (u8, u8))> {
        self.0
            .entry(kind)
            .or_insert_with(|| {
                let def = items.catalogue.get(kind).ok()?;
                let file = icons::file(&character::item_id(&def.name));
                let baked = terra_world::data::assets().join(&file).exists();
                baked.then(|| (assets.load(file), (def.size.w, def.size.h)))
            })
            .clone()
    }
}

/// Draws the backpack's grid again whenever what is in it changes.
fn lay_out(
    mut commands: Commands,
    items: Res<Items>,
    screen: Res<InventoryScreen>,
    backpack: Single<Ref<Backpack>, IsLocalPlayer>,
    area: Single<(Entity, &mut Node), With<GridArea>>,
    mut icons: ResMut<ItemIcons>,
    assets: Res<AssetServer>,
    mut drawn: Local<bool>,
) {
    // Laid out when the screen opens, and again while it is open if the backpack changes;
    // not while it is shut, though what is carried changes all the time.
    if !screen.open {
        *drawn = false;
        return;
    }
    if *drawn && !backpack.is_changed() {
        return;
    }
    *drawn = true;
    let (area, mut node) = area.into_inner();
    lay_grid(
        &mut commands,
        (area, &mut node),
        &backpack.0,
        &items,
        (&mut icons, &assets),
        |placed| Shown(placed.item.id, placed.item.kind),
    );
}

/// Lays a grid of items out in an area of the screen, each item a button marked by `mark`:
/// the backpack's here, a chest's in `storage_screen`.
pub(super) fn lay_grid<B: Bundle>(
    commands: &mut Commands,
    (area, node): (Entity, &mut Node),
    grid: &terra_items::Grid,
    items: &Items,
    (icons, assets): (&mut ItemIcons, &AssetServer),
    mark: impl Fn(&terra_items::Placed) -> B,
) {
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
            let image = icons.get(items, assets, kind).map(|(image, _)| image);
            let name = items.name(kind);
            // A container says how full it is; an item without an icon says what it is.
            let label = match (placed.stack, image.is_some()) {
                (Some(stack), true) => format!("{}/{}", stack.held, stack.most),
                (Some(stack), false) => format!("{name} {}/{}", stack.held, stack.most),
                (None, true) => String::new(),
                (None, false) => name.to_string(),
            };
            let background = if image.is_some() {
                FULL_CELL
            } else {
                colour(items, kind)
            };
            area.spawn((
                mark(placed),
                button(),
                cell(
                    placed.at.x as f32,
                    placed.at.y as f32,
                    placed.size.w as f32,
                    placed.size.h as f32,
                ),
                BackgroundColor(background),
                BorderColor::all(Color::NONE),
            ))
            .with_children(|tile| {
                if let Some(image) = image {
                    tile.spawn((
                        ImageNode::new(image),
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0),
                            top: px(0),
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                    ));
                }
                tile.spawn((
                    Text::new(label),
                    TextFont::from_font_size(9.0),
                    TextColor(Color::WHITE),
                ));
                if let Some(condition) = worn(items, placed.item) {
                    tile.spawn((
                        wear_node(condition),
                        BackgroundColor(wear_colour(condition)),
                    ));
                }
            });
        }
    });
}

/// The size of an icon `cells` across and down, as big as fits in `room`.
fn fit(cells: (u8, u8), room: Vec2) -> Vec2 {
    let aspect = f32::from(cells.0.max(1)) / f32::from(cells.1.max(1));
    let width = room.x.min(room.y * aspect);
    Vec2::new(width, width / aspect)
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn draw(
    time: Res<Time>,
    screen: Res<InventoryScreen>,
    options: Res<crate::shell::Options>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut panel: Single<&mut Visibility, (With<Panel>, Without<Hud>)>,
    mut hud: Query<&mut Visibility, With<Hud>>,
    mut slots: Query<
        (&Slot, &mut BackgroundColor, &mut BorderColor),
        (Without<Shown>, Without<GearBox>),
    >,
    mut shown: Query<
        (&Hovered, Has<Pressed>, &Shown, &mut BorderColor),
        (Without<Slot>, Without<GearBox>),
    >,
    mut boxes: Query<
        (&Hovered, Has<Pressed>, &GearBox, &mut BorderColor),
        (Without<Slot>, Without<Shown>),
    >,
    mut icons: ResMut<ItemIcons>,
    assets: Res<AssetServer>,
    mut icon_nodes: Query<(&IconAt, &mut ImageNode, &mut Node)>,
    mut texts: ParamSet<(
        Query<(&SlotLabel, &mut Text)>,
        Single<&mut Text, With<About>>,
        Single<&mut Text, With<Capacity>>,
        Single<
            (&mut Text, &mut TextColor, &mut BackgroundColor),
            (With<NoticeText>, Without<Slot>),
        >,
        Query<(&GearName, &mut Text)>,
    )>,
) {
    let Some(belongings) = belongings.mine() else {
        return;
    };
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
    // Icons on the bar and the sheet; for things with none, the bar shows the name.
    let mut bar_icons = [false; BAR_SLOTS];
    for (at, mut image, mut node) in &mut icon_nodes {
        let kind = match at {
            IconAt::Bar(slot) => bar
                .get(*slot)
                .and_then(|id| backpack.get(id))
                .map(|placed| placed.item.kind),
            IconAt::Gear(slot) => gear.worn(*slot).map(|item| item.kind),
        };
        let shown = kind
            .and_then(|kind| icons.get(items, &assets, kind))
            .map(|(handle, cells)| (cells, handle));
        if let (IconAt::Bar(slot), Some(_)) = (at, &shown) {
            bar_icons[*slot] = true;
        }
        let display = match shown {
            Some((cells, handle)) => {
                if image.image != handle {
                    image.image = handle;
                }
                let size = fit(cells, at.room());
                if node.width != px(size.x) || node.height != px(size.y) {
                    node.width = px(size.x);
                    node.height = px(size.y);
                }
                Display::Flex
            }
            None => Display::None,
        };
        if node.display != display {
            node.display = display;
        }
    }
    for (label, mut text) in &mut texts.p0() {
        let name = bar
            .get(label.0)
            .and_then(|id| backpack.get(id))
            .filter(|_| !bar_icons[label.0])
            .map_or("", |placed| items.name(placed.item.kind));
        set(&mut text, name.to_string());
    }
    for (name, mut text) in &mut texts.p4() {
        // A worn container says how full it is.
        let worn = match (gear.worn(name.0), gear.stack(name.0)) {
            (Some(item), Some(stack)) => {
                format!("{} {}/{}", items.name(item.kind), stack.held, stack.most)
            }
            (Some(item), None) => items.name(item.kind).to_string(),
            (None, _) => "empty".to_string(),
        };
        set(&mut text, worn);
    }

    let mut about = " ".to_string();
    for (hovered, pressed, shown, mut border) in &mut shown {
        let pointed = pointer_on(hovered, pressed);
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
            (Some(_), _) => {
                let slot = (0..bar.len()).find(|&slot| bar.get(slot) == Some(shown.0));
                format!(
                    "{name}.  {}.  {}",
                    items.def(shown.1).about.as_deref().unwrap_or_default(),
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
    for (hovered, pressed, gear_box, mut border) in &mut boxes {
        let pointed = pointer_on(hovered, pressed);
        outline(&mut border, pointed);
        if pointed && let Some(item) = gear.worn(gear_box.0) {
            about = format!("{}.  Click to take it off.", items.name(item.kind));
        }
    }
    set(&mut texts.p1(), about);

    let size = backpack.size();
    let cells = size.w as usize * size.h as usize;
    let used: usize = backpack
        .iter()
        .map(|placed| placed.size.w as usize * placed.size.h as usize)
        .sum();
    set(
        &mut texts.p2(),
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
    let fade = if notice.text.is_empty() {
        0.0
    } else {
        (lasts - notice.age).clamp(0.0, 1.0)
    };
    let (mut text, mut text_colour, mut backing) = texts.p3().into_inner();
    let wanted = BackgroundColor(Color::srgba(0.04, 0.05, 0.07, 0.62 * fade));
    if *backing != wanted {
        *backing = wanted;
    }
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
