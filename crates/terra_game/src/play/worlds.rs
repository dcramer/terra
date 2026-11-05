//! A player's worlds (D579): several, each a folder under `worlds/` in the settings folder
//! holding its save (`save.ron`, read and written by `save`) and `world.ron`, its name, its
//! mode, its seed (D596: the world is made from it alone) and when it was made and last played. Made with a name and its mode, Survival or
//! Creative (D494), kept with it; never replaced; deleted only when asked, after asking.
//! As Minecraft's singleplayer list, Valheim's and Enshrouded's worlds.
//!
//! Which world a run plays is chosen here at its start from its command line
//! (`terra_world::choice`, `--fresh`, `--save`), or later on the title screen (`title`).
//! A scripted run keeps no worlds unless given `--worlds=<dir>`, so it does the same on
//! every machine.

use super::controls::Settings;
use super::save::SavePath;
use crate::shell::{Options, TEST_RADIUS_M, WORLD_RADIUS_M};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use terra_world::choice::Choice;
use terra_world::planet::Planet;
use terra_world::planet::entering::{FIRST_SEED, KnownStart, KnownTowns};
use terra_world::start::{Sited, Towns};

const META: &str = "world.ron";
const SAVE: &str = "save.ron";
/// The name the save from before worlds is kept under.
const FIRST: &str = "My world";

/// What a world's folder says of it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Meta {
    pub name: String,
    /// Creative builds free (D494); survival pays. Chosen when the world is made.
    pub creative: bool,
    /// What the world is made from, its only input (D596), chosen when it is made and never
    /// changed; a world kept from before worlds had seeds was made from the first.
    #[serde(default = "first_seed")]
    pub seed: u64,
    /// The planet's radius in metres, chosen when it is made and never changed, as its seed
    /// (a save's places stand on a planet of this size): [`WORLD_RADIUS_M`] unless `--radius`
    /// asked; a world kept from before worlds had sizes was made at the 2 km default.
    #[serde(default = "first_radius")]
    pub radius_m: f32,
    /// Seconds since 1970, when made and when last played.
    pub made: u64,
    pub played: u64,
    /// The start its land's search found when it was first entered, by the planet's radius
    /// (the way from the centre, D596): searching again takes seconds on a large world, and
    /// the start depends on the seed and radius alone, so it is found once.
    #[serde(default)]
    pub starts: Vec<(f32, [f32; 3])>,
    /// The towns its land's search found round the start when it was first entered, none
    /// found among them, by the planet's radius (D620): as the start, found once.
    #[serde(default)]
    pub towns: Vec<(f32, Vec<terra_land::town::TownSite>)>,
}

impl Meta {
    /// The towns kept for a planet of its seed and that radius.
    pub fn known_towns(&self, radius_m: f32) -> Option<KnownTowns> {
        let (_, towns) = self.towns.iter().find(|(kept, _)| *kept == radius_m)?;
        Some(KnownTowns {
            seed: self.seed,
            radius_m,
            towns: towns.clone(),
        })
    }

    /// The start kept for a planet of its seed and that radius.
    pub fn known_start(&self, radius_m: f32) -> Option<KnownStart> {
        let (_, direction) = self.starts.iter().find(|(kept, _)| *kept == radius_m)?;
        Some(KnownStart {
            seed: self.seed,
            radius_m,
            direction: Vec3::from_array(*direction),
        })
    }
}

fn first_seed() -> u64 {
    FIRST_SEED
}

fn first_radius() -> f32 {
    TEST_RADIUS_M
}

/// A world on disk.
#[derive(Clone, Debug, PartialEq)]
pub struct World {
    pub dir: PathBuf,
    pub meta: Meta,
}

impl World {
    pub fn save(&self) -> PathBuf {
        self.dir.join(SAVE)
    }

    /// Marks it played now, so it is the one Continue plays.
    pub fn played(&mut self, now: u64) -> std::io::Result<()> {
        self.meta.played = now;
        write_meta(&self.dir, &self.meta)
    }

    /// Deletes it, save and all.
    pub fn delete(&self) -> std::io::Result<()> {
        std::fs::remove_dir_all(&self.dir)
    }
}

/// Seconds since 1970.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Where a person's worlds are kept, beside their settings.
pub fn home() -> Option<PathBuf> {
    Settings::path().map(|settings| settings.with_file_name("worlds"))
}

/// The worlds in a folder, the last played first. A folder without a readable `world.ron`
/// is not a world and is left alone.
pub fn list(root: &Path) -> Vec<World> {
    let mut worlds: Vec<World> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let dir = entry.path();
            let text = std::fs::read_to_string(dir.join(META)).ok()?;
            let meta = ron::from_str(&text).ok()?;
            Some(World { dir, meta })
        })
        .collect();
    worlds.sort_by(|a, b| {
        (b.meta.played, b.meta.made)
            .cmp(&(a.meta.played, a.meta.made))
            .then_with(|| a.meta.name.cmp(&b.meta.name))
    });
    worlds
}

