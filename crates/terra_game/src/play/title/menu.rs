//! The title screen's choices, without Bevy's screen: which page is up, what is on it,
//! which choice is picked, and what choosing one does. The screen draws this and feeds it
//! keys and clicks (`super`).

use super::super::worlds::{self, World};

/// The pages of the title screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Main,
    Worlds,
    New,
    /// Asking before the world at this place in the list is deleted.
    Delete(usize),
}

/// A choice on a page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Item {
    Continue,
    Worlds,
    NewWorld,
    Yard,
    Settings,
    Quit,
    /// The world at this place in the list.
    World(usize),
    Name,
    Mode,
    /// The new world's seed: rolled, or typed (D596).
    Seed,
    Create,
    ConfirmDelete,
    Back,
}

/// What a choice asks of the game.
#[derive(Clone, PartialEq, Debug)]
pub enum Act {
    Play(World),
    Make {
        name: String,
        creative: bool,
        seed: u64,
    },
    Yard,
    Settings,
    Quit,
    Delete(World),
}

/// The title screen's state.
#[derive(Clone, Debug)]
pub struct Menu {
    pub page: Page,
    /// The picked choice, by its place on the page.
    pub at: usize,
    /// The worlds, the last played first.
    pub worlds: Vec<World>,
    /// The new world's name and mode, while it is being made.
    pub name: String,
    pub creative: bool,
    /// The new world's seed as shown and typed: a whole number.
    pub seed: String,
    /// Whether the seed shown was typed, not rolled.
    pub seed_typed: bool,
    /// Where the next rolled seed comes from: the screen sets it from the clock as it opens
    /// (`super::open`); each roll steps it on (`terra_gen::hash::splitmix64`).
    pub dice: u64,
    /// What the screen tells the player until they next choose: why the world they were in
    /// was left, when it was not their choice.
    pub notice: Option<String>,
}

/// The most digits a seed takes: a 64-bit number's.
const SEED_DIGITS: usize = 20;

impl Menu {
    pub fn new(worlds: Vec<World>) -> Self {
        let mut menu = Self {
            page: Page::Main,
            at: 0,
            worlds,
            name: String::new(),
            creative: false,
            seed: String::new(),
            seed_typed: false,
            dice: 0,
            notice: None,
        };
        // Continue first when there is a world to continue, else New world.
        menu.at = usize::from(menu.worlds.is_empty()) * 2;
        menu
    }

    /// What is on the page, in order.
    pub fn items(&self) -> Vec<Item> {
        match self.page {
            Page::Main => vec![
                Item::Continue,
                Item::Worlds,
                Item::NewWorld,
                Item::Yard,
                Item::Settings,
                Item::Quit,
            ],
            Page::Worlds => (0..self.worlds.len())
                .map(Item::World)
                .chain([Item::Back])
                .collect(),
            Page::New => vec![Item::Name, Item::Mode, Item::Seed, Item::Create, Item::Back],
            Page::Delete(_) => vec![Item::Back, Item::ConfirmDelete],
        }
    }

    /// Whether a choice can be made: Continue and Worlds need a world, Create a name and a
    /// seed.
    pub fn enabled(&self, item: Item) -> bool {
        match item {
            Item::Continue | Item::Worlds => !self.worlds.is_empty(),
            Item::Create => !self.name.trim().is_empty() && self.seed().is_some(),
            _ => true,
        }
    }

    /// The new world's seed, if what is typed is one.
    pub fn seed(&self) -> Option<u64> {
        self.seed.parse().ok()
    }

    /// Rolls the new world's seed.
    pub fn roll(&mut self) {
        self.dice = terra_gen::hash::splitmix64(self.dice);
        self.seed = self.dice.to_string();
        self.seed_typed = false;
    }

    pub fn picked(&self) -> Option<Item> {
        self.items().get(self.at).copied()
    }

