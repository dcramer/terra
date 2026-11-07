// The kit's one water (kit/water.rs, D634): the body absorbs by depth (Beer-Lambert, per
// channel) and scatters its own dark colour, lit as the ground is; the face fades in over
// the shore; scum and floating leaves mat it; the ripples are one tiling slope map read at
// two scales drifting downwind; the mirror is a planar reflection (a camera under the face
// renders what stands over it, sampled where this point projects into it, moved by the
// ripples), at Fresnel's (Schlick's) few percent; and each glinting light and the key make
// a streak toward the eye (an anisotropic Beckmann lobe, Bruneton, Neyret and Holzschuch
// 2010; its glitter Zirr and Kaplanyan 2016's stochastic facets at their simplest), ported
// from shaders/pool.wesl.

import bevy_pbr::render::{
    mesh_view_bindings::view,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

// Must match water.rs `Look`, field for field.
struct Look {
    clip_from_world: mat4x4<f32>,
    body: vec4<f32>,
    absorb: vec4<f32>,
    mirror: vec4<f32>,
    ripple: vec4<f32>,
    wind: vec4<f32>,
    scum: vec4<f32>,
    float_: vec4<f32>,
    pads: vec4<f32>,
    lane: vec4<f32>,
    gains: vec4<f32>,
    toward: vec4<f32>,
    key: vec4<f32>,
    lights: array<vec4<f32>, 12>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> look: Look;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var mirror_map: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var mirror_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var ripple_map: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(104)
var ripple_sampler: sampler;

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

// Rain's rings (metres of height), `wind.z` a square metre at once, each spreading and
// fading over two to five seconds.
fn rings(p: vec2<f32>, t: f32) -> f32 {
    var h = 0.0;
    for (var k = 0; k < 2; k++) {
        let cell = 1.3 + f32(k) * 0.6;
        let q = p / cell + vec2<f32>(f32(k) * 0.5, f32(k) * 0.37);
        let id = floor(q) + f32(k) * 17.0;
        let f = fract(q) - 0.5;
        let on = hash(id);
        if on > look.wind.z * cell * cell {
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

// Scum matting the margin in clumps, out to `float_.x` deep.
fn scum(p: vec2<f32>, depth: f32) -> f32 {
    if look.scum.w <= 0.0 {
        return 0.0;
    }
    let s = 1.0 / max(look.float_.y, 0.1);
    let clump = noise(p * s) * 0.7 + noise(p * s * 3.1 + 5.0) * 0.3;
    let reach = smoothstep(look.float_.x, look.float_.x * 0.35, depth) * smoothstep(-0.03, 0.01, depth);
    let mat = look.scum.w * reach * smoothstep(0.42, 0.62, clump);
    let grain = hash(floor(p * 70.0));
    return clamp(mat * 1.7 - grain * 0.7, 0.0, 1.0);
}

// Floating leaves in the open shallows: at most one a cell, round with a notch, each its
// own size, turn and age. x: cover, y: its shade, z: how yellowed.
fn pads(p: vec2<f32>, depth: f32) -> vec3<f32> {
    if look.float_.z <= 0.0 {
        return vec3<f32>(0.0);
    }
    let cell = inverseSqrt(look.float_.z);
    let id = floor(p / cell);
    let f = (fract(p / cell) - 0.5) * cell;
    let calm = smoothstep(0.06, 0.14, depth) * smoothstep(0.45, 0.25, depth);
    if hash(id + 41.0) > calm * 0.75 {
        return vec3<f32>(0.0);
    }
    let r = look.float_.w * (0.55 + 0.65 * hash(id + 5.3));
    let room = max(cell * 0.5 - r, 0.0);
    let d = f - (vec2<f32>(hash(id + 13.1), hash(id + 29.7)) - 0.5) * 2.0 * room;
    let dist = length(d);
    let a = atan2(d.y, d.x) + hash(id + 3.3) * 6.283;
    let notch = smoothstep(0.16, 0.26, abs(atan2(sin(a), cos(a))));
    let cover = smoothstep(r, r - 0.012, dist) * mix(1.0, notch, smoothstep(0.0, 0.03, dist));
    let shade = mix(0.75, 1.15, smoothstep(r * 0.4, r, dist)) * (0.8 + 0.4 * hash(id + 7.7));
    return vec3<f32>(cover, shade, smoothstep(0.75, 0.95, hash(id + 17.0)));
}

// The slope a facet needs to mirror `l` into the eye, over the lane's spreads: x along the
// line toward the eye, y across it; z the half vector's cosine.
fn need(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec3<f32> {
    let h = normalize(l + v);
    let flat_ = vec3<f32>(v.x, 0.0, v.z);
    let toward = select(vec3<f32>(0.0, 0.0, 1.0), normalize(flat_), dot(flat_, flat_) > 1e-6);
    let a = normalize(toward - n * dot(toward, n));
    let c = cross(n, a);
    let hn = max(dot(h, n), 0.05);
    return vec3<f32>(dot(h, a) / hn / look.lane.x, dot(h, c) / hn / look.lane.y, hn);
}

// A light's streak: Beckmann's D over 4 cos_v, the radiance one lux gives at a Fresnel of
// 1; the smooth lane and a glitter of facets in cells drifting downwind, reshuffled four
// times a second, whose mean is the same lane.
fn streak(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, p: vec2<f32>, t: f32) -> f32 {
    if dot(n, l) <= 0.0 {
        return 0.0;
    }
    let s = need(n, v, l);
    let norm = 1.0 / (PI * look.lane.x * look.lane.y * s.z * s.z * s.z * s.z);
    let smooth_ = exp(-dot(s.xy, s.xy)) * norm;
    var glitter = 0.0;
    if look.lane.w > 0.0 {
        let k = 0.45;
        let toward = select(vec2<f32>(0.0, 1.0), normalize(v.xz), dot(v.xz, v.xz) > 1e-6);
        let across = vec2<f32>(-toward.y, toward.x);
        let drift = p - look.wind.xy * t * 0.15;
        let q = vec2<f32>(dot(drift, across) / max(look.toward.w, 1.0), dot(drift, toward)) / look.lane.z;
        let id = floor(q);
        let seed = id + floor(t * 4.0 + hash(id + 2.7) * 4.0) * 17.31;
        let g = vec2<f32>(
            hash(seed) + hash(seed + 1.3) + hash(seed + 2.9) - 1.5,
            hash(seed + 4.1) + hash(seed + 5.7) + hash(seed + 6.2) - 1.5,
        ) * 1.414;
        let d = (s.xy - g) / k;
        let dot_ = smoothstep(0.5, 0.25, length(fract(q) - 0.5));
        glitter = exp(-dot(d, d)) * norm / (k * k) * dot_ * 2.0;
    }
    return mix(smooth_, glitter, look.lane.w) / (4.0 * max(dot(n, v), 0.05));
}

// The ripples' slope (world xz): the map read at the fine and the broad tile, turned so its
// +x runs downwind, each drifting its own speed.
fn slope(p: vec2<f32>, t: f32) -> vec2<f32> {
    let speed = length(look.wind.xy);
    let w = select(vec2<f32>(1.0, 0.0), look.wind.xy / max(speed, 1e-4), speed > 1e-4);
    let side = vec2<f32>(-w.y, w.x);
    let local = vec2<f32>(dot(p, w), dot(p, side));
    let fine_uv = (local - vec2<f32>(speed * t * 0.12, 0.0)) / max(look.ripple.x, 0.01);
    let broad_uv = (local.yx * vec2<f32>(0.83, 1.0) + vec2<f32>(0.37, speed * t * 0.05)) / max(look.ripple.y, 0.01);
    let a = textureSample(ripple_map, ripple_sampler, fine_uv).xy * 2.0 - 1.0;
    let b = (textureSample(ripple_map, ripple_sampler, broad_uv).yx * 2.0 - 1.0) * 0.6;
    let s = (a + b) * look.ripple.z;
    return s.x * w + s.y * side;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let p = in.uv;
    let t = look.wind.w;
    let depth = select(in.uv_b.x, look.absorb.w, look.absorb.w > 0.0);
    let shore = smoothstep(0.0, max(look.body.w, 1e-4), in.uv_b.x);

    // The face's tilt: the ripples, stiller in the margin, and rain's rings.
    var g = slope(p, t);
    if look.wind.z > 0.0 {
        let e = 0.01;
        let h = rings(p, t);
        g += vec2<f32>(rings(p + vec2<f32>(e, 0.0), t) - h, rings(p + vec2<f32>(0.0, e), t) - h) / e;
    }
    let calm = mix(1.0 - look.ripple.w, 1.0, smoothstep(0.0, 4.0 * look.body.w, in.uv_b.x));
    let weed = scum(p, in.uv_b.x);
    let pad = pads(p, in.uv_b.x);
    let floating = max(weed, pad.x);
    let still = calm * (1.0 - floating);
    let n = normalize(vec3<f32>(-g.x * still, 1.0, -g.y * still));
    pbr_input.N = n;

    // The body: what is left of the bed's light through it (Beer-Lambert), one alpha a
    // pixel; its scatter colour premultiplied; scum and leaves opaque over it.
    let tr = exp(-look.absorb.rgb * max(depth, 0.0));
    let a = shore * (1.0 - (tr.r + tr.g + tr.b) / 3.0);
    let grain = 0.7 + 0.6 * hash(floor(p * 70.0) + 3.0);
    let weed_c = look.scum.rgb * grain;
    let pad_c = mix(look.pads.rgb * vec3<f32>(0.7, 0.95, 0.7), vec3<f32>(0.09, 0.07, 0.02), pad.z) * pad.y;
    var base = vec4<f32>(look.body.rgb * a, a);
    base = mix(base, vec4<f32>(weed_c, 1.0), weed);
    base = mix(base, vec4<f32>(pad_c, 1.0), pad.x);
    pbr_input.material.base_color = base;

    // The mirror. V points from the face to the eye.
    let v = pbr_input.V;
    let cos_v = clamp(dot(n, v), 0.0, 1.0);
    let fresnel = 0.02 + 0.98 * pow(1.0 - cos_v, 5.0);
    let world = pbr_input.world_position.xyz;
    var reflected = vec3<f32>(0.0);
    if look.mirror.z > 0.0 {
        let moved = world + vec3<f32>(n.x, 0.0, n.z) * look.mirror.y;
        let clip = look.clip_from_world * vec4<f32>(moved, 1.0);
        let uv = clip.xy / max(clip.w, 1e-4) * vec2<f32>(0.5, -0.5) + 0.5;
        if look.mirror.w > 0.0 {
            // A thin film over rough asphalt mirrors through the surface's spread of
            // slopes: GGX's lobe, at a grazing view, long in the plane of incidence, so a
            // window or a sign streaks up and down the frame (Lagarde, "Water drop 2b",
            // 2013: wet roads' stretched reflections); a deep puddle stays a mirror.
            // Twelve taps along the frame's height, a tent's weights, jittered a pixel.
            let thin = 1.0 - smoothstep(0.008, 0.03, in.uv_b.x);
            let reach = look.mirror.w * mix(0.25, 1.0, thin);
            let jitter = hash(floor(in.position.xy)) - 0.5;
            var sum = vec3<f32>(0.0);
            var weights = 0.0;
            for (var i = 0; i < 12; i++) {
                let s = (f32(i) + 0.5 + jitter) / 12.0 * 2.0 - 1.0;
                let w = 1.0 - abs(s);
                let at = uv + vec2<f32>((hash(vec2<f32>(f32(i), jitter)) - 0.5) * reach * 0.06, s * reach);
                sum += textureSampleLevel(mirror_map, mirror_sampler, clamp(at, vec2<f32>(0.001), vec2<f32>(0.999)), 0.0).rgb * w;
                weights += w;
            }
            reflected = sum / weights;
        } else {
            reflected = textureSampleLevel(mirror_map, mirror_sampler, clamp(uv, vec2<f32>(0.001), vec2<f32>(0.999)), 0.0).rgb;
        }
    }
    // The key's lane (the moon's), on a calmer face and under its soft peak, and each
    // glinting light's streak (nits).
    var nits = vec3<f32>(0.0);
    if look.lane.x > 0.0 && look.gains.y > 0.0 {
        let l = normalize(look.toward.xyz);
        let c = 1.0 - look.gains.z;
        let nm = normalize(vec3<f32>(n.x * c, n.y, n.z * c));
        var moon = look.key.rgb * streak(nm, v, l, p, t) * fresnel * look.gains.y;
        if look.gains.w > 0.0 {
            moon = moon / (1.0 + max(moon.r, max(moon.g, moon.b)) / look.gains.w);
        }
        nits += moon;
    }
    let count = i32(look.pads.w);
    for (var i = 0; i < count; i++) {
        let at = look.lights[i * 2];
        let c = look.lights[i * 2 + 1].rgb;
        let to = at.xyz - world;
        let d2 = max(dot(to, to), 0.01);
        let l = to * inverseSqrt(d2);
        let fall = d2 / (at.w * at.w);
        let window = clamp(1.0 - fall * fall, 0.0, 1.0);
        let lux = c / (4.0 * PI * d2) * window * window;
        nits += lux * streak(n, v, l, p, t) * fresnel * look.gains.x;
    }

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    // Premultiplied: what the face mirrors adds over whatever of the bed still shows.
    let mirrored = reflected * fresnel * look.mirror.x + nits * view.exposure;
    out.color = vec4<f32>(out.color.rgb + mirrored * shore * (1.0 - floating), out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
