//! What is in hand, on the figure: the item in the picked slot of the bar, as its own look
//! in the right hand's socket, swapped as soon as another slot is picked. Each item's look is
//! a file found by its name (`character::held_look`, made by
//! `spike/character_gear/fit_held.py`), which also says how it is carried; nothing here knows
//! any one tool.

use super::inventory::Belongings;
use crate::character;
use bevy::{gltf::GltfAssetLabel, prelude::*};
use terra_items::ItemKind;

/// The socket in the figure's right hand (`character::HAND_R`). Held looks are its
/// children.
#[derive(Component)]
pub struct Grip;

/// A look shown in hand.
#[derive(Component)]
struct Shown;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, show);
}

/// Puts the item in the picked slot in the figure's hand, whenever it changes.
fn show(
    mut commands: Commands,
    belongings: Belongings,
    grip: Option<Single<Entity, With<Grip>>>,
    shown: Query<Entity, With<Shown>>,
    assets: Res<AssetServer>,
    mut showing: Local<Option<Option<ItemKind>>>,
) {
    let Some(grip) = grip else {
        return;
    };
    let holding = belongings
        .bar()
        .in_hand(belongings.backpack())
        .map(|item| item.kind);
    if *showing == Some(holding) {
        return;
    }
    *showing = Some(holding);
    for entity in &shown {
        commands.entity(entity).despawn();
    }
    let Some(def) = holding.and_then(|kind| belongings.items().catalogue.get(kind).ok()) else {
        return;
    };
    let file = character::held_look(&character::item_id(&def.name));
    let look = character::hold(
        &mut commands,
        assets.load(GltfAssetLabel::Scene(0).from_asset(file)),
        *grip,
    );
    commands.entity(look).insert(Shown);
}
