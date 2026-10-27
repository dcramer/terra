// The volumetric clouds (docs/sky.md stage 2, D515), marched at half resolution after the depth
// prepass (sky/volume/render.rs) into a picture the veil lays over the scene (cloud_volume.wesl):
// rgb the light the clouds send the eye, exposed and premultiplied, a how much they hide.
//
// Ported in outline from Horizon's Nubis as the bevy branch `mate-h/bevy`
// `volumetric-clouds-nubis` (MIT/Apache-2.0, crates/bevy_pbr/src/atmosphere/clouds*.wesl) has it:
// Schneider, "The real-time volumetric cloudscapes of Horizon Zero Dawn" (SIGGRAPH 2015) and
// "Nubis: authoring real-time volumetric cloudscapes" (2017) for the density, its remaps, the
// in-scattering probability and the coarse and fine march; Hillaire, "Physically based sky,
// atmosphere and cloud rendering in Frostbite" (2016) for the energy-conserving step and the
// aerial perspective a step; Jendersie and d'Eon, "An approximate Mie scattering function for fog
// and cloud rendering" (SIGGRAPH 2023 talks) for the phase. Cut to stage 2's MVP: cumulus,
// stratocumulus and the overcast deck by the weather map's kind; no shadows on the ground, night,
// cirrus or storms.
//
// The layers are shells round the air's sea under the eye (docs/sky.md#heights, D334, D544): a
// sphere of the air's radius in the world's metres, so a layer curves away to the air's horizon
// as on Earth rather than standing as a flat slab over the start. A point is its height over that
// sea, worked out along the ray rather than from a length of millions of metres, and its place on
// the weather map, the eye's level plane through `to_map` as the 2D layer takes it. The columns
// of cloud stand where the sky's 2D layer draws them (the same noise and offsets,
// cloud_noise.wesl), so the low preset's sky and this one agree.

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
    // Tunables (D547, sky/volume.rs): x the most a heaped column covers (Nubis's coverage
    // remap, so the 3D noise cuts it into heaps), y where the body's remapped noise starts,
    // so it spans 0 to 1 (`shape`).
    tuned: vec4<f32>,
    // The kinds' layers over the sea (clouds::layers_over_sea), x the deck's, y stratocumulus's,
    // z cumulus's: base, base by rain, top, top by cover, top by rain.
    layers: array<vec4<f32>, 5>,
    // The planet's sea radius and the air's in the world's metres, the slab's bottom and top.
    shell: vec4<f32>,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var noise: texture_3d<f32>;
@group(0) @binding(4) var map_now: texture_2d<f32>;
@group(0) @binding(5) var map_next: texture_2d<f32>;
@group(0) @binding(6) var air: texture_cube<f32>;
@group(0) @binding(7) var repeating: sampler;
// The weather map's, mirrored past its edges (the air's cube ignores it).
@group(0) @binding(8) var mirrored: sampler;
@group(0) @binding(9) var picture: texture_storage_2d<rgba16float, write>;

