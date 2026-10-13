// The sea: see-through at your feet and mirror-like toward the horizon, with small
// moving ripples that break the sun's reflection into glitter.
//
// What is under the water is coloured by the ground's own shader, which fades the bed
// into the deep; this only adds the surface.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::{globals, view},
};

// Field order must match WaterExtension in planet.rs.
struct Water {
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    sky_brightness: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> water: Water;

fn hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn noise(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(hash(cell), hash(cell + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(hash(cell + vec3<f32>(0.0, 1.0, 0.0)), hash(cell + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(hash(cell + vec3<f32>(0.0, 0.0, 1.0)), hash(cell + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(hash(cell + vec3<f32>(0.0, 1.0, 1.0)), hash(cell + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// The height of the ripples at a place: two sets crossing each other.
fn ripples(p: vec3<f32>, across: vec3<f32>, along: vec3<f32>) -> f32 {
    let t = globals.time;
    return noise(p * 0.9 + across * t * 0.35) * 0.6
        + noise(p * 2.3 - along * t * 0.5 + 11.0) * 0.4;
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

    // Two level directions to measure the ripples' slope along.
    let across = normalize(cross(up, vec3<f32>(0.3, 0.1, 0.9)));
    let along = cross(up, across);
    let here = ripples(world, across, along);
    let slope = vec2<f32>(
        ripples(world + across * 0.15, across, along) - here,
        ripples(world + along * 0.15, across, along) - here,
    );
    // Far away the ripples are smaller than a pixel and would only sparkle.
    let strength = 1.1 / (1.0 + distance / 90.0);
    let normal = normalize(up - (across * slope.x + along * slope.y) * strength);
    pbr_input.world_normal = normal;
    pbr_input.N = normal;

    // Looking down you see into the water; looking along it you see the sky in it.
    let mirror = pow(1.0 - max(dot(normal, toward_eye), 0.0), 4.0);
    pbr_input.material.base_color.a = mix(0.42, 0.97, mirror);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    let reflected = reflect(-toward_eye, normal);
    let sky = mix(water.horizon.rgb, water.zenith.rgb, pow(smoothstep(0.0, 0.62, dot(reflected, up)), 0.55));
    out.color = vec4<f32>(out.color.rgb + sky * water.sky_brightness * mirror * 0.8, out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
