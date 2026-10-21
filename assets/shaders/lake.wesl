// A lake's water (D322): a medium by its depth, mirroring what stands round it. Save this file
// runs and the lakes update.
//
// Ported from Zach Bernal's boujie_water_shader for Godot 4 (MIT, below): the depth of what
// lies under the water along the view, read from the depth prepass
// (`depth_from_water_to_object`); that depth through Beer's law, offset and raised to a power,
// blending the water from its shallow colour to its deep one, and faded to deep with distance,
// where the depth buffer is imprecise (`DEPTH_FOG`, `DEPTH_FOG_FADE`); and foam where the
// water is shallowest against what stands in it (`SHORE_FOAM`). Where Godot's version reads the
// screen behind the water, ours blends over it, so shallow water shows the bed through it and
// deep water is its own colour. The waves are the sea's too (waves.wesl).
//
// MIT License. Copyright © 2023-present Zach Bernal and contributors. Permission is hereby
// granted, free of charge, to any person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following conditions: The above copyright notice and
// this permission notice shall be included in all copies or substantial portions of the
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS
// BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT
// OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

import super::dusk::dusk_sky;
import super::waves::{Surface, TRAINS, SWELL, noise, cats_paws, wave_slopes, wave_bend, wave_train, glint};
import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::{globals, view, lights},
    view_transformations::depth_ndc_to_view_z,
};
@if(DEPTH_PREPASS)
import bevy_pbr::prepass::utils::prepass_depth;
// The opaque scene as drawn before the water, which Bevy copies for transmissive materials:
// what the water's reflection of what is on screen reads (`on_screen`).
@if(VIEW_TRANSMISSION_TEXTURE)
import bevy_pbr::render::mesh_view_bindings::{view_transmission_texture, view_transmission_sampler};

// Field order must match LakeExtension in lakes.rs.
struct Lake {
    // The sky at the horizon and overhead, which the water reflects, and how bright.
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    sky_brightness: f32,
    // Beer's law's strength, a metre, and how far the depth is offset before it.
    beers_law: f32,
    depth_offset: f32,
    // The water's colour where shallow and deep, alpha how much of what is behind it it hides.
    shallow: vec4<f32>,
    deep: vec4<f32>,
    // Shallow water's foam against the shore, from this depth to that, in metres.
    foam_from: f32,
    foam_to: f32,
    // From this distance to that, the water is faded to its deep colour, in metres.
    fade_from: f32,
    fade_to: f32,
    // Which way the wind blows, level, and how hard, 0 still to 1 a stiff breeze.
    wind: vec4<f32>,
    // A point near the lakes the noise is laid out from: its hash reads small numbers there,
    // where tens of kilometres from the planet's centre it would read its own rounding.
    origin: vec4<f32>,
    // East and north at the skyline's middle, and the skyline round it, a way out each, the
    // first due east turning north: how high the ground stands seen from the water (radians),
    // the highest ground's height over the water, and the tree line's and snow line's
    // (terra_land::round::Skyline).
    east: vec4<f32>,
    north: vec4<f32>,
    skyline: array<vec4<f32>, 64>,
    // The waves' trains (waves.rs `trains`): each one's wavenumber (radians a metre), angular
    // frequency (radians a second), the way it runs and its phase (radians), longest first.
    trains: array<vec4<f32>, TRAINS>,
    // The rings spreading where fish rise (rises.rs): each where it is from `origin`, and
    // when it began by the clock, none when 0.
    rises: array<vec4<f32>, RISES>,
    // The wakes behind what swims (wakes.rs): each swimmer where it is from `origin` and its
    // speed, none when 0; and the way it heads, level, and how wide it is.
    wakes_at: array<vec4<f32>, WAKES>,
    wakes_way: array<vec4<f32>, WAKES>,
    // Where each was a wake's length back from `origin`, w 1, or 0 for none: its V bends
    // through it.
    wakes_was: array<vec4<f32>, WAKES>,
    // 1 when the air's picture is bound (air.rs): the water mirrors the sky the air draws.
    over_air: u32,
    // How much of the sky's own dusk colours are drawn over the air, and the glow round the
    // sun (sky.wesl): the water mirrors them as the sky shows them.
    twilight: f32,
    glow: vec4<f32>,
    // Toward the sun, below the horizon too, which the dusk's warm side faces; zero for the
    // world's light's own way.
    sun: vec4<f32>,
    // 1 while the water is drawn as its mask for the looks check (lakes.rs `WaterMask`).
    mask: f32,
}

