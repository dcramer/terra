// The kit's ground (kit/ground.rs, D634): up to eight layers of the `materials/ground` stack,
// each laid by a mask (the layout's per-vertex maps, the vertex's shade and wear, or the
// kit's rule textures), height-blended where they overlap (Mishkinis, "Advanced Terrain
// Texture Splatting", 2013), each sampled twice at different sizes and turned so its repeat
// does not show; then the broad variation, the wet (darker, glossier), the packed soil of
// worn ground and the scorched points.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

struct Params {
    what: array<vec4<f32>, 8>,
    tint: array<vec4<f32>, 8>,
    breakup: array<vec4<f32>, 8>,
    how: array<vec4<f32>, 8>,
    scorch: array<vec4<f32>, 4>,
    info: vec4<f32>,
    masks: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var ground_color: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var ground_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var ground_normal: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var<uniform> ground: Params;
@group(constants::MATERIAL_BIND_GROUP) @binding(104)
var mask_a: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(105)
var mask_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(106)
var mask_b: texture_2d<f32>;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

struct Texel {
    colour: vec3<f32>,
    // The normal map's relief, x and y, and the layer's height (kept in its blue).
    relief: vec2<f32>,
    height: f32,
}

fn fetch(uv: vec2<f32>, layer: u32) -> Texel {
    let c = textureSample(ground_color, ground_sampler, uv, layer).rgb;
    let n = textureSample(ground_normal, ground_sampler, uv, layer).xyz;
    return Texel(c, n.xy * 2.0 - 1.0, n.z);
}

// A layer at a point: two sizes, each turned its own way, mixed by broad noise.
fn layer(world: vec2<f32>, index: u32, metres: f32) -> Texel {
    let a = 0.6 + f32(index) * 1.37;
    let r1 = mat2x2<f32>(cos(a), sin(a), -sin(a), cos(a));
    let r2 = mat2x2<f32>(cos(a + 2.1), sin(a + 2.1), -sin(a + 2.1), cos(a + 2.1));
    let t1 = fetch(r1 * world / metres, index);
    let t2 = fetch(r2 * world / (metres * 2.3) + 0.37, index);
    let m = smoothstep(0.3, 0.7, noise(world * 0.13 + f32(index) * 7.0));
    let rel1 = transpose(r1) * t1.relief;
    let rel2 = transpose(r2) * t2.relief;
    return Texel(mix(t1.colour, t2.colour, m), mix(rel1, rel2, m), mix(t1.height, t2.height, m));
}

// A layer from three sides (rock on the steep).
fn triplanar(world: vec3<f32>, n: vec3<f32>, index: u32, m: f32) -> Texel {
    var w = pow(abs(n), vec3<f32>(4.0));
    w = w / (w.x + w.y + w.z);
    let x = fetch(world.zy / m, index);
    let y = fetch(world.xz / m, index);
    let z = fetch(world.xy / m, index);
    let colour = x.colour * w.x + y.colour * w.y + z.colour * w.z;
    let height = x.height * w.x + y.height * w.y + z.height * w.z;
    return Texel(colour, y.relief * w.y + (x.relief + z.relief) * 0.5 * (w.x + w.z), height);
}

// A mask's weight at the fragment (kit/ground.rs's `source`).
fn source(s: u32, in: VertexOutput, a: vec4<f32>, b: vec4<f32>) -> f32 {
    switch s {
        case 0u: { return in.color.r; }
        case 1u: { return in.color.g; }
        case 2u: { return in.color.b; }
        case 3u: { return in.color.a; }
        case 4u: { return in.uv_b.x; }
        case 5u: { return in.uv_b.y; }
        case 6u: { return in.uv.x; }
        case 7u: { return in.uv.y; }
        case 8u: { return 1.0; }
        case 9u: { return a.r; }
        case 10u: { return a.g; }
        case 11u: { return a.b; }
        case 12u: { return a.a; }
        case 13u: { return b.r; }
        case 14u: { return b.g; }
        default: { return 0.0; }
    }
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let world = in.world_position.xyz;
    let p = world.xz;
    let n = normalize(in.world_normal);
    let muv = (p - ground.masks.xy) / ground.masks.zw;
    let ma = textureSample(mask_a, mask_sampler, muv);
    let mb = textureSample(mask_b, mask_sampler, muv);

    let count = u32(ground.info.x);
    var w: array<f32, 8>;
    var t: array<Texel, 8>;
    var soak = 0.0;
    for (var i = 0u; i < count; i++) {
        let what = ground.what[i];
        var wi = source(u32(what.z), in, ma, mb) * ground.tint[i].a;
        // Drifts: a band of noise at two scales (a band running down lays the inverse).
        let b = ground.breakup[i];
        if b.x > 0.0 {
            let x = noise(p * b.x + 3.0) * 0.7 + noise(p * b.y) * 0.3;
            var s = smoothstep(min(b.z, b.w), max(b.z, b.w), x);
            if b.z > b.w { s = 1.0 - s; }
            wi *= s;
        }
        soak = max(soak, wi * what.w);
        // A layer without a material only wets.
        if ground.how[i].y < 0.5 { wi = 0.0; }
        w[i] = wi;
        t[i] = Texel(vec3<f32>(0.0), vec2<f32>(0.0), 0.0);
        if wi > 0.01 {
            if ground.how[i].x > 0.5 {
                t[i] = triplanar(world, n, u32(what.x), what.y);
            } else {
                t[i] = layer(p, u32(what.x), what.y);
            }
            t[i].colour *= ground.tint[i].rgb;
        }
    }

    // Height blending: the highest of what lies there wins, over a short depth.
    var best = -10.0;
    var a: array<f32, 8>;
    for (var i = 0u; i < count; i++) {
        a[i] = select(-10.0, w[i] * 1.4 + t[i].height, w[i] > 0.01);
        best = max(best, a[i]);
    }
    var colour = vec3<f32>(0.0);
    var relief = vec2<f32>(0.0);
    var total = 0.0;
    for (var i = 0u; i < count; i++) {
        let k = max(a[i] - best + 0.25, 0.0);
        colour += t[i].colour * k;
        relief += t[i].relief * k;
        total += k;
    }
    colour /= max(total, 1e-4);
    relief /= max(total, 1e-4);

    // Large-scale variation of light and hue, so the floor is not one even tone.
    let broad = noise(p * 0.07) * 0.6 + noise(p * 0.31) * 0.4;
    colour *= mix(ground.info.z, ground.info.w, broad);
    // Wet ground: darker and glossier.
    let wet = smoothstep(0.2, 0.8, soak);
    colour *= mix(1.0, 0.65, wet);

    // Worn ground: packed soil, greyer and darker, in blotches where feet went most,
    // flecked with bits of twig and grit.
    let bare = in.uv.y;
    let blotch = noise(p * 1.7 + 11.0) * 0.6 + noise(p * 5.3) * 0.4;
    let packed = smoothstep(0.35, 0.9, bare) * smoothstep(0.25, 0.7, blotch);
    let soil = vec3<f32>(dot(colour, vec3<f32>(0.3, 0.5, 0.2))) * vec3<f32>(0.95, 0.85, 0.72);
    colour = mix(colour, soil * 0.8, packed * 0.75);
    let fleck = smoothstep(0.82, 0.9, hash(floor(p * 23.0)));
    colour = mix(colour, colour * vec3<f32>(1.5, 1.35, 1.1), fleck * bare * 0.6);
    // Ash and char about each scorched point: grey ash in tongues, black charcoal close in.
    for (var i = 0u; i < u32(ground.info.y); i++) {
        let d = distance(p, ground.scorch[i].xy) + (noise(p * 3.1) - 0.5) * 0.55
            + (noise(p * 9.0) - 0.5) * 0.18;
        let ash = smoothstep(1.7, 0.95, d) * mix(0.55, 1.0, noise(p * 6.0 + 5.0));
        let char_ = smoothstep(1.05, 0.7, d);
        let grain = 0.8 + 0.4 * noise(p * 14.0);
        colour = mix(colour, vec3<f32>(0.045, 0.042, 0.04) * grain, ash * 0.7);
        colour = mix(colour, vec3<f32>(0.035, 0.03, 0.028) * grain, char_ * 0.9);
    }
    // The shade over it (crowns): less of the sky's light.
    let canopy = in.uv.x;
    colour *= mix(0.72, 1.0, canopy);

    pbr_input.material.base_color = vec4<f32>(colour, 1.0);
    pbr_input.diffuse_occlusion = vec3<f32>(canopy * canopy);
    pbr_input.specular_occlusion = canopy;
    pbr_input.material.perceptual_roughness = mix(0.92, 0.45, wet);
    // The relief, in a frame laid on the slope: x east, y north (OpenGL's green up).
    let tx = normalize(vec3<f32>(1.0, 0.0, 0.0) - n * n.x);
    let ty = cross(n, tx);
    let r = relief * 1.3;
    pbr_input.N = normalize(n * sqrt(max(1.0 - dot(r, r) * 0.5, 0.2)) + tx * r.x + ty * r.y);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
