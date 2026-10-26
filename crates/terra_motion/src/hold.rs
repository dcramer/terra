//! What each hand holds, and the stance and move set that follow from the pair (docs/holding.md
//! sections 1 and 2, D459): an item's `hold` entry in `assets/data/items.ron`, the stance by
//! rule from the two hands (FromSoftware's: a set per weapon class, the stance from the pair),
//! and the set looked up by class and stance in `assets/data/moves/<class>.<stance>.ron`,
//! falling back from the class to its family to the stance's generic set, so a new class plays
//! before its own clips exist. A new weapon is an entry, never code (D136).

use crate::moves::MoveSet;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// How an item is held, as its `hold` entry in `assets/data/items.ron` says.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Hold {
    /// Which move sets it uses; every item of a class shares them.
    pub class: String,
    pub grip: Grip,
    /// For a `Two` item, the hand its grip socket rides in; the other reaches its second grip.
    #[serde(default)]
    pub lead: Option<Hand>,
    /// Held but with no moves: it only lays its arm's pose (a lit torch, a lantern).
    #[serde(default)]
    pub carried: bool,
    /// Metres, kilograms, and the balance point's distance from the grip in metres: timing and
    /// reach come from these by rule (holding.md section 2).
    pub length: f32,
    pub mass: f32,
    pub balance: f32,
}

/// Which hands may hold an item.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    /// A one-hander for the main hand (a sword).
    Main,
    /// A one-hander for either hand (a dagger, a torch).
    Either,
    /// The off hand only (a shield).
    Off,
    /// Both hands (a greatsword, a spear, a bow).
    Two,
}

/// A body's hands: the main one (its handedness picks right or left) and the other.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    Main,
    Off,
}

/// What is in one hand, as the stance rule sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Holding<'a> {
    Nothing,
    /// An item with a `hold` entry.
    Item(&'a Hold),
    /// A tool, which keeps its own use moves (the items without a `hold` entry).
    Tool,
}

/// How a body stands with what it holds (holding.md, "The stance, by rule from the pair").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stance {
    Unarmed,
    Single,
    Shielded,
    Dual,
    Two,
    Shield,
    Tool,
}

impl Stance {
    /// The stance's part of a set's name, `<class>.<this>`.
    pub fn name(self) -> &'static str {
        match self {
            Stance::Unarmed => "unarmed",
            Stance::Single => "single",
            Stance::Shielded => "shielded",
            Stance::Dual => "dual",
            Stance::Two => "two",
            Stance::Shield => "alone",
            Stance::Tool => "tool",
        }
    }
}

/// A pair the rule refuses, because an item cannot go where it is put (holding.md: refused by
/// the rule, not by the screen).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// An off-hand item (a shield) put in the main hand.
    OffOnly(String),
    /// A two-hander or a tool put in the off hand.
    NotInOff(String),
    /// A blade in the off hand with nothing in the main: the table has no stance for it.
    OffAlone(String),
}

/// The stance a pair of hands gives, by holding.md's table. A `carried` item lays its arm's
/// pose and counts as nothing; a two-hander puts the off hand's item away (stowed, back when
/// the two-hander leaves); a tool in the main hand is the tool's, whatever the off hand holds.
pub fn stance(main: Holding, off: Holding) -> Result<Stance, Refused> {
    if let Holding::Item(item) = main
        && item.grip == Grip::Off
    {
        return Err(Refused::OffOnly(item.class.clone()));
    }
    if main == Holding::Tool {
        return Ok(Stance::Tool);
    }
    match off {
        Holding::Tool => return Err(Refused::NotInOff("tool".into())),
        Holding::Item(item) if item.grip == Grip::Two => {
            return Err(Refused::NotInOff(item.class.clone()));
        }
        _ => {}
    }
    let armed = |hand: Holding<'_>| match hand {
        Holding::Item(item) if !item.carried => Some(item.grip),
        _ => None,
    };
    Ok(match (armed(main), armed(off)) {
        (None, None) => Stance::Unarmed,
        (Some(Grip::Two), _) => Stance::Two,
        (Some(_), None) => Stance::Single,
        (Some(_), Some(Grip::Off)) => Stance::Shielded,
        // Any two one-handers (D462, "Any two").
        (Some(_), Some(_)) => Stance::Dual,
        (None, Some(Grip::Off)) => Stance::Shield,
        (None, Some(_)) => {
            let Holding::Item(item) = off else {
                unreachable!("armed only by an item")
            };
            return Err(Refused::OffAlone(item.class.clone()));
        }
    })
}

