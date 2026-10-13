//! Crafting at stations (D109; game design 001, crafting.md): each station keeps a queue
//! of things to make, paid for when queued. A worked station, such as the workbench or
//! the forge, gets on with it only while you stay within reach, and what it makes goes
//! into your backpack. One that works on its own, such as the smelter or a campfire, gets
//! on with it whether you are there or not, and what it makes waits in it until you come
//! back to it. Its screen is in `station_screen`.
//!
//! Stations and recipes are data, in `assets/data/stations.ron` and `recipes.ron` (D136).
//! A station can need someone of a trade at work at it, as the forge needs a smith, as
//! Enshrouded's rescued survivors run its stations; until one is, it says so. Who works
//! where is read from the people themselves (`Staff`), so there is nothing to keep in step.

use super::camera::grab_mouse;
use super::controls::{Action, Settings};
use super::data;
use super::interact::{Focus, Interactable, REACH_M};
use super::inventory::{Belongings, Inventory, InventoryScreen, Items, Made};
pub use super::kinds::Family;
use super::options::OptionsMenu;
use super::people::{Doing, Person};
use super::props::Prop;
use super::skills::{Did, ONCE, Skill};
use super::tick::Sim;
use super::told::{Notice, Told};
use super::{IsLocalPlayer, Player};
use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use serde::Deserialize;
use std::collections::VecDeque;
use terra_items::ItemKind;

/// How many things a station can have waiting at once.
pub const MOST_QUEUED: usize = 8;

/// A kind of station, as `assets/data/stations.ron` has it. The file says what each part
/// means.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct StationDef {
    pub name: String,
    #[serde(default)]
    pub works_alone: bool,
    #[serde(default)]
    pub worked_by: Option<String>,
    #[serde(default = "aim_at_middle")]
    pub aim: (f32, f32),
    #[serde(default)]
    pub strokes: Option<(String, f32)>,
    #[serde(default)]
    pub fire: Option<Fire>,
}

fn aim_at_middle() -> (f32, f32) {
    (0.8, 0.9)
}

/// When a fire in a station is heard.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fire {
    Always,
    WhileWorking,
}

/// A kind of station: its place in `Stations`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Station(u8);

/// Every kind of station (`assets/data/stations.ron`).
#[derive(Resource, Debug)]
pub struct Stations(Vec<StationDef>);

impl Stations {
    pub fn read() -> Self {
        Self(data::read("stations.ron"))
    }

    /// Every kind of station.
    pub fn all(&self) -> impl Iterator<Item = Station> + use<> {
        (0..self.0.len()).map(|index| Station(index as u8))
    }

    /// The kind of station called a name.
    pub fn named(&self, name: &str) -> Option<Station> {
        self.0
            .iter()
            .position(|station| station.name == name)
            .map(|index| Station(index as u8))
    }

    pub fn get(&self, station: Station) -> &StationDef {
        &self.0[station.0 as usize]
    }

    pub fn name(&self, station: Station) -> &str {
        &self.get(station).name
    }

    /// Whether it works through its queue without anyone at it, like a furnace.
    pub fn works_alone(&self, station: Station) -> bool {
        self.get(station).works_alone
    }

    /// The trade of whoever must be at work at it before it works at all, such as a smith.
    pub fn worked_by(&self, station: Station) -> Option<&str> {
        self.get(station).worked_by.as_deref()
    }

    /// The sound of work at it, a stroke at a time, and seconds between strokes.
    pub fn strokes(&self, station: Station) -> Option<(&str, f32)> {
        self.get(station)
            .strokes
            .as_ref()
            .map(|(sound, every)| (sound.as_str(), *every))
    }
}

/// Something a station makes, from what, and how long it takes.
pub struct Recipe {
    pub station: Station,
    pub cost: Vec<(ItemKind, u32)>,
    pub makes: Made,
    pub seconds: f32,
}

impl Recipe {
    /// The tab it is under at its station: what kind of thing it makes.
    pub fn family(&self, items: &Items) -> Family {
        items.def(self.makes.kind).family()
    }
}

/// A recipe as `assets/data/recipes.ron` has it. The file says what each part means.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeDef {
    station: String,
    cost: Vec<(String, u32)>,
    makes: (String, u32),
    seconds: f32,
}

/// Every recipe (`assets/data/recipes.ron`). Each is known by its place here, which a
/// station's queue keeps.
#[derive(Resource)]
pub struct Recipes(Vec<Recipe>);

