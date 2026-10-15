//! The lush forest through the year (climate.md, D327): its leaves, bark and crowns read the
//! climate's maps as grown plants' do (`terra_world::grown::FoliageClimate`), each time they
//! are painted. Which kind a leaf is, the card it is drawn from says, as each kind's year in
//! `assets/data/plant_years.ron` names its cards. And the sky the crowns hide follows the
//! broadleaves' leaves.

use super::{Foliage, FoliageExtension};
use bevy::prelude::*;
use terra_world::climate::{ClimateFields, ClimateMaps, PastCache};
use terra_world::daylight::Clock;
use terra_world::grown::{Cards, FoliageClimate};
use terra_world::sky_occlusion::{self, SkyOccluders};

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (follow_climate, follow_leaves));
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

/// How much of the sky a broadleaf's crown still hides bare, of what it hides in full leaf:
/// its limbs and twigs. A leafless oak wood lets through something like half to two thirds
/// of the light its summer canopy stops.
const BARE_CROWN: f32 = 0.35;
/// The crowns are worked out again when the broadleaves' leaves have come or gone by this
/// share since: a few times in an autumn and a spring, a few seconds on other threads each.
const RESHADE_BY: f32 = 0.1;

/// The crowns as the forest was planted, which of them are broadleaves', and how far in leaf
/// they were last shaded.
#[derive(Resource)]
struct Crowns {
    planted: Vec<sky_occlusion::Crown>,
    broadleaf: Vec<bool>,
    shaded_at: f32,
}

/// The sky occlusion volume (D270) follows the broadleaves' leaves: a bare oak's crown hides
/// far less of the sky than one in leaf, so under bare oaks the shade is the winter sky's,
/// not summer's green (D327).
fn follow_leaves(
    mut commands: Commands,
    occluders: Option<ResMut<SkyOccluders>>,
    crowns: Option<ResMut<Crowns>>,
    trees: Query<(&Name, &GlobalTransform)>,
    maps: Option<Res<ClimateMaps>>,
    fields: Option<Res<ClimateFields>>,
    clock: Res<Clock>,
    mut cache: Local<Option<PastCache>>,
) {
    let (Some(mut occluders), Some(fields)) = (occluders, fields) else {
        return;
    };
    let Some(mut crowns) = crowns else {
        // Which tree each crown is: the one whose foot is nearest under its middle.
        let feet: Vec<(Vec3, bool)> = trees
            .iter()
            .filter(|(name, _)| matches!(name.as_str(), "Oak" | "Pine" | "Spruce"))
            .map(|(name, at)| (at.translation(), name.as_str() == "Oak"))
            .collect();
        if feet.is_empty() {
            return;
        }
        let up = occluders.up;
        let broadleaf = occluders
            .crowns
            .iter()
            .map(|crown| {
                feet.iter()
                    .min_by(|a, b| {
                        let across = |foot: Vec3| (crown.middle - foot).reject_from(up).length();
                        across(a.0).total_cmp(&across(b.0))
                    })
                    .is_some_and(|(_, oak)| *oak)
            })
            .collect();
        commands.insert_resource(Crowns {
            planted: occluders.crowns.clone(),
            broadleaf,
            shaded_at: 1.0,
        });
        return;
    };
    if !maps.is_some_and(|maps| maps.is_changed()) {
        return;
    }
    let Some(oak) = fields.years.iter().find(|year| year.kind == "oak") else {
        return;
    };
    let cache = cache.get_or_insert_with(|| PastCache::new(4));
    let time = clock.time();
    let past = fields.past_at(cache, occluders.middle, time);
    let hours = fields
        .fields
        .daylight(fields.frame.latitude(occluders.middle), time.day())
        .day_hours;
    let on = oak.leaves(&past, hours, 0.0).on;
    // As planted, in full leaf; again only once the leaves have come or gone far enough.
    if (on - crowns.shaded_at).abs() < RESHADE_BY {
        return;
    }
    crowns.shaded_at = on;
    let thin = BARE_CROWN + (1.0 - BARE_CROWN) * on;
    occluders.crowns = crowns
        .planted
        .iter()
        .zip(&crowns.broadleaf)
        .map(|(crown, broadleaf)| sky_occlusion::Crown {
            closes: crown.closes * if *broadleaf { thin } else { 1.0 },
            ..*crown
        })
        .collect();
}
