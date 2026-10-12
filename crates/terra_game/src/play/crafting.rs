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

use super::bag::Notice;
use super::grab_mouse;
use super::interact::{Focus, Focusing, Interactable, REACH_M};
use super::inventory::{Belongings, Cost, Inventory, InventoryScreen, Items, Made};
pub use super::kinds::Family;
use super::people::{Doing, Person};
use super::props::Prop;
use super::skills::{Did, ONCE, Skill};
use super::{IsLocalPlayer, Player};
use bevy::{ecs::system::SystemParam, prelude::*, window::CursorOptions};
use serde::Deserialize;
use std::{collections::VecDeque, sync::LazyLock};

/// How many things a station can have waiting at once.
pub const MOST_QUEUED: usize = 8;

const STATIONS_FILE: &str = include_str!("../../../../assets/data/stations.ron");
const RECIPES_FILE: &str = include_str!("../../../../assets/data/recipes.ron");

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

/// Every kind of station, built in so every machine has the same (D136).
static STATIONS: LazyLock<Vec<StationDef>> = LazyLock::new(|| {
    ron::from_str(STATIONS_FILE)
        .unwrap_or_else(|problem| panic!("assets/data/stations.ron: {problem}"))
});

/// A kind of station: its place in `assets/data/stations.ron`. It prints as its name.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Station(u8);

impl std::fmt::Debug for Station {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl Station {
    /// Every kind of station.
    pub fn all() -> impl Iterator<Item = Station> {
        (0..STATIONS.len()).map(|index| Station(index as u8))
    }

    /// The kind of station called a name.
    pub fn named(name: &str) -> Option<Station> {
        STATIONS
            .iter()
            .position(|station| station.name == name)
            .map(|index| Station(index as u8))
    }

    fn def(self) -> &'static StationDef {
        &STATIONS[self.0 as usize]
    }

    pub fn name(self) -> &'static str {
        &self.def().name
    }

    /// Whether it works through its queue without anyone at it, like a furnace.
    pub fn works_alone(self) -> bool {
        self.def().works_alone
    }

    /// The trade of whoever must be at work at it before it works at all, such as a smith.
    pub fn worked_by(self) -> Option<&'static str> {
        self.def().worked_by.as_deref()
    }

    /// Where to aim at it, as a height above its foot, and how near the aiming mark has to
    /// pass to count as on it, in metres.
    pub fn aim(self) -> (f32, f32) {
        self.def().aim
    }

    /// The sound of work at it, a stroke at a time, and seconds between strokes.
    pub fn strokes(self) -> Option<(&'static str, f32)> {
        self.def()
            .strokes
            .as_ref()
            .map(|(sound, every)| (sound.as_str(), *every))
    }

    /// When a fire in it is heard, if it has one.
    pub fn fire(self) -> Option<Fire> {
        self.def().fire
    }
}

/// Something a station makes, from what, and how long it takes.
pub struct Recipe {
    pub station: Station,
    pub cost: Cost,
    pub makes: Made,
    pub seconds: f32,
}

impl Recipe {
    /// The tab it is under at its station: what kind of thing it makes.
    pub fn family(&self, items: &Items) -> Family {
        items.def(items.kind(self.makes.item)).family()
    }
}

