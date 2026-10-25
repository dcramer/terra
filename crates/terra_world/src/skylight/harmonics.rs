//! The painted sky's diffuse light as nine spherical harmonics (D492), after Ramamoorthi and
//! Hanrahan, "An Efficient Representation for Irradiance Environment Maps" (SIGGRAPH 2001):
//! the light from every way projected onto the real harmonics of the first three bands, and
//! the irradiance toward any side read back from them, the cosine's convolution being a
//! factor a band. Unreal's sky light and Unity's ambient probe give their diffuse light so.
//! Three bands hold irradiance within a few percent of the exact for any sky, as the cosine
//! lobe passes almost nothing above them.

use bevy::math::Vec3;

/// The nine real spherical harmonics of the first three bands toward a unit direction.
fn basis(d: Vec3) -> [f32; 9] {
    [
        0.282_095,
        0.488_603 * d.y,
        0.488_603 * d.z,
        0.488_603 * d.x,
        1.092_548 * d.x * d.y,
        1.092_548 * d.y * d.z,
        0.315_392 * (3.0 * d.z * d.z - 1.0),
        1.092_548 * d.x * d.z,
        0.546_274 * (d.x * d.x - d.y * d.y),
    ]
}

/// What the clamped cosine keeps of each band, over pi: 1, 2/3 and 1/4.
const KEPT: [f32; 9] = [
    1.0,
    2.0 / 3.0,
    2.0 / 3.0,
    2.0 / 3.0,
    0.25,
    0.25,
    0.25,
    0.25,
    0.25,
];

/// The light from every way, projected.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Harmonics([Vec3; 9]);

impl Harmonics {
    /// Adds the light from a unit direction, standing for so much solid angle.
    pub(super) fn add(&mut self, d: Vec3, light: Vec3, solid_angle: f32) {
        for (c, y) in self.0.iter_mut().zip(basis(d)) {
            *c += light * (y * solid_angle);
        }
    }

    /// The irradiance toward a side over pi, as an environment map's diffuse light is: the
    /// light from a sky of one radiance everywhere is that radiance.
    pub(super) fn diffuse(&self, n: Vec3) -> Vec3 {
        self.0
            .iter()
            .zip(basis(n))
            .zip(KEPT)
            .map(|((c, y), kept)| *c * (y * kept))
            .sum::<Vec3>()
            .max(Vec3::ZERO)
    }

    pub(super) fn plus(&self, other: &Self) -> Self {
        Self(std::array::from_fn(|i| self.0[i] + other.0[i]))
    }
}

/// The solid angle of each texel of a cube's face of `size` texels a side, row by row, the
/// same on every face: by the area of the unit sphere a corner's quarter-plane covers,
/// atan2(x y, sqrt(x^2 + y^2 + 1)) (AMD's CubeMapGen, Driscoll's derivation).
pub(super) fn texel_solid_angles(size: u32) -> Vec<f32> {
    let corner = |x: f32, y: f32| libm::atan2f(x * y, libm::sqrtf(x * x + y * y + 1.0));
    let at = |i: u32| i as f32 / size as f32 * 2.0 - 1.0;
    let mut angles = Vec::with_capacity((size * size) as usize);
    for y in 0..size {
        for x in 0..size {
            let (x0, x1, y0, y1) = (at(x), at(x + 1), at(y), at(y + 1));
            angles.push(corner(x0, y0) - corner(x0, y1) - corner(x1, y0) + corner(x1, y1));
        }
    }
    angles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cubes_texels_cover_the_sphere() {
        let total: f32 = texel_solid_angles(16).iter().sum::<f32>() * 6.0;
        assert!((total - 4.0 * std::f32::consts::PI).abs() < 1e-4, "{total}");
    }

    #[test]
    fn a_sky_lit_above_only_lights_what_faces_up_fully_a_side_half_and_down_not_at_all() {
        // Radiance 1 over the upper hemisphere: irradiance over pi is (1 + cos) / 2, which
        // the first two bands hold exactly.
        let mut h = Harmonics::default();
        let n = 400;
        for i in 0..n {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
            let across = (1.0 - y * y).sqrt();
            for j in 0..n * 2 {
                let round = (j as f32 + 0.5) / (n * 2) as f32 * std::f32::consts::TAU;
                let d = Vec3::new(across * libm::cosf(round), y, across * libm::sinf(round));
                let light = if y > 0.0 { Vec3::ONE } else { Vec3::ZERO };
                h.add(d, light, 4.0 * std::f32::consts::PI / (2 * n * n) as f32);
            }
        }
        for (side, want) in [(Vec3::Y, 1.0), (Vec3::X, 0.5), (-Vec3::Y, 0.0)] {
            let got = h.diffuse(side).x;
            assert!((got - want).abs() < 0.01, "{side}: {got}");
        }
    }
}
