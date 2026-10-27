// Lays the volumetric clouds' half-resolution picture (cloud_volume.wesl) over the scene:
// premultiplied, the light the clouds send the eye and how much they hide behind them.

import bevy_pbr::render::{
    forward_io::VertexOutput,
    mesh_view_bindings::view,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
var picture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1)
var picture_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = (mesh.position.xy - view.viewport.xy) / view.viewport.zw;
    return textureSampleLevel(picture, picture_sampler, uv, 0.0);
}
