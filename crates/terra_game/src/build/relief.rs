//! The relief of block kinds' surfaces (D197, `terra_build::surface`): the height map of
//! the kit's `Masonry` material, kept on the CPU, as the surface is displaced by it when it
//! is laid. Until it has loaded, block kinds are drawn rounded but flat; then every build is
//! drawn again.

use super::{BuildLook, Built, Site, redraw};
use crate::planet::Planet;
use bevy::prelude::*;
use std::sync::Arc;
use terra_build::Material;

/// A material's height, a byte a texel, row by row from the top, as its texture is read.
#[derive(Debug)]
pub struct Relief {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

impl Relief {
    /// The height, from 0 to 1, where the texture is at a point, repeating, between texels.
    pub fn at(&self, uv: [f32; 2]) -> f32 {
        let x = uv[0].rem_euclid(1.0) * self.width as f32 - 0.5;
        let y = uv[1].rem_euclid(1.0) * self.height as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let texel = |i: f32, j: f32| {
            let i = (i as i64).rem_euclid(self.width as i64) as usize;
            let j = (j as i64).rem_euclid(self.height as i64) as usize;
            self.data[j * self.width + i] as f32 / 255.0
        };
        let top = texel(x0, y0) * (1.0 - fx) + texel(x0 + 1.0, y0) * fx;
        let bottom = texel(x0, y0 + 1.0) * (1.0 - fx) + texel(x0 + 1.0, y0 + 1.0) * fx;
        top * (1.0 - fy) + bottom * fy
    }
}

/// The height map being loaded, and the path it is for.
#[derive(Resource, Default)]
pub(super) struct Loading {
    wanted: Option<String>,
    handle: Option<Handle<Image>>,
    done: bool,
}

/// Loads the block kinds' height map when the kits name one, and when it is ready draws
/// every build again with it.
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
    let wanted = look.kits.as_deref().and_then(|kits| {
        kits.0
            .iter()
            .find_map(|kit| kit.textures.get(&Material::Masonry))
            .map(|texture| format!("{}/height.jpg", texture.path))
    });
    if loading.wanted != wanted {
        loading.handle = wanted.as_ref().map(|path| {
            assets
                .load_builder()
                .with_settings(|settings: &mut bevy::image::ImageLoaderSettings| {
                    settings.is_srgb = false;
                })
                .load(path.clone())
        });
        loading.wanted = wanted;
        loading.done = false;
        return;
    }
    if loading.done {
        return;
    }
    let Some(image) = loading
        .handle
        .as_ref()
        .and_then(|handle| images.get(handle))
    else {
        return;
    };
    let Some(data) = image.data.as_ref() else {
        return;
    };
    let size = image.texture_descriptor.size;
    let (width, height) = (size.width as usize, size.height as usize);
    // The first level, its red channel: the map is grey.
    let texels = width * height;
    let channels = (data.len() / texels.max(1)).clamp(1, 4).min(4);
    let channels = if data.len() >= texels * 4 {
        4
    } else {
        channels
    };
    let relief = Arc::new(Relief {
        width,
        height,
        data: (0..texels).map(|i| data[i * channels]).collect(),
    });
    loading.done = true;
    info!("Block kinds are displaced by a height map {width} by {height}.");
    look.relief = Some(relief.clone());
    for build in
        std::iter::once(&mut site.build).chain(builds.iter_mut().map(|b| &mut b.into_inner().0))
    {
        if build.kits.is_some() {
            build.relief = Some(relief.clone());
            redraw(&mut commands, build, &planet, &mut meshes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_relief_repeats_and_blends_between_texels() {
        let relief = Relief {
            width: 2,
            height: 1,
            data: vec![0, 255],
        };
        // Texel centres are a quarter and three quarters across.
        assert!((relief.at([0.25, 0.5]) - 0.0).abs() < 1e-5);
        assert!((relief.at([0.75, 0.5]) - 1.0).abs() < 1e-5);
        assert!((relief.at([0.5, 0.5]) - 0.5).abs() < 1e-5);
        assert!((relief.at([1.25, 0.5]) - relief.at([0.25, 0.5])).abs() < 1e-5);
    }
}
