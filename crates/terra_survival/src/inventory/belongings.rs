//! What players carry, for a system to read or change (`Belongings`), and what changing it
//! tells: what came in, what broke, and what did not fit.

use super::rules::{receive_kind, spend};
use super::*;
use crate::kinds::ToolDef;
use bevy::{ecs::system::SystemParam, prelude::*};
use terra_items::{ActionBar, Equipment, Grid, ItemId, ItemKind, Refused};
use terra_walker::{IsLocalPlayer, Player};

/// What players carry, for a system to read or change: `of` a player, or `mine` for this
/// machine's own, as a screen shows it.
#[derive(SystemParam)]
pub struct Belongings<'w, 's> {
    items: ResMut<'w, Items>,
    carried: Query<
        'w,
        's,
        (
            &'static mut Backpack,
            &'static mut Bar,
            &'static mut Gear,
            Has<HandsFull>,
        ),
        With<Player>,
    >,
    local: Query<'w, 's, Entity, IsLocalPlayer>,
    gained: MessageWriter<'w, Gained>,
    broke: MessageWriter<'w, Broke>,
    spilled: MessageWriter<'w, Spilled>,
}

/// A player's hands are on something other than the bar's tool, such as a drawn sword
/// and shield (D405): the tool is put away, so nothing is in hand for the tools to use.
#[derive(Component)]
pub struct HandsFull;

/// What was given to the player but did not fit: so many of a kind, which drop to the
/// ground at their feet rather than being lost (`pile`).
#[derive(Message, Clone, Copy, Debug)]
pub struct Spilled {
    pub player: Entity,
    pub kind: ItemKind,
    pub count: u32,
}

impl Belongings<'_, '_> {
    /// Makes what acting through `Belongings` needs that is no one player's: the kinds of
    /// item and the messages it writes. Survival's plugins make them; a run that acts through
    /// it without them (building play alone, the sandbox's `build`, D198) calls this, which
    /// leaves be what is made already.
    pub fn init(app: &mut App) {
        app.init_resource::<Items>()
            .add_message::<Gained>()
            .add_message::<Broke>()
            .add_message::<Spilled>();
    }
}

impl<'w> Belongings<'w, '_> {
    pub fn items(&self) -> &Items {
        &self.items
    }

    /// This machine's player, if there is one yet.
    pub fn local_player(&self) -> Option<Entity> {
        self.local.single().ok()
    }

    /// What a player carries, if they are a player.
    pub fn of(&mut self, player: Entity) -> Option<Inventory<'_, 'w>> {
        let (backpack, bar, gear, hands_full) = self.carried.get_mut(player).ok()?;
        Some(Inventory {
            player,
            hands_full,
            // Only to give new items their identities, which is not a change to the kinds
            // of item, so it does not mark them changed.
            items: self.items.bypass_change_detection(),
            backpack,
            bar,
            gear,
            gained: &mut self.gained,
            broke: &mut self.broke,
            spilled: &mut self.spilled,
        })
    }

    /// What this machine's player carries.
    pub fn mine(&mut self) -> Option<Inventory<'_, 'w>> {
        let player = self.local_player()?;
        self.of(player)
    }
}

/// What one player carries, to read or change. Every change either applies in full or is
/// refused (`terra_items`), and what comes in or breaks is told as a message naming them.
pub struct Inventory<'a, 'w> {
    player: Entity,
    hands_full: bool,
    items: &'a mut Items,
    backpack: Mut<'a, Backpack>,
    bar: Mut<'a, Bar>,
    gear: Mut<'a, Gear>,
    gained: &'a mut MessageWriter<'w, Gained>,
    broke: &'a mut MessageWriter<'w, Broke>,
    spilled: &'a mut MessageWriter<'w, Spilled>,
}

impl Inventory<'_, '_> {
    /// Whose it is.
    pub fn player(&self) -> Entity {
        self.player
    }

    pub fn items(&self) -> &Items {
        self.items
    }

    pub fn backpack(&self) -> &Grid {
        &self.backpack.0
    }

    pub fn bar(&self) -> &ActionBar {
        &self.bar.0
    }

    pub fn gear(&self) -> &Equipment {
        &self.gear.0
    }

    /// How much of a blow what is worn takes, out of one.
    pub fn protection(&self) -> f32 {
        protection(self.items, self.gear())
    }

    /// Whether their hands are on something else than the bar's tool (`HandsFull`).
    pub fn hands_full(&self) -> bool {
        self.hands_full
    }

    /// The kind of tool in hand, if it is not broken nor put away.
    pub fn holding(&self) -> Option<ItemKind> {
        if self.hands_full {
            return None;
        }
        holding(self.items, self.backpack(), self.bar())
    }

    /// What the tool in hand does, if there is one and it is not broken.
    pub fn tool_in_hand(&self) -> Option<&ToolDef> {
        self.items.tool(self.holding()?)
    }

