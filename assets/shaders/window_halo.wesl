// A lit window's halo (crates/terra_folk/src/town/windows.rs): the soft glow a bright window
// throws round itself in the eye and the lens, drawn as a card turned to the eye and added to
// what is behind it, as shipped games draw a far light's corona (GTA's and RDR2's coronas,
// Frostbite's light halos) where bloom alone cannot: a window a few pixels across blooms by
// too little to read. The mesh is a unit quad at the window, facing out of the wall; the
// card is turned to the eye about its middle, stood a little out toward the eye so the
// wall does not cut it, never smaller on the screen than a few pixels, and faded as the
// window turns away (none from behind its wall) and with the light it is given.

import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings::{view, globals},
    view_transformations::position_world_to_clip,
};
import super::window_flicker::flicker;

@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{Vertex, VertexOutput, decompress_vertex};
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex};

// Field order must match HaloExtension in windows.rs.
struct Halo {
    // rgb its colour at its middle, linear and before exposure; w how wide it is against the
    // window's own size.
    colour: vec4<f32>,
    // x the least it spans of the view's height, y how far it stands toward the eye in
    // metres, z its look's flicker phase (window_flicker.wesl).
    shape: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> halo: Halo;

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    let middle = (world_from_local * vec4<f32>(0.0, 0.0, 0.0, 1.0)).xyz;
    let facing = normalize((world_from_local * vec4<f32>(0.0, 0.0, 1.0, 0.0)).xyz);
    // The window's size, from its quad's scale.
    let wide = length((world_from_local * vec4<f32>(1.0, 0.0, 0.0, 0.0)).xyz);
    let tall = length((world_from_local * vec4<f32>(0.0, 1.0, 0.0, 0.0)).xyz);
    let to_eye_far = view.world_position - middle;
    let far = length(to_eye_far);
    let to_eye = to_eye_far / max(far, 1e-3);
    // How square to the eye the window is: none from behind its wall or edge on.
    let square = clamp(dot(facing, to_eye), 0.0, 1.0);
    let at = middle + to_eye * min(halo.shape.y, far * 0.5);
    // Its size: the window's own, widened, and never under so much of the view's height.
    let own = sqrt(wide * tall) * halo.colour.w;
    // view.clip_from_view[1][1] is 1 over the tangent of half the view's height.
    let least = halo.shape.x * 2.0 * far / view.clip_from_view[1][1];
    let size = max(own, least);
    var right = cross(view.world_from_view[1].xyz, to_eye);
    if dot(right, right) < 1.0e-6 {
        right = vec3<f32>(1.0, 0.0, 0.0);
    }
    right = normalize(right);
    let up = cross(to_eye, right);
    let corner = vertex.position.xy * size;
    let world = at + right * corner.x + up * corner.y;
    out.position = position_world_to_clip(world);
    out.world_position = vec4<f32>(world, 1.0);
    // The corner, and how much of the window's light the card is given: grown past its own
    // size on the screen, its light spread thinner, but not all the way, so a far window's
    // corona still reads as a point of warm light.
    out.uv = vertex.position.xy * 2.0;
    let spread = own / size;
    out.uv_b = vec2<f32>(square * sqrt(square) * mix(spread, 1.0, 0.35), 0.0);
@if(PREPASS_PIPELINE) {
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS)
    out.world_normal = to_eye;
@if(MOTION_VECTOR_PREPASS)
    out.previous_world_position = out.world_position;
} @else {
    out.world_normal = to_eye;
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
    return out;
}

@if(PREPASS_PIPELINE && PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput) -> bevy_pbr::prepass::io::FragmentOutput {
    discard;
}

@if(PREPASS_PIPELINE && !PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput) {
    discard;
}

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    // A lens's glow round a point: a tight core and a wide soft skirt, nothing at the edge.
    let r2 = dot(in.uv, in.uv);
    if r2 >= 1.0 {
        discard;
    }
    let edge = 1.0 - r2;
    let glow = (0.6 * exp(-r2 * 14.0) + 0.4 * exp(-r2 * 4.0)) * edge * edge;
    var out: FragmentOutput;
    // Added to what is behind it, its alpha nought so the film's mask behind it stays.
    // Flickering with the window's hearth, from the clock, so the material is never written
    // for it.
    let light = halo.colour.rgb * flicker(globals.time, halo.shape.z);
    out.color = vec4<f32>(light * glow * in.uv_b.x, 0.0);
    return out;
}
