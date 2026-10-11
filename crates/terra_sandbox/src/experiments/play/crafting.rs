//! Crafting at stations (game design 001, crafting.md): stand at a workbench or a smithy
//! and press the interact key to see what it makes; pick a recipe with its number or a
//! click. Making pays from the backpack and puts the result in it, all or nothing.
//!
//! The smithy only works with a smith living at your claim, as Enshrouded's rescued
//! survivors run its stations; until one is rescued it says so.
//!
//!   F             open or close the station you are standing at
//!   1 to 9        make that recipe

use super::bag::Notice;
use super::grab_mouse;
use super::interact::{Focus, Focusing, Interactable};
use super::inventory::{Apparel, Belongings, CantMake, Cost, InventoryScreen, Made, Raw, Tool};
use bevy::{prelude::*, window::CursorOptions};

/// Where things are made.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Station {
    Workbench,
    Smithy,
}

impl Station {
    pub fn name(self) -> &'static str {
        match self {
            Station::Workbench => "Workbench",
            Station::Smithy => "Smithy",
        }
    }
}

/// Something a station makes, from what.
pub struct Recipe {
    pub station: Station,
    pub cost: Cost,
    pub makes: Made,
}

const fn recipe(station: Station, cost: Cost, makes: Made) -> Recipe {
    Recipe {
        station,
        cost,
        makes,
    }
}

/// Every recipe. A stand-in list, chosen so the MVP's loop can be walked: stone tools and
/// leather at the workbench, then iron at the smithy (docs/prototypes.md).
pub const RECIPES: &[Recipe] = &[
    recipe(
        Station::Workbench,
        &[(Raw::Wood, 2), (Raw::Stone, 2)],
        Made::Tool(Tool::StoneAxe),
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Wood, 2), (Raw::Stone, 3)],
        Made::Tool(Tool::StonePickaxe),
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Wood, 1), (Raw::Stone, 1)],
        Made::Tool(Tool::Knife),
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Hide, 2), (Raw::Fibre, 1)],
        Made::Apparel(Apparel::LeatherCap),
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Hide, 4), (Raw::Fibre, 2)],
        Made::Apparel(Apparel::LeatherCoat),
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Hide, 4), (Raw::Fibre, 4)],
        Made::Apparel(Apparel::LargePack),
    ),
    recipe(
        Station::Smithy,
        &[(Raw::Ore, 2), (Raw::Wood, 1)],
        Made::Raw(Raw::Iron, 1),
    ),
    recipe(
        Station::Smithy,
        &[(Raw::Wood, 2), (Raw::Iron, 3)],
        Made::Tool(Tool::IronAxe),
    ),
    recipe(
        Station::Smithy,
        &[(Raw::Wood, 2), (Raw::Iron, 3)],
        Made::Tool(Tool::IronPickaxe),
    ),
];

/// The recipes a station makes, in order.
pub fn recipes(station: Station) -> impl Iterator<Item = &'static Recipe> {
    RECIPES
        .iter()
        .filter(move |recipe| recipe.station == station)
}

/// Whether a smith lives at the claim, so the smithy works. Set when one is rescued.
#[derive(Resource, Default)]
pub struct SmithAtHand(pub bool);

impl SmithAtHand {
    fn works(&self, station: Station) -> bool {
        station != Station::Smithy || self.0
    }
}

#[derive(Component)]
struct Panel;

#[derive(Component)]
struct Title;

#[derive(Component)]
struct Row(usize);

#[derive(Component)]
struct RowText(usize);

#[derive(Component)]
struct Footer;

pub fn plugin(app: &mut App) {
    app.init_resource::<SmithAtHand>()
        .add_systems(Startup, setup)
        .add_systems(Update, (open, make, draw).chain().after(Focusing));
}

