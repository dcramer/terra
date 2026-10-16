// Building's debug views (D385, crates/terra_game/src/build/views.rs): what a surface is
// coloured by in each, as Unreal's view modes and Unity's scene view draw modes colour a
// scene, so a look or a fault is read at a glance instead of measured. The construction
// materials' shader (build.wesl) and the one every other mesh of a building is drawn in
// while a view is on (build_view_piece.wesl) both fill a `Seen` and call `shown`, so a
// surface reads the same whichever material draws it.
//
// The colours are views.rs's legend's, in linear light; keep the two the same.

import bevy_pbr::render::mesh_view_bindings::view;

// The views, numbered as views.rs's `View`.
const TEXELS: u32 = 1u;
const ROLES: u32 = 2u;
const DETAIL: u32 = 3u;
const DAMAGE: u32 = 4u;
const SHADE: u32 = 5u;
const NORMALS: u32 = 6u;
const PLAN: u32 = 7u;
// The light alone: not a colour of `shown`'s, but the surface lit as an 18% grey, as a grey
// card is (`sandbox.card`), its own colours left out; each shader lights it itself.
const LIGHT_ALONE: u32 = 8u;
const GREY_CARD: f32 = 0.18;

// What a pixel of a surface is, for a view.
struct Seen {
    view: u32,
    // Where it falls on its texture, a copy from 0 to 1, and how that and the world change
    // across the pixel.
    uv: vec2<f32>,
    uv_dx: vec2<f32>,
    uv_dy: vec2<f32>,
    position: vec3<f32>,
    p_dx: vec3<f32>,
    p_dy: vec3<f32>,
    // Where it is on screen, in pixels.
    pixel: vec2<f32>,
    // How many texels across a copy of its texture is; 0 with none (its colours baked).
    side: f32,
    // The screen pixels a texel should cover seen from a metre off: one (D362).
    aim: f32,
    // Its mesh's normal, the normal its normal map makes of it, and its tangent and
    // bitangent; `framed` 0 where the mesh has no tangents.
    geometric: vec3<f32>,
    mapped: vec3<f32>,
    tangent: vec3<f32>,
    bitangent: vec3<f32>,
    framed: f32,
    // How much of its cube's health is lost (0 to 1), and how much of it the damage drawing
    // breaks here; `masked` 1 where the damage mask is drawn at all.
    lost: f32,
    broken: f32,
    masked: f32,
    // Its texture's occlusion, the screen's (SSAO, as it darkens this colour), and its
    // corners' baked shade, each 1 for none.
    texture_occlusion: f32,
    screen_occlusion: f32,
    baked: f32,
    // How bright its own colour is, 0 to 1, so a texture still reads under a view's colour.
    albedo: f32,
    // The colour a view gives it whole: its detail's, or built, planned or next.
    flat: vec3<f32>,
}

// What is built, in the plan view.
const BUILT: vec3<f32> = vec3<f32>(0.8, 0.36, 0.06);

// A light from up and over the shoulder, the same every time, so a shape reads whatever the
// sun is doing.
const LIGHT: vec3<f32> = vec3<f32>(0.42, 0.82, 0.38);

fn lit(colour: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    return colour * (0.45 + 0.55 * max(dot(n, normalize(LIGHT)), 0.0));
}

// Stripes across the screen, for a warning laid over a colour.
fn hatch(pixel: vec2<f32>) -> f32 {
    return step(0.5, fract((pixel.x + pixel.y) / 10.0));
}

// Texel density: the screen pixels a texel covers seen square on from a metre off, by this
// camera at this resolution, against the aim, a texel a pixel (D362): blue where a texel
// spreads over several pixels (too few texels, blurred up close), green near one, red
// where several texels share a pixel (more than a metre off needs); so a surface's colour
// is its texture's density, wherever the camera is. Hatched where, at the distance it is
// seen now, a texel is already spread over more than a pixel: blurred as it stands.
const TEXEL_RAMP: array<vec3<f32>, 5> = array<vec3<f32>, 5>(
    vec3<f32>(0.75, 0.06, 0.04),
    vec3<f32>(0.78, 0.42, 0.03),
    vec3<f32>(0.10, 0.62, 0.10),
    vec3<f32>(0.04, 0.50, 0.70),
    vec3<f32>(0.10, 0.14, 0.80),
);

