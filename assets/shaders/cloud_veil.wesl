// Lays the volumetric clouds' half-resolution picture (cloud_volume.wesl) over the scene:
// premultiplied, the light the clouds send the eye and how much they hide behind them.
//
// The scene is drawn through the main pass's jittered projection, and a temporal resolve after it
// (TAA, or the upscaler, D616) takes each pixel as a sample at its jittered place, as FSR 2's
// integration guide asks of everything drawn before it. The clouds' resolved picture (D617) was
// marched through the unjittered projection, so it is read where this fragment's jittered ray
// points; read at the fragment's own place it stood still under the jitter, and the upscaler took
// it as a picture a jitter's width off each frame (temporal-default j1: the cloud crop 1.53 from
// the supersampled reference this way, 1.30 read at the jittered ray).

import bevy_pbr::render::{
    forward_io::VertexOutput,
    mesh_view_bindings::view,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
var picture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1)
var picture_sampler: sampler;
// x 1 when the picture was marched unjittered (the clouds' own history on), 0 when the march
// followed the jitter itself.
@group(constants::MATERIAL_BIND_GROUP) @binding(2)
var<uniform> unjittered: vec4<f32>;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = (mesh.position.xy - view.viewport.xy) / view.viewport.zw;
    // The jitter is the projection's view-z column moved in clip space (Bevy's
    // `TemporalJitter::jitter_projection`); the unjittered column through the view's own axes, a
    // direction, so the 30 km of the planet's frame costs it no precision.
    let still = view.unjittered_clip_from_world * view.world_from_view[2];
    let jitter = (view.clip_from_view[2].xy - still.xy) * unjittered.x;
    return textureSampleLevel(picture, picture_sampler, uv + vec2<f32>(0.5, -0.5) * jitter, 0.0);
}
