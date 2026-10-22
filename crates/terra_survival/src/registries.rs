//! The kinds of thing gameplay runs on, read from the files in `assets/data` (D136):
//! items, stations, recipes, what can be built and what the ground gives, and what cutting
//! each kind of flora gives, by the items the world's flora file names. Each is read into a
//! registry, a resource the systems that need it read, so the content a game runs on can be
//! replaced as a whole: by a planet's theme, by what a server sends (D51), or by a file
//! edited while the game runs. The world's own kinds, what grows, are read by `data`. A
//! domain above reads its own file (D407) and has it read with these (`Registry`), as the
//! creatures their bestiary.
//!
//! A file saved while the game runs is read again within a second, with the rest, since
//! they name each other. What reads a kind as it goes, such as a recipe's time or a
//! creature's speed, sees the change at once; what was made from a kind before, such as a
//! creature's health or a tree already grown, keeps what it was made with. Kinds are known
//! by their place in their file, so a change that adds, takes away or reorders kinds needs
//! a restart, and so does a mistake: either is said in the log, and the kinds stay as they
//! were.

use crate::catalogue::Builds;
use crate::crafting::{Recipes, Stations};
use crate::dig::Yields;
use crate::inventory::Items;
use bevy::prelude::*;
use std::time::SystemTime;
use terra_items::ItemKind;
use terra_world::data::path;
use terra_world::flora_kinds::{Flora, FloraKind};

/// Every file read here; those above add theirs (`Above`).
const FILES: [&str; 5] = [
    "items.ron",
    "stations.ron",
    "recipes.ron",
    "builds.ron",
    "ground.ron",
];

/// A registry a domain above reads from its own file in `assets/data`, as the creatures
/// their bestiary: read, compared and put in place with these (`read_with_the_rest`), so a
/// change that breaks it or changes its kinds leaves every registry as it was.
pub trait Registry: Resource + Sized {
    /// Its file in `assets/data`.
    const FILE: &'static str;
    /// Reads it, naming items by the registry of items; panics on a mistake, as these do.
    fn read(items: &Items) -> Self;
    /// Its kinds, in order.
    fn kinds(&self) -> Vec<String>;
}

/// Reads a registry from above now, and again with these whenever one of their files
/// changes. After `plugin`.
pub fn read_with_the_rest<R: Registry>(app: &mut App) {
    let reader = Reader::of::<R>();
    (reader.read)(app.world().resource::<Items>()).insert(app.world_mut());
    let world = app.world_mut();
    world
        .resource_mut::<Watched>()
        .changed
        .push(modified(R::FILE));
    world.resource_mut::<Above>().0.push(reader);
}

/// A registry from above, read: its kinds, and how to put it in a world.
trait Read: Send + Sync {
    fn kinds(&self) -> Vec<String>;
    fn insert(self: Box<Self>, world: &mut World);
}

impl<R: Registry> Read for R {
    fn kinds(&self) -> Vec<String> {
        Registry::kinds(self)
    }

    fn insert(self: Box<Self>, world: &mut World) {
        world.insert_resource(*self);
    }
}

/// How to read a registry from above, and take it out of a world.
#[derive(Clone, Copy)]
struct Reader {
    file: &'static str,
    read: fn(&Items) -> Box<dyn Read>,
    take: fn(&mut World) -> Box<dyn Read>,
}

impl Reader {
    fn of<R: Registry>() -> Self {
        Self {
            file: R::FILE,
            read: |items| Box::new(R::read(items)),
            take: |world| Box::new(world.remove_resource::<R>().unwrap()),
        }
    }
}

/// The registries from above, in the order they were added.
#[derive(Resource, Default, Clone)]
struct Above(Vec<Reader>);

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
/// before what names them; those from above after what can be built.
struct Registries {
    items: Items,
    stations: Stations,
    recipes: Recipes,
    builds: Builds,
    above: Vec<Box<dyn Read>>,
    flora_gives: FloraGives,
    yields: Yields,
}

impl Registries {
    fn read(flora: &Flora, above: &Above) -> Self {
        let items = Items::default();
        let stations = Stations::read();
        let recipes = Recipes::read(&stations, &items);
        let builds = Builds::read(&items, &stations);
        let above = above.0.iter().map(|reader| (reader.read)(&items)).collect();
        let flora_gives =
            FloraGives::resolve(flora, &items).unwrap_or_else(|problem| panic!("{problem}"));
        let yields = Yields::read(&items);
        Self {
            items,
            stations,
            recipes,
            builds,
            above,
            flora_gives,
            yields,
        }
    }

    /// Reads them, or says why not.
    fn try_read(flora: &Flora, above: &Above) -> Option<Self> {
        std::panic::catch_unwind(|| Self::read(flora, above)).ok()
    }

