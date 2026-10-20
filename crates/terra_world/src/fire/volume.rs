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
    fn a_marched_breath_lights_its_wall_yellower_and_dimmer_than_its_cards() {
        let (cards, cards_colour) = super::super::light::jet_light(BreathTier::Cards);
        let (marched, colour) = super::super::light::jet_light(BreathTier::Marched);
        assert!(marched < cards);
        let (a, b) = (cards_colour.to_srgba(), colour.to_srgba());
        // Its green nearer its red, and no more blue for it: orange-yellow, not pinker.
        assert!(b.green / b.red > a.green / a.red);
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
        // Told by its uniform's last field, which the hulls, the march and the sprites read.
        assert!(shader.contains("    volume: vec4<f32>,\n}"));
        assert!(shader.contains("b.marched = fire.volume.x > 0.5;"));
        assert!(shader.contains("|| !b.marched {\n        return nowhere();"));
        assert!(shader.contains("|| !b.marched {\n        discard;"));
        assert!(shader.contains("if b.marched {\n        return nowhere();"));
        assert_eq!(BreathTier::Marched.packed().x, 1.0);
        assert_eq!(BreathTier::Cards.packed().x, 0.0);
    }
}
