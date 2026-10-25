//! A breath marched as a volume, the cinematic tier (D430; research/2026-10-19-raymarched-
//! breath.md): its stream and fireball one emissive, absorbing gas marched front to back
//! through its hull (`fire.wesl`, `breath`), its soot lit by the sun through the gas toward it,
//! where play draws a breath as cards (`breath_sprite`). As Unreal draws Heterogeneous Volumes
//! and Niagara's gas for cinematics only, it costs about 2.5 ms more than the cards on the RTX
//! 3080 seen head-on, a film's to spend and not play's (docs/fire.md, "LOD and budgets"). A
//! film asks for it by the ultra preset (`Quality::breath_marched`, D428) or by its shot's
//! look (`cine::Look::marched_breath`).
//!
//! Its hulls are drawn twice: once for the light, once into the picture's alpha with the
//! flame's share of the pixel, so the tonemapper takes the marched flame through the film as
//! it does a hearth's (`film`). Drawn as cards, both hulls are placed nowhere.

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use crate::{cine::Cine, quality::Quality};

/// The lattice of the gas's value noise, a side (`fire.wesl`, `noise3`): wrapped, so the
/// noise repeats every so many of its cells, some 60 m along the stream's flow.
pub(super) const NOISE_SIZE: u32 = 32;

/// The gas's value noise as a 3D texture, a random value at each lattice point, sampled
/// linearly at the point's smoothed cell coordinate: the same smooth trilinear value noise the
/// shader hashed for itself at each of the eight corners, in one fetch, as Nubis and Unreal's
/// volumes read their noise from tiling volume textures. Hashed, it was most of the march's
/// cost: some twenty noises a sample, each eight hashes.
pub(super) fn noise() -> Image {
    const SEED: u32 = 0x6272_6561; // "brea"
    let n = NOISE_SIZE;
    let mut data = Vec::with_capacity((n * n * n) as usize);
    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                let v = terra_gen::hash::unit3(SEED, x, y, z);
                data.push((v * 255.0).round() as u8);
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: n,
        },
        TextureDimension::D3,
        data,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    let mut sampler = ImageSamplerDescriptor::linear();
    sampler.set_address_mode(ImageAddressMode::Repeat);
    image.sampler = ImageSampler::Descriptor(sampler);
    image
}

/// The breaths' hulls: a box for each jet, its corners 0 to 1 on each axis (its position),
/// which the shader stretches round the jet's stream and fireball and marches through
/// (`fire.wesl`, `breath`), its back faces drawn so it shows with the eye inside it too.
/// `which` rides in each corner's second number: 0 a jet's, 1 a hearth's flame field (D466,
/// `hearth_march`).
pub(super) fn hulls(slots: usize, which: f32) -> Mesh {
    let mut positions = Vec::with_capacity(slots * 24);
    let mut normals = Vec::with_capacity(slots * 24);
    let mut ids = Vec::with_capacity(slots * 24);
    let mut indices = Vec::with_capacity(slots * 36);
    for slot in 0..slots {
        for axis in 0..3 {
            for side in [0.0f32, 1.0] {
                // (u, v, axis) turn as x, y, z do, so the face runs anticlockwise seen from
                // its outside when it is at 1, and is turned over when it is at 0.
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                let first = positions.len() as u32;
                for (a, b) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                    let mut p = [0.0f32; 3];
                    p[axis] = side;
                    p[u] = a;
                    p[v] = b;
                    let mut n = [0.0f32; 3];
                    n[axis] = side * 2.0 - 1.0;
                    positions.push(p);
                    normals.push(n);
                    ids.push([slot as f32, which]);
                }
                if side > 0.5 {
                    indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
                } else {
                    indices.extend([first, first + 2, first + 1, first, first + 3, first + 2]);
                }
            }
        }
    }
    let corners = vec![[0.0f32, 0.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, corners)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, ids)
    .with_inserted_indices(Indices::U32(indices))
}

/// How the breaths are drawn now.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BreathTier {
    /// As cards, play's.
    #[default]
    Cards,
    /// Marched as a volume, the cinematic tier.
    Marched,
}

