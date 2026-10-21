//! Every material here fits the samplers a card lets a shader stage bind. WebGPU promises
//! 16 a stage, and Apple's cards give no more: a material past it has no pipeline and the
//! frame is never drawn (the ground's seventh sampler did that on a Mac, ea93cecc).
//!
//! Each material's own layout is built as Bevy builds it, on wgpu's stub device, so no card is
//! needed; what Bevy's view binds beside it is a number measured below.

use bevy::render::render_resource::{AsBindGroup, BindingType, ShaderStages};
use bevy::render::renderer::RenderDevice;

/// The samplers WebGPU promises a shader stage, and all a Mac gives.
const STAGE_SAMPLERS: u32 = 16;

/// What Bevy's view binds the fragment stage beside a material (bevy_pbr's
/// mesh_view_bindings.rs: the shadows', the environment map's and its look-up tables'), as
/// measured on a Mac: the ground stood at 18 when it bound 13 of its own, the standard
/// material's 6 and 7 of its own.
const VIEW_SAMPLERS: u32 = 5;

/// How many samplers a material binds the fragment stage, from its layout as Bevy makes it.
fn fragment_samplers<M: AsBindGroup>(device: &RenderDevice) -> u32 {
    M::bind_group_layout_entries(device, false)
        .iter()
        .filter(|entry| entry.visibility.contains(ShaderStages::FRAGMENT))
        .filter(|entry| matches!(entry.ty, BindingType::Sampler(_)))
        .map(|entry| entry.count.map_or(1, |n| n.get()))
        .sum()
}

/// A material's name and the samplers it binds the fragment stage.
fn of<M: AsBindGroup>(device: &RenderDevice) -> (&'static str, u32) {
    let name = core::any::type_name::<M>();
    let name = name
        .rsplit("::")
        .next()
        .unwrap_or(name)
        .trim_end_matches('>');
    (name, fragment_samplers::<M>(device))
}

#[test]
fn every_material_fits_a_stages_samplers() {
    let (device, _queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    let device = RenderDevice::from(device);
    let materials = [
        of::<crate::planet::GroundMaterial>(&device),
        of::<crate::planet::WaterMaterial>(&device),
        of::<crate::cover::GrassMaterial>(&device),
        of::<crate::cover::litter::LitterMaterial>(&device),
        of::<crate::grown::Foliage>(&device),
        of::<crate::flora::WindMaterial>(&device),
        of::<crate::woods::FarWoods>(&device),
        of::<crate::lakes::LakeMaterial>(&device),
        of::<crate::lakes::MistMaterial>(&device),
        of::<crate::drift::DriftMaterial>(&device),
        of::<crate::smoke::SmokeMaterial>(&device),
        of::<crate::fire::FireMaterial>(&device),
        of::<crate::fire::FireSmokeMaterial>(&device),
    ];
    let over: Vec<_> = materials
        .iter()
        .filter(|(_, own)| own + VIEW_SAMPLERS > STAGE_SAMPLERS)
        .map(|(name, own)| format!("{name}: {} of {STAGE_SAMPLERS}", own + VIEW_SAMPLERS))
        .collect();
    assert!(
        over.is_empty(),
        "past the samplers a stage may bind; share one sampler between textures that sample \
         alike (ground.wesl's map_sampler): {over:?}"
    );
}
