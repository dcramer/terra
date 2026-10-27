//! The fires drawn after the haze, each card fogged at its own depth (D553). Bevy's volumetric
//! fog runs after the whole main pass, transparents included, and marches each pixel on to the
//! depth buffer; smoke writes no depth, so a puff 6 m off with the sky behind was fogged as if
//! it stood at the sky, dimmed and washed pale, and changed look sharply at the horizon, where
//! what stands behind it jumps from the box's far side to the ground (fire round t12). So the
//! fires' cards ([`AfterFog`]) are taken out of the transparent pass and drawn in a pass of
//! their own after the fog, in the order they were sorted, and their shaders take the haze
//! between the eye and the card by the fog's own law (`assets/shaders/fire_fog.wesl`), from
//! what [`tell`] writes each frame of the haze's fog volume and the camera's fog.
//!
//! The general fix, for later: a froxel volume of the haze's light and transmittance fit to the
//! camera's frustum, which every translucent samples at its own depth (Wronski 2014; Unreal's
//! and HDRP's volumetric fog; Bevy's atmosphere's `aerial_view_lut`), so lakes, clouds and
//! windows are hazed at their depth too. Here a fire's card drawn over another transparent
//! (a lake's water in front of it) is drawn after it whatever their depths.

use super::{FireMaterial, FireSmokeMaterial};
use crate::{planet::MainCamera, scene::Haze};
use bevy::{
    camera::{MainPassResolutionOverride, Viewport},
    core_pipeline::{
        core_3d::Transparent3d,
        schedule::{Core3d, Core3dSystems},
    },
    light::{FogVolume, VolumetricFog},
    platform::collections::HashMap,
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        camera::ExtractedCamera,
        diagnostic::RecordDiagnostics,
        render_phase::{SortedRenderPhase, ViewSortedRenderPhases},
        render_resource::{RenderPassDescriptor, StoreOp},
        renderer::{RenderContext, ViewQuery},
        storage::ShaderBuffer,
        sync_world::{MainEntity, MainEntityHashSet},
        view::{ExtractedView, RetainedViewEntity, ViewDepthStencilTexture, ViewTarget},
    },
};

/// A fire's card mesh, drawn after the fog and fogged by its own shader.
#[derive(Component, Clone, Copy, Default)]
pub(super) struct AfterFog;

/// A smoke material as the fires draw it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Smoke {
    /// Which smoke: 0 over the blazes, 1 a breath's soot (`fire_smoke.wesl`'s `kind.x`).
    pub kind: f32,
    /// The torches' smoke alone (`kind.z`), drawn in Bevy's transparent pass and fogged by the
    /// fog pass as before, at the sky's depth over the sky: drawn after the fog, a torch's thin
    /// thread of puffs over the sky showed as a string of lit, rimmed beads that the old fog had
    /// dimmed (fire round t14's panel), so it stays out until the beads are found. The other
    /// materials draw every smoke but the torches'.
    pub torches: bool,
}

impl Smoke {
    /// Whether its cards are drawn after the fog, fogged by their shader.
    pub(super) fn after_fog(self) -> bool {
        !self.torches
    }
}

/// The smoke materials: the blazes' and hearths', a breath's soot, the torches'.
pub(super) const SMOKES: [Smoke; 3] = [
    Smoke {
        kind: 0.0,
        torches: false,
    },
    Smoke {
        kind: 1.0,
        torches: false,
    },
    Smoke {
        kind: 0.0,
        torches: true,
    },
];

/// How many vec4s the haze is told in: `fire_fog.wesl`'s `Haze`.
pub(super) const HAZE_VEC4S: usize = 9;

/// Bevy's map from a fog volume's 1x1x1 local space to its density picture's
/// (`volumetric_fog/render.rs`'s `UVW_FROM_LOCAL`).
const UVW_FROM_LOCAL: Mat4 =
    Mat4::from_cols(Vec4::X, Vec4::Y, Vec4::Z, Vec4::new(0.5, 0.5, 0.5, 1.0));

/// The haze as `fire_fog.wesl` reads it, as the fog pass is told it (`render.rs`'s
/// `prepare_volumetric_fog_uniforms`): none with no haze, or none on the camera.
pub(super) fn told(
    haze: Option<(&FogVolume, &GlobalTransform)>,
    fog: Option<&VolumetricFog>,
) -> [[f32; 4]; HAZE_VEC4S] {
    let mut told = [[0.0; 4]; HAZE_VEC4S];
    let (Some((volume, at)), Some(fog)) = (haze, fog) else {
        return told;
    };
    let world_from_local = at.affine();
    let uvw_from_world = UVW_FROM_LOCAL * Mat4::from(world_from_local.inverse());
    told[..4].copy_from_slice(&uvw_from_world.to_cols_array_2d());
    let colour = volume.fog_color.to_linear().to_vec3();
    told[4] = colour.extend(volume.density_factor).to_array();
    let tint = volume.light_tint.to_linear().to_vec3();
    told[5] = tint.extend(volume.light_intensity).to_array();
    let ambient = fog.ambient_color.to_linear().to_vec3() * fog.ambient_intensity;
    // The view is rigid, so the bounding sphere's radius is the box's own half diagonal.
    let radius = world_from_local
        .transform_vector3(Vec3::splat(0.5))
        .length();
    told[6] = ambient.extend(radius).to_array();
    told[7] = [
        volume.absorption,
        volume.scattering,
        volume.scattering_asymmetry,
        0.0,
    ];
    told[8] = volume.density_texture_offset.extend(0.0).to_array();
    told
}