const RISES: u32 = 8u;
// The water's mask colour, bright enough to stay magenta through the haze and tonemapping.
const MASK: vec3<f32> = vec3<f32>(4.0, 0.0, 4.0);
const WAKES: u32 = 8u;
// The tangent of Kelvin's half angle, 19.47 degrees, at which a wake's arms open (wakes.rs).
const KELVIN: f32 = 0.3536;
// How long a ring spreads before it is gone, seconds (rises.rs RING_S).
const RING_S: f32 = 6.0;

// The slopes of the rings where fish rise (rises.rs): each a ring spreading from where a
// fish broke the surface, a hump of water at its front with fine crests behind it, smoothing
// and fading as it widens, as a rise's rings do. The crests too fine for the pixel are left
// out, the hump kept, so a far ring still shows as a band of the mirror bent.
fn rise_slopes(local: vec3<f32>, east: vec3<f32>, north: vec3<f32>, t: f32, pixel: f32) -> vec2<f32> {
    var slope = vec2<f32>(0.0);
    for (var i = 0u; i < RISES; i += 1u) {
        let ring = lake.rises[i];
        let age = t - ring.w;
        if ring.w <= 0.0 || age < 0.0 || age > RING_S {
            continue;
        }
        let off = local - ring.xyz;
        let flat = vec2<f32>(dot(off, east), dot(off, north));
        let d = length(flat);
        if d < 1e-3 {
            continue;
        }
        // Its front spreads fast at first and slows; behind it the water settles.
        let front = 0.08 + 0.42 * age - 0.025 * age * age;
        let x = d - front;
        let wide = 0.05 + 0.035 * age;
        let hump = exp(-x * x / (2.0 * wide * wide));
        let fading = exp(-0.55 * age) * (1.0 - smoothstep(RING_S * 0.7, RING_S, age));
        // The hump's slope, and the crests trailing it, 9 cm apart.
        let wavelength = 0.09;
        let fine = 1.0 - smoothstep(wavelength * 0.25, wavelength * 0.6, pixel);
        let crests = sin((x / wavelength) * 6.283) * exp(-max(-x, 0.0) / (3.0 * wavelength))
            * step(x, wide) * fine;
        let magnitude = 0.6 * fading * (-x / wide * hump + 0.6 * crests * hump);
        slope += flat / d * magnitude;
    }
    return slope;
}