impl std::ops::Index<usize> for Recipes {
    type Output = Recipe;

    fn index(&self, index: usize) -> &Recipe {
        &self.0[index]
    }
}

impl Recipes {
    /// Reads the recipes, naming their stations and items as the registries do.
    pub fn read(stations: &Stations, items: &Items) -> Self {
        const FILE: &str = "recipes.ron";
        let defs: Vec<RecipeDef> = data::read(FILE);
        Self(
            defs.into_iter()
                .map(|def| Recipe {
                    station: stations.named(&def.station).unwrap_or_else(|| {
                        panic!("{FILE}: there is no station called {}", def.station)
                    }),
                    cost: items.cost(&def.cost, FILE),
                    makes: Made {
                        kind: items.named(&def.makes.0, FILE),
                        count: def.makes.1,
                    },
                    seconds: def.seconds,
                })
                .collect(),
        )
    }

    pub fn iter(&self) -> impl Iterator<Item = &Recipe> {
        self.0.iter()
    }

    /// The recipes a station makes, in order, by their place here.
    pub fn at(&self, station: Station) -> impl Iterator<Item = usize> + '_ {
        (0..self.0.len()).filter(move |&index| self.0[index].station == station)
    }

    /// A station's recipe for what goes by a name, as a saved game names it.
    pub fn named(&self, station: Station, name: &str, items: &Items) -> Option<usize> {
        self.at(station)
            .find(|&index| self.0[index].makes.name(items) == name)
    }

    /// The kinds of thing a station makes, in the order its recipes first make them.
    pub fn families(&self, station: Station, items: &Items) -> Vec<Family> {
        let mut families = Vec::new();
        for index in self.at(station) {
            let family = self.0[index].family(items);
            if !families.contains(&family) {
                families.push(family);
            }
        }
        families
    }
}

/// The kinds of station and their recipes, for a system that reads both.
#[derive(SystemParam)]
pub struct Crafts<'w> {
    pub stations: Res<'w, Stations>,
    pub recipes: Res<'w, Recipes>,
}

impl Crafts<'_> {
    /// The station that mends a kind of thing: the one its kind says, or else the one that
    /// makes it, if it wears at all.
    pub fn mended_at(&self, items: &Items, kind: ItemKind) -> Option<Station> {
        mended_at(items, &self.stations, &self.recipes, kind)
    }
}

/// The station that mends a kind of thing (D99): the one its kind says, or else the one
/// that makes it, if it wears at all.
pub fn mended_at(
    items: &Items,
    stations: &Stations,
    recipes: &Recipes,
    kind: ItemKind,
) -> Option<Station> {
    let def = items.def(kind);
    def.lasts?;
    if let Some(station) = &def.mended_at {
        return stations.named(station);
    }
    recipes
        .iter()
        .find(|recipe| recipe.makes.kind == kind)
        .map(|recipe| recipe.station)
}

/// How near a station someone at work at it stands.
const WORKER_REACH_M: f32 = 3.0;

/// Who works the stations that need someone of a trade at work at them, as the forge needs a
/// smith.
#[derive(SystemParam)]
pub struct Staff<'w, 's> {
    kinds: Res<'w, Stations>,
    people: Query<'w, 's, &'static Person>,
    stations: Query<'w, 's, &'static Transform, With<Works>>,
}

impl Staff<'_, '_> {
    /// Whoever is at work at a station, if it is one that needs someone.
    pub fn worker(&self, entity: Entity, station: Station) -> Option<&Person> {
        let trade = self.kinds.worked_by(station)?;
        let at = self.stations.get(entity).ok()?.translation;
        self.people
            .iter()
            .find(|person| person.trade == Some(trade) && at_work(person, at))
    }

    /// Whether a station can get on with its queue: always, unless it needs someone to
    /// work it and nobody is.
    pub fn works(&self, entity: Entity, station: Station) -> bool {
        self.kinds.worked_by(station).is_none() || self.worker(entity, station).is_some()
    }
}

/// Whether someone is at work at a station standing at a place.
fn at_work(person: &Person, station: Vec3) -> bool {
    matches!(person.doing, Doing::Working(spot) if spot.distance(station) < WORKER_REACH_M)
}

