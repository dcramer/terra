// How trees, bushes and rocks look (D14): each part in a real material, bark, leaves or
// lichen-covered rock, from one texture array (materials/flora), projected along the three
// axes and blended by which way the surface faces (triplanar), as the ground's are, since
// the free models have no texture coordinates of their own. Each material's own average
// colour is swapped for the part's colour from flora.ron, keeping its detail, so a kind
// still says what colour it is. Save this file while the sandbox runs and plants update.
//
// The vertex shader (wind.wesl) carries, in the texture coordinates, where each point is
// on its plant before the wind moves it, and, in the colour's alpha, the material: a layer
// in eighths, or whole for a part in plain colour, such as grass and flowers.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

// Field order must match WindExtension in flora.rs.
struct Wind {
    direction: vec3<f32>,
    strength: f32,
    layer_metres: array<vec4<f32>, 2>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> wind: Wind;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var flora_color: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var flora_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var flora_normal: texture_2d_array<f32>;

struct Layer {
    colour: vec3<f32>,
    normal: vec3<f32>,
}

fn facing(n: vec3<f32>) -> vec3<f32> {
    let w = pow(abs(n), vec3<f32>(4.0));
    return w / (w.x + w.y + w.z);
}

// One material, projected along each axis the surface faces and blended, its average
// colour swapped for `tint`, and the surface's normal bent by its relief ("UDN" blend).
// The position's gradients are given, as they can be taken only where every pixel
// runs the same code.
fn layer(index: u32, tint: vec3<f32>, p_m: vec3<f32>, gx: vec3<f32>, gy: vec3<f32>, n: vec3<f32>) -> Layer {
    let k = 1.0 / wind.layer_metres[index / 4u][index % 4u];
    let p = p_m * k;
    let dx = gx * k;
    let dy = gy * k;
    let w = facing(n);
    let side = sign(n);
    var colour = vec3<f32>(0.0);
    var tx = vec3<f32>(0.0, 0.0, 1.0);
    var ty = vec3<f32>(0.0, 0.0, 1.0);
    var tz = vec3<f32>(0.0, 0.0, 1.0);
    // Sampled with gradients, as some pixels of a triangle skip a projection.
    {
        let uv = vec2<f32>(p.z * side.x, p.y);
        let ddx = vec2<f32>(dx.z * side.x, dx.y);
        let ddy = vec2<f32>(dy.z * side.x, dy.y);
        colour += textureSampleGrad(flora_color, flora_sampler, uv, index, ddx, ddy).rgb * w.x;
        tx = textureSampleGrad(flora_normal, flora_sampler, uv, index, ddx, ddy).xyz * 2.0 - 1.0;
        tx.x *= side.x;
    }
    {
        let uv = vec2<f32>(p.x * side.y, p.z);
        let ddx = vec2<f32>(dx.x * side.y, dx.z);
        let ddy = vec2<f32>(dy.x * side.y, dy.z);
        colour += textureSampleGrad(flora_color, flora_sampler, uv, index, ddx, ddy).rgb * w.y;
        ty = textureSampleGrad(flora_normal, flora_sampler, uv, index, ddx, ddy).xyz * 2.0 - 1.0;
        ty.x *= side.y;
    }
    {
        let uv = vec2<f32>(-p.x * side.z, p.y);
        let ddx = vec2<f32>(-dx.x * side.z, dx.y);
        let ddy = vec2<f32>(-dy.x * side.z, dy.y);
        colour += textureSampleGrad(flora_color, flora_sampler, uv, index, ddx, ddy).rgb * w.z;
        tz = textureSampleGrad(flora_normal, flora_sampler, uv, index, ddx, ddy).xyz * 2.0 - 1.0;
        tz.x *= -side.z;
    }
    let nx = vec3<f32>(n.x, tx.y + n.y, tx.x + n.z);
    let ny = vec3<f32>(ty.x + n.x, n.y, ty.y + n.z);
    let nz = vec3<f32>(tz.x + n.x, tz.y + n.y, n.z);
    let normal = normalize(nx * w.x + ny * w.y + nz * w.z);
    let average = textureSampleLevel(flora_color, flora_sampler, vec2<f32>(0.5), index, 16.0).rgb;
    return Layer(colour * tint / max(average, vec3<f32>(0.02)), normal);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var vertex = in;
@if(VERTEX_COLORS)
    vertex.color = vec4<f32>(in.color.rgb, 1.0);
    var pbr_input = pbr_input_from_standard_material(vertex, is_front);

@if(VERTEX_COLORS && VERTEX_UVS_A && VERTEX_UVS_B) {
    let code = in.color.a;
    let on_plant = vec3<f32>(in.uv, in.uv_b.x);
    let gx = dpdx(on_plant);
    let gy = dpdy(on_plant);
    if code < 0.97 {
        let index = u32(round(code * 8.0));
        var n = normalize(in.world_normal);
        if !is_front {
            n = -n;
        }
        let drawn = layer(index, in.color.rgb, on_plant, gx, gy, n);
        pbr_input.material.base_color = vec4<f32>(drawn.colour, 1.0);
        pbr_input.N = drawn.normal;
    }
}

    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
