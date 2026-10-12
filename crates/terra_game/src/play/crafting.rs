//! Crafting at stations (D109; game design 001, crafting.md): each station keeps a queue
//! of things to make, paid for when queued. A worked station, such as the workbench or
//! the forge, gets on with it only while you stay within reach, and what it makes goes
//! into your backpack. One that works on its own, such as the smelter or a campfire, gets
//! on with it whether you are there or not, and what it makes waits in it until you come
//! back to it. Its screen is in `station_screen`.
//!
//! The forge only works while a smith is at work at it, as Enshrouded's rescued
//! survivors run its stations; until one is rescued it says so. Who works where is read
//! from the people themselves (`Staff`), so there is nothing to keep in step with them.

use super::bag::Notice;
use super::grab_mouse;
use super::interact::{Focus, Focusing, Interactable, REACH_M};
use super::inventory::{Apparel, Belongings, Container, Cost, InventoryScreen, Made, Raw, Tool};
use super::people::{Doing, Person};
use super::props::Prop;
use super::skills::{Did, ONCE, Skill};
use super::{IsLocalPlayer, Player};
use bevy::{ecs::system::SystemParam, prelude::*, window::CursorOptions};
use std::collections::VecDeque;

/// How many things a station can have waiting at once.
pub const MOST_QUEUED: usize = 8;

/// Where things are made.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Station {
    Workbench,
    Smelter,
    Forge,
    Campfire,
}

impl Station {
    pub fn name(self) -> &'static str {
        match self {
            Station::Workbench => "Workbench",
            Station::Smelter => "Smelter",
            Station::Forge => "Forge",
            Station::Campfire => "Campfire",
        }
    }

    /// Whether it works through its queue without anyone at it, like a furnace.
    pub fn works_alone(self) -> bool {
        matches!(self, Station::Smelter | Station::Campfire)
    }
}

/// What kind of thing a recipe makes, for a station's tabs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    Tools,
    Clothing,
    Materials,
}

impl Family {
    pub fn name(self) -> &'static str {
        match self {
            Family::Tools => "Tools",
            Family::Clothing => "Clothing",
            Family::Materials => "Materials",
        }
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
    pub fn family(&self) -> Family {
        match self.makes {
            Made::Tool(_) | Made::Container(_) | Made::Raw(Raw::Arrow, _) => Family::Tools,
            Made::Apparel(_) => Family::Clothing,
            Made::Raw(..) => Family::Materials,
        }
    }
}

const fn recipe(station: Station, cost: Cost, makes: Made, seconds: f32) -> Recipe {
    Recipe {
        station,
        cost,
        makes,
        seconds,
    }
}

/// Every recipe. A stand-in list, chosen so the MVP's loop can be walked: stone tools and
/// leather at the workbench, then iron from the smelter and iron tools at the forge
/// (docs/prototypes.md).
pub const RECIPES: &[Recipe] = &[
    // Fibre bound round a stick and lit at the fire.
    recipe(
        Station::Campfire,
        &[(Raw::Wood, 1), (Raw::Fibre, 1)],
        Made::Tool(Tool::Torch),
        2.0,
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Wood, 2), (Raw::Stone, 2)],
        Made::Tool(Tool::StoneAxe),
        4.0,
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Wood, 2), (Raw::Stone, 3)],
        Made::Tool(Tool::StonePickaxe),
        4.0,
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Wood, 1), (Raw::Stone, 1)],
        Made::Tool(Tool::Knife),
        3.0,
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Wood, 3), (Raw::Stone, 2)],
        Made::Tool(Tool::Spear),
        5.0,
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Wood, 3), (Raw::Fibre, 2)],
        Made::Tool(Tool::Bow),
        6.0,
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Wood, 1), (Raw::Stone, 1)],
        Made::Raw(Raw::Arrow, 4),
        4.0,
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Hide, 2), (Raw::Fibre, 2)],
        Made::Container(Container::Quiver),
        5.0,
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Hide, 2), (Raw::Fibre, 1)],
        Made::Apparel(Apparel::LeatherCap),
        5.0,
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Hide, 4), (Raw::Fibre, 2)],
        Made::Apparel(Apparel::LeatherCoat),
        8.0,
    ),
    recipe(
        Station::Workbench,
        &[(Raw::Hide, 4), (Raw::Fibre, 4)],
        Made::Apparel(Apparel::LargePack),
        8.0,
    ),
    recipe(
        Station::Campfire,
        &[(Raw::Meat, 1)],
        Made::Raw(Raw::CookedMeat, 1),
        10.0,
    ),
    recipe(
        Station::Smelter,
        &[(Raw::Ore, 2), (Raw::Wood, 1)],
        Made::Raw(Raw::Iron, 1),
        10.0,
    ),
    recipe(
        Station::Forge,
        &[(Raw::Wood, 2), (Raw::Iron, 3)],
        Made::Tool(Tool::IronAxe),
        6.0,
    ),
    recipe(
        Station::Forge,
        &[(Raw::Wood, 2), (Raw::Iron, 3)],
        Made::Tool(Tool::IronPickaxe),
        6.0,
    ),
];