/// Makes a world of that name, mode, seed and radius, in a folder named after it, never over
/// another.
pub fn make(
    root: &Path,
    name: &str,
    creative: bool,
    seed: u64,
    radius_m: f32,
    now: u64,
) -> std::io::Result<World> {
    let taken: Vec<String> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    let dir = root.join(folder(name, &taken));
    std::fs::create_dir_all(&dir)?;
    let meta = Meta {
        name: name.trim().to_string(),
        creative,
        seed,
        radius_m,
        made: now,
        played: now,
        starts: Vec::new(),
        towns: Vec::new(),
    };
    write_meta(&dir, &meta)?;
    Ok(World { dir, meta })
}

fn write_meta(dir: &Path, meta: &Meta) -> std::io::Result<()> {
    let text = ron::ser::to_string_pretty(meta, ron::ser::PrettyConfig::default())
        .map_err(std::io::Error::other)?;
    std::fs::write(dir.join(META), text)
}

/// A folder name for a world's name: its letters and digits, lower case, dashes between,
/// and a number after it if that is taken.
pub fn folder(name: &str, taken: &[String]) -> String {
    let mut base = String::new();
    for c in name.trim().chars() {
        if c.is_alphanumeric() {
            base.extend(c.to_lowercase());
        } else if !base.ends_with('-') && !base.is_empty() {
            base.push('-');
        }
    }
    let base = match base.trim_end_matches('-') {
        "" => "world".to_string(),
        trimmed => trimmed.to_string(),
    };
    let free = |name: &String| !taken.iter().any(|t| t.eq_ignore_ascii_case(name));
    std::iter::once(base.clone())
        .chain((2..).map(|n| format!("{base}-{n}")))
        .find(free)
        .expect("some number is free")
}

/// The name a new world is offered: "World n", or "Creative n", the first not taken.
pub fn next_name(worlds: &[World], creative: bool) -> String {
    let kind = if creative { "Creative" } else { "World" };
    (1..)
        .map(|n| format!("{kind} {n}"))
        .find(|name| {
            !worlds
                .iter()
                .any(|w| w.meta.name.eq_ignore_ascii_case(name))
        })
        .expect("some number is free")
}

/// The save from before worlds (`save.ron` beside the settings) moved, untouched, into the
/// first world, the first time there are none. No converter (D353).
pub fn adopt(root: &Path, old: &Path, now: u64) -> std::io::Result<Option<World>> {
    if !old.exists() || !list(root).is_empty() {
        return Ok(None);
    }
    let world = make(root, FIRST, false, FIRST_SEED, TEST_RADIUS_M, now)?;
    std::fs::rename(old, world.save())?;
    Ok(Some(world))
}

/// Which world a run plays without the title screen, by its command line: the one named,
/// a new one (`--fresh`), the Creative one played last or a new one (`--creative`), or the
/// one played last, made if there is none. A world made is made from `seed` at `radius_m`;
/// one found keeps its own.
pub fn pick(
    root: &Path,
    choice: &Choice,
    fresh: bool,
    seed: u64,
    radius_m: f32,
    now: u64,
) -> std::io::Result<World> {
    let worlds = list(root);
    let found = if let Some(name) = &choice.world {
        worlds
            .into_iter()
            .find(|w| w.meta.name.eq_ignore_ascii_case(name.trim()))
    } else if fresh {
        None
    } else if choice.creative {
        worlds.into_iter().find(|w| w.meta.creative)
    } else {
        worlds.into_iter().next()
    };
    match found {
        Some(mut world) => world.played(now).map(|()| world),
        None => {
            let name = match &choice.world {
                Some(name) => name.clone(),
                None => next_name(&list(root), choice.creative),
            };
            make(root, &name, choice.creative, seed, radius_m, now)
        }
    }
}

/// Where this run keeps its worlds, if it keeps any.
#[derive(Resource, Clone)]
pub struct Root(pub PathBuf);

/// The world being played.
#[derive(Resource, Clone)]
pub struct Playing(pub World);

/// Plays a world: its save, its mode, its seed and its radius, which a `--seed` or `--radius`
/// never overrides.
pub fn play(commands: &mut Commands, options: &mut Options, world: World) {
    let meta = &world.meta;
    info!(
        "Playing {} ({}), seed {}, {} km round its centre.",
        meta.name,
        world.dir.display(),
        meta.seed,
        meta.radius_m / 1000.0
    );
    if options.seed.is_some_and(|asked| asked != meta.seed) {
        warn!(
            "{} was made from seed {}, which it keeps: --seed makes only a new world (--fresh) (D596).",
            meta.name, meta.seed
        );
    }
    if options
        .radius_asked
        .is_some_and(|asked| asked != meta.radius_m)
    {
        warn!(
            "{} was made {} m in radius, which it keeps, its save standing on it: --radius makes only a new world (--fresh) (D13).",
            meta.name, meta.radius_m
        );
    }
    commands.insert_resource(SavePath(world.save()));
    options.free_build |= meta.creative;
    options.seed = Some(meta.seed);
    options.radius_m = meta.radius_m;
    know_the_start(commands, meta);
    commands.insert_resource(Playing(world));
}