impl BreathTier {
    /// The tier a preset draws, or marched if the shot filmed asks for it.
    pub fn of(quality: Quality, shot_asks: bool) -> Self {
        if quality.breath_marched() || shot_asks {
            BreathTier::Marched
        } else {
            BreathTier::Cards
        }
    }

    /// As the fire's shader is told it (`fire.wesl`, `Fire::volume`).
    pub(super) fn packed(self) -> Vec4 {
        Vec4::new(
            f32::from(u8::from(self == BreathTier::Marched)),
            0.0,
            0.0,
            0.0,
        )
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<BreathTier>()
        .add_systems(PostUpdate, choose.before(super::show));
}

/// The tier the preset and the shot ask for, changed only when it changes.
fn choose(quality: Res<Quality>, cine: Option<Res<Cine>>, mut tier: ResMut<BreathTier>) {
    let asks = cine.is_some_and(|cine| cine.shot.look.marched_breath);
    tier.set_if_neq(BreathTier::of(*quality, asks));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_film_marches_a_breath_and_play_keeps_its_cards() {
        for quality in Quality::ALL {
            let want = if quality == Quality::Ultra {
                BreathTier::Marched
            } else {
                BreathTier::Cards
            };
            assert_eq!(BreathTier::of(quality, false), want, "{quality:?}");
            assert_eq!(
                BreathTier::of(quality, true),
                BreathTier::Marched,
                "{quality:?}"
            );
        }
        assert_eq!(BreathTier::default(), BreathTier::Cards);
    }

    #[test]
    fn a_marched_breath_lights_its_ground_oranger_and_dimmer_than_its_cards() {
        let (cards, cards_colour) = super::super::light::jet_light(BreathTier::Cards);
        let (marched, colour) = super::super::light::jet_light(BreathTier::Marched);
        assert!(marched < cards);
        let (a, b) = (cards_colour.to_srgba(), colour.to_srgba());
        // Its green further from its red, and no more blue for it: orange, not the yellow-green
        // flood the grass took under a yellower light (vol10's review), nor pinker.
        assert!(b.green / b.red < a.green / a.red);
        assert!(b.blue / b.red <= a.blue / a.red);
    }

    #[test]
    fn the_gas_noise_is_spread_from_nought_to_one_and_matches_its_lattice() {
        let image = noise();
        let data = image.data.as_ref().unwrap();
        assert_eq!(data.len(), (NOISE_SIZE * NOISE_SIZE * NOISE_SIZE) as usize);
        let mean = data.iter().map(|&v| f32::from(v)).sum::<f32>() / data.len() as f32 / 255.0;
        assert!((mean - 0.5).abs() < 0.02, "{mean}");
        assert!(data.iter().any(|&v| v < 5) && data.iter().any(|&v| v > 250));
        let shader =
            std::fs::read_to_string(crate::data::assets().join("shaders/fire.wesl")).unwrap();
        assert!(shader.contains(&format!("const GAS_NOISE_SIZE: f32 = {NOISE_SIZE}.0;")));
    }

    #[test]
    fn the_shader_hides_one_tier_where_it_draws_the_other() {
        let shader =
            std::fs::read_to_string(crate::data::assets().join("shaders/fire.wesl")).unwrap();
        // Told by its uniform's volume field, which the hulls, the march and the sprites read.
        assert!(shader.contains("    volume: vec4<f32>,\n"));
        assert!(shader.contains("b.marched = fire.volume.x > 0.5;"));
        assert!(shader.contains("|| !b.marched {\n        return nowhere();"));
        assert!(shader.contains("|| !b.marched {\n        discard;"));
        assert!(shader.contains("if b.marched {\n        return nowhere();"));
        // Its embers once off are marched only (`breath_ember`).
        assert!(shader.contains("if !b.marched || i >= BREATH_EMBER_CARDS"));
        assert_eq!(BreathTier::Marched.packed().x, 1.0);
        assert_eq!(BreathTier::Cards.packed().x, 0.0);
    }
}
