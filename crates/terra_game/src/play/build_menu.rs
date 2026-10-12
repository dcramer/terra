//! The build menu (docs/construction.md, the build menu; D67): with the hammer in hand,
//! tap the build menu key for a panel of tabs, each a grid of what can be built, with
//! what each costs. Pick one and it is in hand; hold R for its variants (`dial`). The
//! world does not pause.
//!
//!   Right mouse or B   open and close         1 to 7    a tab
//!   Arrows             move between tiles     Enter     pick
//!   Backspace          put the hammer's part away

use super::IsLocalPlayer;
use super::catalogue::{self, Kind, PARTS, Tab};
use super::controls::{Action, Settings};
use super::grab_mouse;
use super::inventory::{Belongings, InventoryScreen, describe};
use super::options::OptionsMenu;
use super::tiles::{self, PICKED, PLAIN, TILE};
use super::tool::{self, Builder};
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

const COLUMNS: usize = 4;
const RECENT: usize = 6;

#[derive(Resource, Default)]
pub struct BuildMenu {
    pub open: bool,
    tab: usize,
    /// The tile the keys are on, within the tab.
    cursor: usize,
    /// The parts built from most recently, newest first.
    recent: Vec<usize>,
    /// Whether the tiles need drawing again.
    stale: bool,
}

impl BuildMenu {
    fn parts(&self) -> Vec<usize> {
        let tab = Tab::SHOWN[self.tab];
        PARTS
            .iter()
            .enumerate()
            .filter(|(_, part)| part.tab == tab)
            .map(|(index, _)| index)
            .collect()
    }
}

#[derive(Component)]
struct Panel;

#[derive(Component)]
struct TabButton(usize);

#[derive(Component)]
struct Grid;

#[derive(Component)]
struct Recent;

/// A tile for a part, and whether it is in the recent row.
#[derive(Component)]
struct Tile {
    part: usize,
    recent: bool,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<BuildMenu>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (toggle, navigate, click, lay_out, draw)
                .chain()
                .before(tool::Acting),
        );
}

fn setup(mut commands: Commands) {
    let white = TextColor(Color::WHITE);
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
            screen.spawn(tiles::panel()).with_children(|panel| {
                panel.spawn((Text::new("Build"), TextFont::from_font_size(20.0), white));
                panel
                    .spawn(Node {
                        column_gap: px(6),
                        ..default()
                    })
                    .with_children(|tabs| {
                        for (index, tab) in Tab::SHOWN.iter().enumerate() {
                            tiles::tab(tabs, TabButton(index), index, tab.name());
                        }
                    });
                panel.spawn((
                    Text::new("Recent"),
                    TextFont::from_font_size(12.0),
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                ));
                panel.spawn((
                    Recent,
                    Node {
                        column_gap: px(8),
                        min_height: px(TILE.y),
                        ..default()
                    },
                ));
                panel.spawn((Grid, tiles::grid(COLUMNS, 2)));
                panel.spawn((
                    Text::new(
                        "Click a tile, or 1 to 7 for a tab, the arrows and Enter. Hold R in the \
                             world for a thing's kinds. Backspace puts the part away.",
                    ),
                    TextFont::from_font_size(12.0),
                    tiles::FAINT,
                    Node {
                        max_width: px(COLUMNS as f32 * (TILE.x + tiles::GAP)),
                        ..default()
                    },
                ));
            });
        });
}

/// The build menu key opens and closes the menu with the hammer in hand; while carrying
/// something it puts it back instead, as before.
#[allow(clippy::too_many_arguments)]
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    settings: Res<Settings>,
    options: Res<OptionsMenu>,
    screen: Res<InventoryScreen>,
    belongings: Belongings,
    window: Single<(&Window, &mut CursorOptions)>,
    mut menu: ResMut<BuildMenu>,
    mut builder: Single<&mut Builder, IsLocalPlayer>,
) {
    let (window, mut cursor) = window.into_inner();
    let hammer = belongings.tool_in_hand().is_some_and(|tool| tool.builds);
    if menu.open && (!hammer || screen.open || options.open) {
        menu.open = false;
        return;
    }
    if options.open || screen.open || !hammer {
        return;
    }
    let captured = cursor.grab_mode != CursorGrabMode::None;
    // With the menu open the pointer is free, so its button is read directly.
    let pressed = settings.went_down(Action::BuildMenu, &keys, &mouse, captured)
        || (menu.open && mouse.just_pressed(MouseButton::Right));
    if !pressed {
        return;
    }
    if menu.open {
        menu.open = false;
        grab_mouse(&mut cursor, window.focused);
    } else if builder.is_carrying() {
        builder.cancel = true;
    } else {
        let part = builder.part;
        if let Some(tab) = Tab::SHOWN.iter().position(|&tab| tab == PARTS[part].tab) {
            menu.tab = tab;
            menu.cursor = menu
                .parts()
                .iter()
                .position(|&each| each == part)
                .unwrap_or(0);
        }
        menu.open = true;
        menu.stale = true;
        grab_mouse(&mut cursor, false);
    }
}

