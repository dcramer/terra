//! A material's height map on the CPU (D197, D200): what a block kind's surface and a log's
//! bark are displaced by when they are made, read from the same `height.jpg` the material's
//! textures come with, so the shape and the picture on it agree.

/// A material's height, a byte a texel, row by row from the top, as its texture is read.
#[derive(Clone, Debug, PartialEq)]
pub struct Relief {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

impl Relief {
    /// A height map `width` by `height`, a byte a texel; `None` if the bytes are not that
    /// many.
    pub fn new(width: usize, height: usize, data: Vec<u8>) -> Option<Relief> {
        (width > 0 && height > 0 && data.len() == width * height).then_some(Relief {
            width,
            height,
            data,
        })
    }

    /// The height, from 0 to 1, where the texture is at a point, repeating, between texels.
    pub fn at(&self, uv: [f32; 2]) -> f32 {
        let x = uv[0].rem_euclid(1.0) * self.width as f32 - 0.5;
        let y = uv[1].rem_euclid(1.0) * self.height as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let texel = |i: f32, j: f32| {
            let i = (i as i64).rem_euclid(self.width as i64) as usize;
            let j = (j as i64).rem_euclid(self.height as i64) as usize;
            self.data[j * self.width + i] as f32 / 255.0
        };
        let top = texel(x0, y0) * (1.0 - fx) + texel(x0 + 1.0, y0) * fx;
        let bottom = texel(x0, y0 + 1.0) * (1.0 - fx) + texel(x0 + 1.0, y0 + 1.0) * fx;
        top * (1.0 - fy) + bottom * fy
    }
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
    fn a_relief_must_be_as_many_bytes_as_texels() {
        assert!(Relief::new(2, 2, vec![0; 3]).is_none());
        assert!(Relief::new(0, 0, Vec::new()).is_none());
    }
}
