//! The render world's half of the volumetric clouds: a compute pass per marked view after the
//! depth prepass and before the main pass, as Bevy's own ambient occlusion runs
//! (`bevy_pbr::ssao`), that marches the clouds at half resolution into the picture the veil lays
//! over the scene (`assets/shaders/cloud_volume.wesl`). Its GPU time is the "clouds" span.

use super::{CloudVolume, CloudVolumeView, Params};
use bevy::{
    core_pipeline::{Core3d, Core3dSystems, prepass::ViewPrepassTextures},
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        diagnostic::RecordDiagnostics,
        render_asset::RenderAssets,
        render_resource::{
            binding_types::{
                sampler, texture_2d, texture_3d, texture_cube, texture_depth_2d,
                texture_storage_2d, uniform_buffer,
            },
            *,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        texture::{FallbackImage, GpuImage},
        view::{ViewUniform, ViewUniformOffset, ViewUniforms},
    },
};

const SHADER: &str = "shaders/cloud_volume.wesl";
/// The march's workgroups, so many pixels a side.
const GROUP: u32 = 8;

pub(super) struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareBindGroups))
            .add_systems(
                Core3d,
                march
                    .after(Core3dSystems::Prepass)
                    .before(Core3dSystems::MainPass),
            );
    }
}

#[derive(Resource)]
struct Pipeline {
    layout: BindGroupLayoutDescriptor,
    id: CachedComputePipelineId,
    repeating: Sampler,
    // The weather map mirrored past its edges, so the sky past its 32 km never repeats its edge
    // texels in stripes to the horizon; the air's cube ignores it.
    mirrored: Sampler,
}

fn init_pipeline(
    mut commands: Commands,
    device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let float = TextureSampleType::Float { filterable: true };
    let layout = BindGroupLayoutDescriptor::new(
        "cloud_volume",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<ViewUniform>(true),
                texture_depth_2d(),
                uniform_buffer::<Params>(false),
                texture_3d(float),
                texture_2d(float),
                texture_2d(float),
                texture_cube(float),
                sampler(SamplerBindingType::Filtering),
                sampler(SamplerBindingType::Filtering),
                texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::WriteOnly),
            ),
        ),
    );
    let id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("cloud_volume".into()),
        layout: vec![layout.clone()],
        shader: asset_server.load(SHADER),
        entry_point: Some("march".into()),
        ..default()
    });
    let linear = |mode: AddressMode| SamplerDescriptor {
        address_mode_u: mode,
        address_mode_v: mode,
        address_mode_w: mode,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_filter: MipmapFilterMode::Linear,
        ..default()
    };
    commands.insert_resource(Pipeline {
        layout,
        id,
        repeating: device.create_sampler(&linear(AddressMode::Repeat)),
        mirrored: device.create_sampler(&linear(AddressMode::MirrorRepeat)),
    });
}

/// The march's bind group for a view, and the picture's size.
#[derive(Component)]
struct Marching {
    bind_group: BindGroup,
    size: UVec2,
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    pipeline: Option<Res<Pipeline>>,
    pipeline_cache: Res<PipelineCache>,
    volume: Option<Res<CloudVolume>>,
    images: Res<RenderAssets<GpuImage>>,
    fallback: Res<FallbackImage>,
    view_uniforms: Res<ViewUniforms>,
    views: Query<(Entity, &ViewPrepassTextures), With<CloudVolumeView>>,
) {
    let (Some(pipeline), Some(volume), Some(view_binding)) =
        (pipeline, volume, view_uniforms.uniforms.binding())
    else {
        return;
    };
    let (Some(target), Some(noise)) = (images.get(&volume.target), images.get(&volume.noise))
    else {
        return;
    };
    let map = |slot: usize| {
        volume
            .maps
            .as_ref()
            .and_then(|m| images.get(&m[slot]))
            .unwrap_or(&fallback.d2)
    };
    let air = volume
        .air
        .as_ref()
        .and_then(|a| images.get(a))
        .unwrap_or(&fallback.cube);
    let mut params = UniformBuffer::from(volume.params);
    params.write_buffer(&device, &queue);
    for (entity, prepass) in &views {
        let Some(depth) = prepass.depth_only_view() else {
            continue;
        };
        let bind_group = device.create_bind_group(
            "cloud_volume",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                view_binding.clone(),
                depth,
                &params,
                &noise.texture_view,
                &map(0).texture_view,
                &map(1).texture_view,
                &air.texture_view,
                &pipeline.repeating,
                &pipeline.mirrored,
                &target.texture_view,
            )),
        );
        commands.entity(entity).insert(Marching {
            bind_group,
            size: target.size_2d(),
        });
    }
}

fn march(
    view: ViewQuery<(&ViewUniformOffset, &Marching), With<CloudVolumeView>>,
    pipeline: Option<Res<Pipeline>>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (offset, marching) = view.into_inner();
    let Some(compute) = pipeline.and_then(|p| pipeline_cache.get_compute_pipeline(p.id)) else {
        return;
    };
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let span = diagnostics.time_span(ctx.command_encoder(), "clouds");
    {
        let mut pass = ctx
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("clouds"),
                timestamp_writes: None,
            });
        pass.set_pipeline(compute);
        pass.set_bind_group(0, &marching.bind_group, &[offset.offset]);
        pass.dispatch_workgroups(
            marching.size.x.div_ceil(GROUP),
            marching.size.y.div_ceil(GROUP),
            1,
        );
    }
    span.end(ctx.command_encoder());
}
