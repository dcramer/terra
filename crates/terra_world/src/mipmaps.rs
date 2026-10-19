//! Mipmaps for models' and materials' textures (docs/art-style.md, "No shimmer"): a texture loaded from a
//! model gets its chain of smaller copies, each half the last, and a filter that blends them
//! and looks across a surface seen at a slant, so wood grain and stone do not sparkle at a
//! distance. Bevy does not make mipmaps for a glTF's images; shipped games bake them into
//! compressed textures, as the bake does (`terra_bake`, D188); this makes them for a
//! picture read as it was made, where none is cooked or the GPU samples no BC. A cooked
//! picture comes with its mipmaps and gets the filter alone.

use bevy::asset::AssetEvent;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// How far a texture is filtered along a slanted surface, at most.
const ANISOTROPY: u16 = 16;

pub use terra_core::cooked::mips::CUT_OUT;
use terra_core::cooked::mips::{chain, cut_out_chain, is_cut_out};

pub fn plugin(app: &mut App) {
    // Run as the image's arrival is announced, in the frame it arrived, so it is made on the
    // GPU once with its mipmaps, never first without them and then again (D420).
    app.add_systems(PostUpdate, mipmaps.after(bevy::asset::AssetEventSystems));
}

fn mipmaps(
    mut events: MessageReader<AssetEvent<Image>>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    for event in events.read() {
        // Loaded, or loaded again as its file changed.
        let (AssetEvent::Added { id } | AssetEvent::Modified { id }) = event else {
            continue;
        };
        let from_a_model = assets.get_path(*id).is_some_and(|path| {
            path.source() == &crate::cooked::SOURCE.into()
                || path.path().starts_with("models")
                || path.path().starts_with("materials")
        });
        if !from_a_model {
            continue;
        }
        let cooked = images
            .get(*id)
            .is_some_and(|image| image.texture_descriptor.format.is_compressed());
        // Changed unannounced: it has not been sent to the GPU yet, and an announced change
        // would send it again.
        let Some(image) = images.get_mut_untracked(*id) else {
            continue;
        };
        if cooked {
            image.sampler = filter();
            continue;
        }
        // A picture read on the CPU only is never sampled, so needs none.
        if image.texture_descriptor.mip_level_count > 1
            || !image
                .asset_usage
                .contains(bevy::asset::RenderAssetUsages::RENDER_WORLD)
        {
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
        let srgb = format == TextureFormat::Rgba8UnormSrgb;
        for pixels in data.chunks_exact(layer) {
            let cut_out = is_cut_out(pixels, srgb);
            let (chain, count) = if cut_out {
                cut_out_chain(pixels, size.width, size.height, CUT_OUT, srgb)
            } else {
                chain(pixels, size.width, size.height)
            };
            all.extend(chain);
            levels = count;
        }
        image.data = Some(all);
        image.texture_descriptor.mip_level_count = levels;
        image.sampler = filter();
    }
}

/// Blends a texture's mipmaps, looks across a slanted surface and repeats.
fn filter() -> ImageSampler {
    let mut sampler = ImageSamplerDescriptor::linear();
    sampler.set_anisotropic_filter(ANISOTROPY);
    sampler.set_address_mode(bevy::image::ImageAddressMode::Repeat);
    ImageSampler::Descriptor(sampler)
}
