//! A theme as data (D136): what a dungeon of it is for, how big it is, what lives in it, what
//! it gives and the modules it is dressed from. One file a theme in `assets/data/dungeons/`;
//! a new theme is a new file, no code.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// What a room is for in the mission (Dormans' mission graphs, as Unexplored lays them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
pub enum Role {
    /// Where the player comes down from the surface.
    Entrance,
    /// A room on the main path.
    Passage,
    /// A room on the loop that leaves the main path and rejoins it.
    Branch,
    /// The room behind the locked gate, before the goal.
    Guard,
    /// What the dungeon is for: the relic, the trapped miner, the sanctum.
    Goal,
    /// A dead end holding a key (a key, a lever, a brazier to light).
    Puzzle,
    /// A room by the gate with a way back, open from one side only.
    Shortcut,
    /// A room behind a hidden way.
    Secret,
    /// A quiet room with a fire, part way down.
    Rest,
    /// A dead end with loot.
    Treasure,
}

/// The kinds of space a cell belongs to, for which modules a cell may take.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Space {
    Room,
    Corridor,
    Stair,
}

/// A creature the theme sends, by role and depth.
#[derive(Clone, Debug, Deserialize)]
pub struct Mob {
    /// A species of `assets/data/creatures.ron`.
    pub species: String,
    /// The rooms it is found in.
    pub roles: Vec<Role>,
    /// The least danger (0 at the entrance, 1 at the goal) it is found at.
    #[serde(default)]
    pub danger: f32,
    /// How many to a group, least and most.
    pub group: (u32, u32),
    #[serde(default = "one")]
    pub weight: f32,
}

/// An item the theme gives, by role and depth.
#[derive(Clone, Debug, Deserialize)]
pub struct Loot {
    /// An item of `assets/data/items.ron`.
    pub item: String,
    pub roles: Vec<Role>,
    #[serde(default)]
    pub danger: f32,
    /// How many, least and most.
    pub count: (u32, u32),
    #[serde(default = "one")]
    pub weight: f32,
}

/// A module: what fills one cell (3 m square, a level high), and the sockets each of its sides
/// takes. Sides are north, east, south, west before rotation; each is a `|`-joined list of
/// sockets: `open`, `wall`, `door`, `stair`, `rubble`, `water`.
#[derive(Clone, Debug, Deserialize)]
pub struct Module {
    pub name: String,
    pub sides: Vec<String>,
    #[serde(default = "one")]
    pub weight: f32,
    /// The spaces it may stand in.
    #[serde(default = "rooms_and_corridors")]
    pub spaces: Vec<Space>,
    /// Turned to all four facings (the default) or kept as written.
    #[serde(default = "yes")]
    pub rotates: bool,
    /// Placed only where placement asks for it (the goal's dais, an altar, a stair).
    #[serde(default)]
    pub fixed: bool,
    /// Nothing walks through it (a collapse).
    #[serde(default)]
    pub blocks: bool,
    /// One of the plain floors the dressing falls back to.
    #[serde(default)]
    pub safe: bool,
}

/// The modules placement holds as given.
#[derive(Clone, Debug, Deserialize)]
pub struct Features {
    /// In the goal room.
    pub goal: String,
    /// In each puzzle room.
    pub key: String,
    /// In the rest room.
    pub rest: String,
    /// Every stair cell.
    pub stair: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Theme {
    /// The file's stem: `crypt`.
    pub name: String,
    /// What the player would call it: "The Ossuary Gate".
    pub title: String,
    /// The goal in a sentence.
    pub goal: String,
    /// Rooms, least and most.
    pub rooms: (u32, u32),
    /// Levels below the surface.
    pub levels: u32,
    /// Keys to the one lock on the gate (three braziers are three keys).
    pub keys: u32,
    /// The building kind of its walls and floors (D181), for laying it in cubes.
    pub walls: String,
    pub floors: String,
    /// The building kind of its pillars, timbering, door jambs and lintels.
    #[serde(default = "hewn")]
    pub trim: String,
    pub features: Features,
    pub bestiary: Vec<Mob>,
    pub loot: Vec<Loot>,
    pub modules: Vec<Module>,
}

fn one() -> f32 {
    1.0
}

fn hewn() -> String {
    "Hewn".to_string()
}

fn yes() -> bool {
    true
}

fn rooms_and_corridors() -> Vec<Space> {
    vec![Space::Room, Space::Corridor]
}

/// Where the themes are, from the asset root.
pub const DIR: &str = "data/dungeons";

/// The asset root of this checkout.
pub fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

impl Theme {
    pub fn parse(text: &str) -> Result<Theme, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// Every theme in `assets/data/dungeons`, by name.
    pub fn all(assets: &Path) -> Result<Vec<Theme>, String> {
        let dir = assets.join(DIR);
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.extension().is_some_and(|x| x == "ron"))
            .collect();
        files.sort();
        files
            .iter()
            .map(|path| {
                let text = std::fs::read_to_string(path)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                Theme::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
            })
            .collect()
    }

    pub fn module(&self, name: &str) -> Option<usize> {
        self.modules.iter().position(|m| m.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names_in(file: &str, key: &str) -> Vec<String> {
        let text = std::fs::read_to_string(assets().join(file)).unwrap();
        let pattern = format!("{key}: \"");
        text.split(&pattern)
            .skip(1)
            .filter_map(|rest| rest.split('"').next().map(str::to_string))
            .collect()
    }

    #[test]
    fn themes_name_what_exists() {
        let themes = Theme::all(&assets()).unwrap();
        assert!(themes.len() >= 3, "crypt, flooded mine and ruined keep");
        let items = names_in("data/items.ron", "name");
        let creatures = names_in("data/creatures.ron", "name");
        for theme in &themes {
            for loot in &theme.loot {
                assert!(
                    items.contains(&loot.item),
                    "{}: no item {}",
                    theme.name,
                    loot.item
                );
            }
            for mob in &theme.bestiary {
                assert!(
                    creatures.contains(&mob.species),
                    "{}: no creature {}",
                    theme.name,
                    mob.species
                );
            }
            let f = &theme.features;
            for name in [&f.goal, &f.key, &f.rest, &f.stair] {
                let at = theme
                    .module(name)
                    .unwrap_or_else(|| panic!("{}: no module {name}", theme.name));
                assert!(
                    theme.modules[at].fixed,
                    "{}: {name} is placed, so fixed",
                    theme.name
                );
            }
            assert!(
                theme.modules.iter().any(|m| m.safe),
                "{}: no safe floor",
                theme.name
            );
            assert!(theme.rooms.0 <= theme.rooms.1 && theme.levels >= 1 && theme.keys >= 1);
        }
    }
}
