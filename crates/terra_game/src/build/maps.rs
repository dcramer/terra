//! The maps kits' pieces are made with on the CPU (D197, D200; `terra_build_mesh::relief::Maps`):
//! a block kind's `Masonry` material's height, which its surface is displaced by, and its
//! `Floor` material's, which its tops are; a timber kind's `Log` material's, which its bark
//! stands out by; and every material's colours, small, which a section far off is painted
//! with (its textures baked, D211). Until they have all loaded, what they shape is drawn
//! without them; then every build is drawn again.

use super::{BuildLook, Built, Site};
use bevy::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;
use terra_build::kit::Kits;
use terra_build_mesh::relief::{Maps, Relief, Swatch, cored_by, shaped_by, topped_by};

/// How many texels across and down a material's colours are kept, each the average of a
/// patch of its texture.
const SWATCH: usize = 16;
/// How many texels of the texture each swatch texel averages, across and down.
const SAMPLES: usize = 6;

/// The first channel of each of the first `texels` texels of an image's bytes, a byte a
/// channel and one to four channels a texel, however many it has, its first level only if
/// it has smaller ones after: the map is grey, so its red is its height.
fn first_channel(data: &[u8], texels: usize) -> Vec<u8> {
    let channels = (data.len() / texels.max(1)).clamp(1, 4);
    (0..texels).map(|i| data[i * channels]).collect()
}

/// A texture's colours averaged into a swatch, linear, from its first level's bytes, four
/// to a texel in sRGB; darkened by its occlusion where it has one (its `arm.jpg`'s red, a
/// byte a channel, four to a texel), as the texture is darkened in its creases near.
fn swatch(
    data: &[u8],
    width: usize,
    height: usize,
    occlusion: Option<(&[u8], usize, usize)>,
) -> Option<Swatch> {
    if data.len() < width * height * 4 {
        return None;
    }
    let occlusion = occlusion.filter(|(data, w, h)| data.len() >= w * h * 4);
    let linear = |byte: u8| Color::srgb_u8(byte, byte, byte).to_linear().red;
    let table: Vec<f32> = (0..=255).map(linear).collect();
    let mut colours = Vec::with_capacity(SWATCH * SWATCH);
    for sy in 0..SWATCH {
        for sx in 0..SWATCH {
            let mut sum = [0.0f32; 3];
            for j in 0..SAMPLES {
                for i in 0..SAMPLES {
                    let x = ((sx * SAMPLES + i) * width) / (SWATCH * SAMPLES);
                    let y = ((sy * SAMPLES + j) * height) / (SWATCH * SAMPLES);
                    let at = (y * width + x) * 4;
                    for c in 0..3 {
                        sum[c] += table[data[at + c] as usize];
                    }
                }
            }
            let mut shade = 1.0;
            if let Some((ao, w, h)) = occlusion {
                let mut total = 0.0;
                for j in 0..SAMPLES {
                    for i in 0..SAMPLES {
                        let x = ((sx * SAMPLES + i) * w) / (SWATCH * SAMPLES);
                        let y = ((sy * SAMPLES + j) * h) / (SWATCH * SAMPLES);
                        total += f32::from(ao[(y * w + x) * 4]) / 255.0;
                    }
                }
                shade = total / (SAMPLES * SAMPLES) as f32;
            }
            colours.push(sum.map(|c| c * shade / (SAMPLES * SAMPLES) as f32));
        }
    }
    Swatch::new(SWATCH, SWATCH, colours)
}

/// The maps being loaded: heights and colours, by folder.
#[derive(Resource, Default)]
pub(super) struct Loading {
    wanted: Option<(Vec<String>, Vec<String>)>,
    heights: BTreeMap<String, Handle<Image>>,
    colours: BTreeMap<String, Handle<Image>>,
    occlusion: BTreeMap<String, Handle<Image>>,
    done: bool,
}

