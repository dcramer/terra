//! The kinds of thing gameplay runs on, read from the files in `assets/data` (D136):
//! items, stations, recipes, what can be built, creatures and what the ground gives, and
//! what cutting each kind of flora gives, by the items the world's flora file names. Each
//! is read into a registry, a resource the systems that need it read, so the content a game
//! runs on can be replaced as a whole: by a planet's theme, by what a server sends (D51),
//! or by a file edited while the game runs. The world's own kinds, what grows, are read by
//! `data`.
//!
//! A file saved while the game runs is read again within a second, with the rest, since
//! they name each other. What reads a kind as it goes, such as a recipe's time or a
//! creature's speed, sees the change at once; what was made from a kind before, such as a
//! creature's health or a tree already grown, keeps what it was made with. Kinds are known
//! by their place in their file, so a change that adds, takes away or reorders kinds needs
//! a restart, and so does a mistake: either is said in the log, and the kinds stay as they
//! were.

use super::catalogue::Builds;
use super::crafting::{Recipes, Stations};
use super::data::path;
use super::dig::Yields;
use super::flora_kinds::{Flora, FloraKind};
use super::inventory::Items;
use super::species::Bestiary;
use bevy::prelude::*;
use std::time::SystemTime;
use terra_items::ItemKind;

/// Every file read here.
const FILES: [&str; 6] = [
    "items.ron",
    "stations.ron",
    "recipes.ron",
    "builds.ron",
    "creatures.ron",
    "ground.ron",
];

/// How often the files are looked at for changes, in seconds.
const LOOK_EVERY_S: f32 = 1.0;

/// What cutting each kind of flora gives (`Cuttable::gives`), as the registry of items
/// knows it. The flora file is the world's and names items by name; this is where those
/// names become items.
#[derive(Resource, Debug, PartialEq)]
pub struct FloraGives(Vec<Option<ItemKind>>);

impl FloraGives {
    /// Each kind's item, or the first name that is no kind of item.
    pub fn resolve(flora: &Flora, items: &Items) -> Result<Self, String> {
        flora
            .kinds()
            .iter()
            .map(|kind| {
                kind.gives()
                    .map(|name| {
                        items.find(name).ok_or_else(|| {
                            format!("flora.ron: there is no kind of item called {name}")
                        })
                    })
                    .transpose()
            })
            .collect::<Result<_, _>>()
            .map(Self)
    }

    /// The item cutting a kind gives.
    pub fn item(&self, kind: FloraKind) -> ItemKind {
        self.0[kind.0 as usize].expect("a kind that can be cut gives an item")
    }
}

/// Every registry, read together: items first, since the others name items, and stations
/// before what names them.
struct Registries {
    items: Items,
    stations: Stations,
    recipes: Recipes,
    builds: Builds,
    bestiary: Bestiary,
    flora_gives: FloraGives,
    yields: Yields,
}

impl Registries {
    fn read(flora: &Flora) -> Self {
        let items = Items::default();
        let stations = Stations::read();
        let recipes = Recipes::read(&stations, &items);
        let builds = Builds::read(&items, &stations);
        let bestiary = Bestiary::read(&items);
        let flora_gives =
            FloraGives::resolve(flora, &items).unwrap_or_else(|problem| panic!("{problem}"));
        let yields = Yields::read(&items);
        Self {
            items,
            stations,
            recipes,
            builds,
            bestiary,
            flora_gives,
            yields,
        }
    }

    /// Reads them, or says why not.
    fn try_read(flora: &Flora) -> Option<Self> {
        std::panic::catch_unwind(|| Self::read(flora)).ok()
    }

    /// Every kind of every registry, in order: what the world's ids are places in.
    fn kinds(&self) -> Vec<String> {
        let Self {
            items,
            stations,
            recipes,
            builds,
            bestiary,
            flora_gives: _,
            yields,
        } = self;
        let mut kinds = Vec::new();
        kinds.extend(items.kinds().map(|kind| items.name(kind).to_string()));
        kinds.extend(
            stations
                .all()
                .map(|station| stations.name(station).to_string()),
        );
        kinds.extend(recipes.iter().map(|recipe| {
            let station = stations.name(recipe.station);
            format!("{station}: {}", recipe.makes.name(items))
        }));
        kinds.extend(builds.parts().iter().map(|part| part.name.clone()));
        kinds.extend(builds.items().map(|item| item.file.clone()));
        kinds.extend(
            bestiary
                .all()
                .map(|species| bestiary.name(species).to_string()),
        );
        kinds.extend(yields.names().map(str::to_string));
        kinds
    }

    fn insert(self, world: &mut World) {
        world.insert_resource(self.items);
        world.insert_resource(self.stations);
        world.insert_resource(self.recipes);
        world.insert_resource(self.builds);
        world.insert_resource(self.bestiary);
        world.insert_resource(self.flora_gives);
        world.insert_resource(self.yields);
    }