/// The sets a pair of hands may play, most particular first: the class's own, its family's,
/// then the stance's generic one (`sabre.single`, `blade.single`, `one_hand.single`). Dual
/// plays `C.dual` when its two classes share a family, and otherwise the main hand's single
/// set (Elden Ring's power stance); a tool has none of these, its use moves being its own.
pub fn candidates(
    main: Holding,
    off: Holding,
    families: &HashMap<String, String>,
) -> Result<(Stance, Vec<String>), Refused> {
    let stance = stance(main, off)?;
    fn class(hand: Holding<'_>) -> Option<&str> {
        match hand {
            Holding::Item(item) => Some(item.class.as_str()),
            _ => None,
        }
    }
    let family = |class: &str| families.get(class).map(String::as_str);
    let ladder = |class: Option<&str>, stance: Stance, generic: &str| {
        let mut names: Vec<String> = Vec::new();
        let mut put = |name: String| {
            if !names.contains(&name) {
                names.push(name);
            }
        };
        if let Some(class) = class {
            put(format!("{class}.{}", stance.name()));
            if let Some(family) = family(class) {
                put(format!("{family}.{}", stance.name()));
            }
        }
        put(format!("{generic}.{}", stance.name()));
        names
    };
    let names = match stance {
        Stance::Unarmed => vec!["unarmed".to_string()],
        Stance::Single | Stance::Shielded => ladder(class(main), stance, "one_hand"),
        Stance::Dual => {
            let (one, other) = (class(main), class(off));
            let kin = one == other
                || (one.and_then(family).is_some()
                    && one.and_then(family) == other.and_then(family));
            let single = ladder(one, Stance::Single, "one_hand");
            if kin {
                let mut names = ladder(one, Stance::Dual, "one_hand");
                names.extend(single);
                names
            } else {
                single
            }
        }
        Stance::Two => ladder(class(main), stance, "two_hand"),
        Stance::Shield => ladder(class(off), stance, "shield"),
        Stance::Tool => Vec::new(),
    };
    Ok((stance, names))
}

/// Every move set there is, by name (`sword.shielded`), and the family each class falls back
/// to, as `assets/data/moves` holds them.
#[derive(Clone, Debug, Default)]
pub struct Sets {
    pub families: HashMap<String, String>,
    pub sets: HashMap<String, MoveSet>,
}

/// The file in the sets' folder that names each class's family rather than a set.
pub const FAMILIES: &str = "families.ron";

/// The stance a pair of hands gives and the set it plays, if one is made at any step of the
/// fallback.
#[derive(Clone, Copy, Debug)]
pub struct Picked<'a> {
    pub stance: Stance,
    pub set: Option<(&'a str, &'a MoveSet)>,
}

impl Sets {
    /// Reads every set in a folder, each named by its file (`sword.shielded.ron` is
    /// `sword.shielded`), and its `families.ron`.
    pub fn read_folder(folder: &Path) -> Result<Self, String> {
        let read = |path: &Path| {
            std::fs::read_to_string(path).map_err(|why| format!("{}: {why}", path.display()))
        };
        let families = ron::from_str(&read(&folder.join(FAMILIES))?)
            .map_err(|why| format!("{FAMILIES}: {why}"))?;
        let mut sets = HashMap::new();
        let entries =
            std::fs::read_dir(folder).map_err(|why| format!("{}: {why}", folder.display()))?;
        for entry in entries {
            let path = entry.map_err(|why| why.to_string())?.path();
            let file = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let Some(name) = file.strip_suffix(".ron") else {
                continue;
            };
            if file == FAMILIES {
                continue;
            }
            let set = MoveSet::read(&read(&path)?).map_err(|why| format!("{file}: {why}"))?;
            sets.insert(name.to_string(), set);
        }
        Ok(Self { families, sets })
    }

    /// The stance a pair gives and the first set of its fallback that is made.
    pub fn pick(&self, main: Holding, off: Holding) -> Result<Picked<'_>, Refused> {
        let (stance, names) = candidates(main, off, &self.families)?;
        let set = names
            .iter()
            .find_map(|name| self.sets.get_key_value(name.as_str()))
            .map(|(name, set)| (name.as_str(), set));
        Ok(Picked { stance, set })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(class: &str, grip: Grip) -> Hold {
        Hold {
            class: class.into(),
            grip,
            lead: None,
            carried: false,
            length: 1.0,
            mass: 1.0,
            balance: 0.1,
        }
    }

