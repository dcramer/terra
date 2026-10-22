//! Storage on screen (D84): F at a chest opens the inventory with the chest's grid beside
//! the backpack, as Subnautica's lockers open. A click moves a thing across; Shift and a
//! click moves every one of its kind. What is stored here is what building in the claim
//! draws on once the pack runs out (D141), which the screen says.
//!
//!   F                   open and close, at a chest
//!   Click               move a thing between the backpack and the chest
//!   Shift and click     move every one of its kind

use crate::bag::{Beside, ItemIcons, POINTED, Shown, lay_grid};
use crate::inventory::{Belongings, InventoryScreen, Items};
use crate::props::Prop;
use crate::storage::Storage;
use crate::told::Notice;
use bevy::{picking::hover::Hovered, prelude::*, ui::Pressed};
use terra_items::{ItemId, ItemKind};
use terra_walker::button::{left_click, pointer_on};

/// A thing in the open chest, shown in its grid.
#[derive(Component)]
struct Stored(ItemId, ItemKind);

/// Where the chest's grid is drawn.
#[derive(Component)]
struct StoreGrid;

/// The chest's name, and how full it is.
#[derive(Component)]
struct StoreTitle;

#[derive(Component)]
struct StoreCapacity;

/// What the pointer is on, and what a click does.
#[derive(Component)]
struct StoreAbout;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (fill, click, lay_out, draw).chain());
}

/// Fills the column beside the backpack with the chest's parts, once.
fn fill(mut commands: Commands, beside: Query<Entity, (With<Beside>, Without<Children>)>) {
    let Ok(beside) = beside.single() else {
        return;
    };
    let small = TextFont::from_font_size(12.0);
    let dim = TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75));
    commands.entity(beside).with_children(|column| {
        column.spawn((
            StoreTitle,
            Text::new(""),
            TextFont::from_font_size(20.0),
            TextColor(Color::WHITE),
        ));
        column.spawn((StoreCapacity, Text::new(""), small.clone(), dim));
        column.spawn((
            StoreGrid,
            Node {
                position_type: PositionType::Relative,
                ..default()
            },
        ));
        column.spawn((
            StoreAbout,
            Text::new(""),
            small,
            dim,
            Node {
                max_width: px(390),
                ..default()
            },
        ));
    });
}

/// A click on a thing in the backpack puts it in the chest; on a thing in the chest takes
/// it out. With Shift, every one of its kind.
#[allow(clippy::too_many_arguments)]
fn click(
    screen: Res<InventoryScreen>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    packed: Query<&Shown, Added<Pressed>>,
    stored: Query<&Stored, Added<Pressed>>,
    mut stores: Query<(&mut Storage, &Prop)>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
) {
    let Some(open) = screen.storage.filter(|_| screen.open) else {
        return;
    };
    if !left_click(&mouse) {
        return;
    }
    let Ok((mut storage, prop)) = stores.get_mut(open) else {
        return;
    };
    let Some(mut carried) = belongings.mine() else {
        return;
    };
    let all = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let name = prop.item.name.to_lowercase();
    for shown in &packed {
        let ids: Vec<ItemId> = if all {
            same_kind(carried.backpack().iter(), shown.1)
        } else {
            vec![shown.0]
        };
        let moved = ids
            .into_iter()
            .filter(|&id| carried.put_into(&mut storage.0, id).is_ok())
            .count();
        if moved == 0 {
            notice.say(format!("No room in the {name}"));
        }
    }
    for thing in &stored {
        let ids: Vec<ItemId> = if all {
            same_kind(storage.0.iter(), thing.1)
        } else {
            vec![thing.0]
        };
        let moved = ids
            .into_iter()
            .filter(|&id| carried.take_out_of(&mut storage.0, id).is_ok())
            .count();
        if moved == 0 {
            notice.say("No room in your backpack");
        }
    }
}