    /// The registries in a world, taken out of it.
    fn take(world: &mut World) -> Self {
        Self {
            items: world.remove_resource().unwrap(),
            stations: world.remove_resource().unwrap(),
            recipes: world.remove_resource().unwrap(),
            builds: world.remove_resource().unwrap(),
            bestiary: world.remove_resource().unwrap(),
            flora_gives: world.remove_resource().unwrap(),
            yields: world.remove_resource().unwrap(),
        }
    }
}

/// When each file was last changed, as last looked.
#[derive(Resource)]
struct Watched {
    changed: Vec<Option<SystemTime>>,
    since: f32,
}

fn changed() -> Vec<Option<SystemTime>> {
    FILES
        .iter()
        .map(|file| {
            std::fs::metadata(path(file))
                .and_then(|it| it.modified())
                .ok()
        })
        .collect()
}

/// Needs the world's flora read first (`data`).
pub fn plugin(app: &mut App) {
    Registries::read(app.world().resource::<Flora>()).insert(app.world_mut());
    app.insert_resource(Watched {
        changed: changed(),
        since: 0.0,
    })
    .add_systems(
        Last,
        (reload, give_for_flora.run_if(resource_changed::<Flora>)),
    );
}

/// When the world's flora file is read again, works out again what each kind gives.
fn give_for_flora(flora: Res<Flora>, items: Res<Items>, mut gives: ResMut<FloraGives>) {
    match FloraGives::resolve(&flora, &items) {
        Ok(fresh) => {
            gives.set_if_neq(fresh);
        }
        Err(problem) => warn!("{problem}; what flora gives stays as it was."),
    }
}

/// Reads the files again when one has changed, if the kinds are the same ones.
fn reload(world: &mut World) {
    let dt = world.resource::<Time<Real>>().delta_secs();
    let mut watched = world.resource_mut::<Watched>();
    watched.since += dt;
    if watched.since < LOOK_EVERY_S {
        return;
    }
    watched.since = 0.0;
    let now = changed();
    if now == watched.changed {
        return;
    }
    watched.changed = now;
    let Some(fresh) = Registries::try_read(world.resource::<Flora>()) else {
        warn!("A data file has a mistake, said above; the kinds stay as they were.");
        return;
    };
    match replace(world, fresh) {
        Ok(()) => info!("Read the data files again."),
        Err(problem) => {
            warn!("A data file changed which kinds there are ({problem}); restart to see it.");
        }
    }
}

/// Puts registries read afresh in place of the world's, if they have the same kinds,
/// keeping the identities of items already given out.
fn replace(world: &mut World, fresh: Registries) -> Result<(), String> {
    let was = Registries::take(world);
    if let Err(problem) = same_kinds(&was.kinds(), &fresh.kinds()) {
        was.insert(world);
        return Err(problem);
    }
    let mut items = was.items;
    items.take_kinds(fresh.items);
    Registries { items, ..fresh }.insert(world);
    Ok(())
}

/// Whether two readings have the same kinds in the same places, or the first that differs.
fn same_kinds(was: &[String], now: &[String]) -> Result<(), String> {
    match was.iter().zip(now).find(|(was, now)| was != now) {
        Some((was, now)) => Err(format!("{was} became {now}")),
        None if was.len() != now.len() => Err(format!("{} kinds became {}", was.len(), now.len())),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_flora_gives_is_a_kind_of_item_there_is() {
        let flora = Flora::read();
        let items = Items::default();
        let gives = FloraGives::resolve(&flora, &items).unwrap();
        let ore_rock = flora
            .kinds()
            .iter()
            .find(|kind| kind.name == "Ore rock")
            .unwrap();
        let cut = ore_rock.cuttable(0.8).unwrap();
        assert_eq!(gives.item(cut.gives.0), items.kind("Ore"));
    }

    #[test]
    fn the_same_files_read_twice_are_the_same_kinds() {
        let kinds = Registries::read(&Flora::read()).kinds();
        assert_eq!(
            same_kinds(&kinds, &Registries::read(&Flora::read()).kinds()),
            Ok(())
        );
    }

    #[test]
    fn kinds_added_or_moved_are_noticed() {
        let kinds = |names: &[&str]| {
            names
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>()
        };
        let was = kinds(&["Wood", "Stone"]);
        assert!(same_kinds(&was, &kinds(&["Stone", "Wood"])).is_err());
        assert!(same_kinds(&was, &kinds(&["Wood", "Stone", "Iron"])).is_err());
        assert!(same_kinds(&was, &kinds(&["Wood"])).is_err());
    }

    #[test]
    fn reading_again_keeps_the_identities_given_out() {
        let mut world = World::new();
        Registries::read(&Flora::read()).insert(&mut world);
        let wood = world.resource::<Items>().kind("Wood");
        let first = world.resource_mut::<Items>().make(wood).id;
        replace(&mut world, Registries::read(&Flora::read())).unwrap();
        let next = world.resource_mut::<Items>().make(wood).id;
        assert_ne!(first, next);
    }
}
