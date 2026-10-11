//! A station's screen (D109; crafting.md, the station screen): the build menu's look,
//! with the recipes the station makes for tiles, each with what it takes as have/need,
//! greyed when it cannot be made. Below them, what the pointed recipe does, and the
//! station's queue. The backpack is open beside it. The world does not pause.
//!
//!   F                 open and close, at a station
//!   Click or Enter    queue one of a recipe          Arrows   move between recipes
//!   1 to 9            a tab, where a station has them
//!   Click the queue   take one off it, and get back what it took

use super::bag::{Notice, Shown};
use super::combat::weapon;
use super::crafting::{self, RECIPES, SmithAtHand, Station, Works};
use super::inventory::{Belongings, InventoryScreen, Made, Tool};
use super::people::{Doing, Person};
use super::tiles::{self, PICKED, PLAIN, TILE};
use bevy::prelude::*;

const COLUMNS: usize = 2;
const QUEUE_TILE: Vec2 = Vec2::new(96.0, 44.0);

#[derive(Resource, Default)]
struct StationScreen {
    tab: usize,
    cursor: usize,
    /// The station and tab the tiles were laid out for, and the queue as last shown.
    laid_out: Option<(Entity, usize)>,
    queue_shown: Vec<usize>,
}

#[derive(Component)]
struct Panel;

#[derive(Component)]
struct Title;

#[derive(Component)]
struct Who;

#[derive(Component)]
struct Tabs;

#[derive(Component)]
struct TabButton(usize);

#[derive(Component)]
struct Grid;

/// A recipe's tile, by its place in `RECIPES`, and the line on it saying what it takes.
#[derive(Component)]
struct Tile(usize);

#[derive(Component)]
struct TileLine(usize);

#[derive(Component)]
struct About;

#[derive(Component)]
struct Queue;

/// A place in the queue, and the line on it saying how far along it is.
#[derive(Component)]
struct Queued(usize);

#[derive(Component)]
struct QueuedLine(usize);

#[derive(Component)]
struct Footer;

pub fn plugin(app: &mut App) {
    app.init_resource::<StationScreen>()
        .add_systems(Startup, setup)
        .add_systems(Update, (choose, click, lay_out, draw).chain());
}

