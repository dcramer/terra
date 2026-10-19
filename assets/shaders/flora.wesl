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

// A tangent-space normal from a map's x and y alone, as a cooked map is BC5 (D188).
fn unpacked(t: vec4<f32>) -> vec3<f32> {
    let xy = t.xy * 2.0 - 1.0;
    return vec3<f32>(xy, sqrt(max(1.0 - dot(xy, xy), 0.0)));
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
        tx = unpacked(textureSampleGrad(flora_normal, flora_sampler, uv, index, ddx, ddy));
        tx.x *= side.x;
    }
    {
        let uv = vec2<f32>(p.x * side.y, p.z);
        let ddx = vec2<f32>(dx.x * side.y, dx.z);
        let ddy = vec2<f32>(dy.x * side.y, dy.z);
        colour += textureSampleGrad(flora_color, flora_sampler, uv, index, ddx, ddy).rgb * w.y;
        ty = unpacked(textureSampleGrad(flora_normal, flora_sampler, uv, index, ddx, ddy));
        ty.x *= side.y;
    }
    {
        let uv = vec2<f32>(-p.x * side.z, p.y);
        let ddx = vec2<f32>(-dx.x * side.z, dx.y);
        let ddy = vec2<f32>(-dy.x * side.z, dy.y);
        colour += textureSampleGrad(flora_color, flora_sampler, uv, index, ddx, ddy).rgb * w.z;
        tz = unpacked(textureSampleGrad(flora_normal, flora_sampler, uv, index, ddx, ddy));
        tz.x *= -side.z;
    }
    let nx = vec3<f32>(n.x, tx.y + n.y, tx.x + n.z);
    let ny = vec3<f32>(ty.x + n.x, n.y, ty.y + n.z);
    let nz = vec3<f32>(tz.x + n.x, tz.y + n.y, n.z);
    let normal = normalize(nx * w.x + ny * w.y + nz * w.z);
    let average = textureSampleLevel(flora_color, flora_sampler, vec2<f32>(0.5), index, 16.0).rgb;
    return Layer(colour * tint / max(average, vec3<f32>(0.02)), normal);
}

fn hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn noise(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(hash(cell), hash(cell + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(hash(cell + vec3<f32>(0.0, 1.0, 0.0)), hash(cell + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(hash(cell + vec3<f32>(0.0, 0.0, 1.0)), hash(cell + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(hash(cell + vec3<f32>(0.0, 1.0, 1.0)), hash(cell + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// Iron ore in a rock (flora.ron's `rust`): the rock is its plain self, its own colour and
// scan, with thin veins of oxidised ore running through the stone and the rust their water
// has carried down its faces below them, as weathered ironstone shows, so it reads up close
// and the boulder as a whole keeps its grey (the developers chose "subtle tint" over a
// whole-boulder colour, 2026-10-15: a tinted boulder read as salmon granite).
// The hue of iron oxide, against grey of the same lightness.
const RUST: vec3<f32> = vec3<f32>(1.0, 0.42, 0.17);
// The veins' spacing, in metres, and their width as a share of the noise's range.
const VEIN_M: f32 = 0.55;
const VEIN_WIDTH: f32 = 0.03;
// How far below a vein its rust runs, in metres.
const RUN_M: f32 = 0.35;

// How much of a vein is at `p`, 0 to 1: the middle of a warped noise, where it crosses a
// half, as thin sheets through the stone that cross its faces as lines, gathered in parts
// of the rock and absent from others. `blur`, the pixel's size against the veins', widens
// a line and fades it to keep it from glittering at a distance.
fn vein(p: vec3<f32>, blur: f32) -> f32 {
    let q = p / VEIN_M;
    let warp = vec3<f32>(noise(q * 0.7 + 11.0), noise(q * 0.7 + 23.0), noise(q * 0.7 + 37.0)) - 0.5;
    let r = q + warp * 1.2;
    let width = max(VEIN_WIDTH, blur);
    let fade = VEIN_WIDTH / width;
    let line = 1.0 - smoothstep(0.0, width, abs(noise(r) - 0.5));
    let fine = (1.0 - smoothstep(0.0, width * 0.7, abs(noise(r * 2.6 + 5.0) - 0.5))) * 0.55;
    let gathered = smoothstep(0.3, 0.55, noise(q * 0.35 + 3.0));
    return max(line, fine) * gathered * fade;
}

// The rock's colour with its ore showing: the veins, and below each the rust it has bled,
// in runs down the faces that fade with distance, strongest on steep faces where water runs.
fn rusted(colour: vec3<f32>, p: vec3<f32>, n: vec3<f32>, up: vec3<f32>, pixel_m: f32) -> vec3<f32> {
    let blur = pixel_m / VEIN_M * 0.5;
    let core = vein(p, blur);
    let side = normalize(cross(up, select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(up.x) > 0.9)));
    let across = vec2<f32>(dot(p, side), dot(p, cross(up, side)));
    let runs = smoothstep(0.25, 0.7, noise(vec3<f32>(across * 16.0, dot(p, up) * 1.5)));
    var bled = 0.0;
    for (var k = 1; k <= 6; k += 1) {
        let above = f32(k) / 6.0;
        bled = max(bled, vein(p + up * (above * RUN_M), blur * 2.0) * (1.0 - above * 0.85));
    }
    let steep = 1.0 - abs(dot(n, up));
    bled *= mix(0.35, 1.0, runs) * mix(0.3, 1.0, steep);
    let grey = dot(colour, vec3<f32>(0.2126, 0.7152, 0.0722));
    var out = mix(colour, RUST * grey * 1.6, clamp(bled * 0.5, 0.0, 1.0));
    out = mix(out, RUST * grey * 1.15, clamp(core * 0.8, 0.0, 1.0));
    return out;
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
        // A layer in eighths, a sixteenth more for ore's rust (flora.rs).
        let index = u32(floor(code * 8.0 + 0.25));
        var n = normalize(in.world_normal);
        if !is_front {
            n = -n;
        }
        let drawn = layer(index, in.color.rgb, on_plant, gx, gy, n);
        var colour = drawn.colour;
        if fract(code * 8.0 + 0.25) > 0.5 {
            let up = normalize(in.world_position.xyz);
            colour = rusted(colour, on_plant, drawn.normal, up, max(length(gx), length(gy)));
        }
        pbr_input.material.base_color = vec4<f32>(colour, 1.0);
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
