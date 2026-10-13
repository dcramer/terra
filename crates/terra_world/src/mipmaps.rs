//! Mipmaps for models' and materials' textures (docs/art-style.md, "No shimmer"): a texture loaded from a
//! model gets its chain of smaller copies, each half the last, and a filter that blends them
//! and looks across a surface seen at a slant, so wood grain and stone do not sparkle at a
//! distance. Bevy does not make mipmaps for a glTF's images; shipped games bake them into
//! compressed textures, which this stands in for until the asset pipeline does that.

use bevy::asset::AssetEvent;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// How far a texture is filtered along a slanted surface, at most.
const ANISOTROPY: u16 = 16;

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, mipmaps);
}

fn mipmaps(
    mut events: MessageReader<AssetEvent<Image>>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    for event in events.read() {
        let AssetEvent::LoadedWithDependencies { id } = event else {
            continue;
        };
        let from_a_model = assets.get_path(*id).is_some_and(|path| {
            path.path().starts_with("models") || path.path().starts_with("materials")
        });
        if !from_a_model {
            continue;
        }
        let Some(mut image) = images.get_mut(*id) else {
            continue;
        };
        if image.texture_descriptor.mip_level_count > 1 {
            continue;
        }
        let format = image.texture_descriptor.format;
        if !matches!(
            format,
            TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm
        ) {
            continue;
        }
        let size = image.texture_descriptor.size;
        let Some(data) = image.data.as_ref() else {
            continue;
        };
        // An array's layers each get their own chain, one after the next, as wgpu reads
        // them by default.
        let layer = (size.width * size.height * 4) as usize;
        let mut all = Vec::with_capacity(data.len() * 4 / 3 + 4);
        let mut levels = 1;
        for pixels in data.chunks_exact(layer) {
            let (chain, count) = chain(pixels, size.width, size.height);
            all.extend(chain);
            levels = count;
        }
        image.data = Some(all);
        image.texture_descriptor.mip_level_count = levels;
        let mut sampler = ImageSamplerDescriptor::linear();
        sampler.set_anisotropic_filter(ANISOTROPY);
        sampler.set_address_mode(bevy::image::ImageAddressMode::Repeat);
        image.sampler = ImageSampler::Descriptor(sampler);
    }
}

/// An RGBA image followed by each smaller copy of it, every pixel the average of the four
/// under it, down to one pixel; and how many levels that makes.
fn chain(data: &[u8], width: u32, height: u32) -> (Vec<u8>, u32) {
    let mut all = data.to_vec();
    let mut level = data.to_vec();
    let (mut w, mut h) = (width as usize, height as usize);
    let mut levels = 1;
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                for channel in 0..4 {
                    let mut sum = 0u32;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let sx = (x * 2 + dx).min(w - 1);
                        let sy = (y * 2 + dy).min(h - 1);
                        sum += level[(sy * w + sx) * 4 + channel] as u32;
                    }
                    next[(y * nw + x) * 4 + channel] = (sum / 4) as u8;
                }
            }
        }
        all.extend_from_slice(&next);
        level = next;
        (w, h) = (nw, nh);
        levels += 1;
    }
    (all, levels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_texture_halves_down_to_one_pixel_each_the_average_under_it() {
        // Four by two: a white half and a black half.
        let mut data = Vec::new();
        for _ in 0..2 {
            data.extend([
                255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255,
            ]);
        }
        let (chain, levels) = chain(&data, 4, 2);
        // 4x2, 2x1, 1x1.
        assert_eq!(levels, 3);
        assert_eq!(chain.len(), (8 + 2 + 1) * 4);
        assert_eq!(&chain[32..40], &[255, 255, 255, 255, 0, 0, 0, 255]);
        assert_eq!(&chain[40..44], &[127, 127, 127, 255]);
    }
}
