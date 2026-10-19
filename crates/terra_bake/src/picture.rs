//! A picture cooked (D188): its layers' RGBA pixels into a block-compressed KTX2 with its
//! mipmaps, as `terra_core::cooked::Kind` picks the format: BC7 in sRGB for colour, BC5 for
//! a normal's or a motion's two components, BC7 for other data, BC4 for one channel. The
//! encoders are `ctt`'s: bc7enc-rdo's BC7 (Rich Geldreich's bc7e, the best of the open BC7
//! encoders) and Intel's ISPC texture compressor for BC4 and BC5. Colour cut out by its
//! alpha (a card of leaves) gets the game's own coverage-keeping chain
//! (`terra_core::cooked::mips`), so its leaves do not thin away; everything else `ctt`'s
//! triangle filter, colour averaged in linear light.

use ctt::encoders::Encoder;
use ctt::{
    AlphaMode, ColorSpace, Container, ConvertSettings, Format, FormatDesc, ImageRef,
    PipelineOutput, Quality, SurfaceRef, TargetFormat, TextureKind,
};
use terra_core::cooked::{Kind, mips};

/// One layer's pixels, four bytes each.
pub struct Layer {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// What a picture is cooked into, for the report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    Bc7,
    Bc5,
    Bc4,
}

impl Block {
    pub fn name(self) -> &'static str {
        match self {
            Block::Bc7 => "BC7",
            Block::Bc5 => "BC5",
            Block::Bc4 => "BC4",
        }
    }

    /// Bytes on the GPU for a texel.
    pub fn bytes_per_texel(self) -> f64 {
        match self {
            Block::Bc7 | Block::Bc5 => 1.0,
            Block::Bc4 => 0.5,
        }
    }
}

/// The block a picture of `kind` is stored in; `one_channel` where only its red is read.
pub fn block(kind: Kind, one_channel: bool) -> Block {
    match kind {
        Kind::Normal | Kind::Two => Block::Bc5,
        Kind::Data if one_channel => Block::Bc4,
        Kind::Colour | Kind::Data => Block::Bc7,
    }
}

/// The layers cooked as `kind` into `block`, with every mipmap down to one texel where `mips`.
pub fn cook(layers: &[Layer], kind: Kind, block: Block, mips: bool) -> Result<Vec<u8>, String> {
    let srgb = kind == Kind::Colour;
    let cut_out = srgb && mips && layers.iter().any(|l| mips::is_cut_out(&l.rgba, true));
    // Every layer's levels, made here where the chain must keep coverage.
    let chains: Vec<Vec<(Vec<u8>, u32, u32)>> = layers
        .iter()
        .map(|l| {
            if cut_out {
                let (all, count) =
                    mips::cut_out_chain(&l.rgba, l.width, l.height, mips::CUT_OUT, true);
                split(&all, l.width, l.height, count)
            } else {
                vec![(l.rgba.clone(), l.width, l.height)]
            }
        })
        .collect();
    let surfaces = chains
        .iter()
        .map(|levels| {
            levels
                .iter()
                .map(|(data, width, height)| SurfaceRef {
                    data,
                    width: *width,
                    height: *height,
                    depth: 1,
                    stride: width * 4,
                    slice_stride: 0,
                })
                .collect()
        })
        .collect();
    let image = ImageRef {
        surfaces,
        kind: TextureKind::Texture2D,
        desc: FormatDesc {
            format: if srgb {
                Format::R8G8B8A8_SRGB
            } else {
                Format::R8G8B8A8_UNORM
            },
            color_space: if srgb {
                ColorSpace::Srgb
            } else {
                ColorSpace::Linear
            },
            alpha: if kind == Kind::Colour {
                AlphaMode::Straight
            } else {
                // A data picture's alpha is a channel like the rest, never blended by.
                AlphaMode::Opaque
            },
        },
    };
    let format = match (block, srgb) {
        (Block::Bc7, true) => Format::BC7_SRGB_BLOCK,
        (Block::Bc7, false) => Format::BC7_UNORM_BLOCK,
        (Block::Bc5, _) => Format::BC5_UNORM_BLOCK,
        (Block::Bc4, _) => Format::BC4_UNORM_BLOCK,
    };
    let settings = ConvertSettings {
        format: Some(TargetFormat::Compressed {
            format,
            encoder: Encoder::Auto,
        }),
        // Zstandard over the blocks, as KTX2 allows and Bevy reads: a third smaller on disk.
        container: Container::ktx2_zstd(9),
        quality: Quality::Slow,
        allow_discarding_alpha: true,
        mipmap: mips && !cut_out,
        ..Default::default()
    };
    match ctt::convert(image, settings) {
        Ok(PipelineOutput::Encoded(bytes)) => Ok(bytes),
        Ok(PipelineOutput::Raw(_)) => Err("ctt returned a raw image for a KTX2".into()),
        Err(problem) => Err(problem.to_string()),
    }
}