fn setup(mut commands: Commands) {
    let white = TextColor(Color::WHITE);
    commands
        .spawn((
            Panel,
            Node {
                position_type: PositionType::Absolute,
                left: px(24),
                top: percent(10),
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|screen| {
            screen.spawn(tiles::panel()).with_children(|panel| {
                panel.spawn((Title, Text::new(""), TextFont::from_font_size(20.0), white));
                panel.spawn((
                    Who,
                    Text::new(""),
                    TextFont::from_font_size(12.0),
                    tiles::FAINT,
                ));
                panel.spawn((
                    Tabs,
                    Node {
                        column_gap: px(6),
                        ..default()
                    },
                ));
                panel.spawn((Grid, tiles::grid(COLUMNS, 2)));
                panel.spawn((
                    About,
                    Text::new(""),
                    TextFont::from_font_size(13.0),
                    white,
                    Node {
                        max_width: px(COLUMNS as f32 * (TILE.x + tiles::GAP)),
                        min_height: px(36),
                        ..default()
                    },
                ));
                panel.spawn((
                    Text::new("Queue"),
                    TextFont::from_font_size(12.0),
                    tiles::FAINT,
                ));
                panel.spawn((
                    Queue,
                    Node {
                        column_gap: px(6),
                        min_height: px(QUEUE_TILE.y),
                        max_width: px(COLUMNS as f32 * (TILE.x + tiles::GAP)),
                        flex_wrap: FlexWrap::Wrap,
                        row_gap: px(6),
                        ..default()
                    },
                ));
                panel.spawn((
                    Footer,
                    Text::new(""),
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

/// The recipes shown: the station's, in the tab chosen.
fn shown(station: Station, tab: usize) -> Vec<usize> {
    let families = crafting::families(station);
    let family = families.get(tab).copied();
    crafting::recipes(station)
        .filter(|&index| family.is_none_or(|family| RECIPES[index].family() == family))
        .collect()
}

/// Number keys choose a tab, unless the pointer is on something in the backpack, where
/// they put it on the bar. The arrows move between recipes; Enter queues one.
fn choose(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<InventoryScreen>,
    smith: Res<SmithAtHand>,
    backpack: Query<&Interaction, With<Shown>>,
    mut stations: Query<&mut Works>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut state: ResMut<StationScreen>,
) {
    let Some((entity, station)) = screen.at else {
        return;
    };
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
    let on_backpack = backpack
        .iter()
        .any(|interaction| *interaction != Interaction::None);
    let tabs = crafting::families(station).len();
    if !on_backpack
        && let Some(tab) = NUMBERS.iter().position(|&key| keys.just_pressed(key))
        && tab < tabs
    {
        state.tab = tab;
        state.cursor = 0;
    }
    let list = shown(station, state.tab);
    state.cursor = tiles::step(&keys, state.cursor, list.len(), COLUMNS);
    if keys.just_pressed(KeyCode::Enter)
        && let Some(&index) = list.get(state.cursor)
        && let Ok(mut works) = stations.get_mut(entity)
    {
        crafting::queue(&mut works, index, &smith, &mut belongings, &mut notice);
    }
}

/// A click on a recipe queues one; on something queued, takes it off; on a tab, shows it.
#[allow(clippy::too_many_arguments)]
fn click(
    screen: Res<InventoryScreen>,
    smith: Res<SmithAtHand>,
    recipes: Query<(&Interaction, &Tile), Changed<Interaction>>,
    queued: Query<(&Interaction, &Queued), Changed<Interaction>>,
    tabs: Query<(&Interaction, &TabButton), Changed<Interaction>>,
    mut stations: Query<&mut Works>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut state: ResMut<StationScreen>,
) {
    let Some((entity, _)) = screen.at else { return };
    let pressed = |interaction: &Interaction| *interaction == Interaction::Pressed;
    for (_, tab) in tabs.iter().filter(|(interaction, _)| pressed(interaction)) {
        state.tab = tab.0;
        state.cursor = 0;
    }
    let Ok(mut works) = stations.get_mut(entity) else {
        return;
    };
    if let Some((_, tile)) = recipes.iter().find(|(interaction, _)| pressed(interaction)) {
        crafting::queue(&mut works, tile.0, &smith, &mut belongings, &mut notice);
    }
    if let Some((_, place)) = queued.iter().find(|(interaction, _)| pressed(interaction)) {
        crafting::cancel(&mut works, place.0, &mut belongings, &mut notice);
    }
}

/// Spawns the tabs and tiles when the station or tab changes, and the queue's tiles when
/// the queue does.
#[allow(clippy::too_many_arguments)]
fn lay_out(
    mut commands: Commands,
    screen: Res<InventoryScreen>,
    stations: Query<&Works>,
    tabs: Single<Entity, With<Tabs>>,
    grid: Single<Entity, With<Grid>>,
    queue: Single<Entity, With<Queue>>,
    mut state: ResMut<StationScreen>,
) {
    let Some((entity, station)) = screen.at else {
        state.laid_out = None;
        return;
    };
    // A station opened afresh shows its first tab.
    if state.laid_out.is_none_or(|(shown, _)| shown != entity) {
        state.tab = 0;
        state.cursor = 0;
        state.queue_shown.clear();
    }
    if state.laid_out != Some((entity, state.tab)) {
        state.laid_out = Some((entity, state.tab));
        let families = crafting::families(station);
        commands
            .entity(*tabs)
            .despawn_children()
            .with_children(|row| {
                if families.len() > 1 {
                    for (index, family) in families.iter().enumerate() {
                        tiles::tab(row, TabButton(index), index, family.name());
                    }
                }
            });
        commands
            .entity(*grid)
            .despawn_children()
            .with_children(|grid| {
                for index in shown(station, state.tab) {
                    let name = RECIPES[index].makes.name();
                    tiles::tile(grid, Tile(index), &name, (TileLine(index), Text::new("")));
                }
            });
    }
    let Ok(works) = stations.get(entity) else {
        return;
    };
    let now: Vec<usize> = works.queue.iter().copied().collect();
    if now != state.queue_shown {
        state.queue_shown = now;
        commands
            .entity(*queue)
            .despawn_children()
            .with_children(|row| {
                for (place, &index) in works.queue.iter().enumerate() {
                    row.spawn((
                        Queued(place),
                        Button,
                        Node {
                            width: px(QUEUE_TILE.x),
                            height: px(QUEUE_TILE.y),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(px(6)),
                            border: UiRect::all(px(2)),
                            border_radius: BorderRadius::all(px(6)),
                            ..default()
                        },
                        BackgroundColor(PLAIN),
                        BorderColor::all(Color::NONE),
                    ))
                    .with_children(|tile| {
                        tile.spawn((
                            Text::new(RECIPES[index].makes.name()),
                            TextFont::from_font_size(12.0),
                            TextColor(Color::WHITE),
                        ));
                        tile.spawn((
                            QueuedLine(place),
                            Text::new(""),
                            TextFont::from_font_size(10.0),
                            tiles::FAINT,
                        ));
                    });
                }
            });
    }
}

/// What a thing does, for the strip beneath the tiles.
fn about(made: Made) -> String {
    match made {
        Made::Tool(Tool::Hammer) => Tool::Hammer.about().to_string(),
        Made::Tool(Tool::Bow) => {
            let (damage, speed) = super::archery::bow();
            format!(
                "{}. Hits for {damage:.0} at a full draw, at {speed:.0} m a second",
                Tool::Bow.about()
            )
        }
        Made::Tool(tool) => {
            let (damage, reach) = weapon(tool);
            format!(
                "{}. Hits for {damage:.0}, reaching {reach:.1} m",
                tool.about()
            )
        }
        Made::Apparel(apparel) => apparel.about().to_string(),
        Made::Container(container) => container.about(),
        Made::Raw(raw, _) => match super::vitals::nourishment(raw) {
            fed if fed > 0.0 => format!("Food: fills you by {fed:.0}"),
            _ => format!("{}, for making other things", raw.name()),
        },
    }
}

/// Who works a station, or how it works.
fn who(station: Station, smith: &SmithAtHand, people: &Query<&Person>) -> String {
    if station == Station::Forge {
        return match people
            .iter()
            .find(|person| person.smith && matches!(person.doing, Doing::Working(_)))
        {
            Some(person) if smith.0 => format!("{} works this forge", person.name),
            _ => "No smith works this forge yet".to_string(),
        };
    }
    if station.works_alone() {
        "Works on its own: queue things and come back for them".to_string()
    } else {
        "Works while you stay near it".to_string()
    }
}

/// Shows the screen: whose station, the tiles greyed by what can be made, what the pointed
/// recipe does, and how far along the queue is.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn draw(
    screen: Res<InventoryScreen>,
    smith: Res<SmithAtHand>,
    state: Res<StationScreen>,
    belongings: Belongings,
    stations: Query<&Works>,
    people: Query<&Person>,
    mut panel: Single<&mut Visibility, With<Panel>>,
    mut tabs: Query<(&TabButton, &mut BackgroundColor), (Without<Tile>, Without<Queued>)>,
    mut tiles_shown: Query<
        (&Tile, &Interaction, &mut BorderColor, &mut BackgroundColor),
        (Without<TabButton>, Without<Queued>),
    >,
    mut queued: Query<
        (&Interaction, &mut BorderColor),
        (With<Queued>, Without<Tile>, Without<TabButton>),
    >,
    mut texts: ParamSet<(
        Single<&mut Text, With<Title>>,
        Single<&mut Text, With<Who>>,
        Query<(&TileLine, &mut Text)>,
        Single<&mut Text, With<About>>,
        Query<(&QueuedLine, &mut Text)>,
        Single<&mut Text, With<Footer>>,
    )>,
) {
    tiles::show(&mut panel, screen.at.is_some());
    let Some((entity, station)) = screen.at else {
        return;
    };
    let set = |text: &mut Text, to: String| {
        if text.0 != to {
            text.0 = to;
        }
    };
    set(&mut texts.p0(), station.name().to_string());
    set(&mut texts.p1(), who(station, &smith, &people));
    for (tab, mut background) in &mut tabs {
        tiles::fill(
            &mut background,
            if tab.0 == state.tab { PICKED } else { PLAIN },
        );
    }

    let list = shown(station, state.tab);
    let mut pointed = list.get(state.cursor).copied();
    for (tile, interaction, mut border, mut background) in &mut tiles_shown {
        let hovered = *interaction != Interaction::None;
        if hovered {
            pointed = Some(tile.0);
        }
        tiles::outline(
            &mut border,
            hovered || list.get(state.cursor) == Some(&tile.0),
        );
        let recipe = &RECIPES[tile.0];
        let can = smith.works(station) && belongings.missing(recipe.cost, 1).is_none();
        tiles::fill(
            &mut background,
            if can { PLAIN } else { PLAIN.with_alpha(0.45) },
        );
    }
    for (line, mut text) in &mut texts.p2() {
        let have_need = RECIPES[line.0]
            .cost
            .iter()
            .map(|&(raw, need)| format!("{} {}/{need}", raw.name(), belongings.count(raw)))
            .collect::<Vec<_>>()
            .join("  ");
        set(&mut text, have_need);
    }

    let about_text = pointed.map_or(String::new(), |index| {
        let recipe = &RECIPES[index];
        let mut text = format!("{}. Takes {:.0} s.", about(recipe.makes), recipe.seconds);
        if let Some((raw, more)) = belongings.missing(recipe.cost, 1) {
            text += &format!(
                "\nNeeds {more} more {}: {}.",
                raw.name().to_lowercase(),
                raw.where_found()
            );
        }
        text
    });
    set(&mut texts.p3(), about_text);

    let Ok(works) = stations.get(entity) else {
        return;
    };
    let running = smith.works(station);
    for (line, mut text) in &mut texts.p4() {
        let status = match line.0 {
            0 if running => format!("{:.0}%", works.through() * 100.0),
            0 => "waiting".to_string(),
            _ => "next".to_string(),
        };
        set(&mut text, status);
    }
    for (interaction, mut border) in &mut queued {
        tiles::outline(&mut border, *interaction != Interaction::None);
    }
    let footer = if works.queue.is_empty() {
        "Click a recipe, or use the arrows and Enter, to queue one. F to leave."
    } else {
        "Click something in the queue to take it off and get back what it took. F to leave."
    };
    set(&mut texts.p5(), footer.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiments::play::crafting::Family;

    #[test]
    fn a_tab_shows_only_its_family() {
        let tools = shown(Station::Workbench, 0);
        let clothing = shown(Station::Workbench, 1);
        assert!(
            tools
                .iter()
                .all(|&index| RECIPES[index].family() == Family::Tools)
        );
        assert!(
            clothing
                .iter()
                .all(|&index| RECIPES[index].family() == Family::Clothing)
        );
        assert_eq!(
            tools.len() + clothing.len(),
            crafting::recipes(Station::Workbench).count()
        );
    }
}
