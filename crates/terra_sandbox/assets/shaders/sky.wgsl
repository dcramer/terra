// The sky: a soft gradient from the horizon up, clouds lit by the sun, a glow around the
// sun, thinning to black as the camera climbs out of the air.

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::{globals, view},
}

// Field order must match SkyMaterial in sky.rs.
struct Sky {
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    glow: vec4<f32>,
    sun_direction: vec3<f32>,
    radius_m: f32,
    cloudiness: f32,
    brightness: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> sky: Sky;

// Cheap smooth noise for clouds: random values on a grid, blended between.
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

fn billow(p: vec3<f32>) -> f32 {
    return noise(p) * 0.5 + noise(p * 2.1 + 7.0) * 0.27 + noise(p * 4.3 + 19.0) * 0.15
        + noise(p * 8.9 + 5.0) * 0.08;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let eye = view.world_position;
    // The planet's centre is the world origin, so "up" is away from it.
    let up = normalize(eye);
    let direction = normalize(mesh.world_position.xyz - eye);
    let sun = normalize(sky.sun_direction);

    // -1 looking straight down, 0 at the horizon, 1 straight up.
    let lift = dot(direction, up);
    let toward_sun = max(dot(direction, sun), 0.0);
    var colour = mix(sky.horizon.rgb, sky.zenith.rgb, pow(smoothstep(-0.02, 0.62, lift), 0.55));
    // The air glows toward the sun, most of all low down.
    colour += sky.glow.rgb * pow(toward_sun, 5.0) * 0.35 * (1.0 - smoothstep(0.0, 0.7, lift));

    // Clouds on a flat layer overhead, so they stretch out toward the horizon, drifting
    // slowly. Their sunward edges are bright and their bodies take the sky's shade.
    let layer = direction / max(lift, 0.05);
    let drift = vec3<f32>(globals.time * 0.004, 0.0, globals.time * 0.002);
    let p = (layer - up * dot(layer, up)) * 0.55 + drift;
    let body = billow(p);
    let threshold = 1.0 - sky.cloudiness * 0.62;
    let cloud = smoothstep(threshold - 0.12, threshold + 0.1, body) * smoothstep(0.02, 0.2, lift);
    // Thicker toward the sun means darker; thin edges facing it catch the light.
    let toward = billow(p + (sun - up * dot(sun, up)) * 0.12);
    let lit = clamp((body - toward) * 4.0 + 0.55, 0.0, 1.0);
    let shade = mix(sky.zenith.rgb * 0.75 + sky.horizon.rgb * 0.25, sky.horizon.rgb, 0.35);
    let bright = mix(vec3<f32>(1.0, 0.97, 0.94), sky.glow.rgb, 0.55) * 1.25;
    let cloud_colour = mix(shade, bright, lit * (0.55 + 0.45 * pow(toward_sun, 2.0)));
    colour = mix(colour, cloud_colour, cloud * 0.9);

    // The sun itself.
    colour += sky.glow.rgb * (pow(toward_sun, 40.0) * 0.5 + smoothstep(0.9994, 0.9998, toward_sun) * 30.0);

    let altitude = max(length(eye) - sky.radius_m, 0.0);
    return vec4<f32>(colour * sky.brightness * exp(-altitude / 700.0), 1.0);
}
