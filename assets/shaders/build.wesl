// Construction materials from texture arrays (D290, docs/proposals/construction-002.md): every
// texture set a layer of four arrays (colour, normal, occlusion-roughness-metal, height), each
// corner carrying the layer of its material in its second texture place (uv_b.x), so a
// section's surfaces in any number of materials are one draw. A table maps each layer to the
// slot of the arrays it is resident in and gives its parallax depth and reflectance.
//
// What it computes is what Bevy's StandardMaterial computes with the same textures (its
// colour times the corner's, roughness and metal from ARM, occlusion the least of ARM's and
// the screen's, the normal map in tangent space, relief mapping by the height), so a material
// looks as it did when it had a StandardMaterial of its own. The relief mapping is ported
// from Bevy's `parallax_mapping.wesl` (MIT or Apache-2.0), reading a layer of the array.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing, calculate_tbn_mikktspace},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::view,
    pbr_lighting as lighting,
};

@if(SCREEN_SPACE_AMBIENT_OCCLUSION)
import bevy_pbr::render::mesh_view_bindings::screen_space_ambient_occlusion_texture;
@if(SCREEN_SPACE_AMBIENT_OCCLUSION)
import bevy_pbr::ssao::utils::ssao_multibounce;

// Field order must match BuildArrays in crates/terra_game/src/build/arrays/mod.rs.
struct Layers {
    // For each layer: x its slot in the arrays, y its parallax depth (0 for none), z its
    // reflectance, w unused.
    table: array<vec4<f32>, 512>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> layers: Layers;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var colour_array: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var array_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var normal_array: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(104)
var arm_array: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(105)
var height_array: texture_2d_array<f32>;

// As StandardMaterial's: up to 16 layers, steeper views fewer, then 5 halvings.
const MAX_LAYERS: f32 = 16.0;
const RELIEF_STEPS: u32 = 5u;

fn sample_depth(uv: vec2<f32>, slot: i32) -> f32 {
    // A fixed level, as Bevy's does: a level from derivatives cannot be taken in a loop.
    return textureSampleLevel(height_array, array_sampler, uv, slot, 0.0).r;
}

/// Relief mapping (Bevy's `parallaxed_uv`): where the view ray meets the height field.
fn parallaxed_uv(depth_scale: f32, original_uv: vec2<f32>, Vt: vec3<f32>, slot: i32) -> vec2<f32> {
    var uv = original_uv;
    let view_steepness = abs(Vt.z);
    let layer_count = mix(MAX_LAYERS, 1.0, view_steepness);
    let layer_depth = 1.0 / layer_count;
    var delta_uv = depth_scale * layer_depth * Vt.xy * vec2(1.0, -1.0) / view_steepness;
    var current_layer_depth = 0.0;
    var texture_depth = sample_depth(uv, slot);
    for (var i: i32 = 0; texture_depth > current_layer_depth && i <= i32(layer_count); i++) {
        current_layer_depth += layer_depth;
        uv += delta_uv;
        texture_depth = sample_depth(uv, slot);
    }
    delta_uv *= 0.5;
    var delta_depth = 0.5 * layer_depth;
    uv -= delta_uv;
    current_layer_depth -= delta_depth;
    for (var i: u32 = 0u; i < RELIEF_STEPS; i++) {
        texture_depth = sample_depth(uv, slot);
        delta_uv *= 0.5;
        delta_depth *= 0.5;
        if (texture_depth > current_layer_depth) {
            uv += delta_uv;
            current_layer_depth += delta_depth;
        } else {
            uv -= delta_uv;
            current_layer_depth -= delta_depth;
        }
    }
    return uv;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let layer = min(u32(round(in.uv_b.x)), 511u);
    let entry = layers.table[layer];
    let slot = i32(entry.x);
    let depth = entry.y;

    let TBN = calculate_tbn_mikktspace(pbr_input.world_normal, in.world_tangent);
    let T = TBN[0];
    let B = TBN[1];
    let N = TBN[2];
    var uv = in.uv;
    if depth > 0.0 {
        let V = pbr_input.V;
        let Vt = vec3(dot(V, T), dot(V, B), dot(V, N));
        uv = parallaxed_uv(depth, uv, -Vt, slot);
    }

    let bias = view.mip_bias;
    let colour = textureSampleBias(colour_array, array_sampler, uv, slot, bias);
    let arm = textureSampleBias(arm_array, array_sampler, uv, slot, bias);
    let normal = textureSampleBias(normal_array, array_sampler, uv, slot, bias).rg;

    pbr_input.material.base_color = vec4(pbr_input.material.base_color.rgb * colour.rgb, 1.0);
    pbr_input.material.perceptual_roughness = arm.g;
    pbr_input.material.metallic = arm.b;
    pbr_input.material.reflectance = vec3(entry.z);

    // Occlusion: the least of the texture's and the screen's, as StandardMaterial's.
    var diffuse_occlusion = vec3(arm.r);
    var specular_occlusion = 1.0;
@if(SCREEN_SPACE_AMBIENT_OCCLUSION) {
    let ssao_tex_pos = vec2<i32>(in.position.xy - view.viewport.xy);
    let ssao = textureLoad(screen_space_ambient_occlusion_texture, ssao_tex_pos, 0i).r;
    diffuse_occlusion = min(diffuse_occlusion, ssao_multibounce(ssao, pbr_input.material.base_color.rgb));
    let NdotV = max(dot(pbr_input.N, pbr_input.V), 0.0001);
    let roughness = lighting::perceptualRoughnessToRoughness(arm.g);
    specular_occlusion = saturate(pow(NdotV + ssao, exp2(-16.0 * roughness - 1.0)) - 1.0 + ssao);
}
    pbr_input.diffuse_occlusion = diffuse_occlusion;
    pbr_input.specular_occlusion = specular_occlusion;

    // The normal map's two stored components, its third made up from them.
    var Nt = vec3(normal * 2.0 - 1.0, 0.0);
    Nt.z = sqrt(max(1.0 - dot(Nt.xy, Nt.xy), 0.0));
    pbr_input.N = normalize(Nt.x * T + Nt.y * B + Nt.z * N);
    pbr_input.clearcoat_N = pbr_input.N;

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
