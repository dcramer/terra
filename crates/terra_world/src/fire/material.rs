//! The fire's material (D377): what its shader is told ([`FireUniform`]), the textures and
//! buffers it reads ([`FireExtension`]), and the pipelines it is drawn with ([`FireKey`]).

use super::{HEARTHS, JETS, books, sim};
use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{render_resource::AsBindGroup, storage::ShaderBuffer},
    shader::ShaderRef,
};

/// What a fire's material is told. Field order must match `fire.wesl`.
#[derive(Clone, Copy, Debug, Default, Reflect, bevy::render::render_resource::ShaderType)]
pub struct FireUniform {
    /// x which cards (0 flames, 1 embers, 2 jets' rods, 3 none, 4 drops, 5 jets' sprites), y
    /// how many slots are filled, z the time the cards move by, w how many of the first slots
    /// are kept fires.
    pub(super) kind: Vec4,
    /// The wind where the fire is, metres a second in the world, and its strength.
    pub(super) wind: Vec4,
    /// Each jet: from, and its reach; its way, and its spread; whether it is on (1), since
    /// when it is on or off, and when it last came on; the face it strikes, its normal and
    /// how far off it is.
    pub(super) jets: [Vec4; JETS * 4],
    /// The exposure weight and the dusk it is drawn as tuned at (`FireExposure::packed`).
    pub(super) exposure: Vec4,
    /// Each flipbook set's rate, metres, motion scale and whether it blends along its vectors
    /// (`books::Sidecar::told`), in the order of `books::SETS`.
    pub(super) sets: [Vec4; books::SETS.len()],
    /// x the colour ramp's scale, y its rows.
    pub(super) ramp: Vec4,
    /// How high up its frame each set's flame tips reach (`books::Sidecar::reach`).
    pub(super) reach: Vec4,
    /// x 1 when a breath is marched as a volume, the cinematic tier (`volume::BreathTier`).
    pub(super) volume: Vec4,
    /// Each hearth's logs, two a hearth (`hearth::Logs::packed`).
    pub(super) hearths: [Vec4; HEARTHS * 2],
    /// Each hearth's body, four a vector, in slot order (`hearth::Body::told`).
    pub(super) bodies: [Vec4; HEARTHS / 4],
    /// Each hearth's place in its baked loop (`sim::Placement::packed`), in slot order.
    pub(super) baked: [Vec4; HEARTHS],
    /// The baked loop's sidecar as the shader reads it (`sim::Sim::packed`).
    pub(super) sim: [Vec4; 3],
    /// Each hearth's shed wisps, two a wisp (`sim::wisp::Wisp::drawn`), in slot order.
    pub(super) wisps: [[Vec4; sim::WISPS * 2]; HEARTHS],
}

/// Flames, embers and jets: drawn unlit, their light added to what is behind, on Bevy's own
/// material as smoke is, its shaders ours.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bind_group_data(FireKey)]
pub struct FireExtension {
    #[uniform(100)]
    pub(super) uniform: FireUniform,
    /// Whether these are the breaths' hulls, marched through as volumes (`hulls`).
    pub(super) volume: bool,
    /// Whether this is the hearths' flames' mask, which says where they are to the
    /// tonemapper (`film`).
    pub(super) mask: bool,
    /// Whether these are the breaths' sprites, which write their depth: drawn opaque or not
    /// at all (`breath_eroded`), they stand where the eye sees them for what reads the depth
    /// after them. Left out of it, the volumetric fog marched each ray on to the wall or the
    /// sky behind the flame, and fogged it more over the sky than over the wall, a level band
    /// along the wall's top and the horizon (breath r31's debug tints).
    pub(super) depth: bool,
    #[storage(101, read_only)]
    pub(super) slots: Handle<ShaderBuffer>,
    /// The flames' flipbooks (`books`): a layer a set, its heat, cover, soot and distortion.
    #[texture(102, dimension = "2d_array")]
    #[sampler(103)]
    pub(super) flames: Handle<Image>,
    /// The breaths' soot and drops (`Puffs`).
    #[storage(104, read_only)]
    pub(super) puffs: Handle<ShaderBuffer>,
    /// The exposure the eye is adapted to (`adaptation`), so a flame's hottest cores clip
    /// white on the screen whatever the adaptation does.
    #[storage(105, read_only)]
    pub(super) adaptation: Handle<ShaderBuffer>,
    /// Each set's motion to its next frame, at half size, loaded unfiltered.
    #[texture(106, dimension = "2d_array")]
    pub(super) motions: Handle<Image>,
    /// The colour ramp every flame's heat is coloured through.
    #[texture(107)]
    #[sampler(108)]
    pub(super) ramp: Handle<Image>,
    /// The marched breath's value noise (`volume::noise`).
    #[texture(109, dimension = "3d")]
    #[sampler(110)]
    pub(super) noise: Handle<Image>,
    /// The baked loop (`sim`), sampled through the noise's sampler; a texel until a fire reads it.
    #[texture(111, dimension = "3d")]
    pub(super) sim: Handle<Image>,
}

