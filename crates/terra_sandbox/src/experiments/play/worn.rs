//! What is worn, on each character: each worn item's look for the character's body, found by
//! the item's name and put on as its file says (`character`). Nothing here knows any one
//! item; a new piece of gear is a new look file. Looks change only when what a character
//! wears does.

use super::inventory::{Gear, Items};
use crate::character::{self, Body, BodyReady, Worn};
use bevy::{gltf::GltfAssetLabel, prelude::*};
use terra_items::ItemKind;

/// What a body is shown wearing, so its looks are changed only when that changes.
#[derive(Component)]
struct Dressed(Vec<ItemKind>);

pub fn plugin(app: &mut App) {
    app.add_systems(Update, show);
}

/// Puts on each loaded body what its character wears, when that changes.
fn show(
    mut commands: Commands,
    items: Res<Items>,
    bodies: Query<(Entity, &Body, &ChildOf, Option<&Dressed>), With<BodyReady>>,
    gears: Query<Ref<Gear>>,
    looks: Query<(Entity, &Worn)>,
    assets: Res<AssetServer>,
) {
    for (root, body, parent, dressed) in &bodies {
        let Ok(gear) = gears.get(parent.parent()) else {
            continue;
        };
        if dressed.is_some() && !gear.is_changed() {
            continue;
        }
        let worn: Vec<ItemKind> = gear
            .0
            .iter()
            .filter_map(|(_, item)| Some(item?.kind))
            .collect();
        if dressed.is_some_and(|dressed| dressed.0 == worn) {
            continue;
        }
        for (look, on) in &looks {
            if on.body == root {
                commands.entity(look).despawn();
            }
        }
        for kind in &worn {
            let Ok(def) = items.catalogue.get(*kind) else {
                continue;
            };
            let file = character::look(body.name, &character::item_id(&def.name));
            character::wear(
                &mut commands,
                assets.load(GltfAssetLabel::Scene(0).from_asset(file)),
                root,
            );
        }
        commands.entity(root).insert(Dressed(worn));
    }
}
