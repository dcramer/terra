//! What the climate does to grown plants' materials (D327): every material drawn in
//! [`Foliage`], the flora's plants and the lush forest's trees and floor alike, is told the
//! climate's maps each time they are painted.

use super::{Cards, Foliage, FoliageClimate, atlas};
use bevy::prelude::*;

/// Every grown plant's material reads the climate's maps again each time they are painted,
/// and is changed only if what it is told of them has: the maps are painted again in place,
/// and a changed material has every plant drawn with it specialized again, some 20 ms of the
/// render thread over two frames. A card of the atlas is told which kind each cell is, cooked
/// or, where none is cooked, as the picture it would be cooked from.
pub(super) fn follow_climate(
    maps: Option<Res<crate::climate::ClimateMaps>>,
    fields: Option<Res<crate::climate::ClimateFields>>,
    cards: Res<Cards>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<Foliage>>,
) {
    let Some(maps) = maps.filter(|maps| maps.is_changed()) else {
        return;
    };
    let atlas: Handle<Image> = atlas(&cards, &assets);
    let of_atlas = FoliageClimate::from_maps(&maps, fields.as_deref(), Some(&cards));
    let other = FoliageClimate::from_maps(&maps, None, None);
    let is_atlas = |texture: &Handle<Image>| {
        *texture == atlas
            || texture
                .path()
                .is_some_and(|path| path.path() == std::path::Path::new(&cards.atlas))
    };
    let stale: Vec<_> = materials
        .iter()
        .filter_map(|(id, material)| {
            let climate = if material
                .base
                .base_color_texture
                .as_ref()
                .is_some_and(is_atlas)
            {
                of_atlas
            } else {
                other
            };
            let extension = &material.extension;
            (extension.climate != climate
                || extension.climate_ground != maps.ground
                || extension.climate_leaves != maps.leaves)
                .then_some((id, climate))
        })
        .collect();
    for (id, climate) in stale {
        if let Some(mut material) = materials.get_mut(id) {
            material.extension.climate = climate;
            material.extension.climate_ground = maps.ground.clone();
            material.extension.climate_leaves = maps.leaves.clone();
        }
    }
}
