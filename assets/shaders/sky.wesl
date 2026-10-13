// The sky: a soft gradient from the horizon up, clouds lit by the sun, a glow around the
// sun, thinning to black as the camera climbs out of the air. At night, stars that turn
// with the sky, and a moon opposite the sun.

import bevy_pbr::render::{
    forward_io::VertexOutput,
    mesh_view_bindings::{globals, view},
};

// Field order must match SkyMaterial in sky.rs.
struct Sky {
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    glow: vec4<f32>,
    sun_direction: vec3<f32>,
    radius_m: f32,
    cloudiness: f32,
    brightness: f32,
    night: f32,
    spin_axis: vec3<f32>,
    spin: f32,
    sunlight: vec3<f32>,
    over_air: u32,
    twilight: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
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

// A direction turned about an axis, the other way from how the sky has turned, so the
// stars keep their places on it.
fn unspun(direction: vec3<f32>) -> vec3<f32> {
    let axis = normalize(sky.spin_axis);
    let c = cos(-sky.spin);
    let s = sin(-sky.spin);
    return direction * c + cross(axis, direction) * s + axis * dot(axis, direction) * (1.0 - c);
}

// Stars: a few cells of a fine grid over the sky hold one, brighter or dimmer.
fn stars(direction: vec3<f32>) -> f32 {
    let p = unspun(direction) * 220.0;
    let cell = floor(p);
    let chance = hash(cell);
    if chance < 0.985 {
        return 0.0;
    }
    let centre = cell + 0.5 + (vec3<f32>(hash(cell + 3.1), hash(cell + 7.7), hash(cell + 1.9)) - 0.5) * 0.6;
    let near = 1.0 - smoothstep(0.08, 0.32, length(p - centre));
    return near * (0.35 + (chance - 0.985) / 0.015 * 0.65);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    if sky.over_air != 0u {
        return over_air(mesh);
    }
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
    // By night the clouds are only grey shapes against the stars.
    let bright = mix(vec3<f32>(1.0, 0.97, 0.94), sky.glow.rgb, 0.55) * 1.25
        * (1.0 - 0.9 * sky.night);
    let cloud_colour = mix(shade, bright, lit * (0.55 + 0.45 * pow(toward_sun, 2.0)));
    colour = mix(colour, cloud_colour, cloud * 0.9);

    // The sun itself.
    colour += sky.glow.rgb * (pow(toward_sun, 40.0) * 0.5 + smoothstep(0.9994, 0.9998, toward_sun) * 30.0);

    // At night, the stars where no cloud hides them, fading toward the horizon's haze,
    // and the moon opposite the sun with a faint halo.
    if sky.night > 0.0 {
        let clear = (1.0 - cloud) * smoothstep(0.0, 0.25, lift);
        colour += vec3<f32>(0.85, 0.9, 1.0) * stars(direction) * clear * sky.night * 0.6;
        let toward_moon = max(dot(direction, -sun), 0.0);
        let disc = smoothstep(0.99955, 0.9998, toward_moon);
        let halo = pow(toward_moon, 60.0) * 0.12;
        colour += vec3<f32>(0.8, 0.85, 0.95) * (disc * 1.2 + halo) * sky.night * (1.0 - cloud * 0.7);
    }

    let altitude = max(length(eye) - sky.radius_m, 0.0);
    return vec4<f32>(colour * sky.brightness * exp(-altitude / 700.0), 1.0);
}

// Over Bevy's atmosphere (air.rs), which draws the sky's colour and the sun: only the
// clouds, lit by the same sunlight as the world and exposed as it is, and by night the
// stars and the moon, premultiplied so that clouds cover the sky and stars add to it.
fn over_air(mesh: VertexOutput) -> vec4<f32> {
    let eye = view.world_position;
    let up = normalize(eye);
    let direction = normalize(mesh.world_position.xyz - eye);
    let sun = normalize(sky.sun_direction);
    let lift = dot(direction, up);
    let toward_sun = max(dot(direction, sun), 0.0);

    let layer = direction / max(lift, 0.05);
    let drift = vec3<f32>(globals.time * 0.004, 0.0, globals.time * 0.002);
    let p = (layer - up * dot(layer, up)) * 0.55 + drift;
    let body = billow(p);
    let threshold = 1.0 - sky.cloudiness * 0.62;
    // Thinning toward the horizon, where the haze takes over.
    let cloud = smoothstep(threshold - 0.12, threshold + 0.1, body) * smoothstep(0.02, 0.25, lift);
    let toward = billow(p + (sun - up * dot(sun, up)) * 0.12);
    let lit = clamp((body - toward) * 4.0 + 0.55, 0.0, 1.0);
    // White cloud in sunlight reflects about a quarter of it toward the eye; its shaded
    // underside is lit by the sky, bluish. The sun low, the glow warms them.
    // The light reaching the clouds has come through the air, reddened and dimmed the
    // more the lower the sun: Rayleigh optical depths up through the air, times a path
    // length that grows toward the horizon, capped since clouds stand high in it. They
    // stay lit a little after sunset.
    let height = dot(sun, up);
    let air_mass = 1.0 / max(height + 0.08, 0.08);
    let through = exp(-vec3<f32>(0.06, 0.14, 0.32) * air_mass) * smoothstep(-0.06, 0.02, height);
    let sunlight = sky.sunlight * through;
    let sunlit = sunlight * 0.22;
    let shaded = (length(sunlight) * 0.04 + length(sky.sunlight) * 0.004 * max(height + 0.1, 0.0))
        * normalize(sky.zenith.rgb * 0.6 + sky.horizon.rgb * 0.4 + 0.05);
    let silver = 0.55 + 0.45 * pow(toward_sun, 2.0);
    let cloud_colour = mix(shaded, sunlit, lit * silver) * view.exposure;
    let cover = cloud * 0.9;
    var colour = cloud_colour * cover;

    // After sunset, the dusk's and the night's own colours, where the clouds leave sky.
    if sky.twilight > 0.0 {
        var gradient = mix(sky.horizon.rgb, sky.zenith.rgb, pow(smoothstep(-0.02, 0.62, lift), 0.55));
        gradient += sky.glow.rgb * pow(toward_sun, 5.0) * 0.35 * (1.0 - smoothstep(0.0, 0.7, lift));
        colour += gradient * sky.brightness * sky.twilight * (1.0 - cover);
    }

    if sky.night > 0.0 {
        let clear = (1.0 - cloud) * smoothstep(0.0, 0.25, lift);
        colour += vec3<f32>(0.85, 0.9, 1.0) * stars(direction) * clear * sky.night * 0.6;
        let toward_moon = max(dot(direction, -sun), 0.0);
        let disc = smoothstep(0.99955, 0.9998, toward_moon);
        let halo = pow(toward_moon, 60.0) * 0.12;
        colour += vec3<f32>(0.8, 0.85, 0.95) * (disc * 1.2 + halo) * sky.night * (1.0 - cloud * 0.7);
    }
    return vec4<f32>(colour, cover);
}
