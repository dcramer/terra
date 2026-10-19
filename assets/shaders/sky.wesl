// The sky: a soft gradient from the horizon up, clouds lit by the sun, a glow around the
// sun, thinning to black as the camera climbs out of the air. At night, stars that turn
// with the sky, and a moon opposite the sun.

import bevy_pbr::render::{
    forward_io::VertexOutput,
    mesh_view_bindings::{globals, view},
};
import super::dusk::dusk_sky;

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
    // The far land's canopy, and in w how high its farthest ridges rise, in radians; 0 for
    // none (D273).
    far_land: vec4<f32>,
    // How its ridges are shaped: 0 a forest's rolling hills, 1 a range's peaks.
    far_peaks: f32,
    // Toward the sun, below the horizon too, for the dusk's colours.
    dusk_sun: vec3<f32>,
    // The overcast deck (weather.rs, D418): how much of the sky it covers, how dark rain has
    // made it, its brightness seen from below, and the wind, m/s.
    deck: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0)
var<uniform> sky: Sky;
// The air all round the camera, as Bevy's atmosphere works it out each frame, unexposed.
@group(constants::MATERIAL_BIND_GROUP) @binding(1)
var air_texture: texture_cube<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2)
var air_sampler: sampler;

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

// The air's light from a direction, exposed as the sky is. The cube is left-handed, as
// Bevy's environment maps are.
fn air_seen(direction: vec3<f32>) -> vec3<f32> {
    let d = vec3<f32>(direction.x, direction.y, -direction.z);
    return textureSampleLevel(air_texture, air_sampler, d, 0.0).rgb * view.exposure;
}

// Each ridge of the far land, nearest first: where it stands between the planet's own
// horizon below and the air's above (0 at the planet's, 1 at the air's), how high its hills
// rise as a share of the farthest's, and how much of the land's own colour the haze leaves.
const RIDGES: u32 = 4u;
const RIDGE_AT: array<f32, 4> = array<f32, 4>(0.12, 0.5, 0.8, 1.0);
const RIDGE_HILLS: array<f32, 4> = array<f32, 4>(0.45, 0.55, 0.75, 1.0);
const RIDGE_LEFT: array<f32, 4> = array<f32, 4>(0.55, 0.34, 0.18, 0.08);
// After sunset, how bright the haze before the far land is against the horizon's band, and
// how far it is turned toward the zenith's blue.
const EARTH_SHADOW: f32 = 0.6;
const DUSK_COOL: f32 = 0.45;

