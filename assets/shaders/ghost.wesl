// A plan's pieces not yet built (docs/building-plan.md, drawing): a blueprint in the air,
// each piece a faint glass-blue shape whose edges are drawn as fine lines, and whose
// outline brightens where its faces turn away, so a whole plan reads as its pieces
// rather than a pale sheet. Each shape's texture coordinates run from 0 to 1 across each
// of its faces, so their whole numbers are its edges; a block kind's shell has one a cube.

import bevy_pbr::render::{
    forward_io::VertexOutput,
    mesh_view_bindings::view,
};

// Field order must match GhostMaterial in build/draw.rs.
struct Ghost {
    colour: vec4<f32>,
    // How much an edge line and an outline add to how opaque it is.
    edge: f32,
    rim: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
var<uniform> ghost: Ghost;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let to_eye = normalize(view.world_position - in.world_position.xyz);
    // Brighter where the face turns away from the eye: its outline.
    let rim = pow(1.0 - abs(dot(normal, to_eye)), 3.0);
    // A line about a pixel and a half wide along each edge of each face.
    let uv = in.uv;
    let width = max(fwidth(uv) * 1.5, vec2<f32>(1e-4));
    let off = min(fract(uv), 1.0 - fract(uv));
    let along = vec2<f32>(1.0) - smoothstep(width * 0.5, width, off);
    let line = max(along.x, along.y);
    let alpha = clamp(ghost.colour.a + ghost.rim * rim + ghost.edge * line, 0.0, 0.9);
    let colour = ghost.colour.rgb * (0.8 + 0.5 * rim + 0.5 * line);
    return vec4<f32>(colour, alpha);
}
