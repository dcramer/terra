//! A texture set read and compressed on the workers (`Arrays::want`), as texture streaming
//! reads and decodes off the game's thread: its pictures read and decoded there too, not
//! through the asset server, whose images were copied out on the main thread (perf round 11).

use super::{Codec, Encoded, MIPS, bc, cache, to_side};
use bevy::{asset::RenderAssetUsages, prelude::*};
use terra_build_mesh::cells::CELLS;

/// A picture as read: its first level's bytes, its size and channels a texel.
pub type Pixels = (Vec<u8>, u32, u32, usize);

/// A picture read and decoded as Bevy's image loader does (`ImageLoader::load`), but on the
/// worker that compresses it. Read through the asset server, its bytes went from the loader
/// into `Assets<Image>` and were copied out on the main thread, 16 MiB a picture at 2,048,
/// four a set: up to 28 ms of a frame while playing and 130 ms in the load; dropped, the
/// pictures were freed by `track_assets` holding the asset server's lock, 6 ms in each of
/// its systems. None, said, where it cannot be.
pub(super) async fn picture(assets: &AssetServer, file: &str) -> Option<Pixels> {
    use bevy::asset::io::Reader;
    let source = assets
        .get_source(bevy::asset::io::AssetSourceId::Default)
        .ok()?;
    let mut bytes = Vec::new();
    let read = match source.reader().read(std::path::Path::new(file)).await {
        Ok(mut reader) => reader.read_to_end(&mut bytes).await.map(|_| ()),
        Err(error) => Err(std::io::Error::other(error.to_string())),
    };
    if let Err(error) = read {
        error!("Build: {file} could not be read: {error}");
        return None;
    }
    let extension = file.rsplit('.').next().unwrap_or_default();
    match Image::from_buffer(
        &bytes,
        bevy::image::ImageType::Extension(extension),
        bevy::image::CompressedImageFormats::NONE,
        false,
        bevy::image::ImageSampler::Default,
        RenderAssetUsages::MAIN_WORLD,
    ) {
        Ok(image) => pixels(&image),
        Err(error) => {
            error!("Build: {file} could not be decoded: {error}");
            None
        }
    }
}

/// A texture set's pictures as read (`picture`), compressed for the arrays at their sides,
/// each with its mipmaps, and its cells; read back instead where it was compressed before.
pub(super) fn encode(
    maps: Vec<Option<Pixels>>,
    sides: [u32; 4],
    codecs: [Codec; 4],
    share: f32,
) -> Encoded {
    // Compressed before and kept: read back (`cache`).
    let kept = cache::file(cache::key(&maps, sides, codecs, share));
    if let Some((maps, cells)) = kept.as_deref().and_then(cache::read) {
        return Encoded {
            maps,
            cells,
            kept: true,
        };
    }
    let mut out: [Vec<Vec<u8>>; 4] = Default::default();
    let mut found = Vec::new();
    for (i, map) in maps.into_iter().enumerate() {
        // Its stones or shingles, from its height as read (D305), as the CPU finds them from
        // the same picture; none, one cell.
        if i == 3 {
            found = match &map {
                Some((data, w, h, channels)) => {
                    terra_build_mesh::cells::label(data, *w as usize, *h as usize, *channels, share)
                }
                None => [128u8, 128, 128, 255].repeat((CELLS * CELLS) as usize),
            };
        }
        let side = sides[i];
        let base = match map {
            Some((data, w, h, channels)) => to_side(&data, w, h, channels, side),
            // No height: flat.
            None => vec![0u8; (side * side * 4) as usize],
        };
        out[i] = bc::chain(&base, side, MIPS[i])
            .iter()
            .enumerate()
            .map(|(level, pixels)| codecs[i].encode(pixels, side >> level))
            .collect();
    }
    let stored = (out, found);
    if let Some(path) = &kept {
        cache::write(path, &stored);
    }
    Encoded {
        maps: stored.0,
        cells: stored.1,
        kept: false,
    }
}

/// A loaded picture's first level's bytes, its size and channels a texel.
pub fn pixels(image: &Image) -> Option<Pixels> {
    let size = image.texture_descriptor.size;
    let data = image.data.as_ref()?;
    let texels = (size.width * size.height) as usize;
    let channels = image
        .texture_descriptor
        .format
        .block_copy_size(None)
        .map_or(4, |b| b as usize)
        .clamp(1, 4);
    let bytes = texels * channels;
    (data.len() >= bytes).then(|| (data[..bytes].to_vec(), size.width, size.height, channels))
}
