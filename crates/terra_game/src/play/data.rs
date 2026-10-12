//! The kinds of thing, read from the files in `assets/data` (D136): items, stations,
//! recipes, creatures, what grows, what the ground gives and what can be built. Each is
//! read into a registry, a resource the systems that need it read, so the content a game
//! runs on can be replaced as a whole: by a planet's theme, by what a server sends (D51),
//! or by a file edited while the game runs.
//!
//! Files are read from the asset folder Bevy reads from, so a build finds them next to
//! itself and a test finds them in the repository.
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
use super::dig::Yields;
use super::flora_kinds::Flora;
use super::inventory::Items;
use super::species::Bestiary;
use bevy::asset::io::file::FileAssetReader;
use bevy::prelude::*;
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use std::time::SystemTime;

/// Every file read here.
const FILES: [&str; 7] = [
    "items.ron",
    "stations.ron",
    "recipes.ron",
    "builds.ron",
    "creatures.ron",
    "flora.ron",
    "ground.ron",
];

/// How often the files are looked at for changes, in seconds.
const LOOK_EVERY_S: f32 = 1.0;

/// Where a data file is.
pub fn path(file: &str) -> PathBuf {
    FileAssetReader::get_base_path()
        .join("assets/data")
        .join(file)
}

/// Reads a data file. A mistake in one is a mistake in the game, so it stops it, saying
/// which file and where.
pub fn read<T: DeserializeOwned>(file: &str) -> T {
    let path = path(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|problem| panic!("Could not read {}: {problem}", path.display()));
    ron::from_str(&text).unwrap_or_else(|problem| panic!("{}: {problem}", path.display()))
}

/// Every registry, read together: items first, since the others name items, and stations
/// before what names them.
struct Registries {
    items: Items,
    stations: Stations,
    recipes: Recipes,
    builds: Builds,
    bestiary: Bestiary,
    flora: Flora,
    yields: Yields,
}

impl Registries {
    fn read() -> Self {
        let items = Items::default();
        let stations = Stations::read();
        let recipes = Recipes::read(&stations, &items);
        let builds = Builds::read(&items, &stations);
        let bestiary = Bestiary::read(&items);
        let flora = Flora::read(&items);
        let yields = Yields::read(&items);
        Self {
            items,
            stations,
            recipes,
            builds,
            bestiary,
            flora,
            yields,
        }
    }

    /// Reads them, or says why not.
    fn try_read() -> Option<Self> {
        std::panic::catch_unwind(Self::read).ok()
    }

    /// Every kind of every registry, in order: what the world's ids are places in.
    fn kinds(&self) -> Vec<String> {
        let Self {
            items,
            stations,
            recipes,
            builds,
            bestiary,
            flora,
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
        kinds.extend(flora.kinds().iter().map(|kind| kind.name.clone()));
        kinds.extend(yields.names().map(str::to_string));
        kinds
    }

    fn insert(self, world: &mut World) {
        world.insert_resource(self.items);
        world.insert_resource(self.stations);
        world.insert_resource(self.recipes);
        world.insert_resource(self.builds);
        world.insert_resource(self.bestiary);
        world.insert_resource(self.flora);
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
            flora: world.remove_resource().unwrap(),
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

pub fn plugin(app: &mut App) {
    Registries::read().insert(app.world_mut());
    app.insert_resource(Watched {
        changed: changed(),
        since: 0.0,
    })
    .add_systems(Last, reload);
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
    let Some(fresh) = Registries::try_read() else {
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
    fn the_same_files_read_twice_are_the_same_kinds() {
        let kinds = Registries::read().kinds();
        assert_eq!(same_kinds(&kinds, &Registries::read().kinds()), Ok(()));
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
        Registries::read().insert(&mut world);
        let wood = world.resource::<Items>().kind("Wood");
        let first = world.resource_mut::<Items>().make(wood).id;
        replace(&mut world, Registries::read()).unwrap();
        let next = world.resource_mut::<Items>().make(wood).id;
        assert_ne!(first, next);
    }
}
