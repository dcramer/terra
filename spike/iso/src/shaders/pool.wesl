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

struct Edge {
    fade: vec4<f32>,
    weed: vec4<f32>,
    green: vec4<f32>,
}

struct Sheen {
    toward: vec4<f32>,
    colour: vec4<f32>,
    lane: vec4<f32>,
    moon: vec4<f32>,
    soft: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> pool: Look;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var<uniform> edge: Edge;
struct Body {
    ripple: vec4<f32>,
    clear: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var<uniform> sheen: Sheen;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var<uniform> body: Body;

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

// The breeze's fine ripples by day (Body): two scales a golden ratio apart, each turned its
// own way and drifting its own speed, so their sum never repeats and no lattice shows; in
// calm and ruffled patches (cat's paws) a broad noise lays over them, so the sun's glitter
// is many small sharp glints in drifts, not one even field.
fn fine(p: vec2<f32>, t: f32) -> f32 {
    let f = 1.0 / max(body.ripple.y, 0.01);
    let w = pool.wave.xy;
    let a = mat2x2<f32>(0.825, 0.565, -0.565, 0.825);
    let b = mat2x2<f32>(-0.416, 0.909, -0.909, -0.416);
    let h = noise(a * (p * f) + w * t * 2.1) + noise(b * (p * f * 1.618) - w.yx * t * 3.4 + 7.3) * 0.6;
    let s = 1.0 / max(body.ripple.z, 0.1);
    let paws = smoothstep(0.3, 0.7, noise(p * s - w * t * 0.2) * 0.7 + noise(p * s * 2.43 + 3.7) * 0.3);
    return h * mix(1.0 - body.ripple.w, 1.0, paws) * body.ripple.x;
}

fn height(p: vec2<f32>, t: f32) -> f32 {
    var h = wind(p, t) * pool.wave.z + rings(p, t);
    if body.ripple.x > 0.0 {
        h += fine(p, t);
    }
    return h;
}

// Duckweed matting the margin: how much covers a point, in patches broken grain by grain
// at their edges (a frond is a few millimetres; a cell here about a pixel at the camera).
fn duckweed(p: vec2<f32>, depth: f32) -> f32 {
    if edge.weed.x <= 0.0 {
        return 0.0;
    }
    let s = 1.0 / max(edge.weed.z, 0.1);
    let clump = noise(p * s) * 0.7 + noise(p * s * 3.1 + 5.0) * 0.3;
    let reach = smoothstep(edge.weed.y, edge.weed.y * 0.35, depth) * smoothstep(-0.03, 0.01, depth);
    let mat = edge.weed.x * reach * smoothstep(0.42, 0.62, clump);
    let grain = hash(floor(p * 70.0));
    return clamp(mat * 1.7 - grain * 0.7, 0.0, 1.0);
}

// Lily pads in the open shallows: at most one a cell, a round leaf with its notch, each its
// own size, turn and age. x: cover (0..1), y: its shade (dark at the heart, light at the
// rim), z: how yellowed it is.
fn pads(p: vec2<f32>, depth: f32) -> vec3<f32> {
    if edge.weed.w <= 0.0 {
        return vec3<f32>(0.0);
    }
    let cell = inverseSqrt(edge.weed.w);
    let id = floor(p / cell);
    let f = (fract(p / cell) - 0.5) * cell;
    let calm = smoothstep(0.06, 0.14, depth) * smoothstep(0.45, 0.25, depth);
    if hash(id + 41.0) > calm * 0.75 {
        return vec3<f32>(0.0);
    }
    let r = edge.green.w * (0.55 + 0.65 * hash(id + 5.3));
    let room = max(cell * 0.5 - r, 0.0);
    let d = f - (vec2<f32>(hash(id + 13.1), hash(id + 29.7)) - 0.5) * 2.0 * room;
    let dist = length(d);
    let a = atan2(d.y, d.x) + hash(id + 3.3) * 6.283;
    let notch = smoothstep(0.16, 0.26, abs(atan2(sin(a), cos(a))));
    let cover = smoothstep(r, r - 0.012, dist) * mix(1.0, notch, smoothstep(0.0, 0.03, dist));
    let shade = mix(0.75, 1.15, smoothstep(r * 0.4, r, dist)) * (0.8 + 0.4 * hash(id + 7.7));
    return vec3<f32>(cover, shade, smoothstep(0.75, 0.95, hash(id + 17.0)));
}

// The slope a facet needs to mirror `l` into the eye, in the face's frame, over the lane's
// spreads: x along the line toward the eye, y across it; z the half vector's cosine.
fn need(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec3<f32> {
    let h = normalize(l + v);
    let flat_ = vec3<f32>(v.x, 0.0, v.z);
    let toward = select(vec3<f32>(0.0, 0.0, 1.0), normalize(flat_), dot(flat_, flat_) > 1e-6);
    let a = normalize(toward - n * dot(toward, n));
    let c = cross(n, a);
    let hn = max(dot(h, n), 0.05);
    return vec3<f32>(dot(h, a) / hn / sheen.lane.x, dot(h, c) / hn / sheen.lane.y, hn);
}

// A light's streak on the rippled face: Beckmann's D over 4 cos_v, the radiance one lux of
// it gives at a Fresnel of 1. The smooth lane the ripples average to (Cox and Munk's slope
// spread, as Bruneton, Neyret and Holzschuch 2010 light the sea with), and a glitter of
// facets each tilted its own way, in cells drifting downwind and reshuffled four times a
// second, whose mean is the same lane (stochastic microfacets, Zirr and Kaplanyan 2016's
// idea at its simplest): the lane breaks into sparkles as moonlit water does.
fn streak(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, p: vec2<f32>, t: f32) -> f32 {
    if dot(n, l) <= 0.0 {
        return 0.0;
    }
    let s = need(n, v, l);
    let norm = 1.0 / (PI * sheen.lane.x * sheen.lane.y * s.z * s.z * s.z * s.z);
    let smooth_ = exp(-dot(s.xy, s.xy)) * norm;
    var glitter = 0.0;
    if sheen.lane.w > 0.0 {
        // A facet's own lobe, k of the lane's spread; a dot in its cell, drawn `soft.z`
        // times as long across the line to the eye as along it (a dash).
        let k = 0.45;
        let toward = select(vec2<f32>(0.0, 1.0), normalize(v.xz), dot(v.xz, v.xz) > 1e-6);
        let across = vec2<f32>(-toward.y, toward.x);
        let drift = p - pool.wave.xy * t * 0.15;
        let q = vec2<f32>(dot(drift, across) / max(sheen.soft.z, 1.0), dot(drift, toward)) / sheen.lane.z;
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
    return mix(smooth_, glitter, sheen.lane.w) / (4.0 * max(dot(n, v), 0.05));
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let p = in.uv;
    let depth = in.uv_b.x;
    let t = pool.wave.w;

    // The slope's step: a small share of the fine ripples' wavelength where there are any.
    let e = select(0.02, min(0.02, body.ripple.y * 0.08), body.ripple.x > 0.0);
    let h = height(p, t);
    let gx = (height(p + vec2<f32>(e, 0.0), t) - h) / e;
    let gz = (height(p + vec2<f32>(0.0, e), t) - h) / e;
    // Calmer in the margin, where the reeds and the bank break the wind.
    let calm = mix(0.3, 1.0, smoothstep(0.02, 0.3, depth));
    // What floats there: duckweed in the margin, lily pads in the open shallows; flat, matt,
    // lit as leaves are and mirroring nothing.
    let weed = duckweed(p, depth);
    let pad = pads(p, depth);
    let float_ = max(weed, pad.x);
    let still = calm * (1.0 - float_);
    let n = normalize(vec3<f32>(-gx * still, 1.0, -gz * still));
    pbr_input.N = n;

    // The water's own colour: the mud through the shallows, the dark of the deep, fading in
    // over the bank's first few centimetres of depth (more where the margin is soft).
    let k = smoothstep(0.0, pool.deep.a, depth);
    let colour = mix(pool.shallow.rgb, pool.deep.rgb, k);
    let fade = select(0.06, edge.fade.x, edge.fade.x > 0.0);
    // How much the water hides of the bed: near black in the deep, or clear (Body).
    let opaque = mix(1.0 - pool.shallow.a, select(0.97, body.clear.x, body.clear.x > 0.0), k);
    let alpha = smoothstep(-0.02, fade, depth) * opaque;
    let grain = 0.7 + 0.6 * hash(floor(p * 70.0) + 3.0);
    let weed_c = edge.green.rgb * grain;
    let pad_c = mix(edge.green.rgb * vec3<f32>(0.7, 0.95, 0.7), vec3<f32>(0.09, 0.07, 0.02), pad.z) * pad.y;
    var base = vec4<f32>(colour, alpha);
    base = mix(base, vec4<f32>(weed_c, 1.0), weed);
    base = mix(base, vec4<f32>(pad_c, 1.0), pad.x);
    pbr_input.material.base_color = base;

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
    // Broad blots in the mirrored sky: clouds and the far bank's trees, a few metres across.
    let cloud = 1.0 / select(8.0, edge.fade.z, edge.fade.z > 0.0);
    let blot = noise(p * cloud + vec2<f32>(gx, gz) * 0.3) * 0.65 + noise(p * cloud * 2.7 + 9.0) * 0.35;
    let broken = mix(1.0, mix(0.2, 1.6, smoothstep(0.25, 0.75, blot)), edge.fade.y);
    var mirror = sky * fresnel * pool.sky.a * bank * broken;
    // The moon's sheen: a broad lobe about its mirror direction, darkened by the bank as the
    // sky is and only half broken by the blots, and a sharper glitter the ripples tilt out
    // of it.
    if sheen.toward.w > 0.0 {
        let c = max(dot(r, normalize(sheen.toward.xyz)), 0.0);
        let broad = pow(c, sheen.toward.w);
        let glitter = pow(c, sheen.toward.w * 12.0) * 3.0;
        let lobe = mix(broad, glitter, sheen.colour.w);
        mirror += sheen.colour.rgb * lobe * fresnel * pool.sky.a * bank * mix(1.0, broken, 0.5);
    }
    // The moon's lane: its own light off the ripples, long toward the eye, glittering.
    let lane = sheen.lane.x > 0.0;
    if lane && sheen.moon.w > 0.0 {
        let l = normalize(sheen.toward.xyz);
        // Seen on a calmer face than the lights' (`soft.y`), and brought softly under its
        // peak (`soft.x`), so it is one long soft streak, never a field of blown speckles.
        let calm_ = 1.0 - sheen.soft.y;
        let nm = normalize(vec3<f32>(n.x * calm_, n.y, n.z * calm_));
        var moon = sheen.moon.rgb * streak(nm, v, l, p, t) * fresnel * sheen.moon.w;
        if sheen.soft.x > 0.0 {
            moon = moon / (1.0 + max(moon.r, max(moon.g, moon.b)) / sheen.soft.x);
        }
        mirror += moon;
    }
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
        var lobe = norm * pow(max(dot(r, l), 0.0), s);
        if lane {
            lobe = streak(n, v, l, p, t);
        }
        mirror += lux * lobe * fresnel * pool.look.z;
    }
    // Clear water: what it mirrors lies over the bed it shows, not dimmed by the blend.
    if body.clear.x > 0.0 {
        mirror /= max(mix(opaque, 1.0, float_), 0.05);
    }
    pbr_input.material.emissive = vec4<f32>(mirror * (1.0 - float_), 1.0);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