fn setup(mut commands: Commands) {
    let small = TextFont::from_font_size(12.0);
    let white = TextColor(Color::WHITE);
    commands
        .spawn((
            Panel,
            Node {
                position_type: PositionType::Absolute,
                left: px(24),
                top: percent(18),
                width: px(360),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                padding: UiRect::all(px(18)),
                border_radius: BorderRadius::all(px(12)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.9)),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel.spawn((Title, Text::new(""), TextFont::from_font_size(20.0), white));
            for index in 0..9 {
                panel
                    .spawn((
                        Row(index),
                        Button,
                        Node {
                            padding: UiRect::axes(px(10), px(6)),
                            border_radius: BorderRadius::all(px(6)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                    ))
                    .with_child((RowText(index), Text::new(""), small.clone(), white));
            }
            panel.spawn((
                Footer,
                Text::new(""),
                small,
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
            ));
        });
}

/// The interact key at a station opens it; again, or Tab, closes it.
fn open(
    focus: Res<Focus>,
    window: Single<(&Window, &mut CursorOptions)>,
    mut screen: ResMut<InventoryScreen>,
) {
    if !focus.pressed {
        return;
    }
    let (window, mut cursor) = window.into_inner();
    if screen.at.is_some() {
        screen.open = false;
        screen.at = None;
        grab_mouse(&mut cursor, window.focused);
    } else if let Some(Interactable::Station(station)) = focus.what
        && !screen.open
    {
        screen.open = true;
        screen.at = Some(station);
        grab_mouse(&mut cursor, false);
    }
}

/// Makes the recipe whose number is pressed or whose row is clicked.
fn make(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<InventoryScreen>,
    smith: Res<SmithAtHand>,
    rows: Query<(&Interaction, &Row), Changed<Interaction>>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
) {
    let Some(station) = screen.at else { return };
    const NUMBERS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    let pressed = NUMBERS.iter().position(|&key| keys.just_pressed(key));
    let clicked = rows
        .iter()
        .find(|(interaction, _)| **interaction == Interaction::Pressed)
        .map(|(_, row)| row.0);
    let Some(recipe) = pressed
        .or(clicked)
        .and_then(|index| recipes(station).nth(index))
    else {
        return;
    };
    if !smith.works(station) {
        notice.say("The smithy needs a smith to work it");
        return;
    }
    match belongings.craft(recipe.cost, recipe.makes) {
        Ok(()) => {
            info!("Made {}.", recipe.makes.name());
            notice.say(format!("Made {}", recipe.makes.name()));
        }
        Err(CantMake::Short(raw, more)) => {
            notice.say(format!("You need {more} more {}", raw.name()));
        }
        Err(CantMake::NoRoom) => notice.say("No room in your backpack"),
    }
}

#[allow(clippy::type_complexity)]
fn draw(
    screen: Res<InventoryScreen>,
    smith: Res<SmithAtHand>,
    belongings: Belongings,
    mut panel: Single<&mut Visibility, With<Panel>>,
    mut rows: Query<(&Row, &Interaction, &mut Node, &mut BackgroundColor)>,
    mut texts: ParamSet<(
        Single<&mut Text, With<Title>>,
        Query<(&RowText, &mut Text, &mut TextColor)>,
        Single<&mut Text, With<Footer>>,
    )>,
) {
    let wanted = if screen.at.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if **panel != wanted {
        **panel = wanted;
    }
    let Some(station) = screen.at else { return };
    let set = |text: &mut Text, to: String| {
        if text.0 != to {
            text.0 = to;
        }
    };
    set(&mut texts.p0(), station.name().to_string());
    let list: Vec<&Recipe> = recipes(station).collect();
    for (row, interaction, mut node, mut background) in &mut rows {
        let shown = row.0 < list.len();
        let display = if shown { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        let colour = if shown && *interaction != Interaction::None {
            Color::srgba(0.35, 0.75, 0.85, 0.35)
        } else {
            Color::NONE
        };
        if background.0 != colour {
            background.0 = colour;
        }
    }
    for (row, mut text, mut colour) in &mut texts.p1() {
        let Some(recipe) = list.get(row.0) else {
            continue;
        };
        let short = belongings.missing(recipe.cost, 1).is_some();
        set(
            &mut text,
            format!(
                "{}  {:<14} {}",
                row.0 + 1,
                recipe.makes.name(),
                super::inventory::describe(recipe.cost, 1)
            ),
        );
        let wanted = TextColor(if short || !smith.works(station) {
            Color::srgba(1.0, 1.0, 1.0, 0.45)
        } else {
            Color::WHITE
        });
        if *colour != wanted {
            *colour = wanted;
        }
    }
    set(
        &mut texts.p2(),
        if smith.works(station) {
            "Press a number or click to make it. F to leave.".to_string()
        } else {
            "No smith lives here to work it. Someone might be found who can.".to_string()
        },
    );
}