    /// The tool in hand if it is broken, so it can be said why it does nothing.
    pub fn broken_in_hand(&self) -> Option<ItemKind> {
        if self.hands_full {
            return None;
        }
        let item = self.bar().in_hand(self.backpack())?;
        let def = self.items.catalogue.get(item.kind).ok()?;
        (item.broken(def) && self.items.tool(item.kind).is_some()).then_some(item.kind)
    }

    /// Wears the tool in hand by so much use (D99), saying so if that broke it.
    pub fn wear_in_hand(&mut self, amount: u16) {
        let Some(item) = self.bar().in_hand(self.backpack()) else {
            return;
        };
        let broke = self.backpack.0.wear(&self.items.catalogue, item.id, amount);
        if broke == Ok(true) {
            self.broke.write(Broke {
                player: self.player,
                kind: item.kind,
            });
        }
    }

    /// Wears everything worn that wears, by so much: a blow taken (D99).
    pub fn wear_worn(&mut self, amount: u16) {
        for kind in wear_worn(self.items, &mut self.gear.0, amount) {
            self.broke.write(Broke {
                player: self.player,
                kind,
            });
        }
    }

    /// Mends everything carried or worn that `mends` says can be mended here, returning
    /// the kinds mended (D99).
    pub fn mend(&mut self, mends: impl Fn(ItemKind) -> bool) -> Vec<ItemKind> {
        let mut mended = Vec::new();
        let worn_out: Vec<(ItemId, ItemKind)> = self
            .backpack()
            .iter()
            .filter(|placed| placed.item.wear > 0 && mends(placed.item.kind))
            .map(|placed| (placed.item.id, placed.item.kind))
            .collect();
        for (id, kind) in worn_out {
            if self.backpack.0.set_wear(id, 0).is_ok() {
                mended.push(kind);
            }
        }
        let slots: Vec<(GearSlot, ItemKind)> = self
            .gear()
            .iter()
            .filter_map(|(slot, item)| {
                item.filter(|item| item.wear > 0 && mends(item.kind))
                    .map(|item| (slot, item.kind))
            })
            .collect();
        for (slot, kind) in slots {
            if self.gear.0.set_wear(slot, 0).is_ok() {
                mended.push(kind);
            }
        }
        mended
    }

    /// How many of a kind of item are carried: in the backpack, and in what is worn, such
    /// as a belt quiver's arrows.
    pub fn count(&self, kind: ItemKind) -> u32 {
        self.backpack().count(kind) + self.gear().stacked(kind)
    }

    pub fn missing(&self, cost: &Cost, times: u32) -> Option<(ItemKind, u32)> {
        missing(self.items, self.backpack(), self.gear(), cost, times)
    }

    pub fn room_for(&self, cost: &Cost, times: u32) -> bool {
        room_for(self.items, self.backpack(), cost, times)
    }

    /// Takes one of the most filling food carried, if there is any, and says which.
    pub fn eat_best(&mut self) -> Option<ItemKind> {
        let food = self
            .items
            .foods()
            .into_iter()
            .rev()
            .find(|&food| self.count(food) > 0)?;
        self.eat(food).then_some(food)
    }

    /// Takes one of a food, if it is carried. Returns whether it was.
    pub fn eat(&mut self, food: ItemKind) -> bool {
        let taken = self.backpack.0.take(&[(food, 1)]).is_ok();
        self.bar.0.tidy(&self.backpack.0);
        taken
    }

    /// Empties the backpack into a bag of the same size, as when you go down (D84). The
    /// bar keeps pointing at what was on it, so taking the bag back puts the tools back
    /// in reach.
    pub fn empty_into_bag(&mut self) -> Grid {
        empty_into_bag(self.items, &mut self.backpack.0)
    }

    /// Takes back what fits from a bag. Returns how many things came back.
    pub fn take_from(&mut self, bag: &mut Grid) -> usize {
        let backpack = &mut self.backpack;
        let ids: Vec<(ItemId, ItemKind)> = bag
            .iter()
            .map(|placed| (placed.item.id, placed.item.kind))
            .collect();
        let taken: Vec<ItemKind> = ids
            .into_iter()
            .filter(|&(id, _)| {
                terra_items::transfer(&self.items.catalogue, bag, &mut backpack.0, id, None).is_ok()
            })
            .map(|(_, kind)| kind)
            .collect();
        for &kind in &taken {
            self.tell(kind, 1);
        }
        taken.len()
    }

    /// Moves an item from the backpack into another grid, such as a chest's, wherever it
    /// first fits; the bar lets go of it.
    pub fn put_into(&mut self, into: &mut Grid, id: ItemId) -> Result<(), Refused> {
        terra_items::transfer(&self.items.catalogue, &mut self.backpack.0, into, id, None)?;
        self.bar.0.tidy(&self.backpack.0);
        Ok(())
    }

