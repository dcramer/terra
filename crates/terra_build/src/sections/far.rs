//! A section far off: its textures baked into its corners' colours, one surface, and a
//! coarser box of it further still.

use super::*;

/// A section far off: the hull of its cubes ([`crate::hull`]), a few hundred triangles of
/// its materials' colours, what its pieces fade into past the far detail, and what the game
/// merges with its neighbours' into one mesh a cell of its base further off still, as
/// Unreal's HLOD merges a cell's meshes into an approximated one.
pub fn proxy(section: &Section) -> Surface {
    let origin = section.at * crate::cubes::SECTION;
    crate::hull::hull(
        section.base,
        section.kits,
        section.maps,
        origin,
        origin + IVec3::splat(crate::cubes::SECTION - 1),
    )
}

/// Surfaces by kind and material as one, each corner's colour multiplied by its texture's
/// where it falls on it, as a far mesh's textures are baked (Unreal's HLOD).
pub(super) fn bake(section: &Section, built: BTreeMap<(Kind, Material), Surface>) -> Surface {
    let encode = srgb_table();
    let to_srgb = |c: f32| encode[((c.clamp(0.0, 1.0) * (SRGB_STEPS - 1) as f32) + 0.5) as usize];
    let mut out = Surface::default();
    for ((kind, material), surface) in built {
        let kit = section.kits.of(kind).or(section.kits.0.first());
        let swatch = kit.and_then(|kit| section.maps.colour(kit, material));
        // Runs of corners share a colour, a log's or a face's: each turned once.
        let mut last: Option<([f32; 3], [f32; 3])> = None;
        let mut paint = |colour: [f32; 3], uv: [f32; 2]| -> [f32; 3] {
            let linear = match last {
                Some((from, to)) if from == colour => to,
                _ => {
                    let to = colour.map(to_linear);
                    last = Some((colour, to));
                    to
                }
            };
            let texel = swatch.map_or(fallback(material), |swatch| patch(swatch, uv));
            [0, 1, 2].map(|i| to_srgb(linear[i] * texel[i]))
        };
        let mut surface = surface;
        let textured = surface.uvs.len() == surface.positions.len();
        for i in 0..surface.colours.len() {
            let uv = if textured { surface.uvs[i] } else { [0.0; 2] };
            surface.colours[i] = paint(surface.colours[i], uv);
        }
        let shared = &mut surface.shared;
        for (colour, uv) in shared.colours.iter_mut().zip(&shared.uvs) {
            *colour = paint(*colour, *uv);
        }
        out.append(surface);
    }
    out
}

/// The colour round a corner, the mean of the swatch over the quarter of its tile about it:
/// what a far face's corners, half a metre apart on a two-metre stone texture, stand for,
/// as a texture's smaller copy averages it. One texel at each corner repeated the same few
/// values every tile and blended them across each triangle into a quilt of dimples.
fn patch(swatch: &crate::relief::Swatch, uv: [f32; 2]) -> [f32; 3] {
    const ACROSS: usize = 4;
    const STEP: f32 = 1.0 / 16.0;
    let mut sum = [0.0f32; 3];
    for j in 0..ACROSS {
        for i in 0..ACROSS {
            let at = [
                uv[0] + (i as f32 - (ACROSS as f32 - 1.0) * 0.5) * STEP,
                uv[1] + (j as f32 - (ACROSS as f32 - 1.0) * 0.5) * STEP,
            ];
            let texel = swatch.at(at);
            for c in 0..3 {
                sum[c] += texel[c];
            }
        }
    }
    sum.map(|c| c / (ACROSS * ACROSS) as f32)
}

/// How finely linear colour is turned back to sRGB when baked: finer than a byte.
pub(super) const SRGB_STEPS: usize = 4096;

/// sRGB for each step of linear colour from 0 to 1.
pub(super) fn srgb_table() -> &'static [f32] {
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        (0..SRGB_STEPS)
            .map(|i| {
                let c = i as f32 / (SRGB_STEPS - 1) as f32;
                if c <= 0.003_130_8 {
                    c * 12.92
                } else {
                    1.055 * libm::powf(c, 1.0 / 2.4) - 0.055
                }
            })
            .collect()
    })
}

/// An sRGB colour's value, linear, from a table finer than a byte.
pub(super) fn to_linear(c: f32) -> f32 {
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        (0..SRGB_STEPS)
            .map(|i| {
                let c = i as f32 / (SRGB_STEPS - 1) as f32;
                if c <= 0.04045 {
                    c / 12.92
                } else {
                    libm::powf((c + 0.055) / 1.055, 2.4)
                }
            })
            .collect()
    });
    table[((c.clamp(0.0, 1.0) * (SRGB_STEPS - 1) as f32) + 0.5) as usize]
}

/// A material's colour, linear, before its texture has loaded.
pub(super) fn fallback(material: Material) -> [f32; 3] {
    match material {
        Material::Masonry | Material::Stone | Material::Dressed | Material::Flashing => {
            [0.25, 0.24, 0.22]
        }
        Material::Covering => [0.16, 0.13, 0.11],
        Material::Plain => [0.6, 0.6, 0.6],
        _ => [0.22, 0.15, 0.1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_far_corner_takes_the_stone_round_it_not_one_texel_of_it() {
        // Stones and mortar a texel each, as a fine texture's swatch has them.
        let checker: Vec<[f32; 3]> = (0..256)
            .map(|i| {
                if (i % 16 + i / 16) % 2 == 0 {
                    [0.6; 3]
                } else {
                    [0.1; 3]
                }
            })
            .collect();
        let swatch = crate::relief::Swatch::new(16, 16, checker).unwrap();
        // Corners half a metre apart on a two-metre tile, each a quarter of it on: one
        // texel each would be all stone or all mortar; each is their mean instead.
        for k in 0..8 {
            let uv = [k as f32 * 0.25 + 0.01, (k % 3) as f32 * 0.25 + 0.01];
            let c = patch(&swatch, uv)[0];
            assert!((c - 0.35).abs() < 0.02, "{uv:?}: {c}");
        }
    }
}