// The 3D noise's tiles, metres (clouds::SHAPE_TILE_M, DETAIL_TILE_M), and its side in texels.
const SHAPE_TILE_M: f32 = 6000.0;
const DETAIL_TILE_M: f32 = 750.0;
const NOISE_SIDE: f32 = 64.0;
// How far a ray is marched; past it the layer carries on to the air's horizon drawn flat, as the
// 2D layer is, from the same map (`far_layer`).
const MARCH_M: f32 = 32000.0;
// The march's budget of samples, and how many empty fine ones return it to coarse steps.
const STEPS: u32 = 96u;
const EMPTY_TO_COARSE: u32 = 10u;
// Extinction a metre at density 1: cumulus (liquid water 0.4 g/m3, droplets of 10 um, so some
// 50 m of sight inside), and the thinner stratus deck.
const CUMULUS_EXTINCTION: f32 = 0.06;
const DECK_EXTINCTION: f32 = 0.03;
// The distance over which the air's haze takes the clouds (as the 2D layer fades by lift).
const HAZE_M: f32 = 20000.0;
// The droplets' phase's mean cosine (the fit's below, as the bevy branch's `CLOUD_MIE_MEAN_COS`):
// light many times scattered goes on as if scattered evenly over an optical depth reduced by
// 1 - it (similarity scaling: Joseph, Wiscombe and Weinman 1976).
const MEAN_COS: f32 = 0.8793;
// The diffusion's extrapolation length (the branch's `CLOUD_DIFFUSION_Z0`), for the sky's and
// the ground's light inside a layer.
const DIFFUSION_Z0: f32 = 0.6667;
// How much of a column's cloud-filled depth its body fills on average: the share of the
// coverage the 3D noise leaves, so the depth of cloud above and below a sample (`scattered`) is
// that of the column it stands in.
const COLUMN_FILL: f32 = 0.3;
// The ground's albedo, for the light it sends up into a cloud's base.
const GROUND_ALBEDO: f32 = 0.15;
const PI: f32 = 3.14159265;

// The eye, its up and its place on the map, set once a ray.
var<private> eye_up: vec3<f32>;
var<private> eye_height: f32;
var<private> eye_map: vec2<f32>;

fn remap(v: f32, lo: f32, hi: f32) -> f32 {
    return clamp((v - lo) / max(hi - lo, 1.0e-4), 0.0, 1.0);
}

// The air's light from a direction in the world, exposed (sky.wesl's `air_seen`; the cube is
// left-handed, as Bevy's environment maps are).
fn air_seen(direction: vec3<f32>) -> vec3<f32> {
    let d = vec3<f32>(direction.x, direction.y, -direction.z);
    return textureSampleLevel(air, mirrored, d, 0.0).rgb * view.exposure;
}

// The height over the air's sea of a point `s` along a direction from one `h` over it, the
// direction's cosine to the up there `mu`: r² − R² over r + R, which keeps its metres where the
// length of millions of metres would not.
fn height_along(h: f32, mu: f32, s: f32) -> f32 {
    let r0 = params.shell.y + h;
    let rr = h * (2.0 * params.shell.y + h) + 2.0 * r0 * mu * s + s * s;
    return rr / (sqrt(max(r0 * r0 + 2.0 * r0 * mu * s + s * s, 0.0)) + params.shell.y);
}

// Where a ray from the eye crosses the shell `over` metres over the air's sea: its near and far
// distances, or x > y for none. The roots q and c/q, which cancel nothing (Numerical Recipes 5.6).
fn shell_hits(mu: f32, over: f32) -> vec2<f32> {
    let big = params.shell.y;
    let b = (big + eye_height) * mu;
    let c = (eye_height - over) * (2.0 * big + eye_height + over);
    let disc = b * b - c;
    if disc < 0.0 {
        return vec2<f32>(1.0, -1.0);
    }
    let s = sqrt(disc);
    let q = -(b + select(-s, s, b >= 0.0));
    if abs(q) < 1.0e-6 {
        return vec2<f32>(0.0, 0.0);
    }
    let a = q;
    let z = c / q;
    return vec2<f32>(min(a, z), max(a, z));
}

// A level direction in the map's frame, east and south, scaled by how far.
fn map_way(v: vec3<f32>) -> vec2<f32> {
    return (params.to_map * vec4<f32>(v - eye_up * dot(v, eye_up), 0.0)).xz;
}

// The weather over a place: cover, what falls, the kind (1 heaped, 0.5 stratocumulus, 0 a deck)
// and the snow, blended from this step's map to the next's (sky.wesl's `weather_at`).
fn weather_at(metres: vec2<f32>) -> vec4<f32> {
    if params.map.x == 0.0 {
        return vec4<f32>(params.fallback.x, params.fallback.y, params.fallback.z, 0.0);
    }
    let uv = metres * params.map.z + 0.5;
    let now = textureSampleLevel(map_now, mirrored, uv, 0.0);
    let next = textureSampleLevel(map_next, mirrored, uv, 0.0);
    return mix(now, next, params.map.y);
}

