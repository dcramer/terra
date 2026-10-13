//! A station's screen (D109; crafting.md, the station screen): the build menu's look,
//! with the recipes the station makes for tiles, each with what it takes as have/need,
//! greyed when it cannot be made. Below them, what the pointed recipe does, and the
//! station's queue. The backpack is open beside it. The world does not pause.
//!
//!   F                 open and close, at a station
//!   Click or Enter    queue one of a recipe          Arrows   move between recipes
//!   1 to 9            a tab, where a station has them
//!   Click the queue   take one off it, and get back what it took

use super::bag::Shown;
use super::button::{button, left_click, pointer_on};
use super::crafting::{self, Crafts, Recipes, Staff, Station, Works};
use super::inventory::{Belongings, InventoryScreen, Items, Made};
use super::tiles::{self, PICKED, PLAIN, TILE};
use super::told::Notice;
use bevy::{picking::hover::Hovered, prelude::*, ui::Pressed};

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

/// A recipe's tile, by its place in `Recipes`, and the line on it saying what it takes.
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
fn shown(recipes: &Recipes, station: Station, tab: usize, items: &Items) -> Vec<usize> {
    let families = recipes.families(station, items);
    let family = families.get(tab).copied();
    recipes
        .at(station)
        .filter(|&index| family.is_none_or(|family| recipes[index].family(items) == family))
        .collect()
}

/// Number keys choose a tab, unless the pointer is on something in the backpack, where
/// they put it on the bar. The arrows move between recipes; Enter queues one.
fn choose(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<InventoryScreen>,
    staff: Staff,
    crafts: Crafts,
    backpack: Query<(&Hovered, Has<Pressed>), With<Shown>>,
    mut stations: Query<&mut Works>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut state: ResMut<StationScreen>,
) {
    let Some(mut belongings) = belongings.mine() else {
        return;
    };
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
        .any(|(hovered, pressed)| pointer_on(hovered, pressed));
    let tabs = crafts.recipes.families(station, belongings.items()).len();
    if !on_backpack
        && let Some(tab) = NUMBERS.iter().position(|&key| keys.just_pressed(key))
        && tab < tabs
    {
        state.tab = tab;
        state.cursor = 0;
    }
    let list = shown(&crafts.recipes, station, state.tab, belongings.items());
    state.cursor = tiles::step(&keys, state.cursor, list.len(), COLUMNS);
    if keys.just_pressed(KeyCode::Enter)
        && let Some(&index) = list.get(state.cursor)
        && let Ok(mut works) = stations.get_mut(entity)
    {
        let staffed = staff.works(entity, station);
        let kinds = (&*crafts.recipes, &*crafts.stations);
        crafting::queue(
            &mut works,
            index,
            staffed,
            kinds,
            &mut belongings,
            &mut notice,
        );
    }
}

/// A click on a recipe queues one; on something queued, takes it off; on a tab, shows it.
#[allow(clippy::too_many_arguments)]
fn click(
    screen: Res<InventoryScreen>,
    staff: Staff,
    crafts: Crafts,
    mouse: Res<ButtonInput<MouseButton>>,
    recipes: Query<&Tile, Added<Pressed>>,
    queued: Query<&Queued, Added<Pressed>>,
    tabs: Query<&TabButton, Added<Pressed>>,
    mut stations: Query<&mut Works>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut state: ResMut<StationScreen>,
) {
    let Some(mut belongings) = belongings.mine() else {
        return;
    };
    let Some((entity, station)) = screen.at else {
        return;
    };
    if !left_click(&mouse) {
        return;
    }
    let staffed = staff.works(entity, station);
    for tab in &tabs {
        state.tab = tab.0;
        state.cursor = 0;
    }
    let Ok(mut works) = stations.get_mut(entity) else {
        return;
    };
    if let Some(tile) = recipes.iter().next() {
        let kinds = (&*crafts.recipes, &*crafts.stations);
        crafting::queue(
            &mut works,
            tile.0,
            staffed,
            kinds,
            &mut belongings,
            &mut notice,
        );
    }
    if let Some(place) = queued.iter().next() {
        let recipes = &crafts.recipes;
        crafting::cancel(&mut works, place.0, recipes, &mut belongings, &mut notice);
    }
}

