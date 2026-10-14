// How the stand-in planet's ground looks. Save this file while the sandbox is running
// and the ground updates.
//
// The planet's centre is at the world origin, so a fragment's distance from the origin
// is its height, and the direction from the origin is "up".
//
// Each vertex carries what grows there in its colour: red is how much grass, green is how
// lush it is, blue is rock showing through where the ground has been dug (half for
// stone, all for ore), and alpha is how much of the sky it sees. The mesh decides that so that whatever is planted on the ground can agree
// with it; this only draws it, and roughens the edges.
//
// Covered ground is textured (D14): real soil, grass, rock and sand, layers of one texture
// array, each projected along the three axes and blended by which the surface faces
// (triplanar), so no seams or stretching follow the voxel mesh. Each is tinted by the
// planet's colour for it: its own average colour is swapped for the planet's, keeping its
// detail, so a planet still decides what colour its ground is.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::{
    prepass::io::{VertexOutput, FragmentOutput},
    deferred::functions::deferred_output,
};
@else
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::globals,
};

// Field order must match GroundExtension in planet.rs.
struct Ground {
    radius_m: f32,
    relief_m: f32,
    low: vec4<f32>,
    high: vec4<f32>,
    steep: vec4<f32>,
    faceted: u32,
    cover: f32,
    sea_radius_m: f32,
    grass_dry: vec4<f32>,
    grass_lush: vec4<f32>,
    sand: vec4<f32>,
    sea_deep: vec4<f32>,
    layer_metres: vec4<f32>,
    textured: u32,
    more_metres: vec4<f32>,
    // From a point in the world to its place on the cover map, and whether there is one.
    cover_from_world: mat4x4<f32>,
    cover_on: u32,
    // Each layer at one size, at the low quality preset.
    one_size: u32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> ground: Ground;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var ground_color: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var ground_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var ground_normal: texture_2d_array<f32>;
// What covers the ground near the camera, painted by the ground cover (cover/floor.rs):
// grass in red, how lush in green, forest floor in blue, moss in alpha.
@group(constants::MATERIAL_BIND_GROUP) @binding(104)
var cover_map: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(105)
var cover_sampler: sampler;
// And more of it: fallen needles in red, duff and roots in green, and how much canopy stands
// over in blue.
@group(constants::MATERIAL_BIND_GROUP) @binding(106)
var cover_more: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(107)
var cover_more_sampler: sampler;

// The layers of the texture array, in the order planet.rs gives them.
const SOIL: u32 = 0u;
const GRASS: u32 = 1u;
const ROCK: u32 = 2u;
const SAND: u32 = 3u;
const LITTER: u32 = 4u;
const NEEDLES: u32 = 5u;
const ROOTS: u32 = 6u;
const MOSS: u32 = 7u;

// How much more the layers' relief bends the light than their normal maps say: the floor's
// leaves, clods and needles are lit mostly by the sky under the canopy (D270), where a
// scanned normal map's measured relief reads faint.
const RELIEF: f32 = 1.5;
// How many hexagons of the hex tiling (below) a texture's repeat holds across.
const HEX_PER_REPEAT: f32 = 1.4;
// A layer sampled once, plainly, where its repeat does not show: small patches of it.
const PLAIN: f32 = -1.0;

// A layer as drawn: its colour, the surface's normal bent by its relief, and how high it
// stands, 0 to 1, for blending by height and darkening its hollows.
struct Layer {
    colour: vec3<f32>,
    normal: vec3<f32>,
    height: f32,
}

// One sample of a layer: its colour, its relief (the normal map's x and y) and its height,
// which the stack keeps in the normal map's blue (`textures.ts stack --height`).
struct Texel {
    colour: vec3<f32>,
    relief: vec2<f32>,
    height: f32,
}

fn fetch(uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>, index: u32) -> Texel {
    let colour = textureSampleGrad(ground_color, ground_sampler, uv, index, ddx, ddy).rgb;
    let normal = textureSampleGrad(ground_normal, ground_sampler, uv, index, ddx, ddy).xyz;
    return Texel(colour, normal.xy * 2.0 - 1.0, normal.z);
}

fn turned(v: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2<f32>(c * v.x - s * v.y, s * v.x + c * v.y);
}

fn hash2(p: vec2<f32>) -> vec2<f32> {
    let q = vec3<f32>(p.x, p.y, p.x + p.y);
    return vec2<f32>(hash(q), hash(q + 19.19));
}

// A layer sampled without its repeat showing: hex tiling, ported from Morten Mikkelsen's
// "Practical Real-Time Hex-Tiling" (JCGT 11(2), 2022; github.com/mmikk/hextile-demo, MIT
// licence, copyright (c) 2022 mmikk). The plane is cut into hexagons, each a copy of the
// texture moved and turned at random, and every point blends the three nearest copies,
// weighted toward the nearest and toward whichever stands highest there, so the seams fall
// between leaves and stones rather than across them. `turn` is how far a copy may be turned,
// from none to any way, or `PLAIN` for one plain sample, as every layer is at the low preset.
fn hex_tiled(uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>, index: u32, turn: f32) -> Texel {
    if ground.one_size != 0u || turn < 0.0 {
        return fetch(uv, ddx, ddy, index);
    }
    // The triangle grid the hexagons' middles stand on, skewed to a square one.
    let st = uv * HEX_PER_REPEAT * 2.0 * sqrt(3.0);
    let skewed = vec2<f32>(st.x - 0.57735027 * st.y, 1.15470054 * st.y);
    let base = floor(skewed);
    let f = fract(skewed);
    let z = 1.0 - f.x - f.y;
    let s = step(0.0, -z);
    let s2 = 2.0 * s - 1.0;
    let w = vec3<f32>(-z * s2, s - f.y * s2, s - f.x * s2);
    var corners = array<vec2<f32>, 3>(base + vec2<f32>(s, s), base + vec2<f32>(s, 1.0 - s), base + vec2<f32>(1.0 - s, s));
    var texels: array<Texel, 3>;
    var weights = vec3<f32>(0.0);
    for (var k = 0u; k < 3u; k++) {
        let corner = corners[k];
        let middle = vec2<f32>(corner.x + 0.5 * corner.y, corner.y / 1.15470054) / (2.0 * sqrt(3.0) * HEX_PER_REPEAT);
        let dice = hash2(corner);
        let angle = (dice.x - 0.5) * 6.2831853 * turn;
        let at = turned(uv - middle, angle) + middle + dice;
        var texel = fetch(at, turned(ddx, angle), turned(ddy, angle), index);
        // The relief was sampled turned; turned back.
        texel.relief = turned(texel.relief, -angle);
        texels[k] = texel;
    }
    // Mikkelsen's weights: each copy's own by how near its middle is, sharpened, and by how
    // high it stands (his luminance, our height).
    let lifted = mix(vec3<f32>(1.0), vec3<f32>(texels[0].height, texels[1].height, texels[2].height), 0.6);
    weights = lifted * pow(w, vec3<f32>(7.0));
    weights /= weights.x + weights.y + weights.z;
    return Texel(
        texels[0].colour * weights.x + texels[1].colour * weights.y + texels[2].colour * weights.z,
        texels[0].relief * weights.x + texels[1].relief * weights.y + texels[2].relief * weights.z,
        texels[0].height * weights.x + texels[1].height * weights.y + texels[2].height * weights.z,
    );
}

// Where a surface faces along each axis, how much each axis's projection counts: sharply
// favouring the nearest, so the blend between them is narrow.
fn facing(n: vec3<f32>) -> vec3<f32> {
    let w = pow(abs(n), vec3<f32>(4.0));
    return w / (w.x + w.y + w.z);
}

// One layer, projected along each axis the surface faces and blended, its colour with its
// average swapped for `tint`, and the surface's normal bent by its relief. The position's
// gradients are given so that this can be called where only some pixels need it. `turn` is
// how far its hex tiles may be turned.
fn layer(index: u32, scale: f32, tint: vec3<f32>, world: vec3<f32>, gx: vec3<f32>, gy: vec3<f32>, n: vec3<f32>, turn: f32) -> Layer {
    var metres = 1.0;
    if index < 4u {
        metres = ground.layer_metres[index];
    } else {
        metres = ground.more_metres[index - 4u];
    }
    let k = 1.0 / (metres * scale);
    let p = world * k;
    let dx = gx * k;
    let dy = gy * k;
    let w = facing(n);
    let side = sign(n);
    var colour = vec3<f32>(0.0);
    var height = 0.0;
    var tx = vec2<f32>(0.0);
    var ty = vec2<f32>(0.0);
    var tz = vec2<f32>(0.0);
    // Each projection mirrored on the far side, so the relief does not turn inside out there.
    if w.x > 0.01 {
        let t = hex_tiled(vec2<f32>(p.z * side.x, p.y), vec2<f32>(dx.z * side.x, dx.y), vec2<f32>(dy.z * side.x, dy.y), index, turn);
        colour += t.colour * w.x;
        height += t.height * w.x;
        tx = t.relief * RELIEF;
        tx.x *= side.x;
    }
    if w.y > 0.01 {
        let t = hex_tiled(vec2<f32>(p.x * side.y, p.z), vec2<f32>(dx.x * side.y, dx.z), vec2<f32>(dy.x * side.y, dy.z), index, turn);
        colour += t.colour * w.y;
        height += t.height * w.y;
        ty = t.relief * RELIEF;
        ty.x *= side.y;
    }
    if w.z > 0.01 {
        let t = hex_tiled(vec2<f32>(-p.x * side.z, p.y), vec2<f32>(-dx.x * side.z, dx.y), vec2<f32>(-dy.x * side.z, dy.y), index, turn);
        colour += t.colour * w.z;
        height += t.height * w.z;
        tz = t.relief * RELIEF;
        tz.x *= -side.z;
    }
    // Each projection's relief added to the surface as seen along its axis (the "UDN"
    // blend), then turned back to the world's axes.
    let nx = vec3<f32>(n.x, tx.y + n.y, tx.x + n.z);
    let ny = vec3<f32>(ty.x + n.x, n.y, ty.y + n.z);
    let nz = vec3<f32>(tz.x + n.x, tz.y + n.y, n.z);
    let normal = normalize(nx * w.x + ny * w.y + nz * w.z);
    // The smallest copy of the texture is its average colour.
    let average = textureSampleLevel(ground_color, ground_sampler, vec2<f32>(0.5), index, 16.0).rgb;
    return Layer(colour * tint / max(average, vec3<f32>(0.02)), normal, height);
}

// `under` with `top` laid on it, `amount` of the way, colour and relief alike.
fn over(under: Layer, top: Layer, amount: f32) -> Layer {
    return Layer(
        mix(under.colour, top.colour, amount),
        normalize(mix(under.normal, top.normal, amount)),
        mix(under.height, top.height, amount),
    );
}

// `top` laid on `under` by height, as terrain splatting does in shipped games (Andrey
// Mishkinis, "Advanced Terrain Texture Splatting", 2013): as `amount` rises the top shows
// first where it stands higher than what is under it, so leaves cover soil leaf by leaf and
// needles fill the hollows between them, rather than one fading into the other.
fn settle(under: Layer, top: Layer, amount: f32) -> Layer {
    if amount <= 0.0 {
        return under;
    }
    if amount >= 1.0 {
        return top;
    }
    let a = under.height + 1.0 - amount;
    let b = top.height + amount;
    let edge = max(a, b) - 0.2;
    let wa = max(a - edge, 0.0);
    let wb = max(b - edge, 0.0);
    return over(under, top, wb / (wa + wb));
}

fn hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

// How much of a canopy stands over a place, as the cover map says.
fn canopy_over(world: vec3<f32>) -> f32 {
    if ground.cover_on == 0u {
        return 0.0;
    }
    let at = (ground.cover_from_world * vec4<f32>(world, 1.0)).xy;
    let inside = clamp(min(min(at.x, at.y), min(1.0 - at.x, 1.0 - at.y)) * 20.0, 0.0, 1.0);
    return textureSampleLevel(cover_more, cover_more_sampler, at, 0.0).b * inside * 0.85;
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

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let world = in.world_position.xyz;
    let from_centre = length(world);
    let up = world / from_centre;
    // 0 at the bottom of the deepest valley, 1 at the top of the tallest hill.
    let height = clamp((from_centre - ground.radius_m) / ground.relief_m * 0.5 + 0.5, 0.0, 1.0);
    // 1 on level ground, falling toward 0 on a cliff.
    let flatness = dot(normalize(in.world_normal), up);

    var colour = mix(ground.low.rgb, ground.high.rgb, smoothstep(0.25, 0.75, height));
    // Rock shows through from about 30 degrees, as soil and grass hold on gentler slopes.
    let steep = 1.0 - smoothstep(0.8, 0.9, flatness);

@if(!PREPASS_PIPELINE)
    if ground.cover > 0.0 && ground.textured != 0u {
        // The same patches as the flat ground's, below, in textured layers.
        let blotch = noise(world * 0.23) * 0.6 + noise(world * 0.9 + 17.0) * 0.4;
        let fleck = noise(world * 3.1 + 5.0);
        // Broad swells of lighter and darker ground, tens of metres across, so a floor of one
        // layer is not one tone.
        let swell = noise(world * 0.045 + 3.0);
@if(VERTEX_COLORS)
        var grows = in.color;
@else
        var grows = vec4<f32>(0.0);
        // Near the camera the cover says what grows, as its plants do: grass gives out
        // under the trees to leaf litter, and moss grows round their feet. It fades back to
        // the mesh's own answer at the map's edge.
        var litter = 0.0;
        var moss = 0.0;
        var blades = 0.0;
        var needles = 0.0;
        var roots = 0.0;
        if ground.cover_on != 0u {
            // The map is half a metre a texel: looked up a little way off, by noise, so what
            // it says frays and wanders rather than ending in smooth blobs.
            let warp = vec3<f32>(noise(world * 0.7 + 41.0), 0.0, noise(world * 0.7 + 83.0)) - vec3<f32>(0.5, 0.0, 0.5);
            let fine = vec3<f32>(noise(world * 2.3 + 7.0), 0.0, noise(world * 2.3 + 13.0)) - vec3<f32>(0.5, 0.0, 0.5);
            let at = (ground.cover_from_world * vec4<f32>(world + warp * 1.4 + fine * 0.4, 1.0)).xy;
            let inside = clamp(min(min(at.x, at.y), min(1.0 - at.x, 1.0 - at.y)) * 20.0, 0.0, 1.0);
            let map = textureSampleLevel(cover_map, cover_sampler, at, 0.0);
            grows.r = mix(grows.r, min(grows.r, map.r), inside);
            grows.g = mix(grows.g, map.g, inside);
            litter = map.b * inside;
            moss = map.a * inside;
            blades = map.r * inside;
            let more = textureSampleLevel(cover_more, cover_more_sampler, at, 0.0);
            needles = more.r * inside;
            roots = more.g * inside;
        }
        let n = normalize(in.world_normal);
        let gx = dpdx(world);
        let gy = dpdy(world);
        let flat_ground = 1.0 - steep;
        // Under broadleaves the floor is their fallen leaves, brown with paler and darker
        // ones; under conifers, needles, a warmer red-brown.
        let litter_colour = vec3<f32>(0.1, 0.072, 0.044);
        let needle_colour = vec3<f32>(0.074, 0.056, 0.04);
        let leaves = smoothstep(0.2, 0.75, litter + (blotch - 0.5) * 0.45) * flat_ground;
        let needly = smoothstep(0.25, 0.7, needles + (blotch - 0.5) * 0.4) * flat_ground;
        // Soil, and rock on steep ground, where the floor's litter does not cover it all.
        var ground_is = Layer(colour, n, 0.5);
        if max(leaves, needly) < 0.999 {
            let soil_tint = mix(colour, mix(litter_colour, needle_colour, needles), max(litter, needles)) * (0.9 + 0.2 * blotch);
            ground_is = layer(SOIL, 1.0, soil_tint, world, gx, gy, n, 1.0);
            if steep > 0.0 {
                let rock_tint = ground.steep.rgb * (0.85 + 0.3 * blotch);
                ground_is = over(ground_is, layer(ROCK, 1.0, rock_tint, world + 37.0, gx, gy, n, 0.15), steep);
            }
        }
        // Fallen leaves, laid on the soil leaf by leaf as they thicken.
        if leaves > 0.0 {
            let leaf_tint = litter_colour * (0.85 + 0.3 * fleck);
            var fallen = layer(LITTER, 1.0, leaf_tint, world + 41.0, gx, gy, n, 1.0);
            // The scan's reds and yellows toned down toward the tint, keeping its light and dark.
            let light = dot(fallen.colour, vec3<f32>(0.333)) / dot(leaf_tint, vec3<f32>(0.333));
            fallen.colour = mix(leaf_tint * light, fallen.colour, 0.5);
            ground_is = settle(ground_is, fallen, leaves);
        }
        // Under pines and spruces, their needles, drifting into the leaves at the edges.
        if needly > 0.0 {
            let needle_tint = needle_colour * (0.85 + 0.3 * fleck);
            ground_is = settle(ground_is, layer(NEEDLES, 1.0, needle_tint, world + 29.0, gx, gy, n, 1.0), needly);
        }
        // Round a tree's foot and along a path's edge, dark duff and the roots in it, in
        // broken patches.
        let rooty = smoothstep(0.3, 0.75, roots + (fleck - 0.5) * 0.35 + (blotch - 0.5) * 0.3) * flat_ground;
        if rooty > 0.0 {
            let duff = vec3<f32>(0.08, 0.058, 0.04) * (0.8 + 0.4 * blotch);
            ground_is = over(ground_is, layer(ROOTS, 1.0, duff, world + 3.0, gx, gy, n, PLAIN), rooty);
        }

        // Grass sits in the hollows first, and gives out where the floor stands higher.
        let grass = smoothstep(0.38, 0.62, grows.r + (blotch - 0.5) * 0.5) * ground.cover * flat_ground;
        if grass > 0.0 {
            let lush = smoothstep(0.2, 0.8, grows.g + (blotch - 0.5) * 0.45);
            let tint = mix(ground.grass_dry.rgb, ground.grass_lush.rgb, lush) * (0.92 + 0.16 * fleck);
            ground_is = settle(ground_is, layer(GRASS, 1.0, tint, world + 11.0, gx, gy, n, 1.0), grass);
        }

        // Moss in cushions, over the litter's hollows and up onto it as it thickens.
        let mossy = smoothstep(0.25, 0.75, moss + (blotch - 0.5) * 0.5) * flat_ground;
        if mossy > 0.0 {
            let moss_colour = vec3<f32>(0.056, 0.08, 0.026) * (0.8 + 0.4 * fleck);
            ground_is = settle(ground_is, layer(MOSS, 1.0, moss_colour, world + 5.0, gx, gy, n, PLAIN), mossy);
        }

        // The ground under thick grass is in the blades' own shade, so what shows between
        // them is dark, as their roots are, rather than a lawn.
        ground_is.colour *= 1.0 - 0.6 * smoothstep(0.1, 0.7, blades);
        ground_is.colour *= 0.86 + 0.28 * swell;

        let rock = smoothstep(0.2, 0.4, grows.b);
        if rock > 0.0 {
            let ore = smoothstep(0.7, 0.9, grows.b);
            // Linear colours, as the renderer works: a mid grey-brown stone, a rusty ore.
            let stone_colour = vec3<f32>(0.12, 0.1, 0.08) * (0.85 + 0.3 * blotch);
            let ore_colour = vec3<f32>(0.24, 0.08, 0.04) * (0.8 + 0.4 * fleck);
            ground_is = over(ground_is, layer(ROCK, 0.6, mix(stone_colour, ore_colour, ore), world, gx, gy, n, PLAIN), rock);
        }

        let above_sea = from_centre - ground.sea_radius_m;
        let lap = sin(globals.time * 0.9 + noise(world * 0.12) * 12.0) * 0.06;
        // The shore is the sea's: ground dug down to its level inland is not a beach.
        let undug = 1.0 - smoothstep(0.1, 0.3, grows.b);
        let sandy = (1.0 - smoothstep(0.35, 1.5, above_sea + (blotch - 0.5) * 0.9)) * ground.cover * undug;
        if sandy > 0.0 {
            ground_is = over(ground_is, layer(SAND, 1.0, ground.sand.rgb, world, gx, gy, n, PLAIN), sandy);
        }
        // Hollows between leaves, clods and needles are darker and see less of the sky, which
        // is most of the light under a canopy (D270), so the floor's relief reads in shade.
        let hollow = smoothstep(0.0, 0.6, ground_is.height);
        colour = ground_is.colour * mix(0.8, 1.0, hollow);
        pbr_input.diffuse_occlusion *= vec3<f32>(mix(0.6, 1.0, hollow));
        colour *= mix(0.72, 1.0, smoothstep(0.05, 0.3, above_sea - lap) * undug + (1.0 - undug));
        colour = mix(colour, ground.sea_deep.rgb, smoothstep(0.0, 4.5, -above_sea) * 0.92 * undug);
        let foam_width = 0.07 + 0.05 * noise(world * 1.7 + globals.time * 0.25);
        let foam = 1.0 - smoothstep(0.0, foam_width, abs(above_sea - lap - 0.02));
        colour = mix(colour, vec3<f32>(1.0, 0.99, 0.96), foam * 0.85 * ground.cover * undug);
        pbr_input.N = ground_is.normal;
    } else {
        colour = mix(colour, ground.steep.rgb, steep);
    }
@if(!PREPASS_PIPELINE)
    if ground.cover > 0.0 && ground.textured == 0u {
        // Blotches a few metres across and smaller, to break up every edge and tone.
        let blotch = noise(world * 0.23) * 0.6 + noise(world * 0.9 + 17.0) * 0.4;
        let fleck = noise(world * 3.1 + 5.0);
@if(VERTEX_COLORS)
        let grows = in.color;
@else
        let grows = vec4<f32>(0.0);
        // Soil shows in soft-edged patches, a little darker where it is bare earth.
        colour *= 0.9 + 0.2 * blotch;
        let grass = smoothstep(0.38, 0.62, grows.r + (blotch - 0.5) * 0.5) * ground.cover;
        let lush = smoothstep(0.2, 0.8, grows.g + (blotch - 0.5) * 0.45);
        var grass_colour = mix(ground.grass_dry.rgb, ground.grass_lush.rgb, lush);
        grass_colour *= 0.92 + 0.16 * fleck;
        colour = mix(colour, grass_colour, grass);

        // Dug ground shows what it is made of: grey stone, and ore rusty in it.
        let rock = smoothstep(0.2, 0.4, grows.b);
        let ore = smoothstep(0.7, 0.9, grows.b);
        // Warm, so a pit lit only by the blue sky reads as stone and not as water.
        let stone_colour = vec3<f32>(0.12, 0.1, 0.08) * (0.85 + 0.3 * blotch);
        let ore_colour = vec3<f32>(0.24, 0.08, 0.04) * (0.8 + 0.4 * fleck);
        colour = mix(colour, mix(stone_colour, ore_colour, ore), rock);

        // The shore: sand above the waterline, darker where it is wet, foam where the
        // water meets it, and the bed fading into the deep below.
        let above_sea = from_centre - ground.sea_radius_m;
        let lap = sin(globals.time * 0.9 + noise(world * 0.12) * 12.0) * 0.06;
        let sandy = (1.0 - smoothstep(0.35, 1.5, above_sea + (blotch - 0.5) * 0.9)) * ground.cover;
        colour = mix(colour, ground.sand.rgb * (0.94 + 0.12 * fleck), sandy);
        colour *= mix(0.72, 1.0, smoothstep(0.05, 0.3, above_sea - lap));
        colour = mix(colour, ground.sea_deep.rgb, smoothstep(0.0, 4.5, -above_sea) * 0.92);
        let foam_width = 0.07 + 0.05 * noise(world * 1.7 + globals.time * 0.25);
        let foam = 1.0 - smoothstep(0.0, foam_width, abs(above_sea - lap - 0.02));
        colour = mix(colour, vec3<f32>(1.0, 0.99, 0.96), foam * 0.85 * ground.cover);
    }
    pbr_input.material.base_color = vec4<f32>(colour, 1.0);
    // The vertex's alpha is how much of the sky it sees: the sky's light is dimmed by it,
    // so the bottom of a dug hole is dark rather than lit as open ground.
@if(VERTEX_COLORS)
    pbr_input.diffuse_occlusion *= vec3<f32>(in.color.a);
    // Under the trees the canopy hides most of the sky: its diffuse light is the sky
    // occlusion volume's (D270), which Bevy's lighting reads; the sky it reflects, which
    // that does not hold, is dimmed here, where the cover map says the trees stand over it.
    pbr_input.specular_occlusion *= 1.0 - 0.6 * canopy_over(world);

    // Faceted: light each triangle as one flat plane. The plane's direction comes from
    // how the surface position changes between neighbouring pixels.
    if ground.faceted != 0u {
        var facet = normalize(cross(dpdy(in.world_position.xyz), dpdx(in.world_position.xyz)));
        if dot(facet, in.world_normal) < 0.0 {
            facet = -facet;
        }
        pbr_input.world_normal = facet;
        pbr_input.N = facet;
    }

    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );

    var out: FragmentOutput;
@if(PREPASS_PIPELINE) {
    out = deferred_output(in, pbr_input);
} @else {
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
}
    return out;
}
