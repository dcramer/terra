//! The render world's half of the volumetric clouds: a compute pass per marked view after the
//! depth prepass and before the main pass, as Bevy's own ambient occlusion runs
//! (`bevy_pbr::ssao`), that marches the clouds at half resolution (`assets/shaders/cloud_volume.wesl`),
//! each frame at its own offset inside the pixel, then resolves that into a history of its own,
//! reprojected (`cloud_resolve.wesl`, D617), which is the picture the veil lays over the scene.
//! Their GPU times are the "clouds" and "clouds_resolve" spans.

use super::{CloudVolume, CloudVolumeView, Params};
use bevy::{
    core_pipeline::{Core3d, Core3dSystems, prepass::ViewPrepassTextures},
    diagnostic::FrameCount,
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
        texture::{CachedTexture, FallbackImage, GpuImage, TextureCache},
        view::{ExtractedView, ViewUniform, ViewUniformOffset, ViewUniforms},
    },
};

const SHADER: &str = "shaders/cloud_volume.wesl";
const RESOLVE: &str = "shaders/cloud_resolve.wesl";
/// How many offsets inside the pixel the march cycles through (Halton 2, 3).
const OFFSETS: u32 = 16;

/// What the march and the resolve know of this frame and the last (`cloud_frame.wesl`).
#[derive(Clone, Copy, ShaderType)]
struct Frame {
    world_from_clip: Mat4,
    clip_from_world_before: Mat4,
    jitter: Vec4,
}

/// A view's cloud history, kept in the render world from frame to frame: last frame's turn and
/// projection, how many frames it has resolved and at which picture size.
#[derive(Component)]
struct History {
    clip_from_world: Mat4,
    frames: u32,
    size: UVec2,
}

/// The Halton sequence's `index`th number in `base`, in [0, 1).
fn halton(mut index: u32, base: u32) -> f32 {
    let (mut f, mut r) = (1.0, 0.0);
    while index > 0 {
        f /= base as f32;
        r += f * (index % base) as f32;
        index /= base;
    }
    r
}

/// The view's turn and projection alone, unjittered: a way in the world to its clip space.
fn turned_clip_from_world(clip_from_view: Mat4, world_from_view: &GlobalTransform) -> Mat4 {
    clip_from_view * Mat4::from_mat3a(world_from_view.affine().matrix3).inverse()
}
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
    resolve_layout: BindGroupLayoutDescriptor,
    resolve: CachedComputePipelineId,
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
                uniform_buffer::<Frame>(false),
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
    let storage =
        || texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::WriteOnly);
    let resolve_layout = BindGroupLayoutDescriptor::new(
        "cloud_resolve",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<Frame>(false),
                texture_2d(float),
                texture_2d(float),
                sampler(SamplerBindingType::Filtering),
                storage(),
                storage(),
            ),
        ),
    );
    let resolve = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("cloud_resolve".into()),
        layout: vec![resolve_layout.clone()],
        shader: asset_server.load(RESOLVE),
        entry_point: Some("resolve".into()),
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
        resolve_layout,
        resolve,
        repeating: device.create_sampler(&linear(AddressMode::Repeat)),
        mirrored: device.create_sampler(&linear(AddressMode::MirrorRepeat)),
    });
}

/// The march's and the resolve's bind groups for a view, and the picture's size.
#[derive(Component)]
struct Marching {
    bind_group: BindGroup,
    resolve: BindGroup,
    size: UVec2,
}

/// A storage texture of the picture's size from the cache, kept from frame to frame by `label`.
fn picture(
    cache: &mut TextureCache,
    device: &RenderDevice,
    label: &'static str,
    size: UVec2,
) -> CachedTexture {
    cache.get(
        device,
        TextureDescriptor {
            label: Some(label),
            size: Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
    )
}

/// This frame's `Frame` for a view at the game's frame `count`, and its history moved on a frame.
fn next_frame(
    view: &ExtractedView,
    history: Option<&History>,
    size: UVec2,
    count: u32,
) -> (Frame, History) {
    let now = turned_clip_from_world(view.clip_from_view, &view.world_from_view);
    let kept = history.filter(|h| h.size == size);
    let frames = kept.map_or(0, |h| h.frames.wrapping_add(1));
    // The offset by the game's frame count, a scripted run's script frame, as TAA's jitter
    // takes it (D472), so two runs of one build march one frame of one script alike.
    let index = count % OFFSETS + 1;
    // Off, one ray through each pixel's middle of the view's own (TAA-jittered) projection and
    // nothing kept, as before D617. Under the temporal upscaler it is off whatever the tune says:
    // there the upscaler's own history over the cloud's was 15% further from the supersampled
    // reference than the march alone (play-3080 j12).
    let on = super::HISTORY.get() >= 0.5 && crate::upscale::TEMPORAL.get() < 0.5;
    let offset = Vec2::new(halton(index, 2), halton(index, 3)) - 0.5;
    let frame = Frame {
        world_from_clip: now.inverse(),
        clip_from_world_before: kept.map_or(now, |h| h.clip_from_world),
        jitter: (if on { offset } else { Vec2::ZERO })
            .extend(if on && kept.is_some() { 0.0 } else { 1.0 })
            .extend(if on { 0.0 } else { 1.0 }),
    };
    let history = History {
        clip_from_world: now,
        frames,
        size,
    };
    (frame, history)
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
    mut cache: ResMut<TextureCache>,
    count: Res<FrameCount>,
    views: Query<
        (
            Entity,
            &ViewPrepassTextures,
            &ExtractedView,
            Option<&History>,
        ),
        With<CloudVolumeView>,
    >,
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
    for (entity, prepass, view, history) in &views {
        let Some(depth) = prepass.depth_only_view() else {
            continue;
        };
        let size = target.size_2d();
        let (now, history) = next_frame(view, history, size, count.0);
        let mut frame = UniformBuffer::from(now);
        frame.write_buffer(&device, &queue);
        let marched = picture(&mut cache, &device, "cloud_march", size);
        let a = picture(&mut cache, &device, "cloud_history_a", size);
        let b = picture(&mut cache, &device, "cloud_history_b", size);
        let (before, next) = if history.frames % 2 == 0 {
            (&a, &b)
        } else {
            (&b, &a)
        };
        let resolve = device.create_bind_group(
            "cloud_resolve",
            &pipeline_cache.get_bind_group_layout(&pipeline.resolve_layout),
            &BindGroupEntries::sequential((
                &frame,
                &marched.default_view,
                &before.default_view,
                &pipeline.mirrored,
                &next.default_view,
                &target.texture_view,
            )),
        );
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
                &marched.default_view,
                &frame,
            )),
        );
        commands.entity(entity).insert((
            Marching {
                bind_group,
                resolve,
                size,
            },
            history,
        ));
    }
}