/// The folders whose heights the kits are shaped by, and every folder they take colours
/// from.
fn wanted(kits: &Kits) -> (Vec<String>, Vec<String>) {
    let mut heights: Vec<String> = kits
        .0
        .iter()
        .flat_map(|kit| [shaped_by(kit), topped_by(kit), cored_by(kit)])
        .flatten()
        .map(str::to_string)
        .collect();
    heights.sort();
    heights.dedup();
    let mut colours: Vec<String> = kits
        .0
        .iter()
        .flat_map(|kit| kit.textures.values().map(|texture| texture.path.clone()))
        .collect();
    colours.sort();
    colours.dedup();
    (heights, colours)
}

/// Loads the maps the kits are made with, and when they are all ready draws every build
/// again with them.
pub(super) fn load(
    mut loading: ResMut<Loading>,
    mut look: ResMut<BuildLook>,
    assets: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
) {
    let paths = look.kits.as_deref().map(wanted).unwrap_or_default();
    if loading.wanted.as_ref() != Some(&paths) {
        // Read on the CPU only, as the texture arrays read them (D290): never sent to the GPU,
        // and from their own source, so never the same asset as the picture a material draws.
        let read = |path: String, srgb: bool| -> Handle<Image> {
            assets
                .load_builder()
                .with_settings(move |settings: &mut bevy::image::ImageLoaderSettings| {
                    settings.is_srgb = srgb;
                    settings.asset_usage = bevy::asset::RenderAssetUsages::MAIN_WORLD;
                })
                .load(terra_world::data::cpu_only(&path))
        };
        let linear = |path: String| read(path, false);
        loading.heights = paths
            .0
            .iter()
            .map(|path| (path.clone(), linear(format!("{path}/height.jpg"))))
            .collect();
        // The same image the kit's material shows, as it is loaded the same way.
        loading.colours = paths
            .1
            .iter()
            .map(|path| (path.clone(), read(format!("{path}/color.jpg"), true)))
            .collect();
        loading.occlusion = paths
            .1
            .iter()
            .map(|path| (path.clone(), linear(format!("{path}/arm.jpg"))))
            .collect();
        loading.wanted = Some(paths);
        loading.done = false;
        return;
    }
    if loading.done || (loading.heights.is_empty() && loading.colours.is_empty()) {
        return;
    }
    let ready = |handle: &Handle<Image>| {
        images.get(handle).is_some()
            || matches!(
                assets.get_load_state(handle),
                Some(bevy::asset::LoadState::Failed(_))
            )
    };
    if !loading
        .heights
        .values()
        .chain(loading.colours.values())
        .chain(loading.occlusion.values())
        .all(ready)
    {
        return;
    }
    let mut maps = Maps::default();
    for (path, handle) in &loading.heights {
        let Some(image) = images.get(handle) else {
            error!("{path}/height.jpg did not load");
            continue;
        };
        let Some(data) = image.data.as_ref() else {
            continue;
        };
        let size = image.texture_descriptor.size;
        let (width, height) = (size.width as usize, size.height as usize);
        let Some(relief) = Relief::new(width, height, first_channel(data, width * height)) else {
            error!("{path}/height.jpg is not a height map");
            continue;
        };
        info!("Kits' pieces are shaped by {path}'s height, {width} by {height}.");
        maps.heights.insert(path.clone(), Arc::new(relief));
    }
    for (path, handle) in &loading.colours {
        let Some(image) = images.get(handle) else {
            continue;
        };
        let size = image.texture_descriptor.size;
        let occlusion = loading
            .occlusion
            .get(path)
            .and_then(|handle| images.get(handle))
            .and_then(|arm| {
                let size = arm.texture_descriptor.size;
                arm.data
                    .as_deref()
                    .map(|data| (data, size.width as usize, size.height as usize))
            });
        let Some(swatch) = image
            .data
            .as_ref()
            .and_then(|data| swatch(data, size.width as usize, size.height as usize, occlusion))
        else {
            warn!("{path}/color.jpg could not be read for building's far colours");
            continue;
        };
        maps.colours.insert(path.clone(), Arc::new(swatch));
    }
    loading.done = true;
    // Their pictures are let go: what was needed of them is in the maps.
    loading.heights.clear();
    loading.colours.clear();
    loading.occlusion.clear();
    look.maps = Arc::new(maps);
    for build in super::store::each(&mut site, &mut builds) {
        if build.kits.is_some() {
            build.redraw_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_build::Material;
    use terra_build::kit::Kit;

    #[test]
    fn a_height_is_read_from_the_first_channel_however_many_there_are() {
        // Two texels, heights 10 and 20, in each layout an image may come in.
        let heights = vec![10, 20];
        assert_eq!(first_channel(&[10, 20], 2), heights);
        assert_eq!(first_channel(&[10, 99, 20, 99], 2), heights);
        assert_eq!(first_channel(&[10, 99, 98, 20, 99, 98], 2), heights);
        assert_eq!(first_channel(&[10, 99, 98, 97, 20, 99, 98, 97], 2), heights);
        // Smaller levels after the first are left alone: four texels of one channel and a
        // level of one, and four of four channels and a level of one.
        assert_eq!(first_channel(&[1, 2, 3, 4, 5], 4), vec![1, 2, 3, 4]);
        let mut rgba: Vec<u8> = (1..=4).flat_map(|h| [h, 99, 98, 97]).collect();
        rgba.extend([5, 99, 98, 97]);
        assert_eq!(first_channel(&rgba, 4), vec![1, 2, 3, 4]);
    }

    #[test]
    fn a_swatch_averages_its_texture_linear() {
        // Half white and half black, left and right: grey in the middle, linear.
        let (width, height) = (64, 64);
        let data: Vec<u8> = (0..width * height)
            .flat_map(|i| {
                let v = if i % width < width / 2 { 255 } else { 0 };
                [v, v, v, 255]
            })
            .collect();
        let made = swatch(&data, width, height, None).unwrap();
        assert_eq!(made.at([0.1, 0.5]), [1.0; 3]);
        assert_eq!(made.at([0.9, 0.5]), [0.0; 3]);
        assert!(swatch(&data[..100], width, height, None).is_none());
        // Its occlusion darkens it: half shaded all over.
        let shaded = vec![128u8; width * height * 4];
        let dark = swatch(&data, width, height, Some((&shaded, width, height))).unwrap();
        assert!((dark.at([0.1, 0.5])[0] - 128.0 / 255.0).abs() < 1e-5);
    }

    #[test]
    fn a_kit_is_shaped_by_its_stone_not_its_bark_and_coloured_by_all_its_textures() {
        let texture = |path: &str| terra_build::kit::Texture {
            path: path.into(),
            metres: [1.0, 1.0],
            depth: 0.0,
            reflectance: terra_build::kit::matte(),
            strips: None,
            cells: 1,
            worn: None,
        };
        let stone = Kit {
            pattern: terra_build::kit::Pattern::Masonry,
            relief: 0.02,
            textures: BTreeMap::from([(Material::Masonry, texture("materials/fieldstone"))]),
            ..Kit::default()
        };
        assert_eq!(shaped_by(&stone), Some("materials/fieldstone"));
        let mut smooth = stone.clone();
        smooth.relief = 0.0;
        assert_eq!(shaped_by(&smooth), None);
        // A log's bark stands out by its parallax, which its relief sets (D284).
        let pine = Kit {
            pattern: terra_build::kit::Pattern::Logs,
            textures: BTreeMap::from([(Material::Log, texture("materials/pine_bark"))]),
            ..Kit::default()
        };
        assert!(pine.timber.relief > 0.0);
        assert_eq!(shaped_by(&pine), None);
        assert_eq!(pine.depth(Material::Log), pine.timber.relief);
        let (heights, colours) = wanted(&Kits(vec![stone.clone(), stone, pine]));
        assert_eq!((heights.len(), colours.len()), (1, 2));
    }
}
