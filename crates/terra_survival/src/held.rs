//! What is in hand, on the figure: the item in the picked slot of the bar, as its own look
//! in the right hand's socket, swapped as soon as another slot is picked. Each item's look is
//! a file found by its name (`character::held_look`, made by
//! `tools/characters/fit_held.py`), which also says how it is carried; nothing here knows
//! any one tool. Every player's figure shows what that player holds.

use crate::inventory::{Backpack, Bar, Items};
use bevy::{gltf::GltfAssetLabel, prelude::*};
use terra_items::ItemKind;
use terra_walker::Player;
use terra_walker::character;
use terra_walker::figure::Grip;

/// What a figure's hand (its [`Grip`], which requires this once the plugin is in) shows.
/// Held looks are the grip's children.
#[derive(Component, Default)]
pub struct InHand {
    /// The kind of item shown in it, once something has been decided.
    showing: Option<Option<ItemKind>>,
    /// A sword is drawn (D405): the bar's item is put away while it is.
    pub drawn: bool,
}

/// A look shown in hand.
#[derive(Component)]
struct Shown;

pub fn plugin(app: &mut App) {
    app.register_required_components::<Grip, InHand>();
    app.add_systems(Update, show);
}

/// Puts the item in the picked slot of each player's bar in their figure's hand, whenever
/// it changes.
fn show(
    mut commands: Commands,
    items: Res<Items>,
    players: Query<(&Backpack, &Bar), With<Player>>,
    mut grips: Query<(Entity, &Grip, &mut InHand)>,
    children: Query<&Children>,
    shown: Query<(), With<Shown>>,
    assets: Res<AssetServer>,
) {
    for (hand, grip, mut in_hand) in &mut grips {
        let Ok((backpack, bar)) = players.get(grip.player) else {
            continue;
        };
        let holding = bar
            .0
            .in_hand(&backpack.0)
            .map(|item| item.kind)
            .filter(|_| !in_hand.drawn);
        if in_hand.showing == Some(holding) {
            continue;
        }
        in_hand.showing = Some(holding);
        for look in children
            .iter_descendants(hand)
            .filter(|&each| shown.contains(each))
        {
            commands.entity(look).despawn();
        }
        let Some(def) = holding.and_then(|kind| items.catalogue.get(kind).ok()) else {
            continue;
        };
        let file = character::held_look(&character::item_id(&def.name));
        let look = character::hold(
            &mut commands,
            assets.load(GltfAssetLabel::Scene(0).from_asset(file)),
            hand,
        );
        commands.entity(look).insert(Shown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grip_shows_what_is_in_hand_with_no_sword_drawn() {
        let mut app = App::new();
        plugin(&mut app);
        let player = app.world_mut().spawn_empty().id();
        let hand = app.world_mut().spawn(Grip { player }).id();
        let in_hand = app
            .world()
            .get::<InHand>(hand)
            .expect("held makes a grip show");
        assert!(!in_hand.drawn);
        assert_eq!(in_hand.showing, None);
    }
}
