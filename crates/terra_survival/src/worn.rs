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
use terra_walker::wardrobe::wardrobe;

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
/// the model it stands on (`appearance::gear_for_model`), or on a wardrobe body (D611,
/// Quaternius's man) the pieces the catalogue composes for it, a piece a slot, changing only
/// the pieces that change. Gear made on another body is never drawn on it, though it is still
/// equipped and still counts.
fn show(
    mut commands: Commands,
    bodies: Query<
        (Entity, &ChildOf, Option<&Dressed>, Option<&WorldAssetRoot>),
        (With<Body>, With<BodyReady>),
    >,
    outfits: Query<(Ref<Outfit>, Has<Gear>)>,
    looks: Query<(Entity, &Worn, Option<&WorldAssetRoot>)>,
    assets: Res<AssetServer>,
) {
    for (root, parent, dressed, model) in &bodies {
        let Ok((outfit, equipped)) = outfits.get(parent.parent()) else {
            continue;
        };
        if dressed.is_some_and(|dressed| !outfit.is_changed() || dressed.0 == *outfit) {
            continue;
        }
        let path = model
            .and_then(|model| model.0.path())
            .map(|p| p.to_string());
        commands
            .entity(root)
            .insert(Dressed(Outfit::clone(&outfit)));
        // A wardrobe body (D611): a piece a slot, as the catalogue composes him.
        if let Some((body, at)) = path.as_deref().and_then(|p| wardrobe().body_for_model(p)) {
            let worn: Vec<String> = outfit.0.iter().map(|c| c.item.clone()).collect();
            // Anyone with no equipment of their own (the town, the sparring partner) wears the
            // body's look whole until populations draw theirs; the player's follows its gear.
            let files: Vec<String> = wardrobe()
                .compose(body, &at.look, equipped.then_some(worn.as_slice()))
                .into_iter()
                .filter_map(|piece| wardrobe().file(piece))
                .collect();
            let mut kept = Vec::new();
            for (look, on, file) in &looks {
                let file = file
                    .and_then(|f| f.0.path())
                    .map(|p| p.path().to_string_lossy().into_owned());
                if on.body != root {
                    continue;
                }
                match file.filter(|f| files.contains(f)) {
                    Some(f) => kept.push(f),
                    None => commands.entity(look).despawn(),
                }
            }
            for file in files.into_iter().filter(|f| !kept.contains(f)) {
                character::wear(
                    &mut commands,
                    assets.load(GltfAssetLabel::Scene(0).from_asset(file)),
                    root,
                );
            }
            continue;
        }
        for (look, on, _) in &looks {
            if on.body == root {
                commands.entity(look).despawn();
            }
        }
        let gear = path.as_deref().and_then(gear_for_model);
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
    }
}