/// A station's queue: what is to be made, by its place in `Recipes`, how far into the
/// first it is, and what is made and waiting to be taken.
#[derive(Component, Default, Debug)]
pub struct Works {
    pub queue: VecDeque<usize>,
    pub progress: f32,
    pub done: Vec<usize>,
    /// Whether it has said there is no room for what it made, so it says so once.
    told_full: bool,
}

impl Works {
    /// A queue taken up where it was left, as a saved game has it.
    pub fn resumed(queue: Vec<usize>, progress: f32, done: Vec<usize>) -> Self {
        Self {
            queue: queue.into(),
            progress,
            done,
            told_full: false,
        }
    }

    /// Gets on with the queue for so many seconds. Returns how many things it finished.
    pub fn advance(&mut self, seconds: f32, recipes: &Recipes) -> usize {
        let mut left = seconds;
        let mut finished = 0;
        while let Some(&first) = self.queue.front() {
            let needed = recipes[first].seconds - self.progress;
            if left < needed {
                self.progress += left;
                break;
            }
            left -= needed;
            self.progress = 0.0;
            self.queue.pop_front();
            self.done.push(first);
            finished += 1;
        }
        finished
    }

    /// How far through the first thing it is, out of one.
    pub fn through(&self, recipes: &Recipes) -> f32 {
        self.queue
            .front()
            .map_or(0.0, |&first| self.progress / recipes[first].seconds)
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (fit_out, open, leave).chain())
        .add_systems(FixedUpdate, (work, hand_over).chain().in_set(Sim::World));
}

/// Every station keeps a queue. A station may be gone by the time this applies, when a
/// saved game replaces what stands, so that is not an error.
fn fit_out(mut commands: Commands, stations: Query<(Entity, &Prop), Without<Works>>) {
    for (entity, prop) in &stations {
        if prop.item.station.is_some() {
            commands.entity(entity).try_insert(Works::default());
        }
    }
}

/// Whether the player is close enough to a station to work it.
fn within_reach(player: Vec3, prop: &Prop, at: Vec3) -> bool {
    let up = at.normalize();
    let across = prop.half.x.max(prop.half.z);
    (player - at).reject_from(up).length() < REACH_M + across
}

/// The interact key at a station opens its screen; again closes it.
fn open(
    settings: Res<Settings>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    menu: Res<OptionsMenu>,
    focus: Single<&Focus, IsLocalPlayer>,
    window: Single<(&Window, &mut CursorOptions)>,
    mut screen: ResMut<InventoryScreen>,
) {
    // The screen is this machine's, so its own key opens and closes it.
    let (window, mut cursor) = window.into_inner();
    let captured = cursor.grab_mode != CursorGrabMode::None;
    if menu.open || !settings.went_down(Action::Interact, &keys, &mouse, captured) {
        return;
    }
    if screen.at.is_some() {
        screen.open = false;
        screen.at = None;
        grab_mouse(&mut cursor, window.focused);
    } else if let (Some(Interactable::Station(station)), Some(entity)) = (focus.what, focus.entity)
        && !screen.open
    {
        screen.open = true;
        screen.at = Some((entity, station));
        grab_mouse(&mut cursor, false);
    }
}

/// Walking away from a station closes its screen.
fn leave(
    player: Single<&Transform, IsLocalPlayer>,
    stations: Query<(&Prop, &Transform)>,
    window: Single<(&Window, &mut CursorOptions)>,
    mut screen: ResMut<InventoryScreen>,
) {
    let Some((entity, _)) = screen.at else { return };
    let near = stations
        .get(entity)
        .is_ok_and(|(prop, at)| within_reach(player.translation, prop, at.translation));
    if !near {
        screen.open = false;
        screen.at = None;
        let (window, mut cursor) = window.into_inner();
        grab_mouse(&mut cursor, window.focused);
    }
}

/// Stations get on with their queues: those that work alone always, the rest while a
/// player stays within reach.
fn work(
    time: Res<Time>,
    kinds: Res<Stations>,
    recipes: Res<Recipes>,
    staff: Staff,
    players: Query<&Transform, (With<Player>, Without<Works>)>,
    mut stations: Query<(Entity, &Prop, &Transform, &mut Works)>,
) {
    for (entity, prop, at, mut works) in &mut stations {
        let Some(station) = prop.item.station else {
            continue;
        };
        let attended = || {
            players
                .iter()
                .any(|player| within_reach(player.translation, prop, at.translation))
        };
        if staff.works(entity, station) && (kinds.works_alone(station) || attended()) {
            works.advance(time.delta_secs(), &recipes);
        }
    }
}

