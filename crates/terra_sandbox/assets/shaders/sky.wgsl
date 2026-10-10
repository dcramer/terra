// The sky for the "look" experiment: a soft gradient from the horizon up, a glow around
// the sun, thinning to black as the camera climbs out of the air.

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::view,
}

// Field order must match SkyMaterial in look.rs.
struct Sky {
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    sun_direction: vec3<f32>,
    radius_m: f32,
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

fn clouds(direction: vec3<f32>) -> f32 {
    let p = direction * 3.5;
    let billow = noise(p) * 0.55 + noise(p * 2.1 + 7.0) * 0.3 + noise(p * 4.3 + 19.0) * 0.15;
    return smoothstep(0.52, 0.7, billow);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let eye = view.world_position;
    // The planet's centre is the world origin, so "up" is away from it.
    let up = normalize(eye);
    let direction = normalize(mesh.world_position.xyz - eye);

    // -1 looking straight down, 0 at the horizon, 1 straight up.
    let lift = dot(direction, up);
    var colour = mix(sky.horizon.rgb, sky.zenith.rgb, pow(smoothstep(-0.02, 0.6, lift), 0.6));

    // Soft clouds, fading out toward the horizon, lit warm on the side facing the sun.
    let toward_sun = max(dot(direction, normalize(sky.sun_direction)), 0.0);
    let cloud = clouds(direction) * smoothstep(0.03, 0.3, lift) * 0.85;
    let cloud_colour = mix(vec3<f32>(0.93, 0.9, 0.95), vec3<f32>(1.0, 0.93, 0.84), toward_sun);
    colour = mix(colour, cloud_colour, cloud);

    colour += vec3<f32>(1.0, 0.82, 0.6) * (pow(toward_sun, 6.0) * 0.2 + pow(toward_sun, 600.0) * 5.0);

    // The camera is exposed for sunlit ground, which is far brighter than 1.0; lift the
    // sky to sit with it.
    let brightness = 2.6;
    let altitude = max(length(eye) - sky.radius_m, 0.0);
    return vec4<f32>(colour * brightness * exp(-altitude / 700.0), 1.0);
}