/// Spawns the tabs and tiles when the station or tab changes, and the queue's tiles when
/// the queue does.
#[allow(clippy::too_many_arguments)]
fn lay_out(
    items: Res<Items>,
    recipes: Res<Recipes>,
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
        let families = recipes.families(station, &items);
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
                for index in shown(&recipes, station, state.tab, &items) {
                    let name = recipes[index].makes.name(&items);
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
                        button(),
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
                            Text::new(recipes[index].makes.name(&items)),
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
fn about(items: &Items, made: Made) -> String {
    let kind = made.kind;
    let def = items.def(kind);
    let said = def.about.clone().unwrap_or_default();
    match &def.tool {
        Some(tool) if tool.shoots.is_some() => {
            let (damage, speed) = super::archery::bow();
            format!("{said}. Hits for {damage:.0} at a full draw, at {speed:.0} m a second")
        }
        Some(tool) => match tool.strikes() {
            Some((damage, reach)) => format!("{said}. Hits for {damage:.0}, reaching {reach:.1} m"),
            None => said,
        },
        None if def.worn.is_some() => said,
        None => match items.food(kind) {
            fed if fed > 0.0 => format!("Food: fills you by {fed:.0}"),
            _ => format!("{}, for making other things", def.name),
        },
    }
}

/// Who works a station, or how it works.
fn who(entity: Entity, station: Station, staff: &Staff, stations: &crafting::Stations) -> String {
    if let Some(trade) = stations.worked_by(station) {
        let name = stations.name(station).to_lowercase();
        return match staff.worker(entity, station) {
            Some(person) => format!("{} works this {name}", person.name),
            None => format!("No {trade} works this {name} yet"),
        };
    }
    if stations.works_alone(station) {
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
    staff: Staff,
    crafts: Crafts,
    state: Res<StationScreen>,
    mut belongings: Belongings,
    stations: Query<&Works>,
    mut panel: Single<&mut Visibility, With<Panel>>,
    mut tabs: Query<(&TabButton, &mut BackgroundColor), (Without<Tile>, Without<Queued>)>,
    mut tiles_shown: Query<
        (
            &Tile,
            &Hovered,
            Has<Pressed>,
            &mut BorderColor,
            &mut BackgroundColor,
        ),
        (Without<TabButton>, Without<Queued>),
    >,
    mut queued: Query<
        (&Hovered, Has<Pressed>, &mut BorderColor),
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
    let Some(belongings) = belongings.mine() else {
        return;
    };
    tiles::show(&mut panel, screen.at.is_some());
    let Some((entity, station)) = screen.at else {
        return;
    };
    let set = |text: &mut Text, to: String| {
        if text.0 != to {
            text.0 = to;
        }
    };
    let recipes = &crafts.recipes;
    set(&mut texts.p0(), crafts.stations.name(station).to_string());
    set(
        &mut texts.p1(),
        who(entity, station, &staff, &crafts.stations),
    );
    let running = staff.works(entity, station);
    for (tab, mut background) in &mut tabs {
        tiles::fill(
            &mut background,
            if tab.0 == state.tab { PICKED } else { PLAIN },
        );
    }

    let list = shown(recipes, station, state.tab, belongings.items());
    let mut pointed = list.get(state.cursor).copied();
    for (tile, hovered, pressed, mut border, mut background) in &mut tiles_shown {
        let hovered = pointer_on(hovered, pressed);
        if hovered {
            pointed = Some(tile.0);
        }
        tiles::outline(
            &mut border,
            hovered || list.get(state.cursor) == Some(&tile.0),
        );
        let recipe = &recipes[tile.0];
        let can = running && belongings.missing(&recipe.cost, 1).is_none();
        tiles::fill(
            &mut background,
            if can { PLAIN } else { PLAIN.with_alpha(0.45) },
        );
    }
    for (line, mut text) in &mut texts.p2() {
        let have_need = recipes[line.0]
            .cost
            .iter()
            .map(|&(kind, need)| {
                let have = belongings.count(kind);
                format!("{} {have}/{need}", belongings.items().name(kind))
            })
            .collect::<Vec<_>>()
            .join("  ");
        set(&mut text, have_need);
    }

    let about_text = pointed.map_or(String::new(), |index| {
        let recipe = &recipes[index];
        let mut text = format!(
            "{}. Takes {:.0} s.",
            about(belongings.items(), recipe.makes),
            recipe.seconds
        );
        if let Some((kind, more)) = belongings.missing(&recipe.cost, 1) {
            let items = belongings.items();
            text += &format!(
                "\nNeeds {more} more {}: {}.",
                items.name(kind).to_lowercase(),
                items.found(kind)
            );
        }
        text
    });
    set(&mut texts.p3(), about_text);

    let Ok(works) = stations.get(entity) else {
        return;
    };
    for (line, mut text) in &mut texts.p4() {
        let status = match line.0 {
            0 if running => format!("{:.0}%", works.through(recipes) * 100.0),
            0 => "waiting".to_string(),
            _ => "next".to_string(),
        };
        set(&mut text, status);
    }
    for (hovered, pressed, mut border) in &mut queued {
        tiles::outline(&mut border, pointer_on(hovered, pressed));
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
    use crate::play::crafting::Stations;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn each_tab_shows_one_kind_of_thing_and_the_tabs_every_recipe_once() {
        let items = Items::default();
        let stations = Stations::read();
        let recipes = Recipes::read(&stations, &items);
        let mut tabbed = 0;
        for station in stations.all() {
            let families = recipes.families(station, &items);
            let mut seen = Vec::new();
            for (tab, &family) in families.iter().enumerate() {
                let list = shown(&recipes, station, tab, &items);
                assert!(!list.is_empty());
                assert!(
                    list.iter()
                        .all(|&index| recipes[index].family(&items) == family)
                );
                seen.extend(list);
            }
            seen.sort_unstable();
            assert_eq!(seen, recipes.at(station).collect::<Vec<_>>());
            tabbed += usize::from(families.len() > 1);
        }
        assert!(tabbed > 0, "no station has tabs, so none were tried");
    }

    #[test]
    fn a_station_shows_tabs_only_when_it_makes_more_than_one_kind_of_thing() {
        let stations = Stations::read();
        for station in stations.all() {
            let items = Items::default();
            let recipes = Recipes::read(&stations, &items);
            let families = recipes.families(station, &items).len();
            let mut world = World::new();
            world.insert_resource(items);
            world.insert_resource(recipes);
            world.init_resource::<StationScreen>();
            world.spawn(Tabs);
            world.spawn(Grid);
            world.spawn(Queue);
            let at = world.spawn(Works::default()).id();
            world.insert_resource(InventoryScreen {
                open: true,
                at: Some((at, station)),
            });
            world.run_system_once(lay_out).unwrap();
            let tabs = world.query::<&TabButton>().iter(&world).count();
            assert_eq!(tabs, if families > 1 { families } else { 0 });
        }
    }
}