fn march(
    view: ViewQuery<(&ViewUniformOffset, &Marching), With<CloudVolumeView>>,
    pipeline: Option<Res<Pipeline>>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (offset, marching) = view.into_inner();
    let Some(pipeline) = pipeline else {
        return;
    };
    let (Some(compute), Some(resolve)) = (
        pipeline_cache.get_compute_pipeline(pipeline.id),
        pipeline_cache.get_compute_pipeline(pipeline.resolve),
    ) else {
        return;
    };
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let groups = (
        marching.size.x.div_ceil(GROUP),
        marching.size.y.div_ceil(GROUP),
    );
    // Marched, then resolved into the history in a pass of its own, so the march's writes are
    // done before the resolve reads them; each its own span.
    for (label, compute, bind_group, offsets) in [
        (
            "clouds",
            compute,
            &marching.bind_group,
            &[offset.offset][..],
        ),
        ("clouds_resolve", resolve, &marching.resolve, &[][..]),
    ] {
        let span = diagnostics.time_span(ctx.command_encoder(), label);
        {
            let mut pass = ctx
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some(label),
                    timestamp_writes: None,
                });
            pass.set_pipeline(compute);
            pass.set_bind_group(0, bind_group, offsets);
            pass.dispatch_workgroups(groups.0, groups.1, 1);
        }
        span.end(ctx.command_encoder());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_spread_over_the_pixel_round_its_middle() {
        let offsets: Vec<Vec2> = (1..=OFFSETS)
            .map(|i| Vec2::new(halton(i, 2), halton(i, 3)) - 0.5)
            .collect();
        let mean = offsets.iter().copied().sum::<Vec2>() / OFFSETS as f32;
        assert!(mean.length() < 0.05, "mean offset {mean}");
        // Every quarter of the pixel is visited about as often.
        for quarter in 0..4 {
            let n = offsets
                .iter()
                .filter(|o| (o.x >= 0.0) as u32 + 2 * (o.y >= 0.0) as u32 == quarter)
                .count();
            assert!(
                (3..=5).contains(&n),
                "quarter {quarter} has {n} of {OFFSETS}"
            );
        }
    }

    #[test]
    fn a_way_reprojects_to_where_a_turned_view_sees_it_far_from_the_origin() {
        use bevy::camera::CameraProjection;
        let projection = PerspectiveProjection {
            fov: 1.0,
            aspect_ratio: 16.0 / 9.0,
            near: 0.1,
            ..default()
        }
        .get_clip_from_view();
        // Some 30 km from the planet's middle, where f32 holds a position to 2 mm.
        let at = Vec3::new(1200.0, 30_000.0, -800.0);
        let eye = Transform::from_translation(at).looking_to(Vec3::X, Vec3::Y);
        let now = turned_clip_from_world(projection, &GlobalTransform::from(eye));
        let back = now.inverse() * Vec4::new(0.0, 0.0, 1.0, 1.0);
        let way = (back.xyz() / back.w).normalize();
        assert!(way.distance(Vec3::X) < 1e-5, "the middle looks along {way}");
        // Seen from a view turned to its right by a tenth's tangent, the same way sits left of
        // the middle by that tangent in the projection's units.
        let turned = Transform::from_translation(at).looking_to(Vec3::new(1.0, 0.0, 0.1), Vec3::Y);
        let before =
            turned_clip_from_world(projection, &GlobalTransform::from(turned)) * way.extend(0.0);
        let ndc = before.xy() / before.w;
        let expected = -0.1 * projection.x_axis.x;
        assert!(
            (ndc.x - expected).abs() < 1e-4 && ndc.y.abs() < 1e-5,
            "at {ndc}, wanted x {expected}"
        );
    }
}
