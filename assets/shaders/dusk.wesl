// The dusk's own colours, which the sky draws over the air after sunset (sky.wesl), and which
// the water mirrors (lake.wesl) and the far land's haze takes (sky.wesl `far_land`): one
// function, so the three cannot drift apart. Linear, before the sky's brightness and its
// share of twilight.
//
// A civil dusk is warm only low toward the set sun, amber to peach, its afterglow rising
// higher there than anywhere; elsewhere the horizon is a pale grey-blue, and above it the sky
// deepens to the zenith's blue, green over red. The horizon's colour mixed straight into the
// far dimmer zenith carried its red up the whole sky (lake film r9, magenta overhead); kept to
// a low band everywhere, no glow showed toward the sun, since the far range hid the band
// (r10, no warm sky pixel); and the water and the range kept the old salmon under a sky that
// had lost it (r10, sat 0.45 under 0.10).
// The afterglow toward the set sun: amber at the horizon's brightness, reddening as the glow
// does as the sun sinks (daylight's keys). The day's horizon key goes lavender as the sun
// sinks (its -6 degree key), so a band in the horizon's own colour read mauve by mid-twilight
// (lake film r17, sun 3.7 degrees under: no warm band anywhere); in the glow's own hue it read
// salmon red (sRGB hue 9, `sky::dusk`'s test). A civil dusk keeps a yellow-orange horizon
// toward the sun, about sRGB 255,180,100 (linear 1, 0.46, 0.13), until the sun is six under,
// red at the end, its disc's glow redder. The rule is tested on its CPU twin (`sky::dusk`).
fn afterglow(horizon: vec3<f32>, glow: vec3<f32>) -> vec3<f32> {
    let hue = mix(glow / max(max(glow.r, glow.g), max(glow.b, 1.0e-4)), vec3<f32>(1.0, 0.46, 0.13), 0.6);
    return hue * dot(horizon, vec3<f32>(0.2126, 0.7152, 0.0722)) * 1.6;
}

fn dusk_sky(direction: vec3<f32>, up: vec3<f32>, sun: vec3<f32>, horizon: vec3<f32>, zenith: vec3<f32>, glow: vec3<f32>) -> vec3<f32> {
    let lift = dot(direction, up);
    let flat_sun = normalize(sun - up * dot(sun, up) + up * 1e-4);
    let flat_way = normalize(direction - up * lift + up * 1e-4);
    // Toward the sun's bearing, falling to a half 60 degrees off it and none past 90: the
    // twilight arch spans the western sky, where at a quarter by 60 degrees a far range
    // hid all of it from a view 50 degrees off the sun (lake film r17).
    let bearing = max(dot(flat_way, flat_sun), 0.0);
    let toward = bearing;
    // The warm band: up to 17 degrees toward the sun, a few elsewhere.
    let band = 1.0 - smoothstep(0.0, mix(0.06, 0.3, toward), lift);
    // Above and away from the band, the horizon's light without its hue, a little cool.
    let pale = vec3<f32>(dot(horizon, vec3<f32>(0.2126, 0.7152, 0.0722))) * vec3<f32>(0.86, 0.97, 1.15);
    let warm = mix(horizon, afterglow(horizon, glow), toward);
    var low = mix(pale, warm, band * mix(0.15, 1.0, toward));
    // Opposite the sun, the earth's own shadow on the air, a blue-grey band on the horizon,
    // and over it the Belt of Venus, the air there still in the sun's last red light, pink
    // to mauve from 4 to 20 degrees up: what a lake shows looking away from the set sun.
    let away = max(-dot(flat_way, flat_sun), 0.0);
    let belt = smoothstep(0.04, 0.14, lift) * (1.0 - smoothstep(0.16, 0.34, lift));
    let venus = pale * vec3<f32>(1.3, 0.84, 0.92);
    let earth = (1.0 - smoothstep(0.02, 0.1, lift)) * away;
    let shadowed = normalize(zenith + vec3<f32>(1.0e-4)) * length(pale) * 0.7;
    low = mix(low, venus, belt * away * 0.75);
    low = mix(low, shadowed, earth * 0.5);
    // To the zenith's blue, a little red taken out so it keeps its green over its red and
    // reads as dusk rather than the night's violet. Not greyed here: the grade's dim
    // keep a fifth of their colour from sunset (D302), and the dusk sky is dim to it, so
    // greyed again it came out a mauve grey (r12, saturation 0.19; a civil dusk's zenith
    // is near 0.4); with a tenth of its red out it was still violet, 69,74,101.
    let deep = zenith * vec3<f32>(0.8, 1.0, 1.0);
    var colour = mix(low, deep, pow(smoothstep(-0.02, 0.55, lift), 0.5));
    colour += glow * pow(max(dot(direction, sun), 0.0), 5.0) * 0.35 * (1.0 - smoothstep(0.0, 0.5, lift));
    return colour;
}