/// The recipes a station makes, in order, by their place in `RECIPES`.
pub fn recipes(station: Station) -> impl Iterator<Item = usize> {
    (0..RECIPES.len()).filter(move |&index| RECIPES[index].station == station)
}

/// A station's recipe for what goes by a name, as a saved game names it.
pub fn recipe_named(station: Station, name: &str) -> Option<usize> {
    recipes(station).find(|&index| RECIPES[index].makes.name() == name)
}

/// The kinds of thing a station makes, in the order its recipes first make them.
pub fn families(station: Station) -> Vec<Family> {
    let mut families = Vec::new();
    for index in recipes(station) {
        let family = RECIPES[index].family();
        if !families.contains(&family) {
            families.push(family);
        }
    }
    families
}

/// How near a station someone at work at it stands.
const WORKER_REACH_M: f32 = 3.0;

/// Who works the stations that need someone to: the forge works only while a smith is at
/// work beside it.
#[derive(SystemParam)]
pub struct Staff<'w, 's> {
    people: Query<'w, 's, &'static Person>,
    stations: Query<'w, 's, &'static Transform, With<Works>>,
}

impl Staff<'_, '_> {
    /// Whoever is at work at a station, if it is one that needs someone.
    pub fn worker(&self, entity: Entity, station: Station) -> Option<&Person> {
        if station != Station::Forge {
            return None;
        }
        let at = self.stations.get(entity).ok()?.translation;
        self.people
            .iter()
            .find(|person| person.smith && at_work(person, at))
    }

    /// Whether a station can get on with its queue: always, unless it needs someone to
    /// work it and nobody is.
    pub fn works(&self, entity: Entity, station: Station) -> bool {
        station != Station::Forge || self.worker(entity, station).is_some()
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
        let Some(station) = prop.item.station else {
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
    for (entity, prop, at, mut works) in &mut stations {
        let Some(station) = prop.item.station else {
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
            did.write(Did(Skill::Crafting, ONCE));
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
    belongings: &mut Belongings,
    notice: &mut Notice,
) {
    let recipe = &RECIPES[index];
    if !staffed {
        notice.say("The forge needs a smith to work it");
    } else if works.queue.len() >= MOST_QUEUED {
        notice.say(format!(
            "The {} has enough to do",
            recipe.station.name().to_lowercase()
        ));
    } else if let Some((raw, more)) = belongings.missing(recipe.cost, 1) {
        notice.say(format!(
            "You need {more} more {}: {}",
            raw.name(),
            raw.where_found()
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
pub fn cancel(works: &mut Works, place: usize, belongings: &mut Belongings, notice: &mut Notice) {
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
    use bevy::ecs::system::RunSystemOnce;

    fn index_of(makes: Made) -> usize {
        RECIPES
            .iter()
            .position(|recipe| recipe.makes == makes)
            .unwrap()
    }

    #[test]
    fn a_queue_finishes_in_order_and_carries_time_over() {
        let iron = index_of(Made::Raw(Raw::Iron, 1));
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
        let station = world
            .spawn((Transform::from_translation(forge), Works::default()))
            .id();
        let staffed = |world: &mut World, station_kind| {
            world
                .run_system_once(move |staff: Staff| staff.works(station, station_kind))
                .unwrap()
        };
        assert!(!staffed(&mut world, Station::Forge));
        assert!(staffed(&mut world, Station::Smelter));
        assert!(Station::Smelter.works_alone() && !Station::Forge.works_alone());
        // A smith following you is not at work; one working beside the forge is.
        let smith = world
            .spawn(Person::new(
                "Bren",
                Doing::Following(Entity::PLACEHOLDER),
                true,
            ))
            .id();
        assert!(!staffed(&mut world, Station::Forge));
        world.get_mut::<Person>(smith).unwrap().doing = Doing::Working(forge + Vec3::X * 1.8);
        assert!(staffed(&mut world, Station::Forge));
        // Someone at work at another forge far off does not work this one.
        world.get_mut::<Person>(smith).unwrap().doing = Doing::Working(forge + Vec3::X * 50.0);
        assert!(!staffed(&mut world, Station::Forge));
    }

    #[test]
    fn the_workbench_has_tabs_and_the_smelter_none() {
        assert_eq!(
            families(Station::Workbench),
            [Family::Tools, Family::Clothing]
        );
        assert_eq!(families(Station::Smelter).len(), 1);
    }

    #[test]
    fn a_saved_name_finds_its_recipe_at_its_own_station() {
        let iron = recipe_named(Station::Smelter, "Iron").unwrap();
        assert_eq!(RECIPES[iron].makes, Made::Raw(Raw::Iron, 1));
        assert_eq!(recipe_named(Station::Forge, "Iron"), None);
    }
}
