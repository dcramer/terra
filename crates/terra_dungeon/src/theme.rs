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

/// A module: what fills one cell (4 m square, a level high, D527), and the sockets each of its sides
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
    /// The building kind of its ceilings where no room stands on them (the rock a mine is
    /// cut in, not its trodden floor); the floors' if not given. A ceiling under a room is
    /// that room's floor.
    #[serde(default)]
    pub ceilings: Option<String>,
    /// The building kind of its pillars, timbering, door jambs and lintels.
    #[serde(default = "hewn")]
    pub trim: String,
    /// The metres clear over its corridors' floors and its rooms', under a level's 5.5 m
    /// (D527): a vaulted theme's corridor crowned at 4.5 m and its rooms at 5 m, a mine's
    /// drifts lower. Flat ceilings of cubes, under the kit's vaults where it has one.
    #[serde(default = "full_height")]
    pub headroom: (f32, f32),
    /// The dressed pieces laid over its cubes (D527); none for a theme of plain cubes.
    #[serde(default)]
    pub kit: Option<Kit>,
    pub features: Features,
    pub bestiary: Vec<Mob>,
    pub loot: Vec<Loot>,
    pub modules: Vec<Module>,
    /// What its rooms are dressed with by what they are for, over their modules' props.
    #[serde(default)]
    pub dressing: Vec<Dress>,
}

/// A room's dressing by its purpose (docs/dungeons.md#run-in): so many props laid against its
/// walls, each a pick of `props`, as a level designer sets a room out by what happened there
/// (a guard room's fallen, a treasury's offerings, a rest's stores).
#[derive(Clone, Debug, Deserialize)]
pub struct Dress {
    /// The rooms it dresses; a room of two entries' roles takes both.
    pub roles: Vec<Role>,
    /// Each a model under the asset root and its scale.
    pub props: Vec<(String, f32)>,
    /// Props a room, least and most, more in a bigger room: so many per four cells.
    pub count: (u32, u32),
    /// The room type it makes, as a level designer names it (ossuary, treasury, guard post):
    /// docs/dungeons.md, room types (D539).
    #[serde(default)]
    pub name: String,
    /// The room's focal prop, one a room, on the bare wall farthest from its doorways; a room
    /// whose feature (a dais, an altar, a fire) is its hero takes none.
    #[serde(default)]
    pub hero: Vec<(String, f32)>,
    /// Small things on the floor, in the corners and then by the walls.
    #[serde(default)]
    pub clutter: Vec<(String, f32)>,
    /// Clutter a cell, least and most.
    #[serde(default)]
    pub litter: (u32, u32),
    /// A moment told by props laid together: each a model, its scale and its place from one
    /// wall spot, (along the wall, out from it) in metres; laid all or none.
    #[serde(default)]
    pub vignette: Vec<(String, f32, (f32, f32))>,
    /// What lights the hero where no torch does, as a set dresser lights a focal prop from a
    /// source in the fiction (candles by a coffin, the looter's lantern left by it): a model
    /// laid beside the hero, with its small light (`LightKind::Lamp`).
    #[serde(default)]
    pub lamp: Option<(String, f32)>,
}

/// A built theme's dressed kit (docs/dungeons.md#themes-d527): models from the asset root the
/// stamp lays over the cubes, as Skyrim's kits lay pieces over their grid and a roof's
/// covering lies over its cubes. Each piece's shape and origin is its model's (made by
/// `tools/props/barrow.py` for the barrow halls); the cubes under it still carry what is
/// walked into.
#[derive(Clone, Debug, Deserialize)]
pub struct Kit {
    /// A straight corridor cell's barrel vault: a 3 m span a cell long, its axis along its
    /// Z, its origin the middle of its springing line.
    pub barrel: String,
    /// A 4 m bay's groin vault rising 2 m, its origin the middle of its springing; scaled to
    /// a room's bay between its piers and walls, and by three quarters at a corridor's turn
    /// or crossing.
    pub groin: String,
    /// A doorway's head on a wall's face, its front out of the wall, its origin the wall's
    /// foot under it.
    pub arch: String,
    /// A pier where a hall's bays meet, a pilaster where half of it stands in a wall.
    pub pier: String,
    /// A burial niche's front, its origin the foot of its 2 m by 1 m opening.
    pub niche: String,
    /// What lies in a niche, one of them in each.
    pub remains: Vec<String>,
    /// The vaults' springing over the floor, metres: where the pieces were made to spring.
    pub spring: f32,
    /// A stone tomb over a `sarcophagus` module's 2 m by 1 m by 1 m of cubes, its origin the
    /// middle of its base, its length along its X; the cubes alone where none is given.
    #[serde(default)]
    pub sarcophagus: Option<String>,
}

fn one() -> f32 {
    1.0
}

fn full_height() -> (f32, f32) {
    (5.5, 5.5)
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