    /// Holding.md's table, row by row, written out by hand from the doc.
    #[test]
    fn the_stance_is_the_tables_for_every_pair_of_grips() {
        use Holding::{Item, Nothing, Tool};
        let sword = item("sword", Grip::Main);
        let dagger = item("dagger", Grip::Either);
        let shield = item("shield", Grip::Off);
        let greatsword = item("greatsword", Grip::Two);
        let torch = Hold {
            carried: true,
            ..item("torch", Grip::Either)
        };
        let rows: [(Holding, Holding, Result<Stance, Refused>); 16] = [
            (Nothing, Nothing, Ok(Stance::Unarmed)),
            (Item(&sword), Nothing, Ok(Stance::Single)),
            (Item(&sword), Item(&torch), Ok(Stance::Single)),
            (Item(&sword), Item(&shield), Ok(Stance::Shielded)),
            (Item(&sword), Item(&dagger), Ok(Stance::Dual)),
            (Item(&dagger), Item(&sword), Ok(Stance::Dual)),
            (Item(&greatsword), Nothing, Ok(Stance::Two)),
            (Item(&greatsword), Item(&shield), Ok(Stance::Two)),
            (Nothing, Item(&shield), Ok(Stance::Shield)),
            (Item(&torch), Item(&shield), Ok(Stance::Shield)),
            (Nothing, Item(&torch), Ok(Stance::Unarmed)),
            (Tool, Item(&shield), Ok(Stance::Tool)),
            (
                Item(&shield),
                Nothing,
                Err(Refused::OffOnly("shield".into())),
            ),
            (
                Item(&sword),
                Item(&greatsword),
                Err(Refused::NotInOff("greatsword".into())),
            ),
            (Item(&sword), Tool, Err(Refused::NotInOff("tool".into()))),
            (
                Nothing,
                Item(&dagger),
                Err(Refused::OffAlone("dagger".into())),
            ),
        ];
        for (main, off, want) in rows {
            assert_eq!(stance(main, off), want, "{main:?} with {off:?}");
        }
    }

    fn sets(made: &[&str]) -> Sets {
        let shipped = Sets::read_folder(&folder()).unwrap();
        let any = shipped.sets.values().next().unwrap().clone();
        Sets {
            families: [("sabre", "blade"), ("dagger", "blade"), ("sword", "blade")]
                .map(|(class, family)| (class.to_string(), family.to_string()))
                .into(),
            sets: made
                .iter()
                .map(|name| (name.to_string(), any.clone()))
                .collect(),
        }
    }

    fn folder() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/data/moves")
    }

    #[test]
    fn a_set_falls_back_from_its_class_to_its_family_to_the_stances_generic_one() {
        let sabre = item("sabre", Grip::Main);
        let club = item("club", Grip::Main);
        let pick = |sets: &Sets, main: &Hold| {
            let picked = sets.pick(Holding::Item(main), Holding::Nothing).unwrap();
            assert_eq!(picked.stance, Stance::Single);
            picked.set.map(|(name, _)| name.to_string())
        };
        let all = sets(&["sabre.single", "blade.single", "one_hand.single"]);
        assert_eq!(pick(&all, &sabre).as_deref(), Some("sabre.single"));
        let family = sets(&["blade.single", "one_hand.single"]);
        assert_eq!(pick(&family, &sabre).as_deref(), Some("blade.single"));
        assert_eq!(
            pick(&family, &club).as_deref(),
            Some("one_hand.single"),
            "no family"
        );
        let generic = sets(&["one_hand.single", "sabre.shielded"]);
        assert_eq!(pick(&generic, &sabre).as_deref(), Some("one_hand.single"));
        assert_eq!(pick(&sets(&["sabre.shielded"]), &sabre), None, "none made");
    }

    #[test]
    fn two_blades_of_a_family_play_its_dual_set_and_two_strangers_the_main_hands_single() {
        let sets = sets(&["blade.dual", "sabre.single", "club.single"]);
        let (sabre, dagger, club) = (
            item("sabre", Grip::Main),
            item("dagger", Grip::Either),
            item("club", Grip::Main),
        );
        let pick = |main: &Hold, off: &Hold| {
            let picked = sets.pick(Holding::Item(main), Holding::Item(off)).unwrap();
            assert_eq!(picked.stance, Stance::Dual);
            picked.set.map(|(name, _)| name.to_string()).unwrap()
        };
        assert_eq!(pick(&sabre, &dagger), "blade.dual");
        assert_eq!(pick(&sabre, &club), "sabre.single");
        assert_eq!(pick(&club, &sabre), "club.single");
    }

    #[test]
    fn a_sword_and_shield_play_the_shipped_shielded_set_and_a_tool_none() {
        let sets = Sets::read_folder(&folder()).unwrap();
        let (sword, shield) = (item("sword", Grip::Main), item("shield", Grip::Off));
        let picked = sets
            .pick(Holding::Item(&sword), Holding::Item(&shield))
            .unwrap();
        assert_eq!(picked.stance, Stance::Shielded);
        assert_eq!(picked.set.map(|(name, _)| name), Some("sword.shielded"));
        let tool = sets.pick(Holding::Tool, Holding::Item(&shield)).unwrap();
        assert_eq!((tool.stance, tool.set.is_none()), (Stance::Tool, true));
    }
}