// A column of the air: how much of it the cloud's shapes cover, and the layer they fill there.
struct Column {
    // The share of the sky the weather covers here, and how much of the column the cloud's shapes
    // fill.
    cover: f32,
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
    c.cover = cover;
    c.kind = w.b;
    // Heaped cloud leaves the noise more to cut, so it breaks into heaps; a deck is whole.
    c.coverage = remap(shapes, threshold - 0.12, threshold + 0.1) * mix(1.0, params.tuned.x, c.kind);
    c.rain = w.g;
    // The layer by kind, from the kinds' table (clouds::LAYERS): deck, stratocumulus, cumulus.
    let base = params.layers[0].xyz + params.layers[1].xyz * c.rain;
    let top = params.layers[2].xyz + params.layers[3].xyz * cover + params.layers[4].xyz * c.rain;
    let deck = vec2<f32>(base.x, top.x);
    let strato = vec2<f32>(base.y, top.y);
    var layer = mix(deck, strato, clamp(c.kind * 2.0, 0.0, 1.0));
    if c.kind > 0.5 {
        layer = mix(strato, vec2<f32>(base.z, top.z), clamp(c.kind * 2.0 - 1.0, 0.0, 1.0));
    }
    c.base = layer.x;
    c.thickness = layer.y - layer.x;
    return c;
}

