//! The lush forest through the year (climate.md, D327): its leaves, bark and crowns read the
//! climate's maps as grown plants' do (`terra_world::grown::FoliageClimate`), each time they
//! are painted. Which kind a leaf is, the card it is drawn from says, as each kind's year in
//! `assets/data/plant_years.ron` names its cards.

use super::{Foliage, FoliageExtension};
use bevy::prelude::*;
use terra_world::climate::{ClimateFields, ClimateMaps};
use terra_world::grown::{Cards, FoliageClimate};

pub fn plugin(app: &mut App) {
    app.add_systems(Update, follow_climate);
}

/// Every one of the forest's foliage materials reads the climate's maps again when painted.
fn follow_climate(
    maps: Option<Res<ClimateMaps>>,
    fields: Option<Res<ClimateFields>>,
    cards: Res<Cards>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<Foliage>>,
) {
    let Some(maps) = maps.filter(|maps| maps.is_changed()) else {
        return;
    };
    let atlas: Handle<Image> = assets.load(cards.atlas.clone());
    let of_atlas = FoliageClimate::from_maps(&maps, fields.as_deref(), Some(&cards));
    let other = FoliageClimate::from_maps(&maps, None, None);
    for (_, material) in materials.iter_mut() {
        let extension: &mut FoliageExtension = &mut material.extension;
        extension.climate = if material.base.base_color_texture.as_ref() == Some(&atlas) {
            of_atlas
        } else {
            other
        };
        extension.climate_ground = maps.ground.clone();
        extension.climate_leaves = maps.leaves.clone();
    }
}