/// What a station has made goes to a player: from a worked station at once, to whoever is
/// working it; from one that works alone when they open it.
fn hand_over(
    screen: Res<InventoryScreen>,
    kinds: Res<Stations>,
    recipes: Res<Recipes>,
    ours: Option<Single<Entity, IsLocalPlayer>>,
    players: Query<(Entity, &Transform), (With<Player>, Without<Works>)>,
    mut stations: Query<(Entity, &Prop, &Transform, &mut Works)>,
    mut belongings: Belongings,
    mut told: MessageWriter<Told>,
    mut did: MessageWriter<Did>,
) {
    let ours = ours.map(|player| *player);
    for (entity, prop, at, mut works) in &mut stations {
        let Some(station) = prop.item.station else {
            continue;
        };
        if works.done.is_empty() {
            continue;
        }
        // A worked station hands over to the nearest player working it; one that works
        // alone, to this machine's player when they open it.
        let to = if kinds.works_alone(station) {
            ours.filter(|_| screen.at.is_some_and(|(open, _)| open == entity))
        } else {
            players
                .iter()
                .filter(|(_, them)| within_reach(them.translation, prop, at.translation))
                .min_by(|a, b| {
                    let apart = |them: &Transform| them.translation.distance(at.translation);
                    apart(a.1).total_cmp(&apart(b.1))
                })
                .map(|(player, _)| player)
        };
        let Some(mut inventory) = to.and_then(|player| belongings.of(player)) else {
            continue;
        };
        let player = inventory.player();
        while let Some(&first) = works.done.first() {
            let made = recipes[first].makes;
            let name = made.name(inventory.items());
            if !inventory.receive(made) {
                if !works.told_full {
                    told.write(Told::new(
                        player,
                        format!(
                            "No room in your backpack: the {name} waits in the {}",
                            kinds.name(station).to_lowercase()
                        ),
                    ));
                    works.told_full = true;
                }
                break;
            }
            works.done.remove(0);
            works.told_full = false;
            did.write(Did(player, Skill::Crafting, ONCE));
            info!("Made {name}.");
            told.write(Told::new(player, format!("Made {name}")));
        }
    }
}

/// Queues one of a recipe at a station, paying for it now. Says why not if it cannot.
/// `staffed` is whether the station has whoever it needs to work it (`Staff::works`).
pub fn queue(
    works: &mut Works,
    index: usize,
    staffed: bool,
    (recipes, stations): (&Recipes, &Stations),
    belongings: &mut Inventory,
    notice: &mut Notice,
) {
    let recipe = &recipes[index];
    let station = stations.name(recipe.station).to_lowercase();
    if !staffed {
        notice.say(format!(
            "The {station} needs a {} to work it",
            stations.worked_by(recipe.station).unwrap_or("worker")
        ));
    } else if works.queue.len() >= MOST_QUEUED {
        notice.say(format!("The {station} has enough to do"));
    } else if let Some((kind, more)) = belongings.missing(&recipe.cost, 1) {
        let items = belongings.items();
        notice.say(format!(
            "You need {more} more {}: {}",
            items.name(kind),
            items.found(kind)
        ));
    } else if belongings.pay(&recipe.cost, 1) {
        works.queue.push_back(index);
        info!(
            "Queued {} at the {}.",
            recipe.makes.name(belongings.items()),
            stations.name(recipe.station)
        );
    }
}

