// The sea: see-through at your feet and mirror-like toward the horizon, its waves the lakes'
// (waves.wesl), laid out from a frame that travels with the camera (waves.rs `Travelling`),
// so they stay where they are in the world on a surface as big as the planet.
//
// What is under the water is coloured by the ground's own shader, which fades the bed
// into the deep; this only adds the surface.

import super::waves::{Surface, TRAINS, SWELL, cats_paws, wave_slopes, wave_bend, wave_train, glint};
import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::{globals, view, lights},
};

// Field order must match WaterExtension in planet.rs.
struct Water {
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    sky_brightness: f32,
    // Which way the wind blows, level, and how hard, 0 still to 1 a stiff breeze.
    wind: vec4<f32>,
    // The travelling frame: its origin under the camera, and east and north there.
    origin: vec4<f32>,
    east: vec4<f32>,
    north: vec4<f32>,
    // A point fixed in the world near the start, what the gusts' noise is laid out from.
    anchor: vec4<f32>,
    // The waves' trains, their phases where the frame is (waves.rs).
    trains: array<vec4<f32>, TRAINS>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> water: Water;

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

    // Where the pixel is, metres east and north of the frame's origin, and how much water the
    // pixel covers: taken first, as derivatives must be.
    let off = world - water.origin.xyz;
    let at = vec2<f32>(dot(off, water.east.xyz), dot(off, water.north.xyz));
    let pixel = max(max(length(dpdx(at)), length(dpdy(at))), 1e-4);

    // East and north here, and downwind among them.
    let east = normalize(water.east.xyz - up * dot(water.east.xyz, up));
    let north = cross(up, east);
    var downwind = vec2<f32>(dot(water.wind.xyz, east), dot(water.wind.xyz, north));
    if length(downwind) < 1e-3 {
        downwind = vec2<f32>(0.6, 0.8);
    }
    downwind = normalize(downwind);
    let anchored = world - water.anchor.xyz;
    let t = globals.time;
    let ruffled = cats_paws(anchored, water.wind, up, t);
    let slopes = wave_slopes(water.wind.w, ruffled);
    let spread = smoothstep(0.0, 0.15, water.wind.w);
    var face = Surface(vec2<f32>(0.0), 0.0, 0.0);
    let bent = wave_bend(at, anchored, up, t);
    for (var i = 0u; i < TRAINS; i += 1u) {
        wave_train(&face, water.trains[i], i < SWELL, bent, pixel, downwind, spread, slopes, t);
    }
    let normal = normalize(up - east * face.slope.x - north * face.slope.y);
    pbr_input.world_normal = normal;
    pbr_input.N = normal;

    // Looking down you see into the water; looking along it you see the sky in it.
    let mirror = pow(1.0 - max(dot(normal, toward_eye), 0.0), 4.0);
    pbr_input.material.base_color.a = mix(0.42, 0.97, mirror);
    // The lighting lights the water itself; what it reflects, the sky and the sun's glint, is
    // drawn below.
    pbr_input.material.reflectance = vec3<f32>(0.0);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    // Waves too short for the pixel tilt toward the eye the facets that reflect into it, and
    // those mirror higher, more of the sky.
    let toward_level = normalize(toward_eye - up * dot(toward_eye, up) + up * 1e-4);
    let reflected = reflect(-toward_eye, normalize(normal + toward_level * sqrt(face.unseen) * 0.6));
    let sky = mix(water.horizon.rgb, water.zenith.rgb, pow(smoothstep(0.0, 0.62, dot(reflected, up)), 0.55));
    let sun = lights.directional_lights[0];
    let glinting = sun.color.rgb * view.exposure * glint(normal, toward_eye, sun.direction_to_light, face.unseen) * mirror;
    out.color = vec4<f32>(out.color.rgb + sky * water.sky_brightness * mirror * 0.8 + glinting, out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