/// Tells the fires' materials the haze this frame: its buffer, and its density picture.
pub(super) fn tell(
    drawn: Option<Res<super::Drawn>>,
    haze: Query<(&FogVolume, &GlobalTransform), With<Haze>>,
    camera: Query<Option<&VolumetricFog>, With<MainCamera>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut fires: ResMut<Assets<FireMaterial>>,
    mut smokes: ResMut<Assets<FireSmokeMaterial>>,
) {
    let Some(drawn) = drawn else {
        return;
    };
    let volume = haze.iter().next();
    let told = told(volume, camera.iter().next().flatten());
    if let Some(mut buffer) = buffers.get_mut(&drawn.haze) {
        *buffer = ShaderBuffer::from(told.to_vec());
    }
    let density = volume.and_then(|(v, _)| v.density_texture.clone());
    for handle in &drawn.fire {
        if fires
            .get(handle)
            .is_some_and(|m| m.extension.haze_density != density)
        {
            fires.get_mut(handle).unwrap().extension.haze_density = density.clone();
        }
    }
    for handle in &drawn.smoke {
        if smokes
            .get(handle)
            .is_some_and(|m| m.extension.haze_density != density)
        {
            smokes.get_mut(handle).unwrap().extension.haze_density = density.clone();
        }
    }
}

/// The main entities of the cards drawn after the fog, this frame.
#[derive(Resource, Default)]
struct Seen(MainEntityHashSet);

/// Each view's cards drawn after the fog, as the transparent phase sorted and batched them.
#[derive(Resource, Default)]
struct Late(HashMap<RetainedViewEntity, SortedRenderPhase<Transparent3d>>);

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        tell.after(bevy::transform::TransformSystems::Propagate),
    );
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .init_resource::<Seen>()
        .init_resource::<Late>()
        .add_systems(ExtractSchedule, extract)
        .add_systems(Render, split.in_set(RenderSystems::PrepareBindGroups))
        .add_systems(
            Core3d,
            draw.after(bevy::pbr::volumetric_fog_pass)
                .after(Core3dSystems::MainPass)
                .before(Core3dSystems::EarlyPostProcess),
        );
}

fn extract(mut seen: ResMut<Seen>, cards: Extract<Query<Entity, With<AfterFog>>>) {
    seen.0.clear();
    seen.0.extend(cards.iter().map(MainEntity::from));
}

/// Takes a view's cards drawn after the fog out of its transparent phase into `late`, once
/// sorted and batched, in their order: their batch ranges move with them, and in the
/// transparent phase each is left with none, so its pass steps over it. A batch is of one
/// material, so it moves whole.
fn take_late(
    phase: &mut SortedRenderPhase<Transparent3d>,
    late: &mut SortedRenderPhase<Transparent3d>,
    seen: &MainEntityHashSet,
) {
    late.items.clear();
    for (key, item) in phase.items.iter_mut() {
        if !seen.contains(&item.entity.1) {
            continue;
        }
        late.items.insert(
            *key,
            Transparent3d {
                sorting_info: item.sorting_info,
                distance: item.distance,
                pipeline: item.pipeline,
                entity: item.entity,
                draw_function: item.draw_function,
                batch_range: item.batch_range.clone(),
                extra_index: item.extra_index.clone(),
                indexed: item.indexed,
            },
        );
        item.batch_range = 0..0;
    }
}

fn split(
    seen: Res<Seen>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut late: ResMut<Late>,
) {
    late.0.retain(|view, _| phases.contains_key(view));
    for (view, phase) in phases.iter_mut() {
        take_late(phase, late.0.entry(*view).or_default(), &seen.0);
    }
}

/// The fires' pass, after the fog: as Bevy's `main_transparent_pass_3d`, over the cards it
/// stepped over.
fn draw(
    world: &World,
    view: ViewQuery<(
        &ExtractedCamera,
        &ExtractedView,
        &ViewTarget,
        &ViewDepthStencilTexture,
        Option<&MainPassResolutionOverride>,
    )>,
    late: Res<Late>,
    mut ctx: RenderContext,
) {
    let view_entity = view.entity();
    let (camera, extracted_view, target, depth, resolution_override) = view.into_inner();
    let Some(phase) = late.0.get(&extracted_view.retained_view_entity) else {
        return;
    };
    if phase.items.is_empty() {
        return;
    }
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("fires_after_fog"),
        color_attachments: &[Some(target.get_color_attachment())],
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "fires_after_fog");
    if let Some(viewport) =
        Viewport::from_viewport_and_override(camera.viewport.as_ref(), resolution_override)
    {
        pass.set_camera_viewport(&viewport);
    }
    if let Err(err) = phase.render(&mut pass, world, view_entity) {
        error!("The fires' pass after the fog: {err:?}");
    }
    span.end(&mut pass);
}

#[cfg(test)]
#[path = "fogged_tests.rs"]
mod tests;
