//! What is in hand, on the figure: the item in the picked slot of the bar, as its own look
//! in the right hand's socket, swapped as soon as another slot is picked. Each item's look is
//! a file found by its name (`character::held_look`, made by
//! `tools/characters/fit_held.py`), which also says how it is carried; nothing here knows
//! any one tool. Every player's figure shows what that player holds.

use super::Player;
use super::inventory::{Backpack, Bar, Items};
use crate::character;
use bevy::{gltf::GltfAssetLabel, prelude::*};
use terra_items::ItemKind;

/// The socket in a figure's right hand (`character::HAND_R`), and whose hand it is. Held
/// looks are its children.
#[derive(Component)]
pub struct Grip {
    pub player: Entity,
    /// The kind of item shown in it, once something has been decided.
    showing: Option<Option<ItemKind>>,
}

impl Grip {
    pub fn of(player: Entity) -> Self {
        Self {
            player,
            showing: None,
        }
    }
}

/// A look shown in hand.
#[derive(Component)]
struct Shown;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, show);
}

/// Puts the item in the picked slot of each player's bar in their figure's hand, whenever
/// it changes.
fn show(
    mut commands: Commands,
    items: Res<Items>,
    players: Query<(&Backpack, &Bar), With<Player>>,
    mut grips: Query<(Entity, &mut Grip)>,
    children: Query<&Children>,
    shown: Query<(), With<Shown>>,
    assets: Res<AssetServer>,
) {
    for (hand, mut grip) in &mut grips {
        let Ok((backpack, bar)) = players.get(grip.player) else {
            continue;
        };
        let holding = bar.0.in_hand(&backpack.0).map(|item| item.kind);
        if grip.showing == Some(holding) {
            continue;
        }
        grip.showing = Some(holding);
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