// The slopes of the wakes behind what swims (wakes.rs): two arms opening behind each swimmer
// at Kelvin's angle, a ridge of water along each with fine crests trailing inside the V, a
// bow wave round its front, all fading over a few body lengths and stronger the faster it
// goes. The V's middle runs back along the swimmer's way, bent through where it was a wake's
// length back (a parabola off its heading), so a turning duck's wake curves as its trail
// does. The crests too fine for the pixel are left out, the ridges kept.
fn wake_slopes(local: vec3<f32>, east: vec3<f32>, north: vec3<f32>, pixel: f32) -> vec2<f32> {
    var slope = vec2<f32>(0.0);
    for (var i = 0u; i < WAKES; i += 1u) {
        let at = lake.wakes_at[i];
        if at.w <= 0.0 {
            continue;
        }
        let off = local - at.xyz;
        let flat = vec2<f32>(dot(off, east), dot(off, north));
        let heading = lake.wakes_way[i].xyz;
        var ahead = vec2<f32>(dot(heading, east), dot(heading, north));
        if length(ahead) < 1e-3 {
            continue;
        }
        ahead = normalize(ahead);
        var side = vec2<f32>(-ahead.y, ahead.x);
        let wide = max(lake.wakes_way[i].w, 0.05);
        // How far behind the swimmer along its heading.
        let behind = -dot(flat, ahead);
        let reach = wide * 18.0 * clamp(at.w / 0.4, 0.5, 2.0);
        if behind < -wide * 2.0 || behind > reach {
            continue;
        }
        // The bend of its way: the trail's offset to the side grows as the square of the
        // distance behind, through where it was, its tilt held under about 60 degrees.
        var bend = 0.0;
        let was = lake.wakes_was[i];
        if was.w > 0.0 {
            let back = was.xyz - at.xyz;
            let back_flat = vec2<f32>(dot(back, east), dot(back, north));
            let b = -dot(back_flat, ahead);
            if b > wide {
                let most = 0.85 / reach;
                bend = clamp(dot(back_flat, side) / (b * b), -most, most);
            }
        }
        let bb = max(behind, 0.0);
        // How far to one side of the trail, and the way the trail went there.
        let across = dot(flat, side) - bend * bb * bb;
        ahead = normalize(ahead - side * (2.0 * bend * bb));
        side = vec2<f32>(-ahead.y, ahead.x);
        let strength = clamp(at.w / 0.5, 0.0, 1.5) * (1.0 - smoothstep(reach * 0.4, reach, behind));
        // Each arm's ridge, across its line, widening as it trails.
        let x = abs(across) - (max(behind, 0.0) * KELVIN + wide * 0.5);
        let ridge = 0.03 + 0.04 * max(behind, 0.0);
        let hump = exp(-x * x / (2.0 * ridge * ridge));
        let wavelength = 0.08 + 0.05 * at.w;
        let fine = 1.0 - smoothstep(wavelength * 0.25, wavelength * 0.6, pixel);
        // Inside the V alone: its exponent is held at 0 outside it, where a few metres off the
        // arm it overflowed to infinity and the step's 0 times it made NaN, which drew the
        // lake's far water as dark straight-edged wedges the antialiasing smeared (lake wedge).
        let crests = sin(behind / wavelength * 6.283) * step(x, 0.0) * exp(min(x, 0.0) / (2.0 * wavelength)) * fine;
        let normal_out = side * sign(across) + ahead * KELVIN;
        slope += normal_out * (0.5 * strength * (-x / ridge * hump)) + ahead * (0.25 * strength * crests);
        // The bow wave round its breast.
        let d = length(flat);
        let bow = exp(-(d - wide * 0.6) * (d - wide * 0.6) / (2.0 * 0.03 * 0.03)) * step(0.0, -behind + wide * 0.3);
        if d > 1e-3 {
            slope += flat / d * (0.3 * strength * bow);
        }
    }
    return slope;
}

// The air all round the camera, as Bevy's atmosphere works it out each frame, unexposed: what
// the sky over the water is drawn from, so the water mirrors the same sky.
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var air_texture: texture_cube<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var air_sampler: sampler;

// The air's light from a direction, exposed as the sky is. The cube is left-handed, as
// Bevy's environment maps are.
fn air_seen(direction: vec3<f32>) -> vec3<f32> {
    let d = vec3<f32>(direction.x, direction.y, -direction.z);
    return textureSampleLevel(air_texture, air_sampler, d, 0.0).rgb * view.exposure;
}

