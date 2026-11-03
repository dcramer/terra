//! What is worn, on each character: each item in its outfit (`character::Outfit`) shown as
//! that item's look made on the character's body (`appearance::gear_for_model`), which fits each
//! of that body's morphs, found by the item's name and put on as its
//! file says (`character`). Nothing here knows any one item; a new piece of gear is a new
//! look file. Looks change only when an outfit does. The player's outfit is what its
//! equipment holds.

use crate::inventory::{Gear, Items};
use bevy::world_serialization::WorldAssetRoot;
use bevy::{gltf::GltfAssetLabel, prelude::*};
use terra_walker::appearance::gear_for_model;
use terra_walker::character::{self, Body, BodyReady, Clothing, Outfit, Worn};

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
            // What the off hand's slot holds is shown in hand (`held`), not worn.
            .filter_map(|(_, item)| item.filter(|item| items.worn(item.kind).is_some()))
            .filter_map(|item| items.catalogue.get(item.kind).ok())
            .map(|def| Clothing::new(character::item_id(&def.name)))
            .collect();
        let wears = Outfit(wears);
        if outfit != Some(&wears) {
            commands.entity(entity).insert(wears);
        }
    }
}

/// Puts on each loaded body what its character wears, when that changes: the looks made on
/// the model it stands on (`appearance::gear_for_model`). A body with none, such as
/// Quaternius's man dressed in his Ranger outfit as made (D602), shows only what its model
/// wears: gear made on another body is never drawn on it, though it is still equipped and
/// still counts.
fn show(
    mut commands: Commands,
    bodies: Query<
        (Entity, &ChildOf, Option<&Dressed>, Option<&WorldAssetRoot>),
        (With<Body>, With<BodyReady>),
    >,
    outfits: Query<Ref<Outfit>>,
    looks: Query<(Entity, &Worn)>,
    assets: Res<AssetServer>,
) {
    for (root, parent, dressed, model) in &bodies {
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
        let gear = model
            .and_then(|model| model.0.path())
            .and_then(|path| gear_for_model(&path.to_string()));
        for (gear, clothing) in gear
            .into_iter()
            .flat_map(|gear| outfit.0.iter().map(move |c| (gear, c)))
        {
            let file = character::look_in(gear, &clothing.item);
            let look = character::wear(
                &mut commands,
                assets.load(GltfAssetLabel::Scene(0).from_asset(file)),
                root,
            );
            if !clothing.tint.0.is_empty() {
                commands.entity(look).insert(clothing.tint.clone());
            }
        }
        commands
            .entity(root)
            .insert(Dressed(Outfit::clone(&outfit)));
    }
}
