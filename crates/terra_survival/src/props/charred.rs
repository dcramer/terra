//! A model drawn charred, as fire leaves wood and pitch-soaked cloth: darker than its own
//! colour, matte and without its sheen. A torch's glTF has one material and no emissive, its
//! head a pale grey wrap: drawn as it came, the staged torch's head read as clean cloth and
//! the crypt's wall torches as pale grey spikes (torch rounds t3 and t4), where a game's torch
//! head is black with soot and pitch.

use bevy::{gltf::GltfMaterialName, prelude::*};

/// On a model's root: each of its parts is drawn charred (`charred`) as it is spawned.
#[derive(Component)]
pub struct Charred;

/// How dark a charred part is against its own colour (linear, so about 0.55 in sRGB): its
/// pale wrap goes a sooty dark grey and its wood a dark brown, charcoal's own albedo (about
/// 0.04) left to what has burnt through, not the whole model.
const CHAR_SHADE: f32 = 0.3;
/// Char is matte and dull: rougher than a prop's 0.8 (`lit`), and less reflective than its 0.3.
const CHAR_ROUGHNESS: f32 = 0.95;
const CHAR_REFLECTANCE: f32 = 0.2;

/// A part's material charred: a prop's (`as_prop`), darker, without its roughness map, matte.
pub fn charred(own: &StandardMaterial) -> StandardMaterial {
    let base = own.base_color.to_linear();
    StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba {
            red: base.red * CHAR_SHADE,
            green: base.green * CHAR_SHADE,
            blue: base.blue * CHAR_SHADE,
            alpha: base.alpha,
        }),
        metallic_roughness_texture: None,
        metallic: 0.0,
        perceptual_roughness: CHAR_ROUGHNESS,
        reflectance: CHAR_REFLECTANCE,
        ..super::as_prop(own)
    }
}

/// Each part spawned under a `Charred` root takes a charred material of its own, once, as its
/// glTF names its material.
pub(super) fn char_parts(
    parts: Query<(Entity, &MeshMaterial3d<StandardMaterial>), Added<GltfMaterialName>>,
    parents: Query<&ChildOf>,
    roots: Query<(), With<Charred>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    for (part, material) in &parts {
        if !parents.iter_ancestors(part).any(|up| roots.contains(up)) {
            continue;
        }
        let Some(own) = materials.get(&material.0).map(charred) else {
            continue;
        };
        commands
            .entity(part)
            .insert(MeshMaterial3d(materials.add(own)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_charred_part_is_darker_matte_and_without_its_roughness_map() {
        let own = StandardMaterial {
            base_color: Color::srgb(0.7, 0.68, 0.62),
            perceptual_roughness: 0.5,
            reflectance: 0.5,
            metallic_roughness_texture: Some(Handle::default()),
            ..default()
        };
        let burnt = charred(&own);
        let (was, now) = (own.base_color.luminance(), burnt.base_color.luminance());
        assert!(now < was * 0.5, "{now} against {was}");
        assert!(burnt.perceptual_roughness > 0.9 && burnt.reflectance < 0.3);
        assert!(burnt.metallic_roughness_texture.is_none());
    }
}
