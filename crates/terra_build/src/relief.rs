//! A material's height map on the CPU (D197, D200): what a block kind's surface and a log's
//! bark are displaced by when they are made, read from the same `height.jpg` the material's
//! textures come with, so the shape and the picture on it agree.

use crate::Material;
use crate::kit::{Kit, Pattern};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A material's height, a byte a texel, row by row from the top, as its texture is read.
#[derive(Clone, Debug, PartialEq)]
pub struct Relief {
    width: usize,
    height: usize,
    data: Vec<u8>,
    /// Each row's height across it, from its lowest row's to its highest's as 0 to 1, once
    /// asked for ([`Relief::row`]).
    rows: std::sync::OnceLock<Vec<f32>>,
    /// Its cells (`cells`, D305) for the share of it its joints are, once asked for.
    cells: std::sync::OnceLock<(u32, Arc<Vec<u8>>)>,
    /// Its largest stones (`stones`), once asked for.
    stones: std::sync::OnceLock<Arc<Vec<crate::cells::Stone>>>,
}

/// How many of a material's largest stones its corner stones are cut from, and how far
/// across the least of them is from its middle to its nearest edge, in texels of the cells.
const STONES_KEPT: usize = 24;
const STONE_LEAST: f32 = 6.0;

impl Relief {
    /// A height map `width` by `height`, a byte a texel; `None` if the bytes are not that
    /// many.
    pub fn new(width: usize, height: usize, data: Vec<u8>) -> Option<Relief> {
        (width > 0 && height > 0 && data.len() == width * height).then_some(Relief {
            width,
            height,
            data,
            rows: std::sync::OnceLock::new(),
            cells: std::sync::OnceLock::new(),
            stones: std::sync::OnceLock::new(),
        })
    }

    /// Its largest stones, where its joints are the lowest `share` of it (`cells::largest`):
    /// what a block material's dressed corner stones are cut from (`dress::corner`).
    pub fn stones(&self, share: f32) -> Arc<Vec<crate::cells::Stone>> {
        self.stones
            .get_or_init(|| {
                Arc::new(crate::cells::largest(
                    &self.cells(share),
                    STONES_KEPT,
                    STONE_LEAST,
                ))
            })
            .clone()
    }

    /// The height across a row of the texture, at a point down it, repeating, between rows:
    /// from 0 for its lowest row to 1 for its highest, so a covering's courses stand out by
    /// the steps its texture shows between them (D258).
    pub fn row(&self, v: f32) -> f32 {
        let rows = self.rows.get_or_init(|| {
            let means: Vec<f32> = self
                .data
                .chunks(self.width)
                .map(|row| row.iter().map(|&b| f32::from(b)).sum::<f32>() / row.len() as f32)
                .collect();
            let low = means.iter().copied().fold(f32::MAX, f32::min);
            let high = means.iter().copied().fold(f32::MIN, f32::max);
            let span = (high - low).max(1e-3);
            means.iter().map(|m| (m - low) / span).collect()
        });
        let y = v.rem_euclid(1.0) * self.height as f32 - 0.5;
        let y0 = y.floor();
        let f = y - y0;
        let at = |j: f32| rows[(j as i64).rem_euclid(self.height as i64) as usize];
        at(y0) * (1.0 - f) + at(y0 + 1.0) * f
    }

    /// Its cells, its lowest `share` being joints (`cells::label`): worked out once, the
    /// same as the texture arrays find them from the same picture.
    pub fn cells(&self, share: f32) -> Arc<Vec<u8>> {
        let made = self.cells.get_or_init(|| {
            (
                share.to_bits(),
                Arc::new(crate::cells::label(
                    &self.data,
                    self.width,
                    self.height,
                    1,
                    share,
                )),
            )
        });
        if made.0 == share.to_bits() {
            made.1.clone()
        } else {
            Arc::new(crate::cells::label(
                &self.data,
                self.width,
                self.height,
                1,
                share,
            ))
        }
    }

    /// The height, from 0 to 1, where the texture is at a point, repeating, between texels.
    pub fn at(&self, uv: [f32; 2]) -> f32 {
        // A block kind's surface reads this at every point of its grid, so no division:
        // `v - floor(v)` is `v.rem_euclid(1.0)` to the bit, and a texel is at most one off
        // either end.
        let repeat = |v: f32| v - v.floor();
        let x = repeat(uv[0]) * self.width as f32 - 0.5;
        let y = repeat(uv[1]) * self.height as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let wrap = |i: f32, side: usize| {
            let (i, side) = (i as i64, side as i64);
            (if i < 0 {
                i + side
            } else if i >= side {
                i - side
            } else {
                i
            })
            .clamp(0, side - 1) as usize
        };
        let texel = |i: f32, j: f32| {
            self.data[wrap(j, self.height) * self.width + wrap(i, self.width)] as f32 / 255.0
        };
        let top = texel(x0, y0) * (1.0 - fx) + texel(x0 + 1.0, y0) * fx;
        let bottom = texel(x0, y0 + 1.0) * (1.0 - fx) + texel(x0 + 1.0, y0 + 1.0) * fx;
        top * (1.0 - fy) + bottom * fy
    }
}

