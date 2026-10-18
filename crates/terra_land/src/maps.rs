//! Pictures of a region's rules (D326): each a map of one quantity in a colour ramp, a bar
//! along its foot from its lowest value to its highest, and lines drawn where a value is
//! crossed (an isotherm, a lake's shore), saved as PNG. And working a function out over many
//! cells on every core, which every map needs.

use std::path::Path;

/// `f` over every item, on every core, in order (`terra_gen::par`, its home).
pub use terra_gen::par::map as par_map;

/// A colour ramp: colours at evenly spaced stops from the low end to the high.
#[derive(Clone, Copy, Debug)]
pub struct Ramp(pub &'static [[u8; 3]]);

impl Ramp {
    /// Low land green through brown and grey to white.
    pub const LAND: Ramp = Ramp(&[
        [44, 92, 52],
        [104, 140, 70],
        [170, 150, 96],
        [130, 104, 80],
        [150, 150, 150],
        [250, 250, 250],
    ]);
    /// Cold blue through white to warm red, for a temperature.
    pub const COLD_WARM: Ramp = Ramp(&[
        [40, 60, 160],
        [110, 160, 230],
        [240, 240, 240],
        [240, 170, 90],
        [180, 40, 30],
    ]);
    /// Dark to bright, for a steepness or an amount.
    pub const AMOUNT: Ramp = Ramp(&[
        [20, 24, 60],
        [40, 100, 140],
        [90, 180, 120],
        [230, 220, 80],
        [250, 250, 240],
    ]);
    /// Bare ground brown to snow white.
    pub const SNOW: Ramp = Ramp(&[[110, 96, 80], [180, 196, 220], [255, 255, 255]]);
    /// Exposed red through grey to sheltered blue.
    pub const SHELTER: Ramp = Ramp(&[[200, 60, 40], [200, 200, 200], [40, 90, 200]]);

    /// The colour at a share of the way from the low end, 0 to 1.
    pub fn at(&self, t: f32) -> [u8; 3] {
        let stops = self.0;
        let t = t.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
        let i = (t as usize).min(stops.len() - 2);
        let f = t - i as f32;
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * f).round() as u8;
        [
            mix(stops[i][0], stops[i + 1][0]),
            mix(stops[i][1], stops[i + 1][1]),
            mix(stops[i][2], stops[i + 1][2]),
        ]
    }
}

/// A map: a grid of values drawn in a ramp from `low` to `high`, shaded by the ground's relief
/// if given, with lines where values cross `lines`, and a bar at its foot.
pub struct Map<'a> {
    pub cells: usize,
    pub values: &'a [f32],
    pub ramp: Ramp,
    pub low: f32,
    pub high: f32,
    /// Light from the north-west on the ground, 0 to 1 a cell, multiplied in.
    pub shade: Option<&'a [f32]>,
    /// Values whose crossing is drawn as a black line.
    pub lines: &'a [f32],
    /// Cells drawn in a colour of their own over the ramp (a lake), where not `None`.
    pub over: Option<&'a [Option<[u8; 3]>]>,
}

/// Pixels a cell is drawn as, at least, and the height of the bar at the foot.
const BAR_PX: usize = 14;

impl Map<'_> {
    /// Saves it as a PNG, each cell `scale` pixels on a side.
    pub fn save(&self, path: &Path, scale: usize) -> Result<(), String> {
        let side = self.cells * scale;
        let (width, height) = (side, side + BAR_PX);
        let mut rgb = vec![0u8; width * height * 3];
        let span = (self.high - self.low).max(1e-6);
        let crosses = |a: f32, b: f32| self.lines.iter().any(|&l| (a < l) != (b < l));
        for y in 0..side {
            for x in 0..side {
                let (c, r) = (x / scale, y / scale);
                let k = r * self.cells + c;
                let v = self.values[k];
                let mut colour = self.ramp.at((v - self.low) / span);
                if let Some(shade) = self.shade {
                    colour = colour.map(|ch| (ch as f32 * shade[k]).min(255.0) as u8);
                }
                if let Some(Some(over)) = self.over.map(|o| o[k]) {
                    colour = over;
                }
                // A line along a cell's east or south edge where the value crosses there.
                let edge_e = x % scale == scale - 1 && c + 1 < self.cells;
                let edge_s = y % scale == scale - 1 && r + 1 < self.cells;
                if (edge_e && crosses(v, self.values[k + 1]))
                    || (edge_s && crosses(v, self.values[k + self.cells]))
                {
                    colour = [0, 0, 0];
                }
                rgb[(y * width + x) * 3..][..3].copy_from_slice(&colour);
            }
        }
        for y in side..height {
            for x in 0..width {
                let t = x as f32 / (width - 1).max(1) as f32;
                let mut colour = self.ramp.at(t);
                let v = self.low + t * span;
                let next = self.low + (x + 1) as f32 / (width - 1).max(1) as f32 * span;
                if crosses(v, next) {
                    colour = [0, 0, 0];
                }
                rgb[(y * width + x) * 3..][..3].copy_from_slice(&colour);
            }
        }
        save_rgb(path, width, height, &rgb)
    }
}

/// How lit the ground is from the north-west at 45 degrees, 0.45 to 1.15 a cell: relief
/// shading, so the land's shape reads under any map.
pub fn hillshade(region: &crate::region::Region) -> Vec<f32> {
    let light = glam::Vec3::new(-0.5, 0.5, 0.7).normalize();
    (0..region.cells * region.cells)
        .map(|k| {
            let g = region.gradient(k % region.cells, k / region.cells);
            let n = glam::Vec3::new(-g.x, -g.y, 1.0).normalize();
            0.45 + 0.7 * n.dot(light).max(0.0)
        })
        .collect()
}

/// Saves pixels, three bytes each, as a PNG.
pub fn save_rgb(path: &Path, width: usize, height: usize, rgb: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    writer
        .write_image_data(rgb)
        .map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ramp_runs_from_its_first_colour_to_its_last() {
        assert_eq!(Ramp::SNOW.at(0.0), [110, 96, 80]);
        assert_eq!(Ramp::SNOW.at(1.0), [255, 255, 255]);
        assert_eq!(Ramp::SNOW.at(2.0), [255, 255, 255]);
        assert_eq!(Ramp::SNOW.at(0.5), [180, 196, 220]);
    }

    #[test]
    fn working_out_on_every_core_keeps_the_order() {
        let items: Vec<u32> = (0..1000).collect();
        assert_eq!(
            par_map(&items, |&i| i * 2),
            (0..1000).map(|i| i * 2).collect::<Vec<_>>()
        );
    }
}
