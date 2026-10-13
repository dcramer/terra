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

/// Where a picture cut out by its transparency, such as a card of leaves, is cut: more
/// opaque than this is leaf. Its mipmaps keep as much above it as the full picture has.
pub const CUT_OUT: f32 = 0.45;

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
        let srgb = format == TextureFormat::Rgba8UnormSrgb;
        for pixels in data.chunks_exact(layer) {
            // A picture with transparency is a cut-out, whose leaves must not thin away.
            let cut_out = pixels.chunks_exact(4).any(|p| p[3] < 255);
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

/// The chain for a picture cut out by its transparency. Colour is averaged as light adds,
/// in linear terms where the picture is sRGB, and weighted by alpha, so the empty parts do
/// not bleed into its edges. Averaging alone makes the leaves thin out with distance until
/// a crown goes see-through, so each copy's alpha is scaled to keep as much of it above
/// the cut as the full picture has (Castaño 2010; Unity's `mipMapsPreserveCoverage`).
fn cut_out_chain(data: &[u8], width: u32, height: u32, cut: f32, srgb: bool) -> (Vec<u8>, u32) {
    let linear = |c: u8| {
        let c = f32::from(c) / 255.0;
        if !srgb {
            c
        } else if c <= 0.04045 {
            c / 12.92
        } else {
            libm::powf((c + 0.055) / 1.055, 2.4)
        }
    };
    let encoded = |c: f32| {
        let c = if !srgb {
            c
        } else if c <= 0.003_130_8 {
            c * 12.92
        } else {
            1.055 * libm::powf(c, 1.0 / 2.4) - 0.055
        };
        (c * 255.0).round().clamp(0.0, 255.0) as u8
    };
    let (mut w, mut h) = (width as usize, height as usize);
    let mut level: Vec<[f32; 4]> = data
        .chunks_exact(4)
        .map(|p| {
            [
                linear(p[0]),
                linear(p[1]),
                linear(p[2]),
                f32::from(p[3]) / 255.0,
            ]
        })
        .collect();
    let covered = |pixels: &[[f32; 4]], scale: f32| {
        pixels.iter().filter(|p| p[3] * scale > cut).count() as f32 / pixels.len() as f32
    };
    let wanted = covered(&level, 1.0);
    let mut all = data.to_vec();
    let mut levels = 1;
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![[0.0; 4]; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                let mut sum = [0.0f32; 4];
                let mut plain = [0.0f32; 3];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p = level[(y * 2 + dy).min(h - 1) * w + (x * 2 + dx).min(w - 1)];
                    for c in 0..3 {
                        sum[c] += p[c] * p[3];
                        plain[c] += p[c];
                    }
                    sum[3] += p[3];
                }
                next[y * nw + x] = if sum[3] > 0.0 {
                    [
                        sum[0] / sum[3],
                        sum[1] / sum[3],
                        sum[2] / sum[3],
                        sum[3] / 4.0,
                    ]
                } else {
                    [plain[0] / 4.0, plain[1] / 4.0, plain[2] / 4.0, 0.0]
                };
            }
        }
        // The scale that keeps as much above the cut as the full picture, found by halving.
        let (mut low, mut high) = (0.25f32, 8.0f32);
        for _ in 0..16 {
            let middle = (low + high) / 2.0;
            if covered(&next, middle) < wanted {
                low = middle;
            } else {
                high = middle;
            }
        }
        for p in &next {
            all.extend([encoded(p[0]), encoded(p[1]), encoded(p[2])]);
            all.push(((p[3] * high).min(1.0) * 255.0).round() as u8);
        }
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
    fn a_cut_out_picture_keeps_as_much_above_the_cut_at_every_size() {
        // Eight by eight: one pixel in eight a leaf, the rest empty. Averaged, the next size
        // down would be at most a quarter opaque, below the cut, and the leaves would vanish.
        let mut data = Vec::new();
        for y in 0..8 {
            for x in 0..8 {
                let leaf = x % 2 == 0 && y % 2 == 0 && (x + y) % 4 == 0;
                data.extend(if leaf {
                    [40, 120, 30, 255]
                } else {
                    [0, 0, 0, 0]
                });
            }
        }
        let opaque = |pixels: &[u8]| {
            pixels
                .chunks_exact(4)
                .filter(|p| f32::from(p[3]) / 255.0 > CUT_OUT)
                .count()
        };
        let (chain, levels) = cut_out_chain(&data, 8, 8, CUT_OUT, true);
        assert_eq!(levels, 4);
        let second = &chain[8 * 8 * 4..8 * 8 * 4 + 4 * 4 * 4];
        assert!(opaque(second) >= 2, "{} of 16 left", opaque(second));
        // The colour under a leaf is the leaf's, not darkened by the empty pixels by it.
        let leaf = second.chunks_exact(4).find(|p| p[3] > 0).unwrap();
        assert_eq!(&leaf[..3], &[40, 120, 30]);
    }

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

    #[test]
    fn a_texture_one_pixel_across_or_odd_still_halves_to_one_pixel() {
        let grey =
            |values: &[u8]| -> Vec<u8> { values.iter().flat_map(|&v| [v, v, v, 255]).collect() };
        // Four by one: 2x1, then 1x1; the pixel below the edge is the edge's own.
        let (wide, levels) = chain(&grey(&[0, 100, 200, 40]), 4, 1);
        assert_eq!(levels, 3);
        assert_eq!(wide[16..], grey(&[50, 120, 85]));
        // One by three: halving the three leaves one, from the first two.
        let (tall, levels) = chain(&grey(&[10, 50, 250]), 1, 3);
        assert_eq!(levels, 2);
        assert_eq!(tall[12..], grey(&[30]));
    }
}
