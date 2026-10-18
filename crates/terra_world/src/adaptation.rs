//! The exposure the eye is adapted to, as the frame is shown (D377): for a material that must
//! land on the screen at a known brightness, as fire's hottest cores clip white as they do on
//! film. Bevy's auto exposure meters each frame and adds its correction to the view's colour
//! grading after the scene is drawn, for the tonemapping pass; its own state is private to
//! Bevy, and a material drawn in the main pass sees only the camera's exposure. So after
//! tonemapping a small compute pass copies the view's colour grading exposure, as the
//! tonemapper applied it, into a storage buffer materials bind: a frame late, as adaptation
//! moves over seconds. Without auto exposure it is the grading's own exposure.
//!
//! Unreal hands its materials the same thing, the eye adaptation of the last frame
//! (`EyeAdaptation` in its post-process docs, read by the material graph's `EyeAdaptation`
//! node), so an emissive surface can be authored in screen terms.

use bevy::{
    core_pipeline::{
        schedule::{Core3d, Core3dSystems},
        tonemapping::tonemapping,
    },
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_resource::{binding_types::*, *},
        renderer::{RenderContext, ViewQuery},
        storage::{GpuShaderBuffer, ShaderBuffer},
        view::{ViewUniform, ViewUniformOffset, ViewUniforms},
    },
};

/// The buffer the adapted exposure is kept in: one `vec4`, its x the stops the tonemapper
/// multiplied the frame by (`exp2(x)`), its y the camera's exposure then.
#[derive(Resource, Clone, Debug)]
pub struct Adaptation(pub Handle<ShaderBuffer>);

impl ExtractResource<RenderApp> for Adaptation {
    type Source = Adaptation;

    fn extract_resource(source: &Adaptation) -> Self {
        source.clone()
    }
}

#[derive(Resource)]
struct CopyPipeline {
    layout: BindGroupLayoutDescriptor,
    id: CachedComputePipelineId,
}

const SHADER: &str = "shaders/adaptation.wesl";

pub fn plugin(app: &mut App) {
    let buffer = app
        .world_mut()
        .resource_mut::<Assets<ShaderBuffer>>()
        .add(ShaderBuffer::from(vec![[0.0f32, 1.0, 0.0, 0.0]]));
    app.insert_resource(Adaptation(buffer));
    if app.get_sub_app(RenderApp).is_none() {
        return;
    }
    app.add_plugins(ExtractResourcePlugin::<Adaptation>::default());
    app.sub_app_mut(RenderApp)
        .add_systems(RenderStartup, init_pipeline)
        .add_systems(
            Core3d,
            copy.after(tonemapping).in_set(Core3dSystems::PostProcess),
        );
}

fn init_pipeline(mut commands: Commands, assets: Res<AssetServer>, cache: Res<PipelineCache>) {
    let layout = BindGroupLayoutDescriptor::new(
        "adaptation copy",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                storage_buffer_read_only::<ViewUniform>(true),
                storage_buffer_sized(false, core::num::NonZero::<u64>::new(16)),
            ),
        ),
    );
    let id = cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("adaptation copy".into()),
        layout: vec![layout.clone()],
        shader: assets.load(SHADER),
        entry_point: Some("main".into()),
        ..default()
    });
    commands.insert_resource(CopyPipeline { layout, id });
}

/// After the tonemapper, each 3D view's exposure as it applied it into the buffer; with more
/// than one 3D view the last drawn is kept.
fn copy(
    view: ViewQuery<&ViewUniformOffset>,
    pipeline: Option<Res<CopyPipeline>>,
    cache: Res<PipelineCache>,
    views: Res<ViewUniforms>,
    adaptation: Option<Res<Adaptation>>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    mut ctx: RenderContext,
) {
    let offset = view.into_inner();
    let (Some(pipeline), Some(adaptation)) = (pipeline, adaptation) else {
        return;
    };
    let (Some(compute), Some(views_buffer), Some(target)) = (
        cache.get_compute_pipeline(pipeline.id),
        views.uniforms.buffer(),
        buffers.get(&adaptation.0),
    ) else {
        return;
    };
    let group = ctx.render_device().create_bind_group(
        None,
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((
            BufferBinding {
                buffer: views_buffer,
                size: Some(ViewUniform::min_size()),
                offset: 0,
            },
            target.buffer.as_entire_buffer_binding(),
        )),
    );
    let mut pass = ctx
        .command_encoder()
        .begin_compute_pass(&ComputePassDescriptor {
            label: Some("adaptation copy"),
            timestamp_writes: None,
        });
    pass.set_bind_group(0, &group, &[offset.offset]);
    pass.set_pipeline(compute);
    pass.dispatch_workgroups(1, 1, 1);
}
