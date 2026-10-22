//! A staged figure that burns, as a campfire: drawn as a placed prop is, its flat flame hidden
//! for the fire's own, its painted parts glowing with it.

use bevy::{gltf::GltfMaterialName, light::NotShadowCaster, prelude::*};

/// A model's flat flame (its part of the material `Fire` with no map) hidden on a figure that
/// burns (`fire::Kindled`): the fire drawing burns in its place. A painted `Fire` part is its
/// coals and burning faces, cut from its logs (the campfire's, fire round 10), and is kept:
/// it and its other parts with an emissive map glow as the fire burns, as a placed campfire's
/// do
/// (`props::Embers`), so a staged campfire is drawn as the game's: left at its glTF's
/// strength of 3, its logs were flat black planks (fire review, round 2). Each of its parts
/// takes the placed prop's material (`props::as_prop`), so a review's staged campfire is lit
/// as the one a player places (fire round 37).
pub fn quench(
    parts: Query<
        (Entity, &GltfMaterialName, &MeshMaterial3d<StandardMaterial>),
        Added<GltfMaterialName>,
    >,
    parents: Query<&ChildOf>,
    kindled: Query<(), With<terra_world::fire::Kindled>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    for (part, name, material) in &parts {
        if !parents.iter_ancestors(part).any(|up| kindled.contains(up)) {
            continue;
        }
        let Some(own) = materials
            .get(&material.0)
            .map(terra_survival::props::as_prop)
        else {
            continue;
        };
        let painted = own.emissive_texture.is_some();
        // Drawn as a placed prop's part is (`props::as_prop`), not in the glTF loader's.
        commands
            .entity(part)
            .insert(MeshMaterial3d(materials.add(own)));
        match (name.0 == "Fire", painted) {
            (true, true) => {
                commands
                    .entity(part)
                    .insert((terra_survival::props::Embers, NotShadowCaster));
            }
            (true, false) => {
                commands.entity(part).insert(Visibility::Hidden);
            }
            (false, true) => {
                commands.entity(part).insert(terra_survival::props::Embers);
            }
            (false, false) => {}
        }
    }
}
