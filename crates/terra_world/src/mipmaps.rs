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

/// The pictures kept loaded after the world that asked for them is left, in bytes of their
/// pixels at most: a gigabyte, some worlds' worth of bark, ground and model textures.
const KEPT_BYTES: usize = 1 << 30;
/// And so many things at most, pictures and parts together: some worlds' worth, and a bound
/// on what could otherwise only grow.
const KEPT_MOST: usize = 16_384;

/// What a world loaded from model files, kept loaded for the worlds entered after (D501):
/// its pictures, and its models' meshes and materials. A world left drops its handles, and
/// the next, asking for the same rock's mesh by its label, had its whole file read again,
/// its pictures replaced and their mipmaps made again: a quarter to two thirds of a second
/// of every crossing's cover. The oldest are let go past [`KEPT_BYTES`] of pictures.
#[derive(Resource, Default)]
struct Kept {
    held: std::collections::VecDeque<(bevy::asset::UntypedHandle, usize)>,
    bytes: usize,
}

impl Kept {
    /// Keeps a picture, or the file it is part of (a model's textures are parts of its
    /// glTF, read again whole if the file itself is let go), once.
    fn keep(&mut self, held: bevy::asset::UntypedHandle, bytes: usize) {
        if self.held.iter().any(|(kept, _)| kept.id() == held.id()) {
            return;
        }
        self.held.push_back((held, bytes));
        self.bytes += bytes;
        while (self.bytes > KEPT_BYTES || self.held.len() > KEPT_MOST) && self.held.len() > 1 {
            if let Some((_, gone)) = self.held.pop_front() {
                self.bytes -= gone;
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    // Run as the image's arrival is announced, in the frame it arrived, so it is made on the
    // GPU once with its mipmaps, never first without them and then again (D420).
    app.init_resource::<Kept>().add_systems(
        PostUpdate,
        (
            mipmaps,
            keep_parts::<Mesh>,
            keep_parts::<StandardMaterial>,
            keep_parts::<bevy::gltf::GltfMaterial>,
            keep_parts::<bevy::gltf::GltfMesh>,
        )
            .after(bevy::asset::AssetEventSystems),
    );
}

/// A model file's parts are kept as its pictures are ([`Kept`]): its meshes and materials,
/// and the glTF's own records of them, which is what a label such as `Material0` names. With
/// any part of a file let go, asking for it again reads the whole file again, and every
/// part still held is replaced.
fn keep_parts<A: Asset>(
    mut events: MessageReader<AssetEvent<A>>,
    assets: Res<AssetServer>,
    mut kept: ResMut<Kept>,
) {
    for event in events.read() {
        let AssetEvent::Added { id } = event else {
            continue;
        };
        let of_a_model = assets
            .get_path(*id)
            .is_some_and(|path| path.label().is_some() && path.path().starts_with("models"));
        if let (true, Some(part)) = (of_a_model, assets.get_id_handle(*id)) {
            kept.keep(part.untyped(), 0);
        }
    }
}

fn mipmaps(
    mut events: MessageReader<AssetEvent<Image>>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut kept: ResMut<Kept>,
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
        // Kept loaded for the next world, whichever way its mipmaps come.
        let bytes = image.data.as_ref().map_or(0, Vec::len);
        if let (AssetEvent::Added { .. }, Some(picture)) = (event, assets.get_id_handle(*id)) {
            kept.keep(picture.untyped(), bytes);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// What is kept is bounded (D501's memory rule): a thing is kept once however often it
    /// is seen, and the oldest are let go past the bytes of pictures or the count of things.
    #[test]
    fn what_is_kept_is_kept_once_and_the_oldest_let_go_past_its_bounds() {
        let images = Assets::<Image>::default();
        let mut kept = Kept::default();
        let first = images.reserve_handle().untyped();
        kept.keep(first.clone(), KEPT_BYTES / 2);
        kept.keep(first.clone(), KEPT_BYTES / 2);
        assert_eq!((kept.held.len(), kept.bytes), (1, KEPT_BYTES / 2));
        kept.keep(images.reserve_handle().untyped(), KEPT_BYTES / 2);
        kept.keep(images.reserve_handle().untyped(), KEPT_BYTES / 2);
        assert_eq!(
            kept.held.len(),
            2,
            "the oldest picture is let go past the bytes"
        );
        assert!(kept.bytes <= KEPT_BYTES);
        assert!(kept.held.iter().all(|(held, _)| held.id() != first.id()));
        // Parts weigh nothing and are bounded by their number.
        for _ in 0..KEPT_MOST + 10 {
            kept.keep(images.reserve_handle().untyped(), 0);
        }
        assert_eq!(kept.held.len(), KEPT_MOST);
    }
}
