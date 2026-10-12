//! What is worn, on the figure: a pack on the back, a cap on the head, a coat fitted to the
//! body, a quiver at the hip. Each is a model fitted to the base body
//! (`spike/character_gear/fit_gear.py`), hung on a socket or bound to the skeleton
//! (`character`), and changed whenever what is worn changes.

use super::Player;
use super::figure::PlayerFigure;
use super::inventory::{Apparel, Container, Gear, Items};
use crate::character;
use bevy::{gltf::GltfAssetLabel, prelude::*};
use terra_items::ItemKind;

/// Something worn, shown on the figure.
#[derive(Component)]
struct Shown;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, show);
}

/// A worn thing's model, and the socket it hangs on; none for clothing fitted to the body.
fn look(items: &Items, kind: ItemKind) -> Option<(&'static str, Option<&'static str>)> {
    if let Some(apparel) = items.apparel(kind) {
        return Some(match apparel {
            Apparel::SmallPack => ("models/gear/pack-small.glb", Some("socket.back")),
            Apparel::LargePack => ("models/gear/pack-large.glb", Some("socket.back")),
            Apparel::LeatherCap => ("models/gear/cap.glb", Some("socket.head")),
            Apparel::LeatherCoat => ("models/gear/coat.glb", None),
        });
    }
    match items.container(kind)? {
        Container::Quiver => Some(("models/gear/quiver.glb", Some("socket.belt.R"))),
    }
}

/// Puts on the figure what is worn, whenever that changes, once the figure has loaded.
fn show(
    mut commands: Commands,
    items: Res<Items>,
    gear: Single<&Gear, With<Player>>,
    figure: Option<Single<Entity, With<PlayerFigure>>>,
    children: Query<&Children>,
    names: Query<&Name>,
    assets: Res<AssetServer>,
    shown: Query<Entity, With<Shown>>,
    mut showing: Local<Option<Vec<ItemKind>>>,
) {
    let Some(figure) = figure else { return };
    let figure = *figure;
    // Its sockets exist once its model has been spawned.
    if character::find(figure, character::HAND_R, &children, &names).is_none() {
        return;
    }
    let worn: Vec<ItemKind> = gear
        .0
        .iter()
        .filter_map(|(_, item)| Some(item?.kind))
        .collect();
    if showing.as_ref() == Some(&worn) {
        return;
    }
    *showing = Some(worn.clone());
    for entity in &shown {
        commands.entity(entity).despawn();
    }
    for kind in worn {
        let Some((file, socket)) = look(&items, kind) else {
            continue;
        };
        let model = assets.load(GltfAssetLabel::Scene(0).from_asset(file));
        let entity = match socket {
            Some(socket) => {
                let Some(at) = character::find(figure, socket, &children, &names) else {
                    warn!("The figure has no {socket}.");
                    continue;
                };
                character::hang(&mut commands, model, at)
            }
            None => character::dress(&mut commands, model, figure),
        };
        commands.entity(entity).insert(Shown);
    }
}
