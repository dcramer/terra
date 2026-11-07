//! Paint on a body's parts (D111): a person's skin tone, hair colour and eyes, on each piece
//! of the body they stand on (`creator::Creator::paint`), as [`super::Tint`] dyes what is
//! worn. A part's coat is by the role its material plays; coated materials are shared.

use bevy::gltf::GltfMaterialName;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use std::collections::HashMap;

/// How a part of a body is painted (D111): its texture multiplied by a colour, which may be
/// over 1 to lighten, or its texture another map.
#[derive(Clone, Debug, PartialEq)]
pub enum Coat {
    Colour(LinearRgba),
    /// A base colour map, by its path under `assets/`.
    Map(String),
}

/// Paint for a look's parts, by the role their material plays (its glTF material name before
/// any `.`), as [`super::Tint`] names them: a person's skin tone, hair colour and eyes on each piece
/// of their body (`creator::Creator::paint`). On a look's root; applied once it has loaded.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Paint(pub Vec<(String, Coat)>);

/// Painted materials, shared: one per original material and coat.
#[derive(Resource, Default)]
pub(super) struct Painted(HashMap<(AssetId<StandardMaterial>, String), Handle<StandardMaterial>>);

/// Once a painted look has loaded, gives each part whose role has a coat a material so
/// coated, shared with every other part coated the same.
pub(super) fn paint(
    ready: On<WorldInstanceReady>,
    paints: Query<&Paint>,
    children: Query<&Children>,
    parts: Query<(Entity, &MeshMaterial3d<StandardMaterial>, &GltfMaterialName)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut painted: ResMut<Painted>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(paint) = paints.get(ready.entity) else {
        return;
    };
    for each in children.iter_descendants(ready.entity) {
        let Ok((part, material, name)) = parts.get(each) else {
            continue;
        };
        let role = name.0.split('.').next().unwrap_or_default();
        let Some((_, coat)) = paint.0.iter().find(|(wanted, _)| wanted == role) else {
            continue;
        };
        let key = (material.0.id(), format!("{coat:?}"));
        let handle = match painted.0.get(&key) {
            Some(handle) => handle.clone(),
            None => {
                let Some(mut coated) = materials.get(&material.0).cloned() else {
                    continue;
                };
                match coat {
                    Coat::Colour(colour) => coated.base_color = Color::LinearRgba(*colour),
                    Coat::Map(map) => coated.base_color_texture = Some(assets.load(map)),
                }
                let handle = materials.add(coated);
                painted.0.insert(key, handle.clone());
                handle
            }
        };
        commands.entity(part).insert(MeshMaterial3d(handle));
    }
}