    /// Moves the pick up or down, over what cannot be chosen, wrapping round.
    pub fn step(&mut self, down: bool) {
        let items = self.items();
        for _ in 0..items.len() {
            self.at = if down {
                (self.at + 1) % items.len()
            } else {
                (self.at + items.len() - 1) % items.len()
            };
            if self.enabled(items[self.at]) {
                return;
            }
        }
    }

    fn go(&mut self, page: Page, at: usize) {
        self.page = page;
        self.at = at;
    }

    /// Back a page: Esc, or Back.
    pub fn back(&mut self) {
        match self.page {
            Page::Main => {}
            Page::Worlds => self.go(Page::Main, 1),
            Page::New => self.go(Page::Main, 2),
            Page::Delete(world) => self.go(Page::Worlds, world),
        }
    }

    /// Chooses what is picked; what the game must do, if anything.
    pub fn choose(&mut self) -> Option<Act> {
        let item = self.picked()?;
        if !self.enabled(item) {
            return None;
        }
        self.notice = None;
        match item {
            Item::Continue => return self.worlds.first().cloned().map(Act::Play),
            Item::Worlds => self.go(Page::Worlds, 0),
            Item::NewWorld => {
                self.name = worlds::next_name(&self.worlds, false);
                self.creative = false;
                self.roll();
                self.go(Page::New, 0);
            }
            Item::Yard => return Some(Act::Yard),
            Item::Settings => return Some(Act::Settings),
            Item::Quit => return Some(Act::Quit),
            Item::World(at) => return self.worlds.get(at).cloned().map(Act::Play),
            Item::Name => self.at = 1,
            Item::Mode => self.turn(),
            Item::Seed => self.roll(),
            Item::Create => {
                return Some(Act::Make {
                    name: self.name.trim().to_string(),
                    creative: self.creative,
                    seed: self.seed()?,
                });
            }
            Item::ConfirmDelete => {
                let Page::Delete(at) = self.page else {
                    return None;
                };
                return Some(Act::Delete(self.worlds.get(at)?.clone()));
            }
            Item::Back => self.back(),
        }
        None
    }

    /// Survival to Creative and back. A name still the one offered follows the mode.
    pub fn turn(&mut self) {
        let offered = self.name == worlds::next_name(&self.worlds, self.creative);
        self.creative = !self.creative;
        if offered {
            self.name = worlds::next_name(&self.worlds, self.creative);
        }
    }

    /// Asks before deleting the picked world (Delete on the list, or its ✕).
    pub fn ask_delete(&mut self, at: usize) {
        if at < self.worlds.len() {
            self.go(Page::Delete(at), 0);
        }
    }

    /// The list again after a world went.
    pub fn deleted(&mut self, worlds: Vec<World>) {
        self.worlds = worlds;
        if self.worlds.is_empty() {
            self.go(Page::Main, 2);
        } else {
            self.go(Page::Worlds, 0);
        }
    }

    /// Typing into the new world's name, or its seed's digits: a seed of one's own, which
    /// takes only what keeps it a 64-bit number.
    pub fn type_text(&mut self, text: &str) {
        if self.page != Page::New {
            return;
        }
        match self.picked() {
            Some(Item::Name) => {
                for c in text.chars().filter(|c| !c.is_control()) {
                    if self.name.chars().count() < 32 {
                        self.name.push(c);
                    }
                }
            }
            Some(Item::Seed) => {
                for c in text.chars().filter(char::is_ascii_digit) {
                    // Typing over a rolled seed starts one's own.
                    if !self.seed_typed {
                        self.seed.clear();
                        self.seed_typed = true;
                    }
                    let longer = format!("{}{c}", self.seed);
                    if longer.len() <= SEED_DIGITS && longer.parse::<u64>().is_ok() {
                        self.seed = longer;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn erase(&mut self) {
        if self.page != Page::New {
            return;
        }
        match self.picked() {
            Some(Item::Name) => drop(self.name.pop()),
            // A rolled seed goes whole; one's own a digit at a time.
            Some(Item::Seed) if !self.seed_typed => {
                self.seed.clear();
                self.seed_typed = true;
            }
            Some(Item::Seed) => drop(self.seed.pop()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