/// Which pipeline a fire material is drawn with: a breath's hull is not depth tested, since
/// its far side stands behind the wall it strikes; its shader ends each ray at what the
/// depth prepass holds instead. The hearths' mask writes only the picture's alpha (`film`);
/// sparks keep out of it in the matte's films (`matte`).
#[repr(C)]
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct FireKey {
    volume: bool,
    mask: bool,
    depth: bool,
    spark: bool,
}

impl From<&FireExtension> for FireKey {
    fn from(fire: &FireExtension) -> Self {
        FireKey {
            volume: fire.volume,
            mask: fire.mask,
            depth: fire.depth,
            spark: fire.uniform.kind.x == 1.0,
        }
    }
}

impl MaterialExtension for FireExtension {
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        if key.bind_group_data.volume
            && let Some(depth) = descriptor.depth_stencil.as_mut()
        {
            depth.depth_compare = Some(bevy::render::render_resource::CompareFunction::Always);
            depth.depth_write_enabled = Some(false);
        }
        if key.bind_group_data.depth
            && let Some(depth) = descriptor.depth_stencil.as_mut()
        {
            depth.depth_write_enabled = Some(true);
        }
        if key.bind_group_data.spark && super::matte::on() {
            super::matte::keep_sparks_out(descriptor);
        }
        // A marched breath writes its depth where its gas grows dense (`MARCHED_DEPTH` in
        // fire.wesl), for the volumetric fog after it; its mask draw writes none.
        let prepass = descriptor.fragment.as_ref().is_some_and(|f| {
            f.shader_defs.iter().any(|d| {
                matches!(d, bevy::shader::ShaderDefVal::Bool(n, true) if n == "PREPASS_PIPELINE")
            })
        });
        if key.bind_group_data.volume && !key.bind_group_data.mask && !prepass {
            if let Some(fragment) = descriptor.fragment.as_mut() {
                fragment.shader_defs.push("MARCHED_DEPTH".into());
            }
            if let Some(depth) = descriptor.depth_stencil.as_mut() {
                depth.depth_write_enabled = Some(true);
            }
        }
        if key.bind_group_data.mask
            && let Some(fragment) = descriptor.fragment.as_mut()
        {
            use bevy::render::render_resource::{BlendComponent, BlendFactor, BlendState};
            // The colour left as it is; the mask added to the alpha, past the 1 the scene
            // holds there (`film`).
            fragment.shader_defs.push("FIRE_MASK".into());
            // And to the vertex stage: the jets' mask hull reaches the ground they light
            // (`hull_lit_box`).
            descriptor.vertex.shader_defs.push("FIRE_MASK".into());
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState {
                    color: BlendComponent {
                        src_factor: BlendFactor::Zero,
                        dst_factor: BlendFactor::One,
                        ..default()
                    },
                    alpha: BlendComponent {
                        src_factor: BlendFactor::One,
                        dst_factor: BlendFactor::One,
                        ..default()
                    },
                });
            }
        }
        Ok(())
    }

    fn vertex_shader() -> ShaderRef {
        "shaders/fire.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/fire.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/fire.wesl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/fire.wesl".into()
    }
}

pub type FireMaterial = ExtendedMaterial<StandardMaterial, FireExtension>;