    /// Moves an item from another grid, such as a chest's, into the backpack, wherever it
    /// first fits.
    pub fn take_out_of(&mut self, from: &mut Grid, id: ItemId) -> Result<(), Refused> {
        let kind = from.get(id).ok_or(Refused::NotFound)?.item.kind;
        terra_items::transfer(&self.items.catalogue, from, &mut self.backpack.0, id, None)?;
        self.tell(kind, 1);
        Ok(())
    }

    /// Puts what a station made where it goes, all or nothing: into what is worn that holds
    /// it, such as arrows into a belt quiver, then the backpack.
    pub fn receive(&mut self, made: Made) -> bool {
        let (kind, count) = self.items.made(made);
        let received = self.receive_quietly(made);
        if received {
            self.tell(kind, count);
        }
        received
    }

    fn receive_quietly(&mut self, made: Made) -> bool {
        let (kind, count) = self.items.made(made);
        let mut trial = self.gear.0.clone();
        let worn = trial.stow(kind, count);
        if worn == count
            || receive_kind(
                self.items,
                &mut self.backpack.0,
                &mut self.bar.0,
                kind,
                count - worn,
            )
        {
            self.gear.0 = trial;
            return true;
        }
        false
    }

    /// Whether one more of a kind of item would fit in the backpack.
    pub fn room_for_one(&self, kind: ItemKind) -> bool {
        let size = self
            .items
            .catalogue
            .get(kind)
            .map_or(Size::ONE, |def| def.size);
        self.backpack().space_for(size).is_some()
    }

    /// Spends so many of a kind, if there are that many. Returns whether it did.
    pub fn spend(&mut self, kind: ItemKind, count: u32) -> bool {
        spend(
            &mut self.backpack.0,
            &mut self.bar.0,
            &mut self.gear.0,
            &[(kind, count)],
        )
    }

    pub fn pay(&mut self, cost: &Cost, times: u32) -> bool {
        pay(
            self.items,
            &mut self.backpack.0,
            &mut self.bar.0,
            &mut self.gear.0,
            cost,
            times,
        )
    }

    /// Gives back what a cost so many times over is made of, if all of it fits. Returns
    /// whether it did.
    pub fn refund(&mut self, cost: &Cost, times: u32) {
        for (kind, count) in self.items.wanted(cost, times) {
            self.give(kind, count);
        }
    }

    /// Puts so many of a kind of item where it goes, as many as fit: into what is worn that
    /// holds it, such as arrows into a belt quiver, then the backpack. Returns how many did.
    pub fn give(&mut self, kind: ItemKind, count: u32) -> u32 {
        let worn = self.gear.0.stow(kind, count);
        let given = worn + give(self.items, &mut self.backpack.0, kind, count - worn);
        self.tell(kind, given);
        // What does not fit drops at the player's feet.
        if given < count {
            self.spilled.write(Spilled {
                player: self.player,
                kind,
                count: count - given,
            });
        }
        given
    }

    fn tell(&mut self, kind: ItemKind, count: u32) {
        if count > 0 {
            self.gained.write(Gained {
                player: self.player,
                kind,
                count,
            });
        }
    }

    pub fn pick(&mut self, slot: usize) {
        if self.bar().picked() != slot {
            self.bar.0.pick(slot);
        }
    }

    /// Wears an item from the backpack.
    pub fn wear(&mut self, id: ItemId) -> Result<(), Refused> {
        let worn = wear(self.items, &mut self.backpack.0, &mut self.gear.0, id);
        self.bar.0.tidy(&self.backpack.0);
        worn
    }

    /// Takes off what is in a slot, into the backpack.
    pub fn take_off(&mut self, slot: GearSlot) -> Result<(), Refused> {
        take_off(self.items, &mut self.backpack.0, &mut self.gear.0, slot)
    }

    /// Points a slot of the bar at a tool in the backpack. Only tools go on the bar.
    pub fn put_on_bar(&mut self, slot: usize, id: ItemId) {
        let is_tool = self
            .backpack()
            .get(id)
            .is_some_and(|placed| self.items.tool(placed.item.kind).is_some());
        if is_tool {
            self.bar.0.set(slot, Some(id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{test_player, test_world};
    use bevy::ecs::system::RunSystemOnce;

    fn in_hand(world: &mut World, player: Entity) -> Option<ItemKind> {
        world
            .run_system_once(move |mut belongings: Belongings| {
                belongings.of(player).and_then(|carried| carried.holding())
            })
            .unwrap()
    }

    /// The starting kit has a tool in hand; hands full put it away, and empty they hold it
    /// again, so no tool answers its buttons while a sword is drawn (D405).
    #[test]
    fn full_hands_hold_no_tool() {
        let mut world = test_world();
        let player = test_player(&mut world, Vec3::Y * 2000.0);
        let tool = in_hand(&mut world, player);
        assert!(tool.is_some());
        world.entity_mut(player).insert(HandsFull);
        assert_eq!(in_hand(&mut world, player), None);
        world.entity_mut(player).remove::<HandsFull>();
        assert_eq!(in_hand(&mut world, player), tool);
    }
}