// What the water mirrors along a reflected ray: the slopes round the lake where the ray runs
// below their skyline, their woods, rock and snow by how high up them it meets them, lit as the
// horizon is and a little hazed; the sky above.
fn mirrored(ray: vec3<f32>, up: vec3<f32>, sky: vec3<f32>, horizon: vec3<f32>) -> vec3<f32> {
    let level = ray - up * dot(ray, up);
    let around = atan2(dot(level, lake.north.xyz), dot(level, lake.east.xyz));
    let t = fract(around / 6.2831853) * 64.0;
    let i = u32(floor(t)) % 64u;
    let j = (i + 1u) % 64u;
    let s = mix(lake.skyline[i], lake.skyline[j], fract(t));
    let rises = asin(clamp(dot(ray, up), -1.0, 1.0));
    // Over the skyline the sky, through a soft edge: the skyline is taken a way at a time, and
    // a hard one draws a gap between hills as a square of sky in the water.
    let over = smoothstep(s.x - 0.012, s.x + 0.012, rises);
    if over >= 1.0 {
        return sky;
    }
    // How far up the slope the ray meets it, as a share of the skyline's height: low rays meet
    // the shore's woods, high ones the peaks.
    let height = clamp(rises / s.x, 0.0, 1.0) * s.y;
    let woods = 1.0 - smoothstep(s.z - 60.0, s.z + 60.0, height);
    let snow = smoothstep(s.w - 40.0, s.w + 60.0, height);
    let rock = horizon * vec3<f32>(0.32, 0.31, 0.3);
    let forest = horizon * vec3<f32>(0.035, 0.06, 0.035);
    let white = horizon * 0.9;
    var ground = mix(rock, forest, woods);
    ground = mix(ground, white, snow);
    // The slopes are hazed toward the horizon's colour a little, the peaks, farther, the more,
    // as little as the land itself is over the lake's few kilometres: hazed as much as the sky
    // is bright, the woods' reflection read as the sky's.
    let hazed = mix(ground, horizon, 0.03 + 0.1 * clamp(rises / max(s.x, 0.01), 0.0, 1.0));
    return mix(hazed, sky, over);
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> lake: Lake;

// The dusk's own colours the sky draws over the air along a direction where no cloud is
// (sky.wesl, after sunset, by the same function): mirrored from its own gradient, the water
// was salmon under a sky that had lost it (lake film r10, sat 0.45 under 0.10).
fn dusk(direction: vec3<f32>, up: vec3<f32>, sun: vec3<f32>) -> vec3<f32> {
    if lake.twilight <= 0.0 {
        return vec3<f32>(0.0);
    }
    return dusk_sky(direction, up, sun, lake.horizon.rgb, lake.zenith.rgb, lake.glow.rgb)
        * lake.sky_brightness * lake.twilight;
}

// The reflection of what is on screen along a ray from a point of the water, and how much it
// counts: nothing where the ray finds nothing before it leaves the screen, fading out toward
// the screen's edges, so the skyline's picture takes over smoothly. A linear march through the
// depth prepass in steps growing with distance, narrowed by halving once it passes behind
// what is drawn, and kept only where it passed less than a step behind it (as the screen-space
// reflections of Godot and Unreal and Bevy's own raymarch.hlsl port march, by McGuire and
// Mara's "Efficient GPU Screen-Space Ray Tracing", 2014, in its simplest form).
fn on_screen(world: vec3<f32>, ray: vec3<f32>, pixel: vec2<f32>, distance: f32) -> vec4<f32> {
    var found = vec4<f32>(0.0);
@if(DEPTH_PREPASS && VIEW_TRANSMISSION_TEXTURE)
    {
        let size = view.viewport.zw;
        // Each pixel's march starts a little further along, frame by frame, which the
        // temporal antialiasing resolves into the smooth reflection more steps would give
        // (interleaved gradient noise, Jimenez 2014).
        let frame = f32(globals.frame_count % 64u);
        let jitter = fract(52.9829189 * fract(dot(pixel + frame * 5.588238, vec2<f32>(0.06711056, 0.00583715))));
        var step = SSR_FIRST_M + distance * SSR_FIRST_SHARE;
        var t = step * jitter;
        var before = 0.0;
        var last = vec2<f32>(-1.0);
        for (var i = 0u; i < SSR_STEPS; i += 1u) {
            t += step;
            let seen = where_seen(world + ray * t, size);
            if seen.w < 0.0 {
                // Out over the screen's top still in front of what is drawn at its edge: the
                // ray goes on up into what stands there, the shore's trees overhead, out of
                // the picture. Their colour at the edge stands for them, as screen-space
                // reflections stretch the edge, rather than the open sky's, which drew a
                // pale milky band along every shore under its forest. Where the edge is sky
                // the ray finds sky.
                if last.x >= 0.0 && last.y < 0.08 {
                    let drawn = prepass_depth(vec4<f32>(last * size, 0.0, 1.0), 0u);
                    if drawn > 0.0 {
                        let colour = textureSampleLevel(view_transmission_texture, view_transmission_sampler, last, 0.0).rgb;
                        found = vec4<f32>(colour, 0.85 * smoothstep(0.0, 0.05, min(last.x, 1.0 - last.x)));
                    }
                }
                break;
            }
            last = seen.xy;
            let behind = seen.z;
            if behind > 0.0 {
                // Passed behind what is drawn there: halve back to where it meets it.
                var near = before;
                var far = t;
                for (var k = 0u; k < 5u; k += 1u) {
                    let middle = 0.5 * (near + far);
                    if where_seen(world + ray * middle, size).z > 0.0 {
                        far = middle;
                    } else {
                        near = middle;
                    }
                }
                let hit = where_seen(world + ray * far, size);
                if hit.w >= 0.0 && hit.z < max(step, 1.0) * 1.5 {
                    let colour = textureSampleLevel(view_transmission_texture, view_transmission_sampler, hit.xy, 0.0).rgb;
                    // Faded toward the screen's sides and foot, where what the ray meets
                    // changes as the view turns; not toward its top, past which it goes on.
                    let edge = min(min(hit.x, 1.0 - hit.x), 1.0 - hit.y);
                    found = vec4<f32>(colour, smoothstep(0.0, 0.05, edge));
                }
                break;
            }
            before = t;
            step *= SSR_GROWTH;
        }
    }
    return found;
}

// Where a point is on screen and how far it lies behind what is drawn there, in metres
// along the view (negative in front of it): x and y the screen's 0 to 1, w -1 off screen.
fn where_seen(point: vec3<f32>, size: vec2<f32>) -> vec4<f32> {
    let clip = view.clip_from_world * vec4<f32>(point, 1.0);
    if clip.w <= 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    let ndc = clip.xyz / clip.w;
    if abs(ndc.x) >= 1.0 || abs(ndc.y) >= 1.0 {
        return vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    var behind = -1.0e9;
@if(DEPTH_PREPASS)
    {
        let drawn = prepass_depth(vec4<f32>(uv * size, 0.0, 1.0), 0u);
        // Nothing drawn there is the sky, which nothing passes behind.
        if drawn > 0.0 {
            behind = depth_ndc_to_view_z(drawn) - depth_ndc_to_view_z(ndc.z);
        }
    }
    return vec4<f32>(uv, behind, 1.0);
}

// The reflection's march: its first step, so long and a share of the water's distance more;
// each next step longer by this much; and how many, together reaching past 10 km from water
// near the eye: at 3 km, where the peaks across a lake stand, the first steps' 1.22 had
// spent its 32 and the water mirrored the forest under them but not them.
const SSR_FIRST_M: f32 = 0.6;
const SSR_FIRST_SHARE: f32 = 0.01;
const SSR_GROWTH: f32 = 1.25;
const SSR_STEPS: u32 = 36u;
// How much of the waves' slope bends the ray the shore's reflection is marched along
// (`on_screen`), and how much a thin water film darkens the ground under it.
const REFLECTION_BUMP: f32 = 0.3;
// Off (0): at 0.4 the film drew the shore's straight submerged strips as hard dark bands that
// popped as the eye moved (lake film r14, popin.ts at 15.3 s), until the ground under the
// shallows is fixed.
const WET_FILM: f32 = 0.0;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let world = in.world_position.xyz;
    let up = normalize(world);
    let to_eye = view.world_position - world;
    let distance = length(to_eye);
    let toward_eye = to_eye / distance;

    // Where the pixel is on the water, metres east and north of a point of it (lakes.rs
    // `on_water`), and how much water the pixel covers: taken first, as derivatives must be.
    var at = vec2<f32>(dot(world - lake.origin.xyz, lake.east.xyz), dot(world - lake.origin.xyz, lake.north.xyz));
@if(VERTEX_UVS_A)
    {
        at = in.uv;
    }
    let pixel = max(max(length(dpdx(at)), length(dpdy(at))), 1e-4);

    // East and north here, and downwind among them.
    let east = normalize(lake.east.xyz - up * dot(lake.east.xyz, up));
    let north = cross(up, east);
    var downwind = vec2<f32>(dot(lake.wind.xyz, east), dot(lake.wind.xyz, north));
    if length(downwind) < 1e-3 {
        downwind = vec2<f32>(0.6, 0.8);
    }
    downwind = normalize(downwind);
    let local = world - lake.origin.xyz;
    let t = globals.time;
    let ruffled = cats_paws(local, lake.wind, up, t);
    let slopes = wave_slopes(lake.wind.w, ruffled);
    let spread = smoothstep(0.0, 0.15, lake.wind.w);
    var face = Surface(vec2<f32>(0.0), 0.0, 0.0);
    let bent = wave_bend(at, local, up, t);
    for (var i = 0u; i < TRAINS; i += 1u) {
        wave_train(&face, lake.trains[i], i < SWELL, bent, pixel, downwind, spread, slopes, t);
    }
    face.slope += rise_slopes(local, east, north, t, pixel) + wake_slopes(local, east, north, pixel);
    let normal = normalize(up - east * face.slope.x - north * face.slope.y);
    pbr_input.world_normal = normal;
    pbr_input.N = normal;

    // How deep the water is between its surface and what lies under it along the view
    // (boujie_water_shader's depth_from_water_to_object). With no depth prepass, deep.
    var under = 1000.0;
@if(DEPTH_PREPASS)
    {
        let behind = depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        let surface = depth_ndc_to_view_z(in.position.z);
        under = max(surface - behind, 0.0);
    }
    // At its edge the water thins to nothing over the ground it rises onto: so much of it as
    // is under a few centimetres deep, up from the ground, is faded out, its line broken by
    // noise, as games fade a shoreline by the depth behind it. Without it a pool's water, a
    // flat sheet on its own cells, ended in their straight edges over the moss a hand under its
    // level, mirroring the sky there as brightly as over its middle.
    // The waterline rises and falls with the swell, so it laps at the shore, creeping up a
    // gentle beach and back: the waves' height, a few millimetres, made a little more of, as
    // the swash running up a shore is more than the wave.
    let rise = face.height * 2.5;
    var edge = 1.0;
@if(DEPTH_PREPASS)
    {
        let thick = under * max(dot(toward_eye, up), 0.02);
        let broken = noise(local * 0.9 + 31.0) * 0.6 + noise(local * 3.1 + 7.0) * 0.4;
        // Its reach grows with distance, as a pixel there spans metres of a nearly flat bank:
        // over a few centimetres the ground's own bumps of a centimetre or two stood out of
        // the water cell by cell, a checkerboard of mud and sky round every pool from high.
        let reach = 0.07 + distance * 0.0003;
        edge = smoothstep(0.01, reach, thick + rise + (broken - 0.5) * 0.05 * (reach / 0.07));
    }
    // Beer's law through that depth, sharpened as the original sharpens it, and faded to deep
    // with distance (DEPTH_FOG, DEPTH_FOG_FADE).
    let through = clamp(1.0 - exp(-(under + lake.depth_offset) * lake.beers_law), 0.0, 1.0);
    let deepness = clamp(pow(through, 2.5) + smoothstep(lake.fade_from, lake.fade_to, distance), 0.0, 1.0);
    var colour = mix(lake.shallow, lake.deep, deepness);

    // Looking down one sees into it; looking along it, what stands round it (Fresnel, as
    // Schlick has it, water's F0 0.02).
    let mirror = 0.02 + 0.98 * pow(1.0 - max(dot(normal, toward_eye), 0.0), 5.0);
    pbr_input.material.base_color = vec4<f32>(colour.rgb, mix(colour.a, 0.98, mirror));

    // Foam where the water is shallowest against the shore (SHORE_FOAM), broken by noise.
    let foam_noise = noise(local * 1.3 + vec3<f32>(globals.time * 0.15)) * 0.6 + noise(local * 3.7) * 0.4;
    let washed = under + rise / max(dot(toward_eye, up), 0.1);
    let foam = (1.0 - smoothstep(lake.foam_from, lake.foam_to, washed)) * smoothstep(0.35, 0.75, foam_noise)
        * (0.3 + 0.7 * lake.wind.w) * (1.0 - smoothstep(30.0, 80.0, distance));
    pbr_input.material.base_color = mix(pbr_input.material.base_color, vec4<f32>(0.85, 0.87, 0.84, 1.0), foam * 0.8);
    pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.8, foam);
    // The lighting lights the medium and the foam alone: what the water reflects is drawn
    // below, so the slopes it mirrors can be darker than the sky, as Bevy's own reflection of
    // the sky everywhere would not let them be.
    pbr_input.material.reflectance = vec3<f32>(0.0);
    // The material reads the screen behind it only for its reflection (`on_screen`), not to
    // let light through as Bevy's transmission would.
    pbr_input.material.specular_transmission = 0.0;

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    // Waves too short for the pixel tilt toward the eye the facets that reflect into it, and
    // those mirror higher, more of the sky: so a cat's-paw far off reads as a pale patch of the
    // sky's grey on the shore's dark reflection, as it does on a lake.
    let toward_level = normalize(toward_eye - up * dot(toward_eye, up) + up * 1e-4);
    let reflected = reflect(-toward_eye, normalize(normal + toward_level * sqrt(face.unseen) * 0.6));
    // The sky it mirrors: the air's own, exposed, where the air is drawn; else the painted
    // sky's two colours at its brightness.
    var seen: vec3<f32>;
    if lake.over_air != 0u {
        let level = normalize(reflected - up * dot(reflected, up) + up * 0.02);
        // Once the sun is down the world's light is the moon's: the dusk faces the sun.
        var toward_sun = lake.sun.xyz;
        if dot(toward_sun, toward_sun) < 0.5 {
            toward_sun = lights.directional_lights[0].direction_to_light;
        }
        seen = mirrored(reflected, up, air_seen(reflected) + dusk(reflected, up, toward_sun),
            air_seen(level) + dusk(level, up, toward_sun));
    } else {
        let sky = mix(lake.horizon.rgb, lake.zenith.rgb, pow(smoothstep(0.0, 0.62, dot(reflected, up)), 0.55));
        seen = mirrored(reflected, up, sky, lake.horizon.rgb) * lake.sky_brightness;
    }
    // What stands on screen along the reflected ray, where the ray finds it there: the shore's
    // trees, the far slopes and peaks as they are drawn; the skyline's picture of them where it
    // leaves the screen.
    // It is marched along a calmer ray than the sky's: the waves' slopes bent by
    // REFLECTION_BUMP, as Unreal's and CryEngine's water distort their reflection by a
    // scaled normal. A pixel of water mirrors many facets at once, and their mirror image is
    // the shore smeared up and down a little; one ray from the pixel's one facet, bent by its
    // full slope, swung by twice that slope, which off a far shore at a grazing angle of a
    // fraction of a degree sent every ray over the treeline into the sky or down into the
    // water itself: the far woods mirrored a band a fifth of their height, and the near water
    // broke into rows of dark and pale dashes (lake film r13).
    let calm = normalize(up + (normal - up) * REFLECTION_BUMP);
    let found = on_screen(world, reflect(-toward_eye, calm), in.position.xy, distance);
    seen = mix(seen, found.rgb, found.a);
    // The sun's glint off the ripples (waves.wesl `glint`).
    let sun = lights.directional_lights[0];
    let glinting = sun.color.rgb * view.exposure * glint(normal, toward_eye, sun.direction_to_light, face.unseen) * mirror;
    out.color = vec4<f32>(out.color.rgb + (seen * mirror + glinting) * (1.0 - foam), out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    // Where it thins to nothing over the ground it covers, that ground is wet, and darker for
    // it than the dry shore above the waterline: a film over it, darkening what is behind by
    // WET_FILM and adding nothing, as Unreal's and Frostbite's wetness darkens a surface's
    // albedo under a water film (wet soil and stone reflect about half to two thirds of their
    // dry light, Lekner and Dorf 1988). Faded out with the water alone, the shallows round every
    // shore rock read as a pale rim of dry ground (lake film r13).
    let covers = out.color.a * edge;
    let film = WET_FILM * (1.0 - edge) * (1.0 - smoothstep(40.0, 120.0, distance));
    let alpha = 1.0 - (1.0 - covers) * (1.0 - film);
    out.color = vec4<f32>(out.color.rgb * covers / max(alpha, 1e-4), alpha);
    // The looks check's mask (D429): the water flat magenta, so a shot shows what it covers.
    if lake.mask > 0.5 {
        out.color = vec4<f32>(MASK, alpha);
    }
    return out;
}