    /// Every kind of every registry, in order: what the world's ids are places in.
    fn kinds(&self) -> Vec<String> {
        let Self {
            items,
            stations,
            recipes,
            builds,
            above,
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
        kinds.extend(above.iter().flat_map(|registry| registry.kinds()));
        kinds.extend(yields.names().map(str::to_string));
        kinds
    }

    fn insert(self, world: &mut World) {
        world.insert_resource(self.items);
        world.insert_resource(self.stations);
        world.insert_resource(self.recipes);
        world.insert_resource(self.builds);
        for registry in self.above {
            registry.insert(world);
        }
        world.insert_resource(self.flora_gives);
        world.insert_resource(self.yields);
    }

    /// The registries in a world, taken out of it.
    fn take(world: &mut World, above: &Above) -> Self {
        Self {
            items: world.remove_resource().unwrap(),
            stations: world.remove_resource().unwrap(),
            recipes: world.remove_resource().unwrap(),
            builds: world.remove_resource().unwrap(),
            above: above.0.iter().map(|reader| (reader.take)(world)).collect(),
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

/// When each file, these and those from above, was last changed.
fn changed(above: &Above) -> Vec<Option<SystemTime>> {
    let above = above.0.iter().map(|reader| reader.file);
    FILES.into_iter().chain(above).map(modified).collect()
}

fn modified(file: &str) -> Option<SystemTime> {
    std::fs::metadata(path(file))
        .and_then(|it| it.modified())
        .ok()
}

/// Needs the world's flora read first (`data`).
pub fn plugin(app: &mut App) {
    let above = Above::default();
    Registries::read(app.world().resource::<Flora>(), &above).insert(app.world_mut());
    app.insert_resource(Watched {
        changed: changed(&above),
        since: 0.0,
    })
    .insert_resource(above)
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
    let above = world.resource::<Above>().clone();
    let now = changed(&above);
    let mut watched = world.resource_mut::<Watched>();
    if now == watched.changed {
        return;
    }
    watched.changed = now;
    let Some(fresh) = Registries::try_read(world.resource::<Flora>(), &above) else {
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
    let above = world.resource::<Above>().clone();
    let was = Registries::take(world, &above);
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
    fn every_registry_is_among_the_kinds_in_its_own_order() {
        let above = Above(vec![Reader::of::<Herd>()]);
        let registries = Registries::read(&Flora::read(), &above);
        let kinds = registries.kinds();
        let Registries {
            items,
            stations,
            recipes,
            builds,
            above: _,
            flora_gives: _,
            yields,
        } = &registries;
        let each: [(&str, Vec<String>); 7] = [
            (
                "items",
                items.kinds().map(|kind| items.name(kind).into()).collect(),
            ),
            (
                "stations",
                stations.all().map(|at| stations.name(at).into()).collect(),
            ),
            (
                "recipes",
                recipes
                    .iter()
                    .map(|recipe| recipe.makes.name(items))
                    .collect(),
            ),
            (
                "parts",
                builds
                    .parts()
                    .iter()
                    .map(|part| part.name.clone())
                    .collect(),
            ),
            (
                "placed items",
                builds.items().map(|item| item.file.clone()).collect(),
            ),
            ("from above", Herd::read(items).0),
            ("ground", yields.names().map(Into::into).collect()),
        ];
        // Each registry's kinds stand together and in order, so moving one is noticed.
        for (registry, names) in &each {
            assert!(names.len() > 1, "{registry} has too few kinds to move one");
            assert!(
                kinds.windows(names.len()).any(|window| window
                    .iter()
                    .zip(names)
                    .all(|(kind, name)| kind.ends_with(name.as_str()))),
                "the {registry} are not among the kinds in order"
            );
        }
        let all: usize = each.iter().map(|(_, names)| names.len()).sum();
        assert_eq!(kinds.len(), all);
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
        let above = Above::default();
        Registries::read(&Flora::read(), &above).insert(&mut world);
        world.insert_resource(above.clone());
        let wood = world.resource::<Items>().kind("Wood");
        let first = world.resource_mut::<Items>().make(wood).id;
        replace(&mut world, Registries::read(&Flora::read(), &above)).unwrap();
        let next = world.resource_mut::<Items>().make(wood).id;
        assert_ne!(first, next);
    }

    /// A registry from above, as the creatures' bestiary is.
    #[derive(Resource)]
    struct Herd(Vec<String>);

    impl Registry for Herd {
        const FILE: &'static str = "creatures.ron";

        fn read(_: &Items) -> Self {
            Herd(vec!["Deer".into(), "Fox".into()])
        }

        fn kinds(&self) -> Vec<String> {
            self.0.clone()
        }
    }

    #[test]
    fn a_registry_from_above_is_read_watched_and_replaced_with_the_rest() {
        let mut app = App::new();
        app.insert_resource(Flora::read());
        plugin(&mut app);
        read_with_the_rest::<Herd>(&mut app);
        let world = app.world_mut();
        assert_eq!(world.resource::<Herd>().0, ["Deer", "Fox"]);
        assert_eq!(world.resource::<Watched>().changed.len(), FILES.len() + 1);
        let above = world.resource::<Above>().clone();
        // A kind born in it is a change of kinds: every registry stays as it was.
        let mut fresh = Registries::read(&Flora::read(), &above);
        fresh.above = vec![Box::new(Herd(vec![
            "Deer".into(),
            "Fox".into(),
            "Hare".into(),
        ]))];
        assert!(replace(world, fresh).is_err());
        assert_eq!(world.resource::<Herd>().0.len(), 2);
        assert!(replace(world, Registries::read(&Flora::read(), &above)).is_ok());
        assert_eq!(world.resource::<Herd>().0, ["Deer", "Fox"]);
    }
}
