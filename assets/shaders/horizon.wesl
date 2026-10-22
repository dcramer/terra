// How much of a light falls on the ground at a point, from the land's horizon map (D416,
// terra_land::horizon, its CPU twin `HorizonMap::lit`): the light is under the horizon in its
// way, and the ground shaded, kilometres past where the shadow map reaches. Horizon mapping,
// Max 1988, on terrain as Timonen and Westerholm's height field self-shadowing (2010) reads
// it: the horizon between the two baked ways either side of the light's, the light's own
// elevation against it through a penumbra as wide as `soft`. One lookup a pixel where Unreal
// draws its landscape's far shadow cascades again as the sun moves.
//
// The map: `middle` the direction of its middle and the world's radius, `east` its east and
// width in metres, `north` its north and 1 where there is a map. Its 16 ways are four layers of
// four channels, each elevation from LOWEST to HIGHEST radians.

const LOWEST: f32 = -0.1;
const HIGHEST: f32 = 0.7;

fn horizon_lit(
    map: texture_2d_array<f32>,
    map_sampler: sampler,
    middle: vec4<f32>,
    east: vec4<f32>,
    north: vec4<f32>,
    at: vec3<f32>,
    toward: vec3<f32>,
    soft: f32,
) -> f32 {
    if north.w < 0.5 {
        return 1.0;
    }
    let up = normalize(at);
    let along = dot(up, middle.xyz);
    if along <= 0.0 {
        return 1.0;
    }
    let scale = middle.w / along / east.w;
    let uv = vec2<f32>(dot(up, east.xyz) * scale + 0.5, 0.5 - dot(up, north.xyz) * scale);
    // Faded out to lit over the map's last few cells, where it knows less and less.
    let inside = clamp(min(min(uv.x, uv.y), min(1.0 - uv.x, 1.0 - uv.y)) * 40.0, 0.0, 1.0);
    if inside <= 0.0 {
        return 1.0;
    }
    // East and north at the point: the map's, laid level there.
    let e = normalize(east.xyz - up * dot(east.xyz, up));
    let n = normalize(north.xyz - up * dot(north.xyz, up) - e * dot(north.xyz, e));
    let rises = asin(clamp(dot(toward, up), -1.0, 1.0));
    let t = fract(atan2(dot(toward, n), dot(toward, e)) / 6.2831853) * 16.0;
    let i = u32(floor(t)) % 16u;
    let j = (i + 1u) % 16u;
    let a = textureSampleLevel(map, map_sampler, uv, i32(i / 4u), 0.0)[i % 4u];
    let b = textureSampleLevel(map, map_sampler, uv, i32(j / 4u), 0.0)[j % 4u];
    let horizon = LOWEST + mix(a, b, fract(t)) * (HIGHEST - LOWEST);
    return mix(1.0, smoothstep(horizon - soft, horizon + soft, rises), inside);
}
