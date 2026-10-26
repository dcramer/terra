//! What is in hand, on the figure (docs/holding.md): each hand's item as its own look in that
//! hand's socket, swapped as soon as it changes. The main hand holds the item in the picked
//! slot of the bar (D66); the off hand holds the off hand's slot's (D460) whenever the main
//! hand's item leaves it free. A layer above may put things in hand itself (`Hands::given`), as
//! the sword fight does on drawing (D405's shortcut). Each item's look is a file found by its
//! name (`character::held_look`, made by `tools/characters/fit_held.py`), which also says how it
//! is carried, or that it is strapped to the forearm (a shield); nothing here knows any one
//! item. Every player's figure shows what that player holds.

use crate::inventory::{Backpack, Bar, Gear, Items, OFF_HAND};
use bevy::{gltf::GltfAssetLabel, prelude::*};
use terra_items::ItemKind;
use terra_motion::hold::{Grip as Held, Hold};
use terra_walker::Player;
use terra_walker::character;
use terra_walker::figure::{Grip, Hand};

/// What a body's two hands hold, by kind. Kept from the bar and the off hand's slot, unless a
/// layer above has `given` them.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hands {
    pub main: Option<ItemKind>,
    pub off: Option<ItemKind>,
    /// Put in hand by a layer above (the sword fight, D405), not by the bar and the slot.
    pub given: bool,
}

impl Hands {
    /// What one hand holds.
    pub fn of(&self, hand: Hand) -> Option<ItemKind> {
        match hand {
            Hand::Main => self.main,
            Hand::Off => self.off,
        }
    }
}

/// What a figure's hand (its [`Grip`], which requires this once the plugin is in) shows.
#[derive(Component, Default)]
pub struct InHand {
    /// The kind of item shown in it, once something has been decided.
    showing: Option<Option<ItemKind>>,
    /// The look shown: the socket's child, or the forearm's for a strapped look.
    look: Option<Entity>,
}

impl InHand {
    /// The kind of item shown in this hand, if any.
    pub fn kind(&self) -> Option<ItemKind> {
        self.showing.flatten()
    }
}

pub fn plugin(app: &mut App) {
    app.register_required_components::<Grip, InHand>();
    app.register_required_components::<Player, Hands>();
    app.add_systems(Update, (keep, show).chain());
}

/// The pair the loadout puts in hand: the bar's picked item in the main hand, and the off hand's
/// slot's in the off hand unless the main hand's item takes both hands (a `Two` hold, which
/// stows it until it leaves, holding.md). A tool leaves the off hand free, as the stance table
/// pairs a tool with anything.
pub fn loadout(items: &Items, main: Option<ItemKind>, off: Option<ItemKind>) -> Hands {
    let free = off_hand_free(main.and_then(|kind| items.hold(kind)));
    Hands {
        main,
        off: off.filter(|_| free),
        given: false,
    }
}

/// Whether the main hand's item, by its `hold` (none for a tool or nothing), leaves the off hand
/// free: everything but a two-hander.
pub fn off_hand_free(main: Option<&Hold>) -> bool {
    !main.is_some_and(|hold| hold.grip == Held::Two)
}

/// Keeps each player's hands to their bar and off hand's slot, unless a layer above gave them.
fn keep(items: Res<Items>, mut players: Query<(&Backpack, &Bar, &Gear, &mut Hands)>) {
    for (backpack, bar, gear, mut hands) in &mut players {
        if hands.given {
            continue;
        }
        let main = bar.0.in_hand(&backpack.0).map(|item| item.kind);
        let off = gear.0.worn(OFF_HAND).map(|item| item.kind);
        hands.set_if_neq(loadout(&items, main, off));
    }
}

/// Puts what each hand holds in that hand's socket, whenever it changes.
fn show(
    mut commands: Commands,
    items: Res<Items>,
    players: Query<&Hands>,
    mut grips: Query<(Entity, &Grip, &mut InHand)>,
    assets: Res<AssetServer>,
) {
    for (socket, grip, mut in_hand) in &mut grips {
        let Ok(hands) = players.get(grip.player) else {
            continue;
        };
        let holding = hands.of(grip.hand);
        if in_hand.showing == Some(holding) {
            continue;
        }
        in_hand.showing = Some(holding);
        if let Some(look) = in_hand.look.take() {
            commands.entity(look).try_despawn();
        }
        let Some(def) = holding.and_then(|kind| items.catalogue.get(kind).ok()) else {
            continue;
        };
        let file = character::held_look(&character::item_id(&def.name));
        in_hand.look = Some(character::hold(
            &mut commands,
            assets.load(GltfAssetLabel::Scene(0).from_asset(file)),
            socket,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grip_shows_nothing_until_its_hand_is_decided() {
        let mut app = App::new();
        plugin(&mut app);
        let player = app.world_mut().spawn_empty().id();
        let hand = Hand::Off;
        let socket = app.world_mut().spawn(Grip { player, hand }).id();
        let in_hand = app
            .world()
            .get::<InHand>(socket)
            .expect("held makes a grip show");
        assert_eq!(in_hand.kind(), None);
        assert_eq!(in_hand.showing, None);
    }

    #[test]
    fn the_off_hands_slot_is_in_hand_beside_a_one_hander_and_stowed_by_a_two_hander() {
        let items = Items::default();
        let (sword, torch, shield) = (
            items.kind("Arming sword"),
            items.kind("Torch"),
            items.kind("Heater shield"),
        );
        let hands = loadout(&items, Some(sword), Some(torch));
        assert_eq!(
            (hands.of(Hand::Main), hands.of(Hand::Off)),
            (Some(sword), Some(torch))
        );
        let hands = loadout(&items, None, Some(shield));
        assert_eq!((hands.main, hands.off), (None, Some(shield)));
        // A two-hander stows the off hand's item; a one-hander, a tool or nothing leaves it.
        let held = |grip| Hold {
            class: "greatsword".into(),
            grip,
            lead: None,
            carried: false,
            length: 1.4,
            mass: 2.5,
            balance: 0.2,
        };
        assert!(!off_hand_free(Some(&held(Held::Two))));
        assert!(off_hand_free(Some(&held(Held::Main))));
        assert!(off_hand_free(None));
    }
}