/// Takes something off a station's queue and gives back what it took, if there is room.
pub fn cancel(
    works: &mut Works,
    place: usize,
    recipes: &Recipes,
    belongings: &mut Inventory,
    notice: &mut Notice,
) {
    let Some(&index) = works.queue.get(place) else {
        return;
    };
    // What does not fit in the backpack drops at the player's feet.
    belongings.refund(&recipes[index].cost, 1);
    works.queue.remove(place);
    if place == 0 {
        works.progress = 0.0;
    }
    notice.say(format!(
        "Took {} off the queue",
        recipes[index].makes.name(belongings.items())
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::inventory::{Backpack, Gained, Gear, give};
    use crate::play::{test_player, test_world};
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    struct Kinds {
        items: Items,
        stations: Stations,
        recipes: Recipes,
    }

    /// The kinds as the files have them. Reading them checks that every recipe names a
    /// station and kinds of item there are.
    fn kinds() -> Kinds {
        let items = Items::default();
        let stations = Stations::read();
        let recipes = Recipes::read(&stations, &items);
        Kinds {
            items,
            stations,
            recipes,
        }
    }

    impl Kinds {
        fn station(&self, name: &str) -> Station {
            self.stations.named(name).unwrap()
        }

        fn made(&self, name: &str) -> Made {
            Made {
                kind: self.items.find(name).unwrap(),
                count: 1,
            }
        }

        fn index_of(&self, makes: Made) -> usize {
            self.recipes
                .iter()
                .position(|recipe| recipe.makes == makes)
                .unwrap()
        }
    }

    #[test]
    fn a_queue_finishes_in_order_and_carries_time_over() {
        let kinds = kinds();
        let recipes = &kinds.recipes;
        let iron = kinds.index_of(kinds.made("Iron"));
        let seconds = recipes[iron].seconds;
        let mut works = Works::default();
        works.queue.extend([iron, iron, iron]);
        assert_eq!(works.advance(seconds * 0.5, recipes), 0);
        assert!((works.through(recipes) - 0.5).abs() < 1e-5);
        assert_eq!(works.advance(seconds * 1.75, recipes), 2);
        assert_eq!(works.done, [iron, iron]);
        assert!((works.through(recipes) - 0.25).abs() < 1e-4);
        assert_eq!(works.advance(seconds * 10.0, recipes), 1);
        assert!(works.queue.is_empty());
        assert_eq!(works.through(recipes), 0.0);
    }

    #[test]
    fn the_forge_waits_for_a_smith_and_the_smelter_works_alone() {
        let kinds = kinds();
        let (forge_kind, smelter) = (kinds.station("Forge"), kinds.station("Smelter"));
        assert!(kinds.stations.works_alone(smelter) && !kinds.stations.works_alone(forge_kind));
        let mut world = World::new();
        world.insert_resource(kinds.stations);
        let forge = Vec3::new(0.0, 2000.0, 0.0);
        let built = world
            .spawn((Transform::from_translation(forge), Works::default()))
            .id();
        let staffed = |world: &mut World, kind| {
            world
                .run_system_once(move |staff: Staff| staff.works(built, kind))
                .unwrap()
        };
        assert!(!staffed(&mut world, forge_kind));
        assert!(staffed(&mut world, smelter));
        // A smith following you is not at work; one working beside the forge is.
        let smith = world
            .spawn(Person::new(
                "Bren",
                Doing::Following(Entity::PLACEHOLDER),
                Some("smith"),
            ))
            .id();
        assert!(!staffed(&mut world, forge_kind));
        world.get_mut::<Person>(smith).unwrap().doing = Doing::Working(forge + Vec3::X * 1.8);
        assert!(staffed(&mut world, forge_kind));
        // Someone at work at another forge far off does not work this one.
        world.get_mut::<Person>(smith).unwrap().doing = Doing::Working(forge + Vec3::X * 50.0);
        assert!(!staffed(&mut world, forge_kind));
    }

    #[test]
    fn a_saved_name_finds_its_recipe_at_its_own_station() {
        let kinds = kinds();
        let named = |station, name| {
            kinds
                .recipes
                .named(kinds.station(station), name, &kinds.items)
        };
        let iron = named("Smelter", "Iron").unwrap();
        assert_eq!(kinds.recipes[iron].makes, kinds.made("Iron"));
        assert_eq!(named("Forge", "Iron"), None);
    }

    const GROUND: Vec3 = Vec3::new(0.0, 2000.0, 0.0);

    /// A world with the kinds of station and recipe, and a moment gone by.
    fn station_world() -> World {
        let mut world = test_world();
        let kinds = kinds();
        world.insert_resource(kinds.stations);
        world.insert_resource(kinds.recipes);
        world.init_resource::<InventoryScreen>();
        world.init_resource::<Messages<Did>>();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_millis(500));
        world.insert_resource(time);
        world
    }

    /// The first kind of station that suits, and the first thing it makes.
    fn station_where(
        world: &World,
        suits: impl Fn(&Stations, Station) -> bool,
    ) -> (Station, usize) {
        let stations = world.resource::<Stations>();
        let station = stations
            .all()
            .find(|&station| suits(stations, station))
            .unwrap();
        let recipe = world.resource::<Recipes>().at(station).next().unwrap();
        (station, recipe)
    }

    /// A station standing at a place, with one thing queued or made.
    fn put(world: &mut World, (station, recipe): (Station, usize), at: Vec3, made: bool) -> Entity {
        let works = if made {
            Works::resumed(Vec::new(), 0.0, vec![recipe])
        } else {
            Works::resumed(vec![recipe], 0.0, Vec::new())
        };
        world
            .spawn((
                Prop::boxed(Vec3::splat(0.5), Some(station)),
                Transform::from_translation(at),
                works,
            ))
            .id()
    }

    fn progress(world: &World, station: Entity) -> f32 {
        world.get::<Works>(station).unwrap().progress
    }

    #[test]
    fn a_station_that_needs_a_worker_waits_for_one_with_a_player_beside_it() {
        let mut world = station_world();
        let kind = station_where(&world, |stations, station| {
            stations.worked_by(station).is_some()
        });
        let forge = put(&mut world, kind, GROUND, false);
        test_player(&mut world, GROUND + Vec3::X);
        world.run_system_once(work).unwrap();
        assert_eq!(progress(&world, forge), 0.0);
        let trade = world.resource::<Stations>().worked_by(kind.0).unwrap();
        let trade: &'static str = trade.to_string().leak();
        world.spawn(Person::new(
            "Bren",
            Doing::Working(GROUND + Vec3::Z),
            Some(trade),
        ));
        world.run_system_once(work).unwrap();
        assert!(progress(&world, forge) > 0.0);
    }

    #[test]
    fn a_station_gets_on_while_someone_is_by_it_unless_it_works_alone() {
        let mut world = station_world();
        let unworked = |stations: &Stations, station| stations.worked_by(station).is_none();
        let worked = station_where(&world, |stations, station| {
            unworked(stations, station) && !stations.works_alone(station)
        });
        let alone = station_where(&world, |stations, station| {
            unworked(stations, station) && stations.works_alone(station)
        });
        let bench = put(&mut world, worked, GROUND, false);
        let furnace = put(&mut world, alone, GROUND + Vec3::Z * 30.0, false);
        let player = test_player(&mut world, GROUND + Vec3::X * 30.0);
        world.run_system_once(work).unwrap();
        assert_eq!(progress(&world, bench), 0.0);
        assert!(progress(&world, furnace) > 0.0);
        world.get_mut::<Transform>(player).unwrap().translation = GROUND + Vec3::X;
        world.run_system_once(work).unwrap();
        assert!(progress(&world, bench) > 0.0);
    }

    #[test]
    fn what_is_made_waits_while_the_backpack_is_full_and_says_so_once() {
        let mut world = station_world();
        let kind = station_where(&world, |stations, station| !stations.works_alone(station));
        let bench = put(&mut world, kind, GROUND, true);
        let player = test_player(&mut world, GROUND + Vec3::X);
        let makes = world.resource::<Recipes>()[kind.1].makes.kind;
        let room = (
            world.get::<Backpack>(player).unwrap().clone(),
            world.get::<Gear>(player).unwrap().clone(),
        );
        // Full of what it makes, worn and carried.
        world.resource_scope(|world, mut items: Mut<Items>| {
            let mut player = world.entity_mut(player);
            player.get_mut::<Gear>().unwrap().0.stow(makes, u32::MAX);
            give(
                &mut items,
                &mut player.get_mut::<Backpack>().unwrap().0,
                makes,
                1000,
            );
        });
        world.run_system_once(hand_over).unwrap();
        world.run_system_once(hand_over).unwrap();
        assert_eq!(world.get::<Works>(bench).unwrap().done, [kind.1]);
        let told: Vec<_> = world.resource_mut::<Messages<Told>>().drain().collect();
        assert_eq!(told.len(), 1, "told it is full once");
        assert_eq!(told[0].player, player);
        world.resource_mut::<Messages<Gained>>().clear();
        // With room again, it is handed over.
        world.entity_mut(player).insert(room);
        world.run_system_once(hand_over).unwrap();
        assert!(world.get::<Works>(bench).unwrap().done.is_empty());
        let gained: Vec<_> = world.resource_mut::<Messages<Gained>>().drain().collect();
        assert!(
            gained
                .iter()
                .any(|gained| gained.player == player && gained.kind == makes)
        );
    }
}