/// Where New world's rolled seeds start (`title::menu::Menu::dice`): the clock's nanoseconds
/// read once at the start, the one place the game reads chance from outside it (D596). With
/// no world kept, the land behind the title screen is the world New world first offers, the
/// dice's first roll, so a first launch loads its 30 km land once, behind the menu, and
/// Create plays on it; a seed of one's own, or another roll, crosses to its land (D620).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Dice(pub u64);

impl Dice {
    pub fn from_clock() -> Self {
        Self(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos() as u64),
        )
    }

    /// The first seed New world rolls from these dice (`Menu::roll`).
    pub fn first(self) -> u64 {
        terra_gen::hash::splitmix64(self.0)
    }
}

/// The radius a world made now is made at: `--radius`'s, else the game's world's (D13).
pub fn new_radius_m(options: &Options) -> f32 {
    options.radius_asked.unwrap_or(WORLD_RADIUS_M)
}

/// The start and the towns a world found before taken as they are when its land is made, or
/// none known.
fn know_the_start(commands: &mut Commands, meta: &Meta) {
    match meta.known_start(meta.radius_m) {
        Some(known) => commands.insert_resource(known),
        None => commands.remove_resource::<KnownStart>(),
    }
    match meta.known_towns(meta.radius_m) {
        Some(known) => commands.insert_resource(known),
        None => commands.remove_resource::<KnownTowns>(),
    }
}

/// Keeps in the world played the start and the towns its land's search found, the first time
/// it is entered at this radius, so the next entry takes them unsearched (`KnownStart`,
/// `KnownTowns`, D596, D620). Not the towns of a site asked for (`--site`), which are the
/// site's, not the world's.
pub fn keep_the_start(
    mut playing: ResMut<Playing>,
    planet: Option<Res<Planet>>,
    towns: Option<Res<Towns>>,
    sited: Option<Res<Sited>>,
) {
    let Some(planet) = planet else {
        return;
    };
    let radius_m = planet.radius_m();
    let meta = &playing.0.meta;
    if planet.seed().0 != meta.seed {
        return;
    }
    let start = meta.known_start(radius_m).is_none();
    let towns =
        towns.filter(|_| sited.is_none() && !meta.towns.iter().any(|(kept, _)| *kept == radius_m));
    if !start && towns.is_none() {
        return;
    }
    let world = &mut playing.0;
    if start {
        let direction = planet.start_direction().to_array();
        world.meta.starts.push((radius_m, direction));
    }
    if let Some(towns) = towns {
        world.meta.towns.push((radius_m, towns.0.clone()));
    }
    if let Err(problem) = write_meta(&world.dir, &world.meta) {
        warn!("Could not keep {}'s start: {problem}", world.meta.name);
    }
}

/// At the start: where the worlds are kept, the old save moved in, and, unless the title
/// screen is to choose, the world this run plays.
pub fn choose(mut commands: Commands, mut options: ResMut<Options>, choice: Res<Choice>) {
    // The yard, the flat test level, is never saved, and never touches a save.
    if options.yard() {
        return;
    }
    if let Some(path) = options.save.clone() {
        if options.fresh {
            let _ = std::fs::remove_file(&path);
        }
        commands.insert_resource(SavePath(path));
        return;
    }
    let root = match &choice.worlds {
        Some(dir) => dir.clone(),
        None if options.scripted() => return,
        None => match home() {
            Some(home) => home,
            None => return,
        },
    };
    if choice.worlds.is_none()
        && let Some(old) = Settings::path().map(|s| s.with_file_name(SAVE))
    {
        match adopt(&root, &old, now()) {
            Ok(Some(world)) => info!("The saved game is now the world {}.", world.meta.name),
            Ok(None) => {}
            Err(problem) => warn!("Could not move {} into a world: {problem}", old.display()),
        }
    }
    commands.insert_resource(Root(root.clone()));
    if choice.title {
        // The title screen opens over the world Continue plays, so it is loaded already; with
        // none, over the world New world first offers (`Dice`), so it is loaded already too.
        let dice = Dice::from_clock();
        commands.insert_resource(dice);
        match list(&root).first() {
            Some(world) if options.seed.is_none() => {
                options.seed = Some(world.meta.seed);
                options.radius_m = world.meta.radius_m;
                know_the_start(&mut commands, &world.meta);
            }
            _ => {
                options.radius_m = new_radius_m(&options);
                options.seed.get_or_insert(dice.first());
            }
        }
        return;
    }
    let seed = options.seed.unwrap_or(FIRST_SEED);
    let radius_m = new_radius_m(&options);
    match pick(&root, &choice, options.fresh, seed, radius_m, now()) {
        Ok(world) => play(&mut commands, &mut options, world),
        Err(problem) => warn!("No world to play in {}: {problem}", root.display()),
    }
}

#[cfg(test)]
mod tests;
