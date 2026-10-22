//! The registry of kinds of item (D136), read from `assets/data/items.ron` (`kinds`), and
//! the source of new items' identities.

use super::{Cost, Made, slot_named};
use crate::kinds::{self, ItemDef, ToolDef, WornDef};
use bevy::prelude::*;
use terra_items::{Catalogue, IdSource, Item, ItemKind, KindDef};

/// Every kind of item, read from `assets/data/items.ron` (`kinds`), and the source of new
/// items' identities.
#[derive(Resource)]
pub struct Items {
    pub catalogue: Catalogue,
    defs: Vec<ItemDef>,
    pub start: kinds::Start,
    ids: IdSource,
}

impl Default for Items {
    fn default() -> Self {
        let file = kinds::read();
        // A kind is its place in the file, so one kind can name another wherever it is.
        let kind_of = |name: &str| {
            file.kinds
                .iter()
                .position(|def| def.name == name)
                .map(|index| ItemKind(index as u16))
                .unwrap_or_else(|| panic!("assets/data/items.ron: there is no kind called {name}"))
        };
        let mut catalogue = Catalogue::default();
        for def in &file.kinds {
            let mut kind = KindDef::new(&def.name).size(def.size.0, def.size.1);
            if let Some(lasts) = def.lasts {
                kind = kind.lasts(lasts);
            }
            if let Some(worn) = &def.worn {
                kind = kind.gear(slot_named(&worn.slot));
                if let Some((w, h)) = worn.backpack {
                    kind = kind.backpack(w, h);
                }
                if let Some((held, most)) = &worn.holds {
                    kind = kind.holds(kind_of(held), *most);
                }
            }
            let added = catalogue.add(kind);
            debug_assert_eq!(added, kind_of(&def.name));
        }
        Self {
            catalogue,
            defs: file.kinds,
            start: file.start,
            ids: IdSource::default(),
        }
    }
}

impl Items {
    /// Takes the kinds of another, read afresh, keeping the identities already given out, so
    /// no two items ever share one.
    pub fn take_kinds(&mut self, fresh: Items) {
        self.catalogue = fresh.catalogue;
        self.defs = fresh.defs;
        self.start = fresh.start;
    }

    /// The kind called a name, if there is one.
    pub fn find(&self, name: &str) -> Option<ItemKind> {
        self.catalogue.find(name)
    }

    /// The kind called a name the game itself uses, such as one a cost or a recipe names.
    /// The tests check every such name is a kind.
    pub fn kind(&self, name: &str) -> ItemKind {
        self.find(name)
            .unwrap_or_else(|| panic!("there is no kind of item called {name}"))
    }

    /// What the items file says of a kind.
    pub fn def(&self, kind: ItemKind) -> &ItemDef {
        &self.defs[kind.0 as usize]
    }

    /// Every kind, in the file's order.
    pub fn kinds(&self) -> impl Iterator<Item = ItemKind> + '_ {
        (0..self.defs.len()).map(|index| ItemKind(index as u16))
    }

    pub fn name(&self, kind: ItemKind) -> &str {
        self.defs
            .get(kind.0 as usize)
            .map_or("?", |def| def.name.as_str())
    }

    /// Where a kind comes from, for a prompt that says what is missing.
    pub fn found(&self, kind: ItemKind) -> &str {
        self.def(kind)
            .found
            .as_deref()
            .unwrap_or("make it at a station")
    }

    /// What a kind does in hand, if it is a tool.
    pub fn tool(&self, kind: ItemKind) -> Option<&ToolDef> {
        self.defs.get(kind.0 as usize)?.tool.as_ref()
    }

    /// Where a kind is worn and what it does there, if it is worn.
    pub fn worn(&self, kind: ItemKind) -> Option<&WornDef> {
        self.defs.get(kind.0 as usize)?.worn.as_ref()
    }

    /// How much eating one of a kind fills you, out of 100: nothing if it is not food.
    pub fn food(&self, kind: ItemKind) -> f32 {
        self.defs.get(kind.0 as usize).map_or(0.0, |def| def.food)
    }

    /// What can be eaten, least filling first.
    pub fn foods(&self) -> Vec<ItemKind> {
        let mut foods: Vec<ItemKind> = self.kinds().filter(|&kind| self.food(kind) > 0.0).collect();
        foods.sort_by(|a, b| self.food(*a).total_cmp(&self.food(*b)));
        foods
    }

    /// The kinds of item something made is, and how many.
    pub fn made(&self, made: Made) -> (ItemKind, u32) {
        (made.kind, made.count)
    }

    /// The kind a data file names, or the reason it cannot be read.
    pub fn named(&self, name: &str, file: &str) -> ItemKind {
        self.find(name)
            .unwrap_or_else(|| panic!("{file}: there is no kind of item called {name}"))
    }

    /// A cost a data file gives by item names, as kinds.
    pub fn cost(&self, named: &[(String, u32)], file: &str) -> Vec<(ItemKind, u32)> {
        named
            .iter()
            .map(|(name, count)| (self.named(name, file), *count))
            .collect()
    }

    /// What a kind's icon is baked from (D129): a tool's held look, gear's look (on the
    /// base body), and anything else's own look; and the cells it fills, which set the
    /// icon's shape. Whether the look exists yet is for the caller to find out.
    pub fn icon_source(&self, kind: ItemKind) -> Option<(String, crate::icons::Icon)> {
        let def = self.catalogue.get(kind).ok()?;
        let id = terra_walker::character::item_id(&def.name);
        let look = if self.tool(kind).is_some() {
            terra_walker::character::held_look(&id)
        } else if def.gear.is_some() {
            terra_walker::character::look(&id)
        } else {
            terra_walker::character::item_look(&id)
        };
        Some((id, crate::icons::Icon::new(look, (def.size.w, def.size.h))))
    }

    /// Makes an item of a kind found by its name, as when a saved world is loaded.
    pub fn make_named(&mut self, name: &str) -> Option<Item> {
        let kind = self.catalogue.find(name)?;
        Some(self.make(kind))
    }

    pub fn make(&mut self, kind: ItemKind) -> Item {
        self.ids.make(kind)
    }

    pub fn wanted(&self, cost: &Cost, times: u32) -> Vec<(ItemKind, u32)> {
        cost.iter()
            .map(|&(kind, each)| (kind, each * times))
            .collect()
    }
}
