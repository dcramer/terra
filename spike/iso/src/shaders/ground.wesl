// The spike's ground (terrain.rs): the game's stack of ground scans blended per vertex by
// the weights the land gives each layer, height-blended so leaves lie over the grass in
// clumps and the grass's tufts stand through the moss, each layer sampled twice at
// different sizes and turned so its repeat does not show, the rock projected from three
// sides on the steep.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var ground_color: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var ground_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var ground_normal: texture_2d_array<f32>;

// The stack's layers (assets/materials/README.md) and how many metres each covers.
const MUD: u32 = 0u;
const GRASS: u32 = 1u;
const ROCK: u32 = 2u;
const DRY: u32 = 4u;
const LEAVES: u32 = 5u;
const ROOTS: u32 = 6u;
const MOSS: u32 = 7u;
// The camp fire's place (scene::FIRE).
const FIRE: vec2<f32> = vec2<f32>(0.0, 3.0);

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
    // The relief turned back into the world's frame.
    let rel1 = transpose(r1) * t1.relief;
    let rel2 = transpose(r2) * t2.relief;
    return Texel(mix(t1.colour, t2.colour, m), mix(rel1, rel2, m), mix(t1.height, t2.height, m));
}

// Rock, from three sides.
fn rock(world: vec3<f32>, n: vec3<f32>) -> Texel {
    let m = 2.7;
    var w = pow(abs(n), vec3<f32>(4.0));
    w = w / (w.x + w.y + w.z);
    let x = fetch(world.zy / m, ROCK);
    let y = fetch(world.xz / m, ROCK);
    let z = fetch(world.xy / m, ROCK);
    let colour = x.colour * w.x + y.colour * w.y + z.colour * w.z;
    let height = x.height * w.x + y.height * w.y + z.height * w.z;
    return Texel(colour, y.relief * w.y + (x.relief + z.relief) * 0.5 * (w.x + w.z), height);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let world = in.world_position.xyz;
    let p = world.xz;
    let n = normalize(in.world_normal);

    // The weights the land gave: grass, leaves, moss, rock; mud, roots.
    var w: array<f32, 7>;
    w[0] = in.color.r;
    let leaves = in.color.g;
    // Fallen leaves are fresh or dry in drifts.
    let drift = smoothstep(0.35, 0.65, noise(p * 0.21 + 3.0) * 0.7 + noise(p * 0.9) * 0.3);
    w[1] = leaves * (1.0 - drift);
    w[2] = leaves * drift;
    w[3] = in.color.b;
    w[4] = in.color.a;
    w[5] = in.uv_b.x;
    w[6] = in.uv_b.y;

    var t: array<Texel, 7>;
    let empty = Texel(vec3<f32>(0.0), vec2<f32>(0.0), 0.0);
    t[0] = empty; t[1] = empty; t[2] = empty; t[3] = empty; t[4] = empty; t[5] = empty; t[6] = empty;
    if w[0] > 0.01 { t[0] = layer(p, GRASS, 2.0); }
    if w[1] > 0.01 { t[1] = layer(p, LEAVES, 1.5); }
    if w[2] > 0.01 { t[2] = layer(p, DRY, 2.0); }
    if w[3] > 0.01 { t[3] = layer(p, MOSS, 1.1); }
    if w[4] > 0.01 { t[4] = rock(world, n); }
    if w[5] > 0.01 { t[5] = layer(p, MUD, 1.3); }
    if w[6] > 0.01 { t[6] = layer(p, ROOTS, 1.67); }
    // The scans are dark, as photographed under cloud: each brought to the albedo its
    // stuff has in the open (grass greener, moss far lighter).
    t[0].colour *= vec3<f32>(1.55, 2.1, 1.2);
    t[1].colour *= vec3<f32>(1.5, 1.45, 1.35);
    t[2].colour *= vec3<f32>(1.45, 1.4, 1.3);
    t[3].colour *= vec3<f32>(5.0, 6.0, 3.2);
    t[4].colour *= vec3<f32>(1.35, 1.35, 1.4);
    t[6].colour *= vec3<f32>(1.3, 1.3, 1.3);

    // Height blending: the highest of what lies there wins, over a short depth.
    var best = -10.0;
    var a: array<f32, 7>;
    for (var i = 0u; i < 7u; i++) {
        a[i] = select(-10.0, w[i] * 1.4 + t[i].height, w[i] > 0.01);
        best = max(best, a[i]);
    }
    var colour = vec3<f32>(0.0);
    var relief = vec2<f32>(0.0);
    var total = 0.0;
    for (var i = 0u; i < 7u; i++) {
        let b = max(a[i] - best + 0.25, 0.0);
        colour += t[i].colour * b;
        relief += t[i].relief * b;
        total += b;
    }
    colour /= max(total, 1e-4);
    relief /= max(total, 1e-4);

    // Large-scale variation of light and hue, so the floor is not one even tone.
    let broad = noise(p * 0.07) * 0.6 + noise(p * 0.31) * 0.4;
    colour *= mix(0.78, 1.15, broad);
    // Wet mud by the water: darker and glossier.
    let wet = smoothstep(0.2, 0.8, w[5]);
    colour *= mix(1.0, 0.65, wet);

    // Trodden earth: packed soil, greyer and darker than the litter, in blotches where
    // feet went most, flecked with bits of twig and grit.
    let bare = in.uv.y;
    let blotch = noise(p * 1.7 + 11.0) * 0.6 + noise(p * 5.3) * 0.4;
    let packed = smoothstep(0.35, 0.9, bare) * smoothstep(0.25, 0.7, blotch);
    let soil = vec3<f32>(dot(colour, vec3<f32>(0.3, 0.5, 0.2))) * vec3<f32>(0.95, 0.85, 0.72);
    colour = mix(colour, soil * 0.8, packed * 0.75);
    let fleck = smoothstep(0.82, 0.9, hash(floor(p * 23.0)));
    colour = mix(colour, colour * vec3<f32>(1.5, 1.35, 1.1), fleck * bare * 0.6);
    // Ash and char about the fire: grey ash blown out in tongues, black charcoal close in.
    let d = distance(p, FIRE) + (noise(p * 3.1) - 0.5) * 0.55 + (noise(p * 9.0) - 0.5) * 0.18;
    let ash = smoothstep(1.7, 0.95, d) * mix(0.55, 1.0, noise(p * 6.0 + 5.0));
    let char_ = smoothstep(1.05, 0.7, d);
    let grain = 0.8 + 0.4 * noise(p * 14.0);
    colour = mix(colour, vec3<f32>(0.045, 0.042, 0.04) * grain, ash * 0.7);
    colour = mix(colour, vec3<f32>(0.035, 0.03, 0.028) * grain, char_ * 0.9);
    // The crowns' shade: less of the sky's light under the oaks.
    let canopy = in.uv.x;
    colour *= mix(0.72, 1.0, canopy);

    pbr_input.material.base_color = vec4<f32>(colour, 1.0);
    // Times the screen-space AO already in it (assigning it dropped the AO, so nothing sat
    // on the ground), and never black: a wood's floor still sees some sky between crowns.
    pbr_input.diffuse_occlusion *= vec3<f32>(0.2 + 0.8 * canopy * canopy);
    pbr_input.specular_occlusion *= canopy;
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