/// A material's colour on the CPU, small: what a building far off is painted with, its
/// textures baked into the colour of each corner (Unreal's HLOD bakes them so), each
/// texel the average of a patch of the texture, linear, row by row from the top.
#[derive(Clone, Debug, PartialEq)]
pub struct Swatch {
    width: usize,
    height: usize,
    data: Vec<[f32; 3]>,
}

impl Swatch {
    /// A swatch `width` by `height`; `None` if the colours are not that many.
    pub fn new(width: usize, height: usize, data: Vec<[f32; 3]>) -> Option<Swatch> {
        (width > 0 && height > 0 && data.len() == width * height).then_some(Swatch {
            width,
            height,
            data,
        })
    }

    /// The colour where the texture is at a point, repeating: the texel it falls in.
    /// Its colours' mean, as a building far off is seen in them.
    pub fn mean(&self) -> [f32; 3] {
        let sum = self.data.iter().fold([0.0f32; 3], |sum, c| {
            [sum[0] + c[0], sum[1] + c[1], sum[2] + c[2]]
        });
        sum.map(|s| s / self.data.len() as f32)
    }

    pub fn at(&self, uv: [f32; 2]) -> [f32; 3] {
        let x = (uv[0].rem_euclid(1.0) * self.width as f32) as usize;
        let y = (uv[1].rem_euclid(1.0) * self.height as f32) as usize;
        self.data[y.min(self.height - 1) * self.width + x.min(self.width - 1)]
    }
}

/// The maps kits' pieces are made with on the CPU, by the folder of the material they
/// come from: heights to shape them by, and colours to paint them with far off.
#[derive(Clone, Debug, Default)]
pub struct Maps {
    pub heights: BTreeMap<String, Arc<Relief>>,
    pub colours: BTreeMap<String, Arc<Swatch>>,
}

impl Maps {
    /// Height maps at their full range everywhere, a texel's noise, so every surface is
    /// pushed as far out and in as its kit lets it: the worst case for a hole, which the mesh
    /// checker inspects with (`cargo xtask artifacts`, its fuzz pass). `flat`, every height
    /// the middle one instead.
    pub fn worst(kits: &crate::kit::Kits, flat: bool) -> Maps {
        const SIDE: usize = 256;
        let heights: Vec<u8> = (0..SIDE * SIDE)
            .map(|i| {
                if flat {
                    128
                } else {
                    ((i * 2_654_435_761) >> 13) as u8
                }
            })
            .collect();
        let relief = Arc::new(Relief::new(SIDE, SIDE, heights).expect("a full map"));
        let swatch = Arc::new(Swatch::new(16, 16, vec![[0.4, 0.35, 0.3]; 256]).expect("a swatch"));
        let mut maps = Maps::default();
        for kit in &kits.0 {
            for path in [shaped_by(kit), topped_by(kit), cored_by(kit)]
                .into_iter()
                .flatten()
            {
                maps.heights.insert(path.to_string(), relief.clone());
            }
            for texture in kit.textures.values() {
                maps.colours.insert(texture.path.clone(), swatch.clone());
            }
        }
        maps
    }

    /// The height map that shapes a kit's pieces, once it has loaded: a block kind's
    /// stone, or a timber kind's bark.
    pub fn shape(&self, kit: &Kit) -> Option<&Relief> {
        self.heights.get(shaped_by(kit)?).map(Arc::as_ref)
    }

    /// The height map that shapes a block kind's tops, its floors' flagstones.
    pub fn top(&self, kit: &Kit) -> Option<&Relief> {
        self.heights.get(topped_by(kit)?).map(Arc::as_ref)
    }

    /// The height a framed kind's core is displaced by (D236): its wattle's.
    pub fn core(&self, kit: &Kit) -> Option<&Relief> {
        self.heights.get(cored_by(kit)?).map(Arc::as_ref)
    }

    /// The colours of a kit's material, once they have loaded.
    pub fn colour(&self, kit: &Kit, material: Material) -> Option<&Swatch> {
        let texture = kit.textures.get(&material)?;
        self.colours.get(&texture.path).map(Arc::as_ref)
    }
}

