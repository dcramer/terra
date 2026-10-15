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
// deep water is its own colour. The ripples, the sky they reflect and the glitter are our
// sea's (water.wesl), turned by the wind.
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

// The ripples' height at a place: small cat's-paws running before the wind, and a finer set
// across them, stretched along the wind so they read as its.
fn ripples(p: vec3<f32>, downwind: vec3<f32>, across: vec3<f32>) -> f32 {
    let t = globals.time;
    // Finer along the wind than across it, so the crests run across the wind, as they do:
    // the wind's way is one fixed direction over a lake, not the level one at each point.
    let q = p + lake.wind.xyz * dot(p, lake.wind.xyz) * 0.8;
    return noise(q * 1.6 - downwind * t * 1.2) * 0.6
        + noise(q * 4.3 - downwind * t * 2.2 + across * t * 0.4 + 11.0) * 0.4;
}

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

    // The ripples by the wind: their slope along two level directions.
    var downwind = lake.wind.xyz - up * dot(lake.wind.xyz, up);
    if length(downwind) < 1e-3 {
        downwind = cross(up, vec3<f32>(0.3, 0.1, 0.9));
    }
    downwind = normalize(downwind);
    let across = cross(up, downwind);
    let local = world - lake.origin.xyz;
    let here = ripples(local, downwind, across);
    let slope = vec2<f32>(
        ripples(local + downwind * 0.075, downwind, across) - here,
        ripples(local + across * 0.075, downwind, across) - here,
    );
    // Calm water still shivers a little; far off the ripples are under a pixel.
    let strength = (0.15 + 0.55 * lake.wind.w) / (1.0 + distance / 120.0);
    let normal = normalize(up - (downwind * slope.x + across * slope.y) * strength);
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
    let foam = (1.0 - smoothstep(lake.foam_from, lake.foam_to, under)) * smoothstep(0.35, 0.75, foam_noise)
        * (0.3 + 0.7 * lake.wind.w) * (1.0 - smoothstep(30.0, 80.0, distance));
    pbr_input.material.base_color = mix(pbr_input.material.base_color, vec4<f32>(0.85, 0.87, 0.84, 1.0), foam * 0.8);
    pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.8, foam);
    // The lighting lights the medium and the foam alone: what the water reflects is drawn
    // below, so the slopes it mirrors can be darker than the sky, as Bevy's own reflection of
    // the sky everywhere would not let them be.
    pbr_input.material.reflectance = vec3<f32>(0.0);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    let reflected = reflect(-toward_eye, normal);
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
    // The sun's glint off the ripples: a normalised Blinn-Phong lobe, as sharp as still water's.
    let sun = lights.directional_lights[0];
    let half_way = normalize(sun.direction_to_light + toward_eye);
    let lit = max(dot(normal, sun.direction_to_light), 0.0);
    let glint = pow(max(dot(normal, half_way), 0.0), 600.0) * (608.0 / 25.13) * lit;
    let glinting = sun.color.rgb * view.exposure * glint * mirror;
    out.color = vec4<f32>(out.color.rgb + (seen * mirror + glinting) * (1.0 - foam), out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
