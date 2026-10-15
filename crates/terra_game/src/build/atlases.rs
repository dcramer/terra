//! The pieces' atlases block compressed as they load (D290): the one texture a pack's own
//! modelled pieces (and a pattern's shared ones) are read from by their own UVs, too large to
//! be a layer of the texture arrays, is a material of its own (`draw::KitMaterials`). Read
//! from its pictures it would sit on the GPU uncompressed and without mipmaps, 16 MiB a map
//! at 2,048 square; so once it has loaded, each map is compressed on a worker with its
//! mipmaps, in the arrays' formats (`arrays::bc`: BC7 colour, BC1 occlusion-roughness-metal,
//! BC5 normal, BC4 height), and its material made to read the compressed copy, as shipped
//! games ship their textures compressed. Where the GPU samples no BC they stay as read.

use super::arrays::bc::{self, Codec};
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        renderer::RenderDevice,
    },
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use std::collections::BTreeMap;

/// What a map of a material is, for its format.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Map {
    Colour,
    Normal,
    Arm,
    Height,
}

impl Map {
    fn format(self) -> (TextureFormat, Codec) {
        match self {
            Map::Colour => (TextureFormat::Bc7RgbaUnormSrgb, Codec::Bc7),
            Map::Normal => (TextureFormat::Bc5RgUnorm, Codec::Bc5),
            Map::Arm => (TextureFormat::Bc1RgbaUnorm, Codec::Bc1),
            Map::Height => (TextureFormat::Bc4RUnorm, Codec::Bc4),
        }
    }

    /// How its smaller copies are averaged.
    fn mips(self) -> bc::Mips {
        match self {
            Map::Colour => bc::Mips::Srgb,
            Map::Normal => bc::Mips::Normal,
            Map::Arm | Map::Height => bc::Mips::Linear,
        }
    }
}

enum State {
    Encoding(Task<Option<Image>>),
    Made(Handle<Image>),
    /// Not a size it can be compressed at, or it failed: left as read.
    Left,
}

/// Each atlas map read, by the picture it was read from: being compressed, or its
/// compressed copy. Cleared when the kits' materials are made again.
#[derive(Resource, Default)]
pub(super) struct Atlases {
    maps: BTreeMap<AssetId<Image>, State>,
    /// The kits' materials these are of, by address.
    of: usize,
}