/// Every item of a kind in a grid.
fn same_kind<'a>(
    placed: impl Iterator<Item = &'a terra_items::Placed>,
    kind: ItemKind,
) -> Vec<ItemId> {
    placed
        .filter(|placed| placed.item.kind == kind)
        .map(|placed| placed.item.id)
        .collect()
}

/// Shows the column while a chest is open, and draws its grid again when what is in it
/// changes or another is opened.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn lay_out(
    mut commands: Commands,
    items: Res<Items>,
    screen: Res<InventoryScreen>,
    stores: Query<(Ref<Storage>, &Prop)>,
    mut beside: Query<&mut Node, (With<Beside>, Without<StoreGrid>)>,
    mut grid: Query<(Entity, &mut Node), With<StoreGrid>>,
    mut title: Query<&mut Text, With<StoreTitle>>,
    mut icons: ResMut<ItemIcons>,
    assets: Res<AssetServer>,
    mut drawn: Local<Option<Entity>>,
) {
    let open = screen
        .storage
        .filter(|_| screen.open)
        .and_then(|entity| stores.get(entity).ok().map(|store| (entity, store)));
    let display = if open.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut beside {
        if node.display != display {
            node.display = display;
        }
    }
    let Some((entity, (storage, prop))) = open else {
        *drawn = None;
        return;
    };
    if *drawn == Some(entity) && !storage.is_changed() {
        return;
    }
    *drawn = Some(entity);
    for mut text in &mut title {
        text.0.clone_from(&prop.item.name);
    }
    let Ok((area, mut node)) = grid.single_mut() else {
        return;
    };
    lay_grid(
        &mut commands,
        (area, &mut node),
        &storage.0,
        &items,
        (&mut icons, &assets),
        |placed| Stored(placed.item.id, placed.item.kind),
    );
}

/// Outlines what the pointer is on, and says what it is, what a click does and how full
/// the chest is.
#[allow(clippy::type_complexity)]
fn draw(
    items: Res<Items>,
    screen: Res<InventoryScreen>,
    stores: Query<&Storage>,
    mut stored: Query<(&Hovered, Has<Pressed>, &Stored, &mut BorderColor)>,
    packed: Query<(&Hovered, Has<Pressed>, &Shown)>,
    mut texts: ParamSet<(
        Single<&mut Text, With<StoreAbout>>,
        Single<&mut Text, With<StoreCapacity>>,
    )>,
) {
    let Some(storage) = screen
        .storage
        .filter(|_| screen.open)
        .and_then(|entity| stores.get(entity).ok())
    else {
        return;
    };
    let mut about = None;
    for (hovered, pressed, thing, mut border) in &mut stored {
        let pointed = pointer_on(hovered, pressed);
        let wanted = BorderColor::all(if pointed { POINTED } else { Color::NONE });
        if *border != wanted {
            *border = wanted;
        }
        if pointed {
            about = Some(format!(
                "{}: {} here.  Click to take it, Shift and click to take all of them.",
                items.name(thing.1),
                storage.0.count(thing.1)
            ));
        }
    }
    for (hovered, pressed, shown) in &packed {
        if pointer_on(hovered, pressed) {
            about = Some(format!(
                "{}.  Click to store it, Shift and click to store all of them.",
                items.name(shown.1)
            ));
        }
    }
    let about = about.unwrap_or_else(|| {
        "Building in this claim takes what it needs from your pack first, then from \
         what is stored here."
            .to_string()
    });
    let mut text = texts.p0();
    if text.0 != about {
        text.0 = about;
    }
    let size = storage.0.size();
    let cells = size.w as usize * size.h as usize;
    let used: usize = storage
        .0
        .iter()
        .map(|placed| placed.size.w as usize * placed.size.h as usize)
        .sum();
    let capacity = format!("{} of {} cells free.", cells - used, cells);
    let mut text = texts.p1();
    if text.0 != capacity {
        text.0 = capacity;
    }
}
