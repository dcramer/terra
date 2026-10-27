// The volumetric clouds (docs/sky.md stage 2, D515), marched at half resolution after the depth
// prepass (sky/volume/render.rs) into a picture the veil lays over the scene (cloud_veil.wesl):
// rgb the light the clouds send the eye, exposed and premultiplied, a how much they hide.
//
// Ported in outline from Horizon's Nubis as the bevy branch `mate-h/bevy`
// `volumetric-clouds-nubis` (MIT/Apache-2.0, crates/bevy_pbr/src/atmosphere/clouds*.wesl) has it:
// Schneider, "The real-time volumetric cloudscapes of Horizon Zero Dawn" (SIGGRAPH 2015) and
// "Nubis: authoring real-time volumetric cloudscapes" (2017) for the density, its remaps and the
// coarse and fine march; Hillaire, "Physically based sky, atmosphere and cloud rendering in
// Frostbite" (2016) for the energy-conserving step and the octaves of multiple scattering;
// Jendersie and d'Eon, "An approximate Mie scattering function for fog and cloud rendering"
// (SIGGRAPH 2023 talks) for the phase. Cut to stage 2's MVP: cumulus, stratocumulus and the
// overcast deck by the weather map's kind; no shadows on the ground, night, cirrus or storms.
//
// The frame is the air's (D334): the start's flat frame through `to_map`, x east, y up from the
// start's ground, z south, in metres from the weather map's middle. The columns of cloud stand
// where the sky's 2D layer draws them (the same noise and offsets, cloud_noise.wesl), so the
// low preset's sky and this one agree.

import bevy_render::view::View;
import super::cloud_noise::wrapped_noise;

