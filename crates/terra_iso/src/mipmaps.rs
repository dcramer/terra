//! Mipmaps for every texture loaded from a file, as the game makes them (ported from
//! `terra_world::mipmaps`): Bevy makes none for a JPEG or a glTF's images, and a far view
//! of stone and plaster sparkles without them.

use bevy::asset::{AssetEvent, RenderAssetUsages};
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use terra_core::cooked::mips::{CUT_OUT, chain, cut_out_chain, is_cut_out};

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, mipmaps.after(bevy::asset::AssetEventSystems));
}

fn mipmaps(
    mut events: MessageReader<AssetEvent<Image>>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    for event in events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id }) = event else {
            continue;
        };
        if assets.get_path(*id).is_none() {
            continue;
        }
        let Some(image) = images.get_mut_untracked(*id) else {
            continue;
        };
        let format = image.texture_descriptor.format;
        if image.texture_descriptor.mip_level_count > 1
            || !image.asset_usage.contains(RenderAssetUsages::RENDER_WORLD)
            || !matches!(
                format,
                TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm
            )
        {
            continue;
        }
        let size = image.texture_descriptor.size;
        let Some(data) = image.data.as_ref() else {
            continue;
        };
        let layer = (size.width * size.height * 4) as usize;
        let mut all = Vec::with_capacity(data.len() * 4 / 3 + 4);
        let mut levels = 1;
        let srgb = format == TextureFormat::Rgba8UnormSrgb;
        for pixels in data.chunks_exact(layer) {
            let (mips, count) = if is_cut_out(pixels, srgb) {
                cut_out_chain(pixels, size.width, size.height, CUT_OUT, srgb)
            } else {
                chain(pixels, size.width, size.height)
            };
            all.extend(mips);
            levels = count;
        }
        image.data = Some(all);
        image.texture_descriptor.mip_level_count = levels;
        let mut sampler = ImageSamplerDescriptor::linear();
        sampler.set_anisotropic_filter(16);
        sampler.set_address_mode(ImageAddressMode::Repeat);
        image.sampler = ImageSampler::Descriptor(sampler);
    }
}
