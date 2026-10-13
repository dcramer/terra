// A body that hides the skin under what it wears (docs/characters.md, D128). Each face of the
// body is in a patch, written into its second texture coordinate (u = patch + 0.5); a patch
// whose bit is set in the mask is collapsed to a point, so its triangles draw nothing in the
// main pass, the depth and normal prepass, or shadows. Otherwise Bevy's own mesh and prepass
// vertex shaders (bevy_pbr's render/mesh.wesl and prepass/prepass.wesl), unchanged, for
// skinned and unskinned meshes alike.

import bevy_pbr::render::{
    mesh_bindings::mesh,
    mesh_functions,
    skinning,
    morph,
    view_transformations::position_world_to_clip,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{Vertex, UncompressedVertex, VertexOutput, decompress_vertex};
@else
import bevy_pbr::render::forward_io::{Vertex, UncompressedVertex, VertexOutput, decompress_vertex};

// Field order must match BodyMask in character.rs: 1024 bits, one per patch.
struct BodyMask {
    hidden: array<vec4<u32>, 8>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> mask: BodyMask;

// `patch` is a reserved word in WGSL, so the patch is `which`.
fn is_hidden(which: u32) -> bool {
    let word = mask.hidden[which / 128u][(which / 32u) % 4u];
    return (word & (1u << (which % 32u))) != 0u;
}

@if(MORPH_TARGETS)
fn morph_vertex(vertex_in: UncompressedVertex, instance_index: u32) -> UncompressedVertex {
    var vertex = vertex_in;
    let first_vertex = mesh[instance_index].first_vertex_index;
    let vertex_index = vertex.index - first_vertex;
    let morph_descriptor_index = mesh[instance_index].morph_descriptor_index;
    let weight_count = morph::layer_count(morph_descriptor_index);
    for (var i: u32 = 0u; i < weight_count; i++) {
        let weight = morph::weight_at(i, morph_descriptor_index);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * morph::morph_position(vertex_index, i, morph_descriptor_index);
@if(VERTEX_NORMALS && (!PREPASS_PIPELINE || NORMAL_PREPASS_OR_DEFERRED_PREPASS))
        vertex.normal += weight * morph::morph_normal(vertex_index, i, morph_descriptor_index);
@if(VERTEX_TANGENTS && (!PREPASS_PIPELINE || NORMAL_PREPASS_OR_DEFERRED_PREPASS))
        vertex.tangent += vec4(weight * morph::morph_tangent(vertex_index, i, morph_descriptor_index), 0.0);
    }
    return vertex;
}

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let uncompressed_vertex_no_morph = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);

@if(MORPH_TARGETS)
    var vertex = morph_vertex(uncompressed_vertex_no_morph, vertex_no_morph.instance_index);
@else
    var vertex = uncompressed_vertex_no_morph;

    let mesh_world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

@if(SKINNED)
    var world_from_local = skinning::skin_model(
        vertex.joint_indices,
        vertex.joint_weights,
        vertex_no_morph.instance_index
    );
@else
    var world_from_local = mesh_world_from_local;

    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);

    // Covered skin: every vertex of the patch goes to the same point, so it covers nothing.
@if(VERTEX_UVS_B) {
    if is_hidden(u32(vertex.uv_b.x)) {
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
}

@if(PREPASS_PIPELINE && UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;

@if(VERTEX_NORMALS && SKINNED && (!PREPASS_PIPELINE || NORMAL_PREPASS_OR_DEFERRED_PREPASS))
    out.world_normal = skinning::skin_normals(world_from_local, vertex.normal);
@elif(VERTEX_NORMALS && (!PREPASS_PIPELINE || NORMAL_PREPASS_OR_DEFERRED_PREPASS))
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_no_morph.instance_index);

@if(VERTEX_TANGENTS && (!PREPASS_PIPELINE || NORMAL_PREPASS_OR_DEFERRED_PREPASS))
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_no_morph.instance_index);

@if(VERTEX_COLORS)
    out.color = vertex.color;

@if(PREPASS_PIPELINE && MOTION_VECTOR_PREPASS) {
@if(SKINNED && HAS_PREVIOUS_SKIN)
    let prev_model = skinning::skin_prev_model(
        uncompressed_vertex_no_morph.joint_indices,
        uncompressed_vertex_no_morph.joint_weights,
        vertex_no_morph.instance_index
    );
@else
    let prev_model = mesh_functions::get_previous_world_from_local(vertex_no_morph.instance_index);
    out.previous_world_position = mesh_functions::mesh_position_local_to_world(
        prev_model,
        vec4<f32>(uncompressed_vertex_no_morph.position, 1.0)
    );
}

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_no_morph.instance_index;

@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_no_morph.instance_index, mesh_world_from_local[3]);

    return out;
}
