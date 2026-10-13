//! The height maps kits' pieces are shaped by on the CPU (D197, D200): a block kind's
//! `Masonry` material, which its surface is displaced by, and a timber kind's `Log`
//! material, which its bark stands out by (`terra_build::relief`). Until one has loaded,
//! what it shapes is drawn without it; then every build is drawn again.

use super::{BuildLook, Built, Site, redraw};
use crate::planet::Planet;
use bevy::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;
use terra_build::Material;
use terra_build::kit::{Kit, Kits, Pattern};
use terra_build::relief::Relief;

/// The height maps read, by the folder of the material they belong to.
pub type Reliefs = BTreeMap<String, Arc<Relief>>;

/// The folder of the material whose height shapes a kit's pieces, if any does: a block
/// kind's stone, or a timber kind's bark.
pub fn shaped_by(kit: &Kit) -> Option<&str> {
    let (material, depth) = match kit.pattern {
        Pattern::Masonry => (Material::Masonry, kit.relief),
        Pattern::Logs => (Material::Log, kit.timber.relief),
    };
    (depth > 0.0)
        .then(|| kit.textures.get(&material))
        .flatten()
        .map(|texture| texture.path.as_str())
}

/// The height map that shapes a kit's pieces, once it has loaded.
pub fn of<'a>(reliefs: Option<&'a Reliefs>, kit: &Kit) -> Option<&'a Relief> {
    let path = shaped_by(kit)?;
    reliefs?.get(path).map(|relief| relief.as_ref())
}

/// The first channel of each of the first `texels` texels of an image's bytes, a byte a
/// channel and one to four channels a texel, however many it has, its first level only if
/// it has smaller ones after: the map is grey, so its red is its height.
fn first_channel(data: &[u8], texels: usize) -> Vec<u8> {
    let channels = (data.len() / texels.max(1)).clamp(1, 4);
    (0..texels).map(|i| data[i * channels]).collect()
}

/// The height maps being loaded, by folder.
#[derive(Resource, Default)]
pub(super) struct Loading {
    wanted: Vec<String>,
    handles: BTreeMap<String, Handle<Image>>,
    done: bool,
}

fn wanted(kits: &Kits) -> Vec<String> {
    let mut paths: Vec<String> = kits
        .0
        .iter()
        .filter_map(shaped_by)
        .map(str::to_string)
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// Loads the height maps the kits are shaped by, and when they are all ready draws every
/// build again with them.
#[allow(clippy::too_many_arguments)]
pub(super) fn load(
    mut loading: ResMut<Loading>,
    mut look: ResMut<BuildLook>,
    assets: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut commands: Commands,
    planet: Res<Planet>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let paths = look.kits.as_deref().map(wanted).unwrap_or_default();
    if loading.wanted != paths {
        loading.handles = paths
            .iter()
            .map(|path| {
                let handle = assets
                    .load_builder()
                    .with_settings(|settings: &mut bevy::image::ImageLoaderSettings| {
                        settings.is_srgb = false;
                    })
                    .load(format!("{path}/height.jpg"));
                (path.clone(), handle)
            })
            .collect();
        loading.wanted = paths;
        loading.done = false;
        return;
    }
    if loading.done || loading.handles.is_empty() {
        return;
    }
    let mut reliefs = Reliefs::new();
    for (path, handle) in &loading.handles {
        let Some(image) = images.get(handle) else {
            return;
        };
        let Some(data) = image.data.as_ref() else {
            return;
        };
        let size = image.texture_descriptor.size;
        let (width, height) = (size.width as usize, size.height as usize);
        let Some(relief) = Relief::new(width, height, first_channel(data, width * height)) else {
            error!("{path}/height.jpg is not a height map");
            continue;
        };
        info!("Kits' pieces are shaped by {path}'s height, {width} by {height}.");
        reliefs.insert(path.clone(), Arc::new(relief));
    }
    loading.done = true;
    let reliefs = Arc::new(reliefs);
    look.reliefs = Some(reliefs.clone());
    for build in
        std::iter::once(&mut site.build).chain(builds.iter_mut().map(|b| &mut b.into_inner().0))
    {
        if build.kits.is_some() {
            build.reliefs = Some(reliefs.clone());
            redraw(&mut commands, build, &planet, &mut meshes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn a_kit_is_shaped_by_its_stone_or_its_bark() {
        let pine = Kit {
            textures: BTreeMap::from([(
                Material::Log,
                terra_build::kit::Texture {
                    path: "materials/pine_bark".into(),
                    metres: [1.0, 1.0],
                    depth: 0.0,
                    reflectance: terra_build::kit::matte(),
                },
            )]),
            ..Kit::default()
        };
        assert_eq!(shaped_by(&pine), Some("materials/pine_bark"));
        let mut smooth = pine.clone();
        smooth.timber.relief = 0.0;
        assert_eq!(shaped_by(&smooth), None);
        assert_eq!(wanted(&Kits(vec![pine.clone(), pine])).len(), 1);
    }
}