/// A recipe as `assets/data/recipes.ron` has it. The file says what each part means.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeDef {
    station: &'static str,
    cost: Vec<(&'static str, u32)>,
    makes: (&'static str, u32),
    seconds: f32,
}

/// Every recipe, built in so every machine has the same (D136). Each is known by its place
/// here, which a station's queue keeps.
pub static RECIPES: LazyLock<Vec<Recipe>> = LazyLock::new(|| {
    let defs: Vec<RecipeDef> = ron::from_str(RECIPES_FILE)
        .unwrap_or_else(|problem| panic!("assets/data/recipes.ron: {problem}"));
    defs.into_iter()
        .map(|def| Recipe {
            station: Station::named(def.station).unwrap_or_else(|| {
                panic!(
                    "assets/data/recipes.ron: there is no station called {}",
                    def.station
                )
            }),
            // The list lives as long as the game, like the recipe it is part of.
            cost: def.cost.leak(),
            makes: Made::many(def.makes.0, def.makes.1),
            seconds: def.seconds,
        })
        .collect()
});

/// The recipes a station makes, in order, by their place in `RECIPES`.
pub fn recipes(station: Station) -> impl Iterator<Item = usize> {
    (0..RECIPES.len()).filter(move |&index| RECIPES[index].station == station)
}

/// A station's recipe for what goes by a name, as a saved game names it.
pub fn recipe_named(station: Station, name: &str) -> Option<usize> {
    recipes(station).find(|&index| RECIPES[index].makes.name() == name)
}

/// The kinds of thing a station makes, in the order its recipes first make them.
pub fn families(station: Station, items: &Items) -> Vec<Family> {
    let mut families = Vec::new();
    for index in recipes(station) {
        let family = RECIPES[index].family(items);
        if !families.contains(&family) {
            families.push(family);
        }
    }
    families
}

/// How near a station someone at work at it stands.
const WORKER_REACH_M: f32 = 3.0;

/// Who works the stations that need someone of a trade at work at them, as the forge needs a
/// smith.
#[derive(SystemParam)]
pub struct Staff<'w, 's> {
    people: Query<'w, 's, &'static Person>,
    stations: Query<'w, 's, &'static Transform, With<Works>>,
}

impl Staff<'_, '_> {
    /// Whoever is at work at a station, if it is one that needs someone.
    pub fn worker(&self, entity: Entity, station: Station) -> Option<&Person> {
        let trade = station.worked_by()?;
        let at = self.stations.get(entity).ok()?.translation;
        self.people
            .iter()
            .find(|person| person.trade == Some(trade) && at_work(person, at))
    }

    /// Whether a station can get on with its queue: always, unless it needs someone to
    /// work it and nobody is.
    pub fn works(&self, entity: Entity, station: Station) -> bool {
        station.worked_by().is_none() || self.worker(entity, station).is_some()
    }
}

/// Whether someone is at work at a station standing at a place.
fn at_work(person: &Person, station: Vec3) -> bool {
    matches!(person.doing, Doing::Working(spot) if spot.distance(station) < WORKER_REACH_M)
}

/// A station's queue: what is to be made, by its place in `RECIPES`, how far into the
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
    pub fn advance(&mut self, seconds: f32) -> usize {
        let mut left = seconds;
        let mut finished = 0;
        while let Some(&first) = self.queue.front() {
            let needed = RECIPES[first].seconds - self.progress;
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
    pub fn through(&self) -> f32 {
        self.queue
            .front()
            .map_or(0.0, |&first| self.progress / RECIPES[first].seconds)
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (fit_out, open, leave, work, hand_over)
            .chain()
            .after(Focusing),
    );
}

/// Every station keeps a queue. A station may be gone by the time this applies, when a
/// saved game replaces what stands, so that is not an error.
fn fit_out(mut commands: Commands, stations: Query<(Entity, &Prop), Without<Works>>) {
    for (entity, prop) in &stations {
        if prop.item.station().is_some() {
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
    staff: Staff,
    players: Query<&Transform, (With<Player>, Without<Works>)>,
    mut stations: Query<(Entity, &Prop, &Transform, &mut Works)>,
) {
    for (entity, prop, at, mut works) in &mut stations {
        let Some(station) = prop.item.station() else {
            continue;
        };
        let attended = || {
            players
                .iter()
                .any(|player| within_reach(player.translation, prop, at.translation))
        };
        if staff.works(entity, station) && (station.works_alone() || attended()) {
            works.advance(time.delta_secs());
        }
    }
}

/// What a station has made goes to the player: from a worked station at once, since they
/// are there; from one that works alone when they open it.
fn hand_over(
    screen: Res<InventoryScreen>,
    player: Single<&Transform, IsLocalPlayer>,
    mut stations: Query<(Entity, &Prop, &Transform, &mut Works)>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut did: MessageWriter<Did>,
) {
    let Some(mut belongings) = belongings.mine() else {
        return;
    };
    for (entity, prop, at, mut works) in &mut stations {
        let Some(station) = prop.item.station() else {
            continue;
        };
        // A worked station hands over to whoever is working it, so only to this machine's
        // player while they are there; one that works alone, when they open it.
        let at_it = if station.works_alone() {
            screen.at.is_some_and(|(open, _)| open == entity)
        } else {
            within_reach(player.translation, prop, at.translation)
        };
        if works.done.is_empty() || !at_it {
            continue;
        }
        while let Some(&first) = works.done.first() {
            let made = RECIPES[first].makes;
            if !belongings.receive(made) {
                if !works.told_full {
                    notice.say(format!(
                        "No room in your backpack: the {} waits in the {}",
                        made.name(),
                        station.name().to_lowercase()
                    ));
                    works.told_full = true;
                }
                break;
            }
            works.done.remove(0);
            works.told_full = false;
            did.write(Did(belongings.player(), Skill::Crafting, ONCE));
            info!("Made {}.", made.name());
            notice.say(format!("Made {}", made.name()));
        }
    }
}

/// Queues one of a recipe at a station, paying for it now. Says why not if it cannot.
/// `staffed` is whether the station has whoever it needs to work it (`Staff::works`).
pub fn queue(
    works: &mut Works,
    index: usize,
    staffed: bool,
    belongings: &mut Inventory,
    notice: &mut Notice,
) {
    let recipe = &RECIPES[index];
    if !staffed {
        notice.say(format!(
            "The {} needs a {} to work it",
            recipe.station.name().to_lowercase(),
            recipe.station.worked_by().unwrap_or("worker")
        ));
    } else if works.queue.len() >= MOST_QUEUED {
        notice.say(format!(
            "The {} has enough to do",
            recipe.station.name().to_lowercase()
        ));
    } else if let Some((kind, more)) = belongings.missing(recipe.cost, 1) {
        let items = belongings.items();
        notice.say(format!(
            "You need {more} more {}: {}",
            items.name(kind),
            items.found(kind)
        ));
    } else if belongings.pay(recipe.cost, 1) {
        works.queue.push_back(index);
        info!(
            "Queued {} at the {}.",
            recipe.makes.name(),
            recipe.station.name()
        );
    }
}

/// Takes something off a station's queue and gives back what it took, if there is room.
pub fn cancel(works: &mut Works, place: usize, belongings: &mut Inventory, notice: &mut Notice) {
    let Some(&index) = works.queue.get(place) else {
        return;
    };
    if !belongings.refund(RECIPES[index].cost, 1) {
        notice.say("No room in your backpack for what it would give back");
        return;
    }
    works.queue.remove(place);
    if place == 0 {
        works.progress = 0.0;
    }
    notice.say(format!(
        "Took {} off the queue",
        RECIPES[index].makes.name()
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(name: &str) -> Station {
        Station::named(name).unwrap()
    }
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn every_recipe_makes_and_costs_kinds_of_item_there_are() {
        let items = Items::default();
        for recipe in RECIPES.iter() {
            let named = recipe.cost.iter().map(|(name, _)| *name);
            for name in named.chain([recipe.makes.item]) {
                assert!(items.find(name).is_some(), "{name} is not a kind of item");
            }
        }
    }

    fn index_of(makes: Made) -> usize {
        RECIPES
            .iter()
            .position(|recipe| recipe.makes == makes)
            .unwrap()
    }

    #[test]
    fn a_queue_finishes_in_order_and_carries_time_over() {
        let iron = index_of(Made::one("Iron"));
        let seconds = RECIPES[iron].seconds;
        let mut works = Works::default();
        works.queue.extend([iron, iron, iron]);
        assert_eq!(works.advance(seconds * 0.5), 0);
        assert!((works.through() - 0.5).abs() < 1e-5);
        assert_eq!(works.advance(seconds * 1.75), 2);
        assert_eq!(works.done, [iron, iron]);
        assert!((works.through() - 0.25).abs() < 1e-4);
        assert_eq!(works.advance(seconds * 10.0), 1);
        assert!(works.queue.is_empty());
        assert_eq!(works.through(), 0.0);
    }

    #[test]
    fn the_forge_waits_for_a_smith_and_the_smelter_works_alone() {
        let mut world = World::new();
        let forge = Vec3::new(0.0, 2000.0, 0.0);
        let built = world
            .spawn((Transform::from_translation(forge), Works::default()))
            .id();
        let staffed = |world: &mut World, kind| {
            world
                .run_system_once(move |staff: Staff| staff.works(built, kind))
                .unwrap()
        };
        assert!(!staffed(&mut world, station("Forge")));
        assert!(staffed(&mut world, station("Smelter")));
        assert!(station("Smelter").works_alone() && !station("Forge").works_alone());
        // A smith following you is not at work; one working beside the forge is.
        let smith = world
            .spawn(Person::new(
                "Bren",
                Doing::Following(Entity::PLACEHOLDER),
                Some("smith"),
            ))
            .id();
        assert!(!staffed(&mut world, station("Forge")));
        world.get_mut::<Person>(smith).unwrap().doing = Doing::Working(forge + Vec3::X * 1.8);
        assert!(staffed(&mut world, station("Forge")));
        // Someone at work at another forge far off does not work this one.
        world.get_mut::<Person>(smith).unwrap().doing = Doing::Working(forge + Vec3::X * 50.0);
        assert!(!staffed(&mut world, station("Forge")));
    }

    #[test]
    fn the_workbench_has_tabs_and_the_smelter_none() {
        let items = Items::default();
        assert_eq!(
            families(station("Workbench"), &items),
            [Family::Tools, Family::Clothing]
        );
        assert_eq!(families(station("Smelter"), &items).len(), 1);
    }

    #[test]
    fn a_saved_name_finds_its_recipe_at_its_own_station() {
        let iron = recipe_named(station("Smelter"), "Iron").unwrap();
        assert_eq!(RECIPES[iron].makes, Made::one("Iron"));
        assert_eq!(recipe_named(station("Forge"), "Iron"), None);
    }
}
