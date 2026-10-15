//! What kinds of item there are, read from `assets/data/items.ron` (D127, D136): what each
//! is called, how big it is, what it does in hand and worn, and what a player starts with.
//! Nothing else names an item; code asks what a kind can do, such as whether it builds or
//! what it cuts.
//!
//! The file is read into a registry, `Items` (`inventory`, `data`).

use super::data;
use super::skills::Skill;
use serde::Deserialize;

/// The file: every kind, and what a player starts with.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct ItemsFile {
    pub start: Start,
    pub kinds: Vec<ItemDef>,
}

/// What a player starts with: worn, and on the bar in order.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub wears: Vec<String>,
    pub tools: Vec<String>,
}

/// One kind of item, as the file has it. The file says what each part means.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {
    pub name: String,
    #[serde(default = "one_cell")]
    pub size: (u8, u8),
    #[serde(default)]
    pub found: Option<String>,
    #[serde(default)]
    pub about: Option<String>,
    #[serde(default)]
    pub food: f32,
    #[serde(default)]
    pub colour: Option<(f32, f32, f32)>,
    #[serde(default)]
    pub lasts: Option<u16>,
    #[serde(default)]
    pub mended_at: Option<String>,
    #[serde(default)]
    pub tab: Option<Family>,
    #[serde(default)]
    pub tool: Option<ToolDef>,
    #[serde(default)]
    pub worn: Option<WornDef>,
}

fn one_cell() -> (u8, u8) {
    (1, 1)
}

/// What an item does in hand.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct ToolDef {
    #[serde(default)]
    pub builds: bool,
    #[serde(default)]
    pub cuts: Vec<(Cut, f32)>,
    #[serde(default)]
    pub strikes: Option<(f32, f32)>,
    #[serde(default)]
    pub shoots: Option<String>,
    #[serde(default)]
    pub lights: bool,
    #[serde(default)]
    pub hint: Option<String>,
}

impl ToolDef {
    /// Whether this is the kind of tool for a sort of thing, whatever its material: an
    /// axe for trees, great or not.
    pub fn works(&self, what: Cut) -> bool {
        self.cuts
            .iter()
            .any(|&(cut, _)| cut.skill() == what.skill())
    }

    /// How fast it cuts a sort of thing: 1 takes the time the thing itself says, 2 half of
    /// it. `None` if it cannot: the wrong tool, or too poor a material (D96).
    pub fn cuts(&self, what: Cut) -> Option<f32> {
        self.cuts
            .iter()
            .find(|&&(cut, _)| cut == what)
            .map(|&(_, speed)| speed)
    }

    /// How hard it hits and how far it reaches, if it is swung at all.
    pub fn strikes(&self) -> Option<(f32, f32)> {
        self.strikes
    }

    /// What the prompt says it is for, with the use key's name put in.
    pub fn hint(&self, press: &str, hold: &str) -> Option<String> {
        let hint = self.hint.as_ref()?;
        Some(hint.replace("{press}", press).replace("{hold}", hold))
    }
}

/// Where an item is worn and what it does there.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct WornDef {
    pub slot: String,
    #[serde(default)]
    pub backpack: Option<(u8, u8)>,
    #[serde(default)]
    pub protection: f32,
    /// How warm it keeps its wearer, in clo (`warmth`).
    #[serde(default)]
    pub warmth: f32,
    #[serde(default)]
    pub holds: Option<(String, u32)>,
}

pub use super::flora_kinds::Cut;

/// Which skill working a sort of thing grows: the world says what can be cut; skills are
/// gameplay's.
pub trait CutSkill {
    /// The skill working it grows (D88), which is also which family of tool works it:
    /// axes chop, pickaxes mine, knives forage.
    fn skill(self) -> Skill;
}

impl CutSkill for Cut {
    fn skill(self) -> Skill {
        match self {
            Cut::Tree | Cut::GreatTree => Skill::Chopping,
            Cut::Rock | Cut::Ore => Skill::Mining,
            Cut::Plant | Cut::Flesh => Skill::Foraging,
        }
    }
}

/// What kind of thing an item is, for a station's tabs.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
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

impl ItemDef {
    /// The tab it is made under: its own if it says, else tools for tools, clothing for
    /// what is worn, and materials for the rest.
    pub fn family(&self) -> Family {
        self.tab.unwrap_or(if self.tool.is_some() {
            Family::Tools
        } else if self.worn.is_some() {
            Family::Clothing
        } else {
            Family::Materials
        })
    }
}

/// Reads the kinds.
pub fn read() -> ItemsFile {
    data::read("items.ron")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_reads_and_names_each_kind_once() {
        let file = read();
        let mut names: Vec<_> = file.kinds.iter().map(|kind| kind.name.as_str()).collect();
        names.sort();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "a name is used twice");
    }

    #[test]
    fn everything_a_kind_names_is_a_kind() {
        let file = read();
        let known = |name: &str| file.kinds.iter().any(|kind| kind.name == name);
        let named =
            file.start
                .wears
                .iter()
                .chain(&file.start.tools)
                .chain(file.kinds.iter().flat_map(|kind| {
                    let tool = kind.tool.iter().flat_map(|tool| &tool.shoots);
                    let worn = kind
                        .worn
                        .iter()
                        .flat_map(|worn| &worn.holds)
                        .map(|(held, _)| held);
                    tool.chain(worn)
                }));
        for name in named {
            assert!(known(name), "{name} is named but is not a kind");
        }
    }

    #[test]
    fn a_tool_works_one_family_of_thing_and_its_material_decides_how_much() {
        let file = read();
        let tool = |name: &str| {
            file.kinds
                .iter()
                .find(|kind| kind.name == name)
                .and_then(|kind| kind.tool.clone())
                .unwrap()
        };
        let (stone_axe, iron_axe) = (tool("Stone axe"), tool("Iron axe"));
        assert!(stone_axe.works(Cut::GreatTree));
        assert_eq!(stone_axe.cuts(Cut::GreatTree), None, "too great for stone");
        assert!(iron_axe.cuts(Cut::GreatTree).is_some());
        assert!(!stone_axe.works(Cut::Rock));
        assert!(tool("Stone pickaxe").cuts(Cut::Ore) < tool("Iron pickaxe").cuts(Cut::Ore));
        assert!(!tool("Hammer").works(Cut::Tree));
        for what in [
            Cut::Tree,
            Cut::GreatTree,
            Cut::Rock,
            Cut::Ore,
            Cut::Plant,
            Cut::Flesh,
        ] {
            let workers = file
                .kinds
                .iter()
                .filter_map(|kind| kind.tool.as_ref())
                .filter(|tool| tool.works(what))
                .count();
            assert!(workers >= 1, "something works {what:?}");
        }
    }
}
