//! What is worn, on each character: each item in its outfit (`character::Outfit`) shown as
//! that item's look for the character's body, found by the item's name and put on as its
//! file says (`character`). Nothing here knows any one item; a new piece of gear is a new
//! look file. Looks change only when an outfit does. The player's outfit is what its
//! equipment holds.

use super::inventory::{Gear, Items};
use crate::character::{self, Body, BodyReady, Outfit, Worn};
use bevy::{gltf::GltfAssetLabel, prelude::*};

/// The outfit a body is shown wearing, so its looks are changed only when that changes.
#[derive(Component)]
struct Dressed(Outfit);

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (outfit_from_gear, show).chain());
}

/// Keeps a character's outfit to what its equipment holds.
fn outfit_from_gear(
    mut commands: Commands,
    items: Res<Items>,
    gears: Query<(Entity, Ref<Gear>, Option<&Outfit>)>,
) {
    for (entity, gear, outfit) in &gears {
        if outfit.is_some() && !gear.is_changed() {
            continue;
        }
        let wears = gear
            .0
            .iter()
            .filter_map(|(_, item)| items.catalogue.get(item?.kind).ok())
            .map(|def| character::item_id(&def.name))
            .collect();
        let wears = Outfit(wears);
        if outfit != Some(&wears) {
            commands.entity(entity).insert(wears);
        }
    }
}

/// Puts on each loaded body what its character wears, when that changes.
fn show(
    mut commands: Commands,
    bodies: Query<(Entity, &Body, &ChildOf, Option<&Dressed>), With<BodyReady>>,
    outfits: Query<Ref<Outfit>>,
    looks: Query<(Entity, &Worn)>,
    assets: Res<AssetServer>,
) {
    for (root, body, parent, dressed) in &bodies {
        let Ok(outfit) = outfits.get(parent.parent()) else {
            continue;
        };
        if dressed.is_some_and(|dressed| !outfit.is_changed() || dressed.0 == *outfit) {
            continue;
        }
        for (look, on) in &looks {
            if on.body == root {
                commands.entity(look).despawn();
            }
        }
        for item in &outfit.0 {
            let file = character::look(body.name, item);
            character::wear(
                &mut commands,
                assets.load(GltfAssetLabel::Scene(0).from_asset(file)),
                root,
            );
        }
        commands
            .entity(root)
            .insert(Dressed(Outfit::clone(&outfit)));
    }
}