// How far up its layer a height is, 0 at the base and 1 at the top.
fn lift_in(c: Column, h: f32) -> f32 {
    return (h - c.base) / c.thickness;
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

// The cloud's body without its eroded edges at a place on the map `m` and height `h`:
// Perlin-Worley remapped by its Worley fBm, cut by the column's coverage (Nubis 2015).
fn shape(c: Column, m: vec2<f32>, h: f32, lod: f32) -> f32 {
    let profiled = profile(c, lift_in(c, h));
    if profiled <= 0.0 || c.coverage <= 0.0 {
        return 0.0;
    }
    let uvw = vec3<f32>(m.x / SHAPE_TILE_M + params.volume.x, h / SHAPE_TILE_M, m.y / SHAPE_TILE_M + params.volume.y);
    let n = textureSampleLevel(noise, repeating, uvw, lod);
    let fbm = n.g * 0.625 + n.b * 0.25 + n.a * 0.125;
    // The remap leaves it between about 0.7 and 0.95; stretched, so a coverage cuts it
    // gradually rather than all or nothing.
    let body = remap(remap(n.r, fbm - 1.0, 1.0), params.tuned.y, 1.0);
    return remap(body * profiled, 1.0 - c.coverage, 1.0) * c.coverage;
}

// The edges eroded by the finer Worley noise: wispy low down, billowing higher up (Nubis 2015).
fn eroded(c: Column, m: vec2<f32>, h: f32, body: f32, lod: f32) -> f32 {
    let uvw = vec3<f32>(m.x / DETAIL_TILE_M + params.volume.z, h / DETAIL_TILE_M, m.y / DETAIL_TILE_M + params.volume.w);
    let d = textureSampleLevel(noise, repeating, uvw, lod);
    let fbm = d.g * 0.625 + d.b * 0.25 + d.a * 0.125;
    let lift = clamp(lift_in(c, h) * 4.0, 0.0, 1.0);
    let edge = mix(fbm, 1.0 - fbm, lift);
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

// Optical depth toward the sun from a point at `m`, `h`, the sun's cosine to the up there
// `mu`, its way on the map `way`, on the body without erosion: eight taps from 30 m growing by
// half again each (1.5 km), and one long tap over the next 1.5 km, so a heap's whole depth
// shades its base (the branch's march has twelve taps to 2.8 km and its shadow map beyond; six
// taps to 0.8 km left a 2 km heap's base lit as if through less than half of it).
fn toward_sun(c: Column, m: vec2<f32>, h: f32, mu: f32, way: vec2<f32>) -> f32 {
    var tau = 0.0;
    var along = 0.0;
    var step = 30.0;
    for (var k = 0u; k < 8u; k++) {
        let s = along + step * 0.5;
        tau += shape(c, m + way * s, height_along(h, mu, s), log2(step / (SHAPE_TILE_M / NOISE_SIDE)) + 1.0) * step;
        along += step;
        step *= 1.5;
    }
    let s = along + 750.0;
    tau += shape(c, m + way * s, height_along(h, mu, s), 3.0) * 1500.0;
    return tau * extinction(c);
}

// The second exponential integral E2(x), the share of light leaving a slab of optical depth x
// over a hemisphere: the bevy branch's rational fit (`exp_integral_e2`, within 0.64% on 0 to 40).
fn exp_integral_e2(x: f32) -> f32 {
    return exp(-x) * (x * x + 1.6706291 * x + 0.13822509)
        / (x * x * x + 3.6251676 * x * x + 2.2005648 * x + 0.13822509);
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

// The light a sample of cloud scatters toward the eye, a metre of density 1, before extinction.
// Ported from the bevy branch's march (clouds_raymarch.wesl, after Schneider's Nubis Cubed,
// "Direct = Transmittance * Phase + Multiple Scattering * Secondary Phase", 2023, with its terms
// derived and checked against a path tracer there):
// - the sun's light scattered once, through the cloud toward it, with the droplets' phase;
// - and many times: the sunlight left at the similarity-reduced depth toward the sun, times
//   f / (1 - f), the series of further bounces (Hillaire 2020), f the chance light scattered
//   here does not escape, from how deep it sits toward the sun and the eye and in its column.
//   Light escapes a thin heap's edge, so its edges facing away from the sun are darker than its
//   middle (the "powder" look), and little sunlight reaches a deep base;
// - the sky's light and the ground's (its sunlit part shadowed by the cover over it), as the
//   diffusion through the layer mixes them by the depth of cloud above and below.
// The former two-stream share, 1 / (1 + 3/4 (1 - g) tau), is a wide deck's: it lit every heap's
// base with a fifth of the sun however deep, so in our exposure the bases, the sides away from the
// sun and the shade between billows all sat high in the tonemapper's shoulder, one flat white.
struct Lights {
    sun: vec3<f32>,
    sky: vec3<f32>,
    // The ground's light from the sun when not under cloud, and from the sky.
    ground_sun: vec3<f32>,
    ground_sky: vec3<f32>,
    phase: f32,
}

fn scattered(l: Lights, c: Column, h: f32, tau_sun: f32, tau_view: f32) -> vec3<f32> {
    let k = 1.0 - MEAN_COS;
    // The depth of cloud above and below in the column the sample stands in, as a level layer.
    let column = extinction(c) * c.coverage * COLUMN_FILL * c.thickness;
    let lift = clamp(lift_in(c, h), 0.0, 1.0);
    let tau_up = column * (1.0 - lift);
    let tau_down = column * lift;
    // The chance light scattered here escapes: the column's (the branch's plane-parallel limit,
    // (E2 up + E2 down) / 2), or nearer an edge the local one toward the sun and the eye.
    let p_column = 0.5 * (exp_integral_e2(k * tau_up) + exp_integral_e2(k * tau_down));
    let p_local = 0.5 * (exp(-k * tau_sun) + exp(-k * tau_view));
    let f = 1.0 - max(min(p_column, p_local), 6.1035e-5);
    let multiple = exp(-k * tau_sun) * f / (1.0 - f) * (0.25 / PI);
    let sunward = l.sun * (exp(-tau_sun) * l.phase + multiple);
    let ground = l.ground_sun * (1.0 - c.cover) + l.ground_sky;
    let w = (k * tau_up + DIFFUSION_Z0) / (k * (tau_up + tau_down) + 2.0 * DIFFUSION_Z0);
    return sunward + mix(l.sky, ground, w);
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
    // The ray's way from the view's own space, turned into the world's: a point on the near plane
    // in the world less the eye is the difference of two numbers some 30 km from the planet's
    // middle, which f32 holds to 2 mm, a 2% error over the 10 cm to the near plane, and the
    // picture fell into blocks a degree across.
    let near = view.view_from_clip * vec4<f32>(ndc, 1.0, 1.0);
    let direction = normalize((view.world_from_view * vec4<f32>(near.xyz / near.w, 0.0)).xyz);

    // The scene's nearest depth under this half-resolution pixel (reversed: 0 is infinitely far).
    let full = textureDimensions(depth);
    let corner = min(id.xy * 2u, full - 1u);
    var nearest = 0.0;
    for (var k = 0u; k < 4u; k++) {
        let at = min(corner + vec2<u32>(k & 1u, k >> 1u), full - 1u);
        nearest = max(nearest, textureLoad(depth, at, 0));
    }
    var scene = 1.0e9;
    if nearest > 0.0 {
        let hit = view.view_from_clip * vec4<f32>(ndc, nearest, 1.0);
        scene = length(hit.xyz / hit.w);
    }

    // The eye over the air's sea, which lies under it at the planet's sea.
    eye_up = normalize(eye);
    eye_height = length(eye) - params.shell.x;
    eye_map = (params.to_map * vec4<f32>(eye, 1.0)).xz;
    let mu = dot(direction, eye_up);
    let way = map_way(direction);

    // Where the ray is inside the shells: from below it leaves the bottom's sphere and then the
    // top's; inside, it runs to the bottom if it falls to it, else out of the top; from above,
    // it enters the top and runs to the bottom or out of the top again.
    let bottom = shell_hits(mu, params.shell.z);
    let top = shell_hits(mu, params.shell.w);
    var t0 = 0.0;
    var t1 = 0.0;
    if eye_height < params.shell.z {
        t0 = bottom.y;
        t1 = top.y;
    } else if eye_height <= params.shell.w {
        t1 = select(top.y, bottom.x, bottom.x <= bottom.y && bottom.x > 0.0);
    } else if top.x <= top.y && top.x > 0.0 {
        t0 = top.x;
        t1 = select(top.y, bottom.x, bottom.x <= bottom.y && bottom.x > 0.0);
    }
    // A ray falling to the air's sea meets its ground there, under the horizon.
    let sea = shell_hits(mu, 0.0);
    if eye_height > 0.0 && sea.x <= sea.y && sea.x > 0.0 {
        t1 = min(t1, sea.x);
    }
    t1 = min(t1, scene);
    let reach = t1;
    t1 = min(t1, MARCH_M);
    if t1 <= t0 && reach <= t0 {
        textureStore(picture, id.xy, vec4<f32>(0.0));
        return;
    }

    // The light: the sun's as it reaches the ground (`daylight`: already through the air, warm
    // at dusk), the sky's from above and the ground's from below, each exposed.
    let sun = normalize(params.sun.xyz);
    let height = dot(sun, eye_up);
    let sunlight = params.sunlight.rgb * smoothstep(-0.06, 0.02, height) * view.exposure;
    let level = normalize(sun - eye_up * height + vec3<f32>(1.0e-4, 0.0, 0.0));
    let sky = (air_seen(eye_up) * 2.0 + air_seen(normalize(eye_up + level)) + air_seen(normalize(eye_up - level))) * 0.25;
    var lights: Lights;
    lights.sun = sunlight;
    lights.sky = sky;
    lights.ground_sun = GROUND_ALBEDO * sunlight * max(height, 0.0) / PI;
    lights.ground_sky = GROUND_ALBEDO * sky;
    lights.phase = mie(dot(direction, sun));
    let sun_way = map_way(sun);

    var transmittance = 1.0;
    var light = vec3<f32>(0.0);
    // How much of what the clouds hide the air's haze between them and the eye takes instead.
    var hazed = 0.0;
    var fine = false;
    var empty = 0u;
    // The optical depth of cloud between the eye and the step.
    var seen_through = 0.0;
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
            let c = column(eye_map + way * t);
            if c.coverage * profile(c, lift_in(c, height_along(eye_height, mu, t))) > 0.0 {
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
        let s = t + dt * 0.5;
        let m = eye_map + way * s;
        let h = height_along(eye_height, mu, s);
        let c = column(m);
        let lod = log2(max(dt, 1.0) / (SHAPE_TILE_M / NOISE_SIDE));
        let body = shape(c, m, h, max(lod, 0.0));
        var density = 0.0;
        if body > 0.0 {
            density = eroded(c, m, h, body, max(log2(max(dt, 1.0) / (DETAIL_TILE_M / NOISE_SIDE)), 0.0));
        }
        if density > 0.0 {
            empty = 0u;
            let sigma = density * extinction(c);
            // The sun's cosine to the up where the sample is, as the shells curve.
            let up_here = normalize(eye_up * (params.shell.y + eye_height) + direction * s);
            let tau = toward_sun(c, m, h, dot(sun, up_here), sun_way);
            let lit = scattered(lights, c, h, tau, seen_through + 0.5 * sigma * dt) * sigma;
            seen_through += sigma * dt;
            let through = exp(-sigma * dt);
            let took = transmittance * (1.0 - through);
            // The air between the eye and this sample (Hillaire 2016's aerial perspective a step).
            let clear = exp(-s / HAZE_M);
            light += transmittance * (lit - lit * through) / max(sigma, 1.0e-6) * clear;
            hazed += took * (1.0 - clear);
            transmittance *= through;
        } else {
            empty += 1u;
            if empty >= EMPTY_TO_COARSE {
                fine = false;
            }
        }
        t += dt;
    }

    // Past the march, the layer on to the air's horizon, flat at its kind's middle height as the
    // 2D layer is drawn, from the same map and shapes, so the field never ends in a ring.
    // A ray still inside the layer where the march ends sees it edge-on for tens of kilometres:
    // as much of it as the weather there covers, drawn whole rather than as the columns at 32 km,
    // which stood as walls round the eye.
    if reach > MARCH_M && transmittance >= 0.01 {
        let c = column(eye_map + way * MARCH_M);
        let h = c.base + c.thickness * 0.35;
        let crossing = shell_hits(mu, h);
        let at = select(crossing.y, crossing.x, eye_height > h && crossing.x > 0.0);
        let lift = lift_in(c, height_along(eye_height, mu, MARCH_M));
        var opacity = 0.0;
        var far = c;
        var far_at = MARCH_M;
        if lift > 0.0 && lift < 1.0 {
            let w = weather_at(eye_map + way * MARCH_M);
            opacity = smoothstep(0.0, 0.2, max(params.map.w, w.r)) * 0.9;
        } else if crossing.x <= crossing.y && at >= MARCH_M && at < reach {
            far = column(eye_map + way * at);
            far_at = at;
            opacity = smoothstep(0.0, 0.35, far.coverage) * 0.9;
        }
        if opacity > 0.0 {
            let at = far_at;
            // Lit as the march lights a cloud's middle: a little way in from the sunlit side.
            let lit = scattered(lights, far, h, 8.0, 2.0);
            let clear = exp(-at / HAZE_M);
            light += transmittance * opacity * lit * clear;
            hazed += transmittance * opacity * (1.0 - clear);
            transmittance *= 1.0 - opacity;
        }
    }

    let opacity = 1.0 - transmittance;
    if opacity <= 0.0 {
        textureStore(picture, id.xy, vec4<f32>(0.0));
        return;
    }
    let seen = light + air_seen(direction) * hazed;
    textureStore(picture, id.xy, vec4<f32>(seen, opacity));
}