fn texels(s: Seen) -> vec3<f32> {
    if s.side <= 0.0 {
        return lit(vec3(0.22) * (0.6 + 0.4 * s.albedo), s.geometric);
    }
    // Texels a metre, from the area a pixel covers on the texture and in the world.
    let texture_area = abs(s.uv_dx.x * s.uv_dy.y - s.uv_dx.y * s.uv_dy.x) * s.side * s.side;
    let world_area = length(cross(s.p_dx, s.p_dy));
    let per_metre = sqrt(texture_area / max(world_area, 1e-12));
    // Pixels a metre a metre off, square on: the screen's height over the view's height there.
    let pixels_a_metre = view.clip_from_view[1][1] * view.viewport.w * 0.5;
    let pixels_a_texel = pixels_a_metre / max(per_metre, 1e-3);
    // As a power of two against the aim: 0 four texels a pixel, 4 four pixels a texel.
    let e = clamp(log2(pixels_a_texel / max(s.aim, 1e-3)), -2.0, 2.0) + 2.0;
    let i = min(u32(floor(e)), 3u);
    var ramps = TEXEL_RAMP;
    let ramp = mix(ramps[i], ramps[i + 1u], select(fract(e), 1.0, e >= 4.0));
    var colour = lit(ramp * (0.65 + 0.35 * s.albedo), s.geometric);
    // Texels a pixel where it is seen now.
    let now = max(length(s.uv_dx), length(s.uv_dy)) * s.side;
    if now < 1.0 {
        colour *= 1.0 - 0.55 * hatch(s.pixel);
    }
    return colour;
}

// Damage: grey whole; its cube's health lost from yellow, a little, through orange to red,
// all but gone; white where the damage drawing breaks it (cracks, chips, splits, holes).
fn damage(s: Seen) -> vec3<f32> {
    var colour = vec3(0.3);
    if s.lost > 0.001 {
        let l = clamp(s.lost, 0.0, 1.0);
        colour = mix(vec3(0.8, 0.7, 0.05), vec3(0.8, 0.3, 0.02), smoothstep(0.0, 0.5, l));
        colour = mix(colour, vec3(0.75, 0.04, 0.04), smoothstep(0.5, 1.0, l));
    }
    colour = mix(colour, vec3(0.85), clamp(s.broken, 0.0, 1.0) * s.masked);
    return lit(colour, s.geometric);
}

// Occlusion and shade: how much light the surface keeps after everything that darkens it,
// white keeping all, black none; where one thing takes a tenth or more, tinted by the one
// that takes the most: red its texture's occlusion, green the screen's, blue its corners'
// colour (baked dirt and streaks, a piece's tint, a damaged cube darkened where its damage
// is not drawn). Unlit, so the sun plays no part.
const SHADE_TINTS: array<vec3<f32>, 3> = array<vec3<f32>, 3>(
    vec3<f32>(0.95, 0.3, 0.25),
    vec3<f32>(0.35, 0.9, 0.35),
    vec3<f32>(0.4, 0.5, 1.0),
);

