//! What glows of a burning prop (a campfire's coals, its logs' cracks and hot feet) drawn as
//! Bevy's StandardMaterial draws it, with the share of each pixel's light its glow gives
//! written to the flames' mask (`film`): the tonemapper takes that share through the film,
//! ungraded, as it takes a flame. A dull coal is dim, so the night's grade took it for the
//! moonlit shade and faded its colour, its red with no green going pink (fire round 31: a
//! glow's sRGB green under 0.3 of its red read pink at night); and Neutral took its brightest
//! cracks past its white to pink-white. Frostbite and The Witcher 3 tonemap their fire, embers
//! included, apart from the scene so. What it lights (the stones, the ground) is graded with
//! the world (D415).

use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

/// A glowing part's material: its own StandardMaterial, its glow in the flames' mask.
pub type GlowMaterial = ExtendedMaterial<StandardMaterial, Glow>;

/// What `shaders/fire_glow.wesl` is given. Field order must match the struct there.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Glow {
    /// x how much of the glow's share goes to the mask (1 all of it).
    #[uniform(100)]
    pub mask: Vec4,
}

impl Default for Glow {
    fn default() -> Self {
        Self {
            mask: Vec4::new(1.0, 0.0, 0.0, 0.0),
        }
    }
}

impl MaterialExtension for Glow {
    fn fragment_shader() -> ShaderRef {
        "shaders/fire_glow.wesl".into()
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<GlowMaterial>::default());
}

/// The share of a pixel's light its glow gives, as the shader takes it: the glow's
/// luminance over the whole's, so a coal lit by the flames above its glow keeps the film
/// for its glow's part alone.
#[cfg(test)]
pub(super) fn share(glow: Vec3, whole: Vec3) -> f32 {
    let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
    (luma(glow) / luma(whole).max(1e-6)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pixel_lit_only_by_its_glow_is_all_mask_and_an_unlit_one_none() {
        let glow = Vec3::new(0.3, 0.1, 0.01);
        assert!((share(glow, glow) - 1.0).abs() < 1e-6);
        assert_eq!(share(Vec3::ZERO, Vec3::new(0.2, 0.1, 0.05)), 0.0);
        // A coal as bright again from the flames' light as from its own glow: half.
        assert!((share(glow, glow * 2.0) - 0.5).abs() < 1e-5);
        // Black is no mask, not a division by nothing.
        assert_eq!(share(Vec3::ZERO, Vec3::ZERO), 0.0);
    }

    #[test]
    fn the_shader_writes_the_share_past_the_alpha_the_scene_holds() {
        let source = include_str!("../../../../assets/shaders/fire_glow.wesl");
        // Past 1, as the flames' mask is, so the tonemapper reads it as theirs.
        assert!(source.contains("out.color.a = 1.0 +"));
        assert!(source.contains("0.2126, 0.7152, 0.0722"));
        assert!(source.contains("pbr_input.material.emissive"));
    }
}
