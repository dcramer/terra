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
// deep water is its own colour. The waves are trains of plane waves summed, as Finch's
// "Effective Water Simulation from Physical Models" (GPU Gems, 2004) sums them, each running at
// its own speed by water's dispersion, as steep as Cox and Munk measured the sea's slopes by
// the wind (1954), and filtered by the pixel's footprint, what a pixel cannot hold widening
// the sun's glint and lifting the reflection, as Bruneton, Neyret and Holzschuch's ocean does
// ("Real-time Realistic Ocean Lighting using Seamless Transitions from Geometry to BRDF",
// 2010).
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
    // The waves' trains (lakes.rs `trains`): each one's wavenumber (radians a metre), angular
    // frequency (radians a second), turn from the wind and phase (radians), longest first.
    trains: array<vec4<f32>, TRAINS>,
    // 1 when the air's picture is bound (air.rs): the water mirrors the sky the air draws.
    over_air: u32,
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

// A lattice point's value, by Jarzynski and Olano's pcg3d ("Hash Functions for GPU
// Rendering", 2020) on its whole coordinates: exact at any distance from the lakes' origin,
// with none of a hash of float products' rounding or patterns.
fn hash(p: vec3<f32>) -> f32 {
    var v = bitcast<vec3<u32>>(vec3<i32>(p));
    v = v * 1664525u + 1013904223u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v = v ^ (v >> vec3<u32>(16u));
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    return f32(v.x >> 8u) / 16777216.0;
}

fn noise(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    // Perlin's quintic fade, smooth in its curvature too: with the cubic the ripples' normals
    // turn sharply at every cell's edge, and still water's reflection draws the lattice.
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let x00 = mix(hash(cell), hash(cell + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(hash(cell + vec3<f32>(0.0, 1.0, 0.0)), hash(cell + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(hash(cell + vec3<f32>(0.0, 0.0, 1.0)), hash(cell + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(hash(cell + vec3<f32>(0.0, 1.0, 1.0)), hash(cell + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// How many trains, the first SWELL of them the long ones (lakes.rs).
const TRAINS: u32 = 28u;
const SWELL: u32 = 11u;

// The water's surface at a place: its slope east and north, its height (metres), and the mean
// square slope of the waves too short for the pixel to show, which the light spreads instead.
struct Surface {
    slope: vec2<f32>,
    height: f32,
    unseen: f32,
}

// The waves at a place on the water (metres east and north), the pixel this many metres
// across, downwind a level direction there, and each set's mean square slope, the swell's
// and the wind's ripples', shared out evenly over its trains. A train shorter than four pixels
// fades out before it is under one and a half, where it would only shimmer, its slope counted
// as unseen roughness instead; the temporal antialiasing smooths what sparkle is left, as a
// sun's glitter should.
fn waves(at: vec2<f32>, pixel: f32, downwind: vec2<f32>, swell: f32, ripples: f32) -> Surface {
    let t = globals.time;
    var surface = Surface(vec2<f32>(0.0), 0.0, 0.0);
    // The place bent a little by a slow noise, half a metre over some metres, so no train's
    // crests run dead straight across the lake and none cross another's in a lattice, as
    // real wave fields wander.
    let bent = at + vec2<f32>(
        noise(vec3<f32>(at * 0.13, t * 0.05)) - 0.5,
        noise(vec3<f32>(at * 0.13 + 17.0, t * 0.05)) - 0.5,
    ) * 1.2;
    for (var i = 0u; i < TRAINS; i += 1u) {
        let train = lake.trains[i];
        let k = train.x;
        let turn = train.z;
        let way = vec2<f32>(
            downwind.x * cos(turn) - downwind.y * sin(turn),
            downwind.x * sin(turn) + downwind.y * cos(turn),
        );
        var share = ripples / f32(TRAINS - SWELL);
        if i < SWELL {
            share = swell / f32(SWELL);
        }
        // A train's mean square slope is half its steepness squared.
        let steepness = sqrt(2.0 * share);
        let phase = k * dot(way, bent) - train.y * t + train.w;
        let seen = clamp((6.2831853 / k / pixel - 1.5) * 0.4, 0.0, 1.0);
        surface.slope += way * (steepness * cos(phase) * seen);
        surface.height += steepness / k * sin(phase);
        surface.unseen += share * (1.0 - seen * seen);
    }
    return surface;
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
    // A light wind ruffles a lake in patches, cat's-paws a few tens of metres across racing
    // downwind as the gusts do and changing as they go, with glassy water between that mirrors
    // the shore, as a mountain lake in the morning calm is (Morskie Oko's photographs): the
    // stiffer the wind, the more of it is ruffled, all of it in a breeze.
    let gusting = lake.wind.xyz * (1.5 + 4.0 * lake.wind.w) + up * 0.4;
    let paws = noise(local * 0.018 - gusting * globals.time * 0.018 + 5.0) * 0.65
        + noise(local * 0.05 - gusting * globals.time * 0.05 + 19.0) * 0.35;
    let ruffled = smoothstep(0.62 - 0.5 * lake.wind.w, 0.78 - 0.5 * lake.wind.w, paws);
    // Glassy water still heaves a little with the swell the last breeze left, enough to waver
    // the shore it mirrors; the ripples are as steep as Cox and Munk's sea by the wind, its
    // mean square slope 0.003 + 0.00512 a metre a second of it (taken as a gust's, at least
    // 1.5 m/s, in a cat's-paw), on the sea's 0.003 of swell, which a lake has less of.
    let wind_m_s = 8.0 * lake.wind.w;
    let swell = 0.0004 + 0.0012 * lake.wind.w;
    let rippled = 0.0002 + ruffled * 0.00512 * max(wind_m_s, 1.5);
    let face = waves(at, pixel, downwind, swell, rippled);
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
        edge = smoothstep(0.01, 0.07, thick + rise + (broken - 0.5) * 0.05);
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
        seen = mirrored(reflected, up, air_seen(reflected), air_seen(level));
    } else {
        let sky = mix(lake.horizon.rgb, lake.zenith.rgb, pow(smoothstep(0.0, 0.62, dot(reflected, up)), 0.55));
        seen = mirrored(reflected, up, sky, lake.horizon.rgb) * lake.sky_brightness;
    }
    // What stands on screen along the reflected ray, where the ray finds it there: the shore's
    // trees, the far slopes and peaks as they are drawn; the skyline's picture of them where it
    // leaves the screen.
    let found = on_screen(world, reflected, in.position.xy, distance);
    seen = mix(seen, found.rgb, found.a);
    // The sun's glint off the ripples: a normalised Blinn-Phong lobe, as sharp as still water's
    // where the pixel sees every wave, widened by the slopes of those it cannot (Blinn-Phong's
    // power as Beckmann's for the mean square slope, 2/m² − 2), so far off the glitter merges
    // into the sun's broad path on the water rather than flickering.
    let sun = lights.directional_lights[0];
    let half_way = normalize(sun.direction_to_light + toward_eye);
    let lit = max(dot(normal, sun.direction_to_light), 0.0);
    let power = 2.0 / (2.0 / 602.0 + face.unseen) - 2.0;
    let glint = pow(max(dot(normal, half_way), 0.0), power) * ((power + 8.0) / 25.13) * lit;
    let glinting = sun.color.rgb * view.exposure * glint * mirror;
    out.color = vec4<f32>(out.color.rgb + (seen * mirror + glinting) * (1.0 - foam), out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    out.color.a *= edge;
    return out;
}
