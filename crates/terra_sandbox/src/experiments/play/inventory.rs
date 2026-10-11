//! What the player carries: raw items, and tools.
//!
//! Raw items are what things are built from. The builder draws on them: with enough of
//! the right ones you can build anything you are able to build, and taking a thing apart
//! gives them back. Tools are used from the action bar, a row of slots along the bottom
//! of the screen; whichever slot is picked is the tool in hand.
//!
//! All of it is a stand-in for a design nobody has made (docs/construction.md, D55): the
//! list of raw items, how many you start with, what things cost, and that everything is
//! refunded in full. The rules are kept free of the engine so they can move to a shared
//! crate once they are worth keeping.

use bevy::prelude::Resource;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Raw {
    Metal,
    Glass,
    Wood,
    Cloth,
    Parts,
}

impl Raw {
    pub const ALL: [Raw; 5] = [Raw::Metal, Raw::Glass, Raw::Wood, Raw::Cloth, Raw::Parts];

    pub fn name(self) -> &'static str {
        match self {
            Raw::Metal => "Metal",
            Raw::Glass => "Glass",
            Raw::Wood => "Wood",
            Raw::Cloth => "Cloth",
            Raw::Parts => "Parts",
        }
    }
}

/// What one of something costs: so many of each raw item.
pub type Cost = &'static [(Raw, u32)];

/// A cost as a player would read it: "2 Wood, 3 Cloth".
pub fn describe(cost: Cost, times: u32) -> String {
    cost.iter()
        .map(|&(raw, each)| format!("{} {}", each * times, raw.name()))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    /// Builds and takes apart.
    Builder,
    /// Cuts down trees and breaks up rocks, for the raw items in them.
    Cutter,
}

impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Tool::Builder => "Builder",
            Tool::Cutter => "Cutter",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Tool::Builder => "Builds rooms, walls and items from what you carry",
            Tool::Cutter => "Cuts trees, rocks and bushes into raw items",
        }
    }
}

pub const BAR_SLOTS: usize = 6;

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Inventory {
    raw: [u32; Raw::ALL.len()],
    /// Every tool carried, on the bar or not.
    pub tools: Vec<Tool>,
    pub bar: [Option<Tool>; BAR_SLOTS],
    /// Which slot of the bar is picked.
    pub held: usize,
    /// Whether the inventory is open on screen.
    pub open: bool,
}

impl Default for Inventory {
    /// What a player starts the demo with: both tools on the bar, and enough to build a
    /// room or two.
    fn default() -> Self {
        let mut inventory = Inventory {
            raw: [0; Raw::ALL.len()],
            tools: vec![Tool::Builder, Tool::Cutter],
            bar: [None; BAR_SLOTS],
            held: 0,
            open: false,
        };
        inventory.bar[0] = Some(Tool::Builder);
        inventory.bar[1] = Some(Tool::Cutter);
        for (raw, count) in [
            (Raw::Metal, 40),
            (Raw::Glass, 12),
            (Raw::Wood, 30),
            (Raw::Cloth, 12),
            (Raw::Parts, 8),
        ] {
            inventory.add(raw, count);
        }
        inventory
    }
}

impl Inventory {
    pub fn count(&self, raw: Raw) -> u32 {
        self.raw[raw as usize]
    }

    pub fn add(&mut self, raw: Raw, count: u32) {
        self.raw[raw as usize] += count;
    }

    /// The first raw item there is not enough of to pay a cost so many times over, and
    /// how many more are needed.
    pub fn missing(&self, cost: Cost, times: u32) -> Option<(Raw, u32)> {
        cost.iter().find_map(|&(raw, each)| {
            let wanted = each * times;
            (self.count(raw) < wanted).then(|| (raw, wanted - self.count(raw)))
        })
    }

    /// Pays a cost so many times over, if it can be paid in full. Returns whether it was.
    pub fn pay(&mut self, cost: Cost, times: u32) -> bool {
        if self.missing(cost, times).is_some() {
            return false;
        }
        for &(raw, each) in cost {
            self.raw[raw as usize] -= each * times;
        }
        true
    }

    pub fn refund(&mut self, cost: Cost, times: u32) {
        for &(raw, each) in cost {
            self.add(raw, each * times);
        }
    }

    /// The tool in hand: whatever is in the picked slot of the bar.
    pub fn holding(&self) -> Option<Tool> {
        self.bar[self.held]
    }

    /// Puts a carried tool in a slot of the bar. A tool is on the bar once, so it leaves
    /// any slot it was in.
    pub fn put_on_bar(&mut self, tool: Tool, slot: usize) {
        if slot >= BAR_SLOTS || !self.tools.contains(&tool) {
            return;
        }
        for other in &mut self.bar {
            if *other == Some(tool) {
                *other = None;
            }
        }
        self.bar[slot] = Some(tool);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COUCH: Cost = &[(Raw::Wood, 2), (Raw::Cloth, 3)];

    #[test]
    fn a_cost_is_paid_in_full_or_not_at_all() {
        let mut inventory = Inventory::default();
        let (wood, cloth) = (inventory.count(Raw::Wood), inventory.count(Raw::Cloth));
        assert!(inventory.pay(COUCH, 2));
        assert_eq!(inventory.count(Raw::Wood), wood - 4);
        assert_eq!(inventory.count(Raw::Cloth), cloth - 6);
        // Four more couches' worth of cloth is more than is left; nothing is taken.
        assert_eq!(inventory.missing(COUCH, 4), Some((Raw::Cloth, 6)));
        let before = inventory.clone();
        assert!(!inventory.pay(COUCH, 4));
        assert_eq!(inventory, before);
        // Taking them apart gives it all back.
        inventory.refund(COUCH, 2);
        assert_eq!(inventory.count(Raw::Cloth), cloth);
        assert_eq!(describe(COUCH, 2), "4 Wood, 6 Cloth");
    }

    #[test]
    fn the_bar_holds_each_tool_once_and_the_picked_slot_is_in_hand() {
        let mut inventory = Inventory::default();
        assert_eq!(inventory.holding(), Some(Tool::Builder));
        inventory.put_on_bar(Tool::Builder, 4);
        assert_eq!(inventory.bar[0], None);
        assert_eq!(inventory.holding(), None);
        inventory.held = 4;
        assert_eq!(inventory.holding(), Some(Tool::Builder));
        // Out of range, or a tool not carried: nothing happens.
        inventory.tools.retain(|tool| *tool != Tool::Cutter);
        let before = inventory.clone();
        inventory.put_on_bar(Tool::Cutter, 2);
        inventory.put_on_bar(Tool::Builder, BAR_SLOTS);
        assert_eq!(inventory, before);
    }
}