/// An RGBA picture's mipmaps, each compressed, as one image to sample.
fn compressed(pixels: Vec<u8>, side: u32, map: Map, sampler: bevy::image::ImageSampler) -> Image {
    let (format, codec) = map.format();
    let levels = bc::chain(&pixels, side, map.mips());
    let count = levels.len() as u32;
    let data: Vec<u8> = levels
        .iter()
        .enumerate()
        .flat_map(|(level, pixels)| codec.encode(pixels, side >> level))
        .collect();
    let mut image = Image::new_uninit(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        format,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    image.texture_descriptor.mip_level_count = count;
    image.data = Some(data);
    image.sampler = sampler;
    image
}

/// Compresses each atlas map once it has loaded, and points its material at the copy.
pub(super) fn compress(
    look: Res<super::BuildLook>,
    mut atlases: ResMut<Atlases>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    device: Option<Res<RenderDevice>>,
) {
    let samples_bc = device.is_some_and(|device| {
        device
            .features()
            .contains(bevy::render::render_resource::WgpuFeatures::TEXTURE_COMPRESSION_BC)
    });
    if !samples_bc {
        return;
    }
    let of = std::sync::Arc::as_ptr(&look.materials) as usize;
    if atlases.of != of {
        *atlases = Atlases { of, ..default() };
    }
    let handles: Vec<Handle<StandardMaterial>> = look.materials.all().cloned().collect();
    let mut swaps = false;
    for handle in &handles {
        let Some(material) = materials.get(handle) else {
            continue;
        };
        for (image, map) in [
            (&material.base_color_texture, Map::Colour),
            (&material.normal_map_texture, Map::Normal),
            (&material.metallic_roughness_texture, Map::Arm),
            (&material.occlusion_texture, Map::Arm),
            (&material.depth_map, Map::Height),
        ] {
            let Some(image) = image else { continue };
            let id = image.id();
            if let Some(state) = atlases.maps.get_mut(&id) {
                if let State::Encoding(task) = state
                    && let Some(made) = block_on(poll_once(task))
                {
                    *state = match made {
                        Some(made) => State::Made(images.add(made)),
                        None => State::Left,
                    };
                }
                swaps |= matches!(state, State::Made(_));
                continue;
            }
            // A copy made already, a map not loaded yet, or already compressed.
            if atlases
                .maps
                .values()
                .any(|state| matches!(state, State::Made(made) if made.id() == id))
            {
                continue;
            }
            let Some(read) = images.get(id) else { continue };
            if read.texture_descriptor.format.is_compressed() {
                continue;
            }
            let size = read.texture_descriptor.size;
            let side = size.width;
            let pixels = super::arrays::pixels(read);
            let sampler = read.sampler.clone();
            let state = match pixels {
                Some((data, w, h, 4))
                    if w == h && side.is_power_of_two() && side >= 4 && w == side =>
                {
                    State::Encoding(
                        AsyncComputeTaskPool::get()
                            .spawn(async move { Some(compressed(data, side, map, sampler)) }),
                    )
                }
                _ => State::Left,
            };
            atlases.maps.insert(id, state);
        }
    }
    if !swaps {
        return;
    }
    // Every material reading a map that has its copy made reads the copy.
    let made: BTreeMap<AssetId<Image>, Handle<Image>> = atlases
        .maps
        .iter()
        .filter_map(|(id, state)| match state {
            State::Made(handle) => Some((*id, handle.clone())),
            _ => None,
        })
        .collect();
    for handle in &handles {
        let reads = |material: &StandardMaterial| {
            [
                &material.base_color_texture,
                &material.normal_map_texture,
                &material.metallic_roughness_texture,
                &material.occlusion_texture,
                &material.depth_map,
            ]
            .into_iter()
            .flatten()
            .any(|image| made.contains_key(&image.id()))
        };
        if !materials.get(handle).is_some_and(reads) {
            continue;
        }
        let Some(mut material) = materials.get_mut(handle) else {
            continue;
        };
        let material = &mut *material;
        for image in [
            &mut material.base_color_texture,
            &mut material.normal_map_texture,
            &mut material.metallic_roughness_texture,
            &mut material.occlusion_texture,
            &mut material.depth_map,
        ]
        .into_iter()
        .flatten()
        {
            if let Some(copy) = made.get(&image.id()) {
                *image = copy.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_atlas_map_is_compressed_with_every_mipmap_to_a_quarter_or_less() {
        let side = 64;
        let pixels: Vec<u8> = (0..side * side * 4).map(|i| (i * 7 % 251) as u8).collect();
        for map in [Map::Colour, Map::Normal, Map::Arm, Map::Height] {
            let image = compressed(pixels.clone(), side, map, default());
            let levels = image.texture_descriptor.mip_level_count;
            assert_eq!(levels, bc::levels(side));
            let codec = map.format().1;
            let expected: usize = (0..levels).map(|l| codec.bytes(side >> l)).sum();
            let data = image.data.as_ref().map_or(0, Vec::len);
            assert_eq!(data, expected, "{map:?}");
            // A quarter of its mipmaps uncompressed, four bytes a texel, or less.
            let raw: usize = (0..levels).map(|l| Codec::Raw(4).bytes(side >> l)).sum();
            assert!(data * 4 <= raw, "{map:?}");
            assert_eq!(
                super::super::memory::texture_bytes(&image),
                data,
                "{map:?}: what the memory report counts"
            );
        }
    }
}