// Field order must match `Params` in sky/volume.rs.
struct Params {
    to_map: mat4x4<f32>,
    shapes: array<vec4<f32>, 4>,
    volume: vec4<f32>,
    map: vec4<f32>,
    fallback: vec4<f32>,
    sun: vec4<f32>,
    sunlight: vec4<f32>,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var noise: texture_3d<f32>;
@group(0) @binding(4) var map_now: texture_2d<f32>;
@group(0) @binding(5) var map_next: texture_2d<f32>;
@group(0) @binding(6) var air: texture_cube<f32>;
@group(0) @binding(7) var repeating: sampler;
@group(0) @binding(8) var clamped: sampler;
@group(0) @binding(9) var picture: texture_storage_2d<rgba16float, write>;

// The 3D noise's tiles, metres (clouds::SHAPE_TILE_M, DETAIL_TILE_M), and its side in texels.
const SHAPE_TILE_M: f32 = 6000.0;
const DETAIL_TILE_M: f32 = 750.0;
const NOISE_SIDE: f32 = 64.0;
// The slab every kind of cloud stands in, over the start's ground, and how far a ray is marched.
const SLAB_BOTTOM_M: f32 = 300.0;
const SLAB_TOP_M: f32 = 3600.0;
const FARTHEST_M: f32 = 32000.0;
// The march's budget of samples, and how many empty fine ones return it to coarse steps.
const STEPS: u32 = 96u;
const EMPTY_TO_COARSE: u32 = 10u;
// Extinction a metre at density 1: cumulus (liquid water 0.4 g/m3, droplets of 10 um, so some
// 50 m of sight inside), and the thinner stratus deck.
const CUMULUS_EXTINCTION: f32 = 0.06;
const DECK_EXTINCTION: f32 = 0.03;
// The distance over which the air's haze takes the clouds (as the 2D layer fades by lift).
const HAZE_M: f32 = 20000.0;
// The most a heaped column covers, so the 3D noise cuts it into heaps (Nubis's coverage remap).
const HEAPED_COVERAGE: f32 = 0.5;
// Where the body's remapped noise starts, so it spans 0 to 1 (`shape`).
const BODY_LOW: f32 = 0.6;
// The asymmetry of light many times scattered in cloud, for the share of sunlight that diffuses
// through a depth of it (the two-stream approximation: Bohren, "Multiple scattering of light and
// some of its observable consequences", Am. J. Phys. 1987): 1 / (1 + 3/4 (1 - g) tau).
const DIFFUSE_G: f32 = 0.85;
const PI: f32 = 3.14159265;

fn remap(v: f32, lo: f32, hi: f32) -> f32 {
    return clamp((v - lo) / max(hi - lo, 1.0e-4), 0.0, 1.0);
}

// The air's light from a direction in the world, exposed (sky.wesl's `air_seen`; the cube is
// left-handed, as Bevy's environment maps are).
fn air_seen(direction: vec3<f32>) -> vec3<f32> {
    let d = vec3<f32>(direction.x, direction.y, -direction.z);
    return textureSampleLevel(air, clamped, d, 0.0).rgb * view.exposure;
}

// The weather over a place: cover, what falls, the kind (1 heaped, 0.5 stratocumulus, 0 a deck)
// and the snow, blended from this step's map to the next's (sky.wesl's `weather_at`).
fn weather_at(metres: vec2<f32>) -> vec4<f32> {
    if params.map.x == 0.0 {
        return vec4<f32>(params.fallback.x, params.fallback.y, params.fallback.z, 0.0);
    }
    let uv = metres * params.map.z + 0.5;
    let now = textureSampleLevel(map_now, clamped, uv, 0.0);
    let next = textureSampleLevel(map_next, clamped, uv, 0.0);
    return mix(now, next, params.map.y);
}

// A column of the air: how much of it the cloud's shapes cover, and the layer they fill there.
struct Column {
    coverage: f32,
    base: f32,
    thickness: f32,
    kind: f32,
    rain: f32,
}

fn column(metres: vec2<f32>) -> Column {
    let w = weather_at(metres);
    let cover = max(params.map.w, w.r);
    // The 2D layer's shapes on their first two octaves (sky.wesl's `drifting`, field 0), cut as
    // it cuts them, so the columns stand where the 2D clouds would.
    let p = vec3<f32>(metres.x, 0.0, metres.y) * params.sun.w;
    let shapes = (wrapped_noise(p + params.shapes[0].xyz) * 0.5
        + wrapped_noise(p * 2.1 + params.shapes[1].xyz) * 0.27) / 0.77;
    let threshold = 1.0 - cover * 0.62;
    var c: Column;
    c.kind = w.b;
    // Heaped cloud leaves the noise more to cut, so it breaks into heaps; a deck is whole.
    c.coverage = remap(shapes, threshold - 0.12, threshold + 0.1) * mix(1.0, HEAPED_COVERAGE, c.kind);
    c.rain = w.g;
    // The layer by kind (sky.md's table): cumulus from 1.1 km, thicker the more it covers;
    // stratocumulus from 900 m, 500 m thick; the deck lower and thicker the more it rains.
    let cumulus = vec2<f32>(1100.0, 600.0 + 1400.0 * cover);
    let strato = vec2<f32>(900.0, 500.0);
    let deck = vec2<f32>(500.0 - 200.0 * c.rain, 600.0 + 900.0 * c.rain);
    var layer = mix(deck, strato, clamp(c.kind * 2.0, 0.0, 1.0));
    if c.kind > 0.5 {
        layer = mix(strato, cumulus, clamp(c.kind * 2.0 - 1.0, 0.0, 1.0));
    }
    c.base = layer.x;
    c.thickness = layer.y;
    return c;
}

// How far up its layer a height is, 0 at the base and 1 at the top.
fn lift_in(c: Column, y: f32) -> f32 {
    return (y - c.base) / c.thickness;
}

// The height profile: heaps rise from a flat base and round off toward their tops; a deck is
// even through its depth.
fn profile(c: Column, h: f32) -> f32 {
    if h <= 0.0 || h >= 1.0 {
        return 0.0;
    }
    let heaped = smoothstep(0.0, 0.07, h) * smoothstep(1.0, 0.45, h);
    let even = smoothstep(0.0, 0.15, h) * smoothstep(1.0, 0.75, h);
    return mix(even, heaped, c.kind);
}

// The cloud's body without its eroded edges: Perlin-Worley remapped by its Worley fBm, cut by
// the column's coverage (Nubis 2015).
fn shape(c: Column, p: vec3<f32>, lod: f32) -> f32 {
    let profiled = profile(c, lift_in(c, p.y));
    if profiled <= 0.0 || c.coverage <= 0.0 {
        return 0.0;
    }
    let uvw = vec3<f32>(p.x / SHAPE_TILE_M + params.volume.x, p.y / SHAPE_TILE_M, p.z / SHAPE_TILE_M + params.volume.y);
    let n = textureSampleLevel(noise, repeating, uvw, lod);
    let fbm = n.g * 0.625 + n.b * 0.25 + n.a * 0.125;
    // The remap leaves it between about 0.7 and 0.95; stretched, so a coverage cuts it
    // gradually rather than all or nothing.
    let body = remap(remap(n.r, fbm - 1.0, 1.0), BODY_LOW, 1.0);
    return remap(body * profiled, 1.0 - c.coverage, 1.0) * c.coverage;
}

// The edges eroded by the finer Worley noise: wispy low down, billowing higher up (Nubis 2015).
fn eroded(c: Column, p: vec3<f32>, body: f32, lod: f32) -> f32 {
    let uvw = vec3<f32>(p.x / DETAIL_TILE_M + params.volume.z, p.y / DETAIL_TILE_M, p.z / DETAIL_TILE_M + params.volume.w);
    let d = textureSampleLevel(noise, repeating, uvw, lod);
    let fbm = d.g * 0.625 + d.b * 0.25 + d.a * 0.125;
    let h = clamp(lift_in(c, p.y) * 4.0, 0.0, 1.0);
    let edge = mix(fbm, 1.0 - fbm, h);
    return remap(body, edge * 0.4, 1.0) * (1.0 + 2.0 * c.rain);
}

fn extinction(c: Column) -> f32 {
    return mix(DECK_EXTINCTION, CUMULUS_EXTINCTION, c.kind);
}

// Jendersie and d'Eon's approximate Mie phase for droplets 20 um across: a Henyey-Greenstein
// lobe for the diffraction peak blended with Draine's for the rest (their eqs. 4 to 7, fitted
// constants evaluated at d = 20).
fn henyey_greenstein(cos_t: f32, g: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / (4.0 * PI * pow(max(1.0 + g2 - 2.0 * g * cos_t, 1.0e-4), 1.5));
}

fn draine(cos_t: f32, g: f32, alpha: f32) -> f32 {
    let g2 = g * g;
    return henyey_greenstein(cos_t, g) * (1.0 + alpha * cos_t * cos_t) / (1.0 + alpha * (1.0 + 2.0 * g2) / 3.0);
}

fn mie(cos_t: f32) -> f32 {
    let g_hg = 0.99461;
    let g_d = 0.59380;
    let alpha = 27.1137;
    let w_d = 0.49816;
    return mix(henyey_greenstein(cos_t, g_hg), draine(cos_t, g_d, alpha), w_d);
}

// Optical depth toward the sun: six taps from 40 m growing by half again each, and one long
// tap at a kilometre, on the body without erosion.
fn toward_sun(c: Column, p: vec3<f32>, sun: vec3<f32>) -> f32 {
    var tau = 0.0;
    var along = 0.0;
    var step = 40.0;
    for (var k = 0u; k < 6u; k++) {
        let at = p + sun * (along + step * 0.5);
        tau += shape(c, at, log2(step / (SHAPE_TILE_M / NOISE_SIDE)) + 1.0) * step;
        along += step;
        step *= 1.5;
    }
    tau += shape(c, p + sun * 1000.0, 3.0) * 600.0;
    return tau * extinction(c);
}

// Interleaved gradient noise (Jimenez 2014), turned each frame, to start each ray at its own
// offset for TAA to resolve.
fn jitter(pixel: vec2<f32>) -> f32 {
    let p = pixel + params.sunlight.w * 5.588238;
    return fract(52.9829189 * fract(0.06711056 * p.x + 0.00583715 * p.y));
}

// The fine step: 20 m near the eye, growing with distance so far clouds take as many samples.
fn fine_step(t: f32) -> f32 {
    return clamp(t * 0.02, 20.0, 500.0);
}

@compute @workgroup_size(8, 8, 1)
fn march(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(picture);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let uv = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(size);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let eye = view.world_position;
    let near = view.world_from_clip * vec4<f32>(ndc, 1.0, 1.0);
    let direction = normalize(near.xyz / near.w - eye);

    // The scene's nearest depth under this half-resolution pixel (reversed: 0 is infinitely far).
    let full = textureDimensions(depth);
    let corner = min(id.xy * 2u, full - 1u);
    var nearest = 0.0;
    for (var k = 0u; k < 4u; k++) {
        let at = min(corner + vec2<u32>(k & 1u, k >> 1u), full - 1u);
        nearest = max(nearest, textureLoad(depth, at, 0));
    }
    var scene = FARTHEST_M;
    if nearest > 0.0 {
        let hit = view.world_from_clip * vec4<f32>(ndc, nearest, 1.0);
        scene = min(scene, length(hit.xyz / hit.w - eye));
    }

    // Into the map's frame.
    let e = (params.to_map * vec4<f32>(eye, 1.0)).xyz;
    let d = normalize((params.to_map * vec4<f32>(direction, 0.0)).xyz);
    let sun = normalize((params.to_map * vec4<f32>(params.sun.xyz, 0.0)).xyz);

    // Where the ray is inside the slab.
    var t0 = 0.0;
    var t1 = scene;
    if abs(d.y) > 1.0e-5 {
        let a = (SLAB_BOTTOM_M - e.y) / d.y;
        let b = (SLAB_TOP_M - e.y) / d.y;
        t0 = max(min(a, b), 0.0);
        t1 = min(max(a, b), scene);
    } else if e.y < SLAB_BOTTOM_M || e.y > SLAB_TOP_M {
        t1 = 0.0;
    }
    if t1 <= t0 {
        textureStore(picture, id.xy, vec4<f32>(0.0));
        return;
    }

    // The light: the sun's through the air (sky.wesl's `over_air`), the sky's from above and
    // the ground's from below, each exposed.
    let up = normalize(eye);
    let height = sun.y;
    let air_mass = 1.0 / max(height + 0.08, 0.08);
    let sunlight = params.sunlight.rgb * exp(-vec3<f32>(0.06, 0.14, 0.32) * air_mass)
        * smoothstep(-0.06, 0.02, height) * view.exposure;
    let world_sun = normalize(params.sun.xyz);
    let level = normalize(world_sun - up * dot(world_sun, up) + vec3<f32>(1.0e-4, 0.0, 0.0));
    let sky = (air_seen(up) * 2.0 + air_seen(normalize(up + level)) + air_seen(normalize(up - level))) * 0.25;
    let ground = 0.2 * (sunlight * max(height, 0.0) / PI + sky);
    let cos_t = dot(d, sun);
    let phase = mie(cos_t);
    let iso = 1.0 / (4.0 * PI);

    var transmittance = 1.0;
    var light = vec3<f32>(0.0);
    var first = -1.0;
    var fine = false;
    var empty = 0u;
    var t = t0;
    var dt = max((t1 - t0) / f32(STEPS), fine_step(t0));
    t += dt * jitter(vec2<f32>(id.xy));
    for (var i = 0u; i < STEPS; i++) {
        if t >= t1 || transmittance < 0.01 {
            break;
        }
        if !fine {
            // Coarse: only the weather and the profile, stretched over what is left.
            dt = max((t1 - t) / f32(STEPS - i), fine_step(t));
            let p = e + d * t;
            let c = column(p.xz);
            if c.coverage * profile(c, lift_in(c, p.y)) > 0.0 {
                // A hit: step back and walk it finely.
                fine = true;
                empty = 0u;
                t = max(t - dt, t0);
                continue;
            }
            t += dt;
            continue;
        }
        dt = min(fine_step(t), t1 - t);
        let p = e + d * (t + dt * 0.5);
        let c = column(p.xz);
        let lod = log2(max(dt, 1.0) / (SHAPE_TILE_M / NOISE_SIDE));
        let body = shape(c, p, max(lod, 0.0));
        var density = 0.0;
        if body > 0.0 {
            density = eroded(c, p, body, max(log2(max(dt, 1.0) / (DETAIL_TILE_M / NOISE_SIDE)), 0.0));
        }
        if density > 0.0 {
            empty = 0u;
            if first < 0.0 {
                first = t;
            }
            let sigma = density * extinction(c);
            let tau = toward_sun(c, p, sun);
            // The sun's light scattered once, through the cloud toward it, with its forward peak;
            // and what the cloud has scattered many times on the way, diffused, which lights a
            // thick cloud's shaded side and base (Beer's law alone left them black).
            let direct = exp(-tau);
            let diffused = (1.0 - direct) / (1.0 + 0.75 * (1.0 - DIFFUSE_G) * tau);
            let sunward = sunlight * (direct * phase + diffused * 2.0 * iso);
            // The sky's light from above and the ground's from below, isotropic over their
            // hemisphere, less of each the deeper into the layer from that side.
            let h = clamp(lift_in(c, p.y), 0.0, 1.0);
            let ambient = 0.5 * (sky * mix(0.5, 1.0, h) + ground * mix(1.0, 0.5, h));
            let scattered = (sunward + ambient) * sigma;
            let through = exp(-sigma * dt);
            light += transmittance * (scattered - scattered * through) / max(sigma, 1.0e-6);
            transmittance *= through;
        } else {
            empty += 1u;
            if empty >= EMPTY_TO_COARSE {
                fine = false;
            }
        }
        t += dt;
    }
    let opacity = 1.0 - transmittance;
    if first < 0.0 || opacity <= 0.0 {
        textureStore(picture, id.xy, vec4<f32>(0.0));
        return;
    }
    // The air between the eye and the cloud.
    let haze = 1.0 - exp(-first / HAZE_M);
    let seen = mix(light, air_seen(direction) * opacity, haze);
    textureStore(picture, id.xy, vec4<f32>(seen, opacity));
}
