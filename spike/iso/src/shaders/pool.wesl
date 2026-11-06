// Still dark water (pool.rs): the lit face is dark and has no specular of its own; this
// adds what water mirrors at an action RPG's camera, as a game fakes it: the sky by a
// Fresnel term, dark near the banks, and each glinting light's reflection off the rippled
// face as a sharp normalised lobe the ripples break into a streak toward the eye.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

struct Look {
    deep: vec4<f32>,
    shallow: vec4<f32>,
    sky: vec4<f32>,
    horizon: vec4<f32>,
    wave: vec4<f32>,
    look: vec4<f32>,
    lights: array<vec4<f32>, 12>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> pool: Look;

const PI: f32 = 3.14159265;

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

// The wind's ripples (metres of height): noise at three sizes drifting downwind, in gusts.
fn wind(p: vec2<f32>, t: f32) -> f32 {
    let w = pool.wave.xy;
    let gust = 0.35 + 0.65 * noise(p * 0.15 - w * t * 0.12);
    return (noise(p * 1.7 + w * t * 0.5) * 0.6
        + noise(p * 4.3 - w.yx * t * 0.8) * 0.3
        + noise(p * 9.1 + w * t * 1.3) * 0.1) * gust;
}

// Rings spreading from drips, midges and rising bubbles: a cell a ring, some cells quiet,
// in two offset grids so they overlap.
fn rings(p: vec2<f32>, t: f32) -> f32 {
    var h = 0.0;
    for (var k = 0; k < 2; k++) {
        let cell = 1.3 + f32(k) * 0.6;
        let q = p / cell + vec2<f32>(f32(k) * 0.5, f32(k) * 0.37);
        let id = floor(q) + f32(k) * 17.0;
        let f = fract(q) - 0.5;
        let on = hash(id);
        if on > pool.look.w * cell * cell {
            continue;
        }
        let centre = (vec2<f32>(hash(id + 3.1), hash(id + 7.7)) - 0.5) * 0.3;
        let period = 2.2 + 2.5 * hash(id + 11.3);
        let age = fract(t / period + on * 7.0);
        let r = age * 0.42 * cell;
        let x = length(f - centre) * cell - r;
        let fade = (1.0 - age) * (1.0 - age);
        h += sin(x * 45.0) * exp(-x * x * 350.0) * fade;
    }
    return h * 0.004;
}

fn height(p: vec2<f32>, t: f32) -> f32 {
    return wind(p, t) * pool.wave.z + rings(p, t);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let p = in.uv;
    let depth = in.uv_b.x;
    let t = pool.wave.w;

    let e = 0.02;
    let h = height(p, t);
    let gx = (height(p + vec2<f32>(e, 0.0), t) - h) / e;
    let gz = (height(p + vec2<f32>(0.0, e), t) - h) / e;
    // Calmer in the margin, where the reeds and the bank break the wind.
    let calm = mix(0.3, 1.0, smoothstep(0.02, 0.3, depth));
    let n = normalize(vec3<f32>(-gx * calm, 1.0, -gz * calm));
    pbr_input.N = n;

    // The water's own colour: the mud through the shallows, the dark of the deep.
    let k = smoothstep(0.0, pool.deep.a, depth);
    let colour = mix(pool.shallow.rgb, pool.deep.rgb, k);
    let alpha = smoothstep(-0.02, 0.06, depth) * mix(1.0 - pool.shallow.a, 0.97, k);
    pbr_input.material.base_color = vec4<f32>(colour, alpha);

    // The mirror. V points from the face to the eye.
    let v = pbr_input.V;
    let r = reflect(-v, n);
    let cos_v = clamp(dot(n, v), 0.0, 1.0);
    let fresnel = 0.02 + 0.98 * pow(1.0 - cos_v, 5.0);
    // The sky: the horizon's glow low, the deep sky high; the bank's dark near the margin
    // and in broad blots where what stands on it would be mirrored.
    let up = clamp(r.y, 0.0, 1.0);
    let sky = mix(pool.horizon.rgb, pool.sky.rgb, smoothstep(0.1, 0.9, up));
    let bank = smoothstep(0.0, pool.horizon.a, depth)
        * mix(0.35, 1.0, smoothstep(0.3, 0.7, noise(p * 0.22 + vec2<f32>(gx, gz) * 0.5)));
    var mirror = sky * fresnel * pool.sky.a * bank;
    // Each light's reflection: its illuminance at the face times a narrow normalised lobe
    // about the mirror direction.
    let s = pool.look.y;
    let norm = (s + 2.0) / (2.0 * PI);
    let world = pbr_input.world_position.xyz;
    let count = i32(pool.look.x);
    for (var i = 0; i < count; i++) {
        let at = pool.lights[i * 2];
        let c = pool.lights[i * 2 + 1].rgb;
        let to = at.xyz - world;
        let d2 = max(dot(to, to), 0.01);
        let l = to * inverseSqrt(d2);
        let fall = d2 / (at.w * at.w);
        let window = clamp(1.0 - fall * fall, 0.0, 1.0);
        let lux = c / (4.0 * PI * d2) * window * window;
        let lobe = norm * pow(max(dot(r, l), 0.0), s);
        mirror += lux * lobe * fresnel * pool.look.z;
    }
    pbr_input.material.emissive = vec4<f32>(mirror, 1.0);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
