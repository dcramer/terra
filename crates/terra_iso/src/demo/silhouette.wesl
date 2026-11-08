// The hero drawn through what stands before him (demo/silhouette.rs): a second draw of his
// meshes, unlit, where something is nearer the lens than he is by more than `look.w` metres.
// The pipeline's depth test is set to pass only behind what is drawn (silhouette.rs
// `specialize`); this fragment moves its own depth that far toward the camera first, so his
// own arm before his chest, or grass at his feet, does not tint him.

import bevy_pbr::render::{
    forward_io::VertexOutput,
    mesh_view_bindings::view,
    view_transformations::position_world_to_ndc,
};

// rgb the fill as it reaches the screen, a its alpha; then the edge's colour and alpha.
@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> fill: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var<uniform> edge: vec4<f32>;
// xyz unused; w the gap (m) a surface must stand before him to draw him through it.
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var<uniform> look: vec4<f32>;
// xyz a point on his front's plane (silhouette.rs `Front`, his nearest joint less a limb's
// thickness), w 1 once known: no fragment lies behind it, so his limbs never draw him through
// himself (mid-roll they stand a metre before his back).
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var<uniform> front: vec4<f32>;

struct Out {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fragment(in: VertexOutput) -> Out {
    let p = in.world_position.xyz;
    let to_eye = normalize(view.world_position - p);
    let n = normalize(in.world_normal);
    // Fresnel: the fill at the middle of his figure, the edge toward its outline.
    let f = pow(1.0 - clamp(abs(dot(n, to_eye)), 0.0, 1.0), 2.0);
    var out: Out;
    out.color = mix(fill, edge, f);
    // Reverse Z: the nearer of the two is the larger.
    var depth = position_world_to_ndc(p + to_eye * look.w).z;
    if front.w > 0.5 {
        depth = max(depth, position_world_to_ndc(front.xyz).z);
    }
    out.depth = depth;
    return out;
}