/// The folder of the material whose height shapes a kit's pieces, if any does: a block
/// kind's stone or a roof's covering.
pub fn shaped_by(kit: &Kit) -> Option<&str> {
    let (material, depth) = match kit.pattern {
        Pattern::Masonry => (Material::Masonry, kit.relief),
        // A log's bark stands out by its material's parallax, not its points (D284).
        Pattern::Logs => return None,
        Pattern::Roof => (Material::Covering, kit.roof.relief),
    };
    (depth > 0.0)
        .then(|| kit.textures.get(&material))
        .flatten()
        .map(|texture| texture.path.as_str())
}

/// The folder of the material whose height shapes a block kind's tops, its floors'
/// flagstones, if it has one.
pub fn topped_by(kit: &Kit) -> Option<&str> {
    (kit.pattern == Pattern::Masonry && kit.relief > 0.0)
        .then(|| kit.textures.get(&Material::Floor))
        .flatten()
        .map(|texture| texture.path.as_str())
}

/// The folder a framed kind's core is displaced by (D236), if it has one with relief.
pub fn cored_by(kit: &Kit) -> Option<&str> {
    let framing = kit.framing.as_ref()?;
    (framing.core_relief > 0.0)
        .then(|| kit.textures.get(&framing.core?))
        .flatten()
        .map(|texture| texture.path.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_relief_repeats_and_blends_between_texels() {
        // Row by row from the top: 0 and 51 above, 102 and 255 below.
        let relief = Relief::new(2, 2, vec![0, 51, 102, 255]).unwrap();
        let near = |uv: [f32; 2], height: f32| {
            let at = relief.at(uv);
            assert!((at - height).abs() < 1e-5, "{uv:?}: {at}, not {height}");
        };
        // Texel centres are a quarter and three quarters across and down.
        near([0.25, 0.25], 0.0);
        near([0.75, 0.25], 0.2);
        near([0.25, 0.75], 0.4);
        near([0.75, 0.75], 1.0);
        // Halfway across the top row, halfway down the left column, and in the middle.
        near([0.5, 0.25], 0.1);
        near([0.25, 0.5], 0.2);
        near([0.5, 0.5], (0.0 + 0.2 + 0.4 + 1.0) / 4.0);
        // It repeats: past the right edge is the left, past the bottom is the top.
        near([1.25, 0.25], 0.0);
        near([0.25, 1.75], 0.4);
        // Between the last row and the first, as it wraps.
        near([0.25, 0.0], 0.2);
    }

    #[test]
    fn the_relief_reads_as_wrapping_by_remainders_does_on_both_sides_of_zero() {
        // The plain way: each place's remainder, each texel's index by remainder.
        let (width, height) = (5, 3);
        let data: Vec<u8> = (0..width * height).map(|i| (i * 37 % 256) as u8).collect();
        let relief = Relief::new(width, height, data.clone()).unwrap();
        let plain = |uv: [f32; 2]| {
            let x = uv[0].rem_euclid(1.0) * width as f32 - 0.5;
            let y = uv[1].rem_euclid(1.0) * height as f32 - 0.5;
            let (x0, y0) = (x.floor(), y.floor());
            let (fx, fy) = (x - x0, y - y0);
            let texel = |i: f32, j: f32| {
                let i = (i as i64).rem_euclid(width as i64) as usize;
                let j = (j as i64).rem_euclid(height as i64) as usize;
                data[j * width + i] as f32 / 255.0
            };
            let top = texel(x0, y0) * (1.0 - fx) + texel(x0 + 1.0, y0) * fx;
            let bottom = texel(x0, y0 + 1.0) * (1.0 - fx) + texel(x0 + 1.0, y0 + 1.0) * fx;
            top * (1.0 - fy) + bottom * fy
        };
        let mut checked = 0;
        for i in -2000..2000 {
            let u = i as f32 * 0.0137 - 1e-9;
            let v = -(i as f32) * 0.0291 + 3.0;
            assert_eq!(
                relief.at([u, v]).to_bits(),
                plain([u, v]).to_bits(),
                "{u}, {v}"
            );
            checked += 1;
        }
        // Just below whole numbers, where a remainder rounds up to one.
        for u in [-1e-9, -1.0 - 1e-8, 2.0 - 1e-8, -0.0] {
            assert_eq!(relief.at([u, u]).to_bits(), plain([u, u]).to_bits(), "{u}");
        }
        assert_eq!(checked, 4000);
    }

    #[test]
    fn a_relief_must_be_as_many_bytes_as_texels() {
        assert!(Relief::new(2, 2, vec![0; 3]).is_none());
        assert!(Relief::new(0, 0, Vec::new()).is_none());
    }
}
