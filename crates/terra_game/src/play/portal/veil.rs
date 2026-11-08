//! The veil in a gate's opening (D470): light drawn in toward the middle in slow arms, bright
//! at the stone and deep at the centre, seen through a little, so the way onward reads as a
//! depth and not a lit disc. As shipped portals are drawn (Valheim's and Enshrouded's rings:
//! a swirl of scrolling noise, lit, on a flat sheet).
//!
//! What it glows as is its colour here, the same for every gate until a realm's theme says
//! its own (art-style.md: what glows, and in what colour, is the theme's).

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

pub type VeilMaterial = ExtendedMaterial<StandardMaterial, Veil>;

/// What `shaders/portal_veil.wesl` is given. Field order must match the struct there.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Veil {
    /// Its colour, linear, and in `w` how fast its arms turn, in turns of the pattern a
    /// second.
    #[uniform(100)]
    pub glow: Vec4,
}

impl Default for Veil {
    fn default() -> Self {
        Self {
            // The violet the first gates glowed.
            glow: Vec4::new(0.55, 0.22, 1.0, 0.12),
        }
    }
}

impl MaterialExtension for Veil {
    fn fragment_shader() -> ShaderRef {
        "shaders/portal_veil.wesl".into()
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<VeilMaterial>::default());
}

/// A veil's material: unlit by the sun (it is light), blended over what is behind it, seen
/// from both sides.
pub fn material(materials: &mut Assets<VeilMaterial>) -> Handle<VeilMaterial> {
    materials.add(VeilMaterial {
        base: StandardMaterial {
            base_color: Color::BLACK,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        },
        extension: Veil::default(),
    })
}