/// A chain laid end to end (`mips::cut_out_chain`) cut into its levels.
fn split(all: &[u8], width: u32, height: u32, count: u32) -> Vec<(Vec<u8>, u32, u32)> {
    let (mut w, mut h, mut at) = (width, height, 0usize);
    let mut levels = Vec::new();
    for _ in 0..count {
        let len = (w * h * 4) as usize;
        levels.push((all[at..at + len].to_vec(), w, h));
        at += len;
        (w, h) = ((w / 2).max(1), (h / 2).max(1));
    }
    levels
}

/// Bytes on the GPU for `layers` of `width` by `height` in `block`, with mipmaps or not.
pub fn gpu_bytes(width: u32, height: u32, layers: u32, block: Block, mips: bool) -> f64 {
    let top = f64::from(width) * f64::from(height) * f64::from(layers) * block.bytes_per_texel();
    if mips { top * 4.0 / 3.0 } else { top }
}

/// A picture's pixels, decoded and made RGBA, from a PNG's or JPEG's bytes.
pub fn decode(bytes: &[u8]) -> Result<Layer, String> {
    let picture = image::load_from_memory(bytes).map_err(|p| p.to_string())?;
    let rgba = picture.to_rgba8();
    Ok(Layer {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

/// A picture no larger than `side` across, halved with a triangle filter until it fits.
pub fn capped(layer: Layer, side: u32) -> Layer {
    if layer.width <= side && layer.height <= side {
        return layer;
    }
    let scale = f64::from(side) / f64::from(layer.width.max(layer.height));
    let (w, h) = (
        ((f64::from(layer.width) * scale) as u32).max(1),
        ((f64::from(layer.height) * scale) as u32).max(1),
    );
    let buffer = image::RgbaImage::from_raw(layer.width, layer.height, layer.rgba)
        .expect("a layer's bytes are its pixels");
    let small = image::imageops::resize(&buffer, w, h, image::imageops::FilterType::Triangle);
    Layer {
        width: w,
        height: h,
        rgba: small.into_raw(),
    }
}

/// A strip of square layers, one above the next, cut into them.
pub fn layers_of(strip: Layer, count: u32) -> Vec<Layer> {
    let height = strip.height / count;
    let len = (strip.width * height * 4) as usize;
    strip
        .rgba
        .chunks_exact(len)
        .map(|rgba| Layer {
            rgba: rgba.to_vec(),
            width: strip.width,
            height,
        })
        .collect()
}

/// Whether only one channel carries anything: red, green and blue alike and alpha full.
pub fn one_channel(layers: &[Layer]) -> bool {
    layers.iter().all(|l| {
        l.rgba
            .chunks_exact(4)
            .all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 255)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(bytes: &[u8]) -> (Format, u32, u32) {
        let reader = ktx2::Reader::new(bytes).expect("a KTX2");
        let header = reader.header();
        (
            header.format.expect("a format"),
            header.level_count,
            header.layer_count,
        )
    }

    #[test]
    fn each_kind_is_cooked_to_its_block_with_every_mipmap() {
        let layer = || Layer {
            rgba: (0..64 * 64 * 4).map(|i| (i % 251) as u8).collect(),
            width: 64,
            height: 64,
        };
        let colour = cook(&[layer()], Kind::Colour, block(Kind::Colour, false), true).unwrap();
        // 64, 32, 16, 8, 4, 2, 1.
        assert_eq!(read(&colour), (Format::BC7_SRGB_BLOCK, 7, 0));
        let normal = cook(&[layer(), layer()], Kind::Normal, Block::Bc5, true).unwrap();
        assert_eq!(read(&normal), (Format::BC5_UNORM_BLOCK, 7, 2));
        let flipbook = cook(&[layer()], Kind::Data, Block::Bc7, false).unwrap();
        assert_eq!(read(&flipbook), (Format::BC7_UNORM_BLOCK, 1, 0));
    }
}