// The land beyond the world's edge (D273), below the horizon, as shipped games draw the
// hills past their maps: ridges one behind another, each paler than the one before in the
// air's own colour at the horizon, the farthest on the horizon itself. The stand-in planet is
// small, so its ground falls away below the air's horizon, by more the higher the eye: the
// ridges spread between the two, nearer ones lower, as land does seen from a height. Their
// tops are broken by the crowns of the forest on them, finer the farther. Premultiplied.
fn far_land(direction: vec3<f32>, up: vec3<f32>, eye: vec3<f32>, sunlight: vec3<f32>) -> vec4<f32> {
    let hills = sky.far_land.w;
    let lift = dot(direction, up);
    let rise = asin(clamp(lift, -1.0, 1.0));
    let pixel = max(fwidth(rise), 1.0e-5);
    if hills <= 0.0 || lift > sin(hills * 1.3) {
        return vec4<f32>(0.0);
    }
    var at = RIDGE_AT;
    var rising = RIDGE_HILLS;
    var leaves = RIDGE_LEFT;
    let r = sky.radius_m;
    let dip = acos(clamp(r / (r + max(length(eye) - r, 0.3)), 0.0, 1.0));
    // Round the horizon: a point on a circle, so the noise along it has no seam.
    let east = normalize(cross(up, vec3<f32>(0.0, 0.0, 1.0)));
    let north = cross(east, up);
    let level = normalize(direction - up * lift + east * 1.0e-5);
    let round = vec2<f32>(dot(level, east), dot(level, north));
    // The air just over the horizon: Bevy's tables darken the air's light below it, most of
    // all straight ahead, as if its own ground showed through.
    // After sunset the dusk's own colour there, as the sky draws it just over the horizon
    // (dusk.wesl): warm toward the sun's bearing, pale elsewhere. The old horizon colour
    // here kept the range magenta under a sky that had lost it (lake film r10, 111,72,111).
    let sun = normalize(sky.sun_direction);
    let over = normalize(level + up * 0.02);
    let behind = air_seen(over)
        + dusk_sky(over, up, sky.dusk_sun, sky.horizon.rgb, sky.zenith.rgb, sky.glow.rgb) * sky.brightness * sky.twilight;
    // The haze before a ridge after sunset: the low air lies in the earth's own shadow, lit
    // only by the sky over it, dimmer and bluer than the horizon's band behind, which is air
    // still sunlit far off and high up. As bright as that band, a range at dusk was as pale
    // and warm as the sky beside it (lake film r7; Bohinj's ridges at 46 and 29 against 133).
    let cool = normalize(sky.zenith.rgb + vec3<f32>(1.0e-4)) * length(behind);
    // The forest on it, lit as the world's canopy is by the sun through the air and the sky.
    let canopy = sky.far_land.rgb;
    // A range's slopes face every way, most away from a low sun: half of them lit.
    let lit = canopy * (sunlight * max(dot(sun, up), 0.0) * 0.6 * (1.0 - 0.5 * sky.far_peaks) + air_seen(up) * 1.6) / 3.14159
        * view.exposure;
    var colour = vec3<f32>(0.0);
    var cover = 0.0;
    for (var k = 0u; k < RIDGES; k++) {
        let f = f32(k);
        // A range's nearer ridges stand over the horizon too, behind the world's own slopes,
        // where a forest's lie between the planet's horizon and the air's: with them below it
        // only the farthest showed, as pale as the sky.
        let base = -dip * (1.0 - at[k]) * (1.0 - 0.85 * sky.far_peaks);
        // Hills along the ridge, and the crowns on them, finer and lower the farther.
        let p = vec3<f32>(round * (5.0 + 4.0 * f), f * 13.7);
        let shape = billow(p) * 0.85 + billow(p * 3.7 + 3.0) * 0.15;
        let crowns = noise(vec3<f32>(round * (260.0 + 240.0 * f), f * 5.1));
        // A range's crests, sharp where two slopes meet, never down to the horizon, so peak
        // stands behind peak along all of it (a ridged noise, as terrain generators make them).
        let crest = 1.0 - abs(noise(p * 3.1 + 11.0) * 2.0 - 1.0);
        let fine = 1.0 - abs(noise(p * 6.1 + 23.0) * 2.0 - 1.0);
        let range = 0.3 + 0.7 * pow(crest * 0.85 + fine * 0.15, 1.3) * mix(0.6, 1.0, shape);
        let rolling = smoothstep(0.25, 0.85, shape);
        let ridge = base + hills * rising[k] * mix(rolling, range, sky.far_peaks)
            + hills * (0.06 / (1.0 + f)) * crowns * (1.0 - sky.far_peaks);
        let inside = clamp((ridge - rise) / pixel + 0.5, 0.0, 1.0);
        if inside <= 0.0 {
            continue;
        }
        // Mist settles below each ridge's crest, paler toward the valley behind the next.
        let deep = smoothstep(0.0, max(dip * 0.35, 0.004), ridge - rise);
        // A range's ridges keep more of their own dark the nearer, against the glow behind
        // them (Bohinj at dusk: 46 against the sky's 133).
        let left = min(leaves[k] * (1.0 + 0.8 * sky.far_peaks), 1.0) * mix(1.0, 0.7, deep);
        // The crowns' light and shade, finer the farther.
        let grain = noise(vec3<f32>(round * (420.0 + 400.0 * f), rise * (420.0 + 400.0 * f) + f * 3.3));
        let land = lit * (0.8 + 0.4 * grain);
        // Its haze: the farthest ridge nearly the sky behind it, a nearer one deeper in the
        // earth's shadow, dimmer and bluer, so the range reads in layers.
        let shadow = 1.0 - f / f32(RIDGES - 1u);
        let haze = mix(behind, cool, DUSK_COOL * sky.twilight * (0.35 + 0.65 * shadow))
            * mix(1.0, mix(0.85, EARTH_SHADOW, shadow), sky.twilight);
        let seen = mix(haze, land, left);
        let take = inside * (1.0 - cover);
        colour += seen * take;
        cover += take;
        if cover > 0.999 {
            break;
        }
    }
    return vec4<f32>(colour, cover);
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
    var cover = cloud * 0.9;
    var colour = cloud_colour * cover;

    // The overcast deck (D418): one grey layer lit from above and seen from below, as bright
    // as the sky's light it gives the ground. Its thicker bellies are darker and its seams
    // paler, the more so as rain thickens it; it is brightest where the sun is behind it and
    // lowest and evenest toward the horizon. Scud under it runs with the wind.
    if sky.deck.x > 0.0 {
        let wind = sky.deck.w;
        let run = vec3<f32>(globals.time, 0.0, globals.time * 0.45) * (0.0015 + 0.0009 * wind);
        let broad = billow(p * 0.28 + run);
        let fine = billow(p * 1.3 + run * 2.6 + 3.1);
        let rain = sky.deck.y;
        let body = (broad - 0.5) * 2.4 + (fine - 0.5) * (0.8 + 1.4 * rain);
        let flat = smoothstep(0.02, 0.35, lift);
        var shade = 1.0 - clamp(body, -0.6, 1.0) * (0.22 + 0.4 * rain) * flat;
        shade *= 1.0 + 0.5 * pow(toward_sun, 6.0) * (1.0 - rain);
        // Toward the horizon one sees the deck's underside from far off, through more air:
        // paler and flatter.
        shade = mix(1.08, shade, flat);
        let grey = vec3<f32>(0.92, 0.95, 1.0) * sky.deck.z * shade * view.exposure;
        let deck = sky.deck.x * smoothstep(-0.12, 0.02, lift);
        colour = mix(colour, grey, deck);
        cover = mix(cover, 1.0, deck);
    }

    // After sunset, the dusk's and the night's own colours, where the clouds leave sky
    // (dusk.wesl).
    if sky.twilight > 0.0 {
        let gradient = dusk_sky(direction, up, sky.dusk_sun, sky.horizon.rgb, sky.zenith.rgb, sky.glow.rgb);
        colour += gradient * sky.brightness * sky.twilight * (1.0 - cover);
    }

    if sky.night > 0.0 {
        let hidden = min(cover / 0.9, 1.0);
        let clear = (1.0 - hidden) * smoothstep(0.0, 0.25, lift);
        colour += vec3<f32>(0.85, 0.9, 1.0) * stars(direction) * clear * sky.night * 0.6;
        let toward_moon = max(dot(direction, -sun), 0.0);
        let disc = smoothstep(0.99955, 0.9998, toward_moon);
        let halo = pow(toward_moon, 60.0) * 0.12;
        colour += vec3<f32>(0.8, 0.85, 0.95) * (disc * 1.2 + halo) * sky.night * (1.0 - hidden * 0.7);
    }
    // The far land stands in front of all of it.
    let land = far_land(direction, up, eye, sunlight);
    return vec4<f32>(land.rgb + colour * (1.0 - land.a), land.a + cover * (1.0 - land.a));
}