/// Number keys pick a tab, the arrows move between tiles, Enter picks one.
#[allow(clippy::too_many_arguments)]
fn navigate(
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<(&Window, &mut CursorOptions)>,
    mut menu: ResMut<BuildMenu>,
    mut builder: Single<&mut Builder, IsLocalPlayer>,
) {
    if !menu.open {
        return;
    }
    const NUMBERS: [KeyCode; 7] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
    ];
    if let Some(tab) = NUMBERS.iter().position(|&key| keys.just_pressed(key)) {
        menu.tab = tab;
        menu.cursor = 0;
        menu.stale = true;
    }
    let count = menu.parts().len();
    menu.cursor = tiles::step(&keys, menu.cursor, count, COLUMNS);
    let (window, mut cursor) = window.into_inner();
    if keys.just_pressed(KeyCode::Enter)
        && let Some(&part) = menu.parts().get(menu.cursor)
    {
        pick(&mut menu, &mut builder, part);
        grab_mouse(&mut cursor, window.focused);
    }
    if keys.just_pressed(KeyCode::Backspace) {
        builder.stowed = true;
        menu.open = false;
        grab_mouse(&mut cursor, window.focused);
    }
}

fn pick(menu: &mut BuildMenu, builder: &mut Builder, part: usize) {
    builder.pick(part);
    menu.recent.retain(|&each| each != part);
    menu.recent.insert(0, part);
    menu.recent.truncate(RECENT);
    menu.open = false;
    menu.stale = true;
    info!("Picked {} from the build menu.", PARTS[part].name);
}

/// A click on a tile picks it; on a tab, shows that tab.
fn click(
    tiles: Query<(&Interaction, &Tile), Changed<Interaction>>,
    tabs: Query<(&Interaction, &TabButton), Changed<Interaction>>,
    window: Single<(&Window, &mut CursorOptions)>,
    mut menu: ResMut<BuildMenu>,
    mut builder: Single<&mut Builder, IsLocalPlayer>,
) {
    if !menu.open {
        return;
    }
    for (interaction, tab) in &tabs {
        if *interaction == Interaction::Pressed {
            menu.tab = tab.0;
            menu.cursor = 0;
            menu.stale = true;
        }
    }
    let picked = tiles
        .iter()
        .find(|(interaction, _)| **interaction == Interaction::Pressed)
        .map(|(_, tile)| tile.part);
    if let Some(part) = picked {
        pick(&mut menu, &mut builder, part);
        let (window, mut cursor) = window.into_inner();
        grab_mouse(&mut cursor, window.focused);
    }
}

/// What a part costs, as a tile says it.
fn cost_of(part: usize, variant: usize) -> String {
    let wall = |cost| describe(cost, 1);
    match PARTS[part].kind {
        Kind::Items(items) => describe(items[variant.min(items.len() - 1)].cost, 1),
        Kind::Room(_) | Kind::Reshape => format!("{} per 3 m² of floor", wall(catalogue::floor())),
        Kind::Wall(_) => format!("{} per 1.5 m", wall(catalogue::wall())),
        Kind::Window(_) => format!("{} per metre", wall(catalogue::window())),
        Kind::Door => wall(catalogue::door()),
        Kind::Remove | Kind::Move => String::new(),
    }
}

/// Spawns the tiles for the tab shown, and the recent row, when they change.
fn lay_out(
    mut commands: Commands,
    builder: Single<&Builder, IsLocalPlayer>,
    grid: Single<Entity, With<Grid>>,
    recent: Single<Entity, With<Recent>>,
    mut menu: ResMut<BuildMenu>,
) {
    if !menu.open || !menu.stale {
        return;
    }
    menu.stale = false;
    let line = |part: usize| {
        let variants = PARTS[part].variants();
        let kinds = if variants > 1 {
            format!("{variants} kinds   ")
        } else {
            String::new()
        };
        format!("{kinds}{}", cost_of(part, builder.variant_of(part)))
    };
    commands
        .entity(*grid)
        .despawn_children()
        .with_children(|grid| {
            for part in menu.parts() {
                let tile = Tile {
                    part,
                    recent: false,
                };
                tiles::tile(grid, tile, PARTS[part].name, Text::new(line(part)));
            }
        });
    commands
        .entity(*recent)
        .despawn_children()
        .with_children(|row| {
            for &part in &menu.recent {
                let tile = Tile { part, recent: true };
                tiles::tile(row, tile, PARTS[part].name, Text::new(line(part)));
            }
        });
}

/// Shows the menu, the tab chosen, the tile the keys are on, and what cannot be paid for.
#[allow(clippy::type_complexity)]
fn draw(
    menu: Res<BuildMenu>,
    builder: Single<&Builder, IsLocalPlayer>,
    belongings: Belongings,
    mut panel: Single<&mut Visibility, With<Panel>>,
    mut tabs: Query<(&TabButton, &mut BackgroundColor), Without<Tile>>,
    mut tiles: Query<
        (&Tile, &Interaction, &mut BorderColor, &mut BackgroundColor),
        Without<TabButton>,
    >,
) {
    tiles::show(&mut panel, menu.open);
    if !menu.open {
        return;
    }
    for (tab, mut background) in &mut tabs {
        tiles::fill(
            &mut background,
            if tab.0 == menu.tab { PICKED } else { PLAIN },
        );
    }
    let cursor_part = menu.parts().get(menu.cursor).copied();
    for (tile, interaction, mut border, mut background) in &mut tiles {
        let pointed =
            *interaction != Interaction::None || (!tile.recent && Some(tile.part) == cursor_part);
        tiles::outline(&mut border, pointed);
        // Greyed when its current kind cannot be paid for.
        let short = match PARTS[tile.part].kind {
            Kind::Items(items) => {
                let item = &items[builder.variant_of(tile.part).min(items.len() - 1)];
                belongings.missing(item.cost, 1).is_some()
            }
            _ => false,
        };
        let colour = if tile.part == builder.part && !builder.stowed {
            PICKED.with_alpha(0.55)
        } else if short {
            PLAIN.with_alpha(0.45)
        } else {
            PLAIN
        };
        tiles::fill(&mut background, colour);
    }
}
