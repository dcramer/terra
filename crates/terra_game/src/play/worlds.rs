//! A player's worlds (D579): several, each a folder under `worlds/` in the settings folder
//! holding its save (`save.ron`, read and written by `save`) and `world.ron`, its name, its
//! mode and when it was made and last played. Made with a name and its mode, Survival or
//! Creative (D494), kept with it; never replaced; deleted only when asked, after asking.
//! As Minecraft's singleplayer list, Valheim's and Enshrouded's worlds.
//!
//! Which world a run plays is chosen here at its start from its command line
//! (`terra_world::choice`, `--fresh`, `--save`), or later on the title screen (`title`).
//! A scripted run keeps no worlds unless given `--worlds=<dir>`, so it does the same on
//! every machine.

use super::controls::Settings;
use super::save::SavePath;
use crate::shell::Options;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use terra_world::choice::Choice;

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
    /// Seconds since 1970, when made and when last played.
    pub made: u64,
    pub played: u64,
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

/// Makes a world of that name and mode, in a folder named after it, never over another.
pub fn make(root: &Path, name: &str, creative: bool, now: u64) -> std::io::Result<World> {
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
        made: now,
        played: now,
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
    let world = make(root, FIRST, false, now)?;
    std::fs::rename(old, world.save())?;
    Ok(Some(world))
}

/// Which world a run plays without the title screen, by its command line: the one named,
/// a new one (`--fresh`), the Creative one played last or a new one (`--creative`), or the
/// one played last, made if there is none.
pub fn pick(root: &Path, choice: &Choice, fresh: bool, now: u64) -> std::io::Result<World> {
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
            make(root, &name, choice.creative, now)
        }
    }
}

/// Where this run keeps its worlds, if it keeps any.
#[derive(Resource, Clone)]
pub struct Root(pub PathBuf);

/// The world being played.
#[derive(Resource, Clone)]
pub struct Playing(pub World);

/// Plays a world: its save, and its mode.
pub fn play(commands: &mut Commands, options: &mut Options, world: World) {
    info!("Playing {} ({}).", world.meta.name, world.dir.display());
    commands.insert_resource(SavePath(world.save()));
    options.free_build |= world.meta.creative;
    commands.insert_resource(Playing(world));
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
        return;
    }
    match pick(&root, &choice, options.fresh, now()) {
        Ok(world) => play(&mut commands, &mut options, world),
        Err(problem) => warn!("No world to play in {}: {problem}", root.display()),
    }
}

#[cfg(test)]
mod tests;