fn shade(s: Seen) -> vec3<f32> {
    let occlusion = min(s.texture_occlusion, s.screen_occlusion);
    let kept = clamp(occlusion * s.baked, 0.0, 1.0);
    // Of the two occlusions only the less counts, as the lighting takes the least of them.
    let by_texture = select(0.0, 1.0 - s.texture_occlusion, s.texture_occlusion <= s.screen_occlusion);
    let by_screen = select(0.0, 1.0 - s.screen_occlusion, s.screen_occlusion < s.texture_occlusion);
    let by_corners = 1.0 - s.baked;
    var which = 0u;
    var most = by_texture;
    if by_screen > most {
        which = 1u;
        most = by_screen;
    }
    if by_corners > most {
        which = 2u;
        most = by_corners;
    }
    var tints = SHADE_TINTS;
    let strength = smoothstep(0.1, 0.5, most) * 0.8;
    return vec3(kept * 0.85) * mix(vec3(1.0), tints[which], strength);
}

// Where a point of the world falls on screen, in pixels.
fn on_screen(p: vec3<f32>) -> vec2<f32> {
    let clip = view.clip_from_world * vec4(p, 1.0);
    let ndc = clip.xy / max(clip.w, 1e-6);
    return view.viewport.xy + (ndc * vec2(0.5, -0.5) + 0.5) * view.viewport.zw;
}

// How far a pixel is from a line on screen, in pixels.
fn to_line(pixel: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let t = clamp(dot(pixel - a, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
    return length(pixel - (a + ab * t));
}

// Normals and tangents: the normal as its normal map makes it, in the world's axes (east
// red, up green, south blue, each half bright where it faces the other way), and on a
// lattice of its texture, eight to a copy, each place's frame drawn as ticks: its tangent
// red, its bitangent green and its normal blue, outlined in black.
fn normals(s: Seen) -> vec3<f32> {
    var colour = (s.mapped * 0.5 + 0.5) * 0.8;
    // Where the middle of this pixel's place on the lattice is in the world, from how the
    // texture and the world change across the pixel.
    let m = mat2x2<f32>(s.uv_dx, s.uv_dy);
    let det = determinant(m);
    if abs(det) < 1e-12 {
        return colour;
    }
    let inverse = mat2x2<f32>(vec2(m[1][1], -m[0][1]), vec2(-m[1][0], m[0][0])) * (1.0 / det);
    let cells = 8.0;
    let to_middle = (floor(s.uv * cells) + 0.5) / cells - s.uv;
    let steps = inverse * to_middle;
    let middle = s.position + s.p_dx * steps.x + s.p_dy * steps.y;
    // A tick a third of a place long, by how far a place reaches in the world.
    let per_uv = inverse * vec2(1.0 / cells, 0.0);
    let reach = length(s.p_dx * per_uv.x + s.p_dy * per_uv.y) * 0.35;
    let foot = on_screen(middle);
    var near = 1e6;
    var which = vec3(0.0);
    var ticks = array<vec3<f32>, 3>(s.tangent * s.framed, s.bitangent * s.framed, s.geometric);
    var colours = array<vec3<f32>, 3>(vec3(0.85, 0.05, 0.05), vec3(0.05, 0.75, 0.05), vec3(0.1, 0.25, 0.95));
    for (var k = 0u; k < 3u; k++) {
        if dot(ticks[k], ticks[k]) < 0.01 {
            continue;
        }
        let d = to_line(s.pixel, foot, on_screen(middle + normalize(ticks[k]) * reach));
        if d < near {
            near = d;
            which = colours[k];
        }
    }
    colour = mix(colour, vec3(0.0), 1.0 - smoothstep(1.6, 2.4, near));
    colour = mix(colour, which, 1.0 - smoothstep(0.6, 1.3, near));
    return colour;
}

// A surface's colour in a view.
fn shown(s: Seen) -> vec3<f32> {
    switch s.view {
        case TEXELS: {
            return texels(s);
        }
        case ROLES: {
            return lit(vec3(0.42), s.geometric);
        }
        case DAMAGE: {
            return damage(s);
        }
        case SHADE: {
            return shade(s);
        }
        case NORMALS: {
            return normals(s);
        }
        case DETAIL, PLAN: {
            return lit(s.flat, s.geometric);
        }
        default: {
            return vec3(0.8, 0.0, 0.8);
        }
    }
}
