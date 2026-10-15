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
    mesh_view_bindings::{globals, view, lights},
    shadows::fetch_directional_shadow,
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
    // How many metres the ninth layer on, scree, gravel and snow, each cover.
    mountain_metres: vec4<f32>,
    // From a point in the world to its place on the cover map, and whether there is one.
    cover_from_world: mat4x4<f32>,
    cover_on: u32,
    // The same for the climate's maps (climate.rs, D327), which painting of them it is, and
    // the leaves map's layer for the broadleaves whose leaves fall on the floor, or -1.
    climate_from_world: mat4x4<f32>,
    climate_on: u32,
    climate_painting: u32,
    fallen_layer: i32,
    // The heath past the trees round the land's middle (planet.rs `LandMap`): the middle's
    // direction and the planet's radius, east and the map's width, north and whether there is
    // a map; the leaves map's layers of the heath's dwarf shrubs and of grass, and the shrubs'
    // autumn colour.
    land_middle: vec4<f32>,
    land_east: vec4<f32>,
    land_north: vec4<f32>,
    heath_layer: i32,
    grass_layer: i32,
    heath_autumn: vec4<f32>,
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
// And more of it: fallen needles in red, duff and roots in green, how much canopy stands
// over in blue, and how trodden the ground is in alpha (a settlement's ways).
@group(constants::MATERIAL_BIND_GROUP) @binding(106)
var cover_more: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(107)
var cover_more_sampler: sampler;
// What the climate leaves round the camera, a 32 m cell a texel (climate.rs): snow's depth in
// red, its brightest SNOW_FULL_M; the ground's wetness in green; rain falling now in blue;
// frost in alpha. And each kind's leaves, a layer a kind: how many it has early and late in
// its year in red and green, how far turned in blue and alpha.
@group(constants::MATERIAL_BIND_GROUP) @binding(108)
var climate_ground: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(109)
var climate_ground_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(110)
var climate_leaves: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(111)
var climate_leaves_sampler: sampler;
// The heath past the trees (terra_land::heath), a 10 m cell a texel over the land round its
// middle: lichen, dwarf shrubs, sedge and moss in the first layer; the fellfield's stones,
// the wetness and how open the ground is to the heath in the second.
@group(constants::MATERIAL_BIND_GROUP) @binding(112)
var land_map: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(113)
var land_sampler: sampler;

// Snow this deep is the climate map's brightest (`climate::SNOW_FULL_M`).
const SNOW_FULL_M: f32 = 0.5;
// Lying snow, linear: a little short of fresh snow's albedo of 0.9, settled and dusted with
// what falls from the trees.
const SNOW: vec3<f32> = vec3<f32>(0.74, 0.76, 0.8);
// Fresh fallen oak leaves, linear: a warm mid brown, lighter and redder than the floor's
// old umber litter.
const FRESH_LITTER: vec3<f32> = vec3<f32>(0.13, 0.072, 0.03);
// A settlement's trodden ways, linear: packed earth, grey-brown and dusty, as a market town's
// streets are in Kingdom Come's and The Witcher's frames, not the planet's warm soil; and the
// straw dropped about their edges.
const TRODDEN: vec3<f32> = vec3<f32>(0.15, 0.132, 0.108);
const STRAW: vec3<f32> = vec3<f32>(0.34, 0.27, 0.13);
// Rime on the floor in a hard frost, linear.
const RIME: vec3<f32> = vec3<f32>(0.5, 0.53, 0.57);

// The layers of the texture array, in the order planet.rs gives them.
const SOIL: u32 = 0u;
const GRASS: u32 = 1u;
const ROCK: u32 = 2u;
const SAND: u32 = 3u;
const LITTER: u32 = 4u;
const NEEDLES: u32 = 5u;
const ROOTS: u32 = 6u;
const MOSS: u32 = 7u;
const SCREE: u32 = 8u;
const GRAVEL: u32 = 9u;
const SNOW_LAYER: u32 = 10u;

// How wet the ground is at so many metres above a lake's water (D322): soaked at the waterline
// and drying over a hand or two above it, its edge broken by `fleck`.
fn wet_band(above: f32, fleck: f32) -> f32 {
    return (1.0 - smoothstep(0.0, 0.3 + 0.25 * fleck, above)) * step(-0.3, above);
}

// The ground by a lake (D322): darker where wet above the water, as wet stone and soil are
// (Lagarde: wet ground darkens and smooths the dry, its lighting the same), and under it
// darkened toward the water's colour with depth, the bed fading into the deep. `above` is how
// far over the nearest lake's water the ground stands, below 0 under it.
fn lake_shore(colour: vec3<f32>, above: f32, fleck: f32) -> vec3<f32> {
    let wet = wet_band(above, fleck);
    let under = smoothstep(0.0, 5.0, -above);
    let damp = colour * mix(1.0, 0.55, wet);
    return mix(damp, vec3<f32>(0.012, 0.03, 0.026), under * 0.75);
}

// How much more the layers' relief bends the light than their normal maps say: the floor's
// leaves, clods and needles are lit mostly by the sky under the canopy (D270), where a
// scanned normal map's measured relief reads faint.
const RELIEF: f32 = 1.5;
// How many hexagons of the hex tiling (below) a texture's repeat holds across.
const HEX_PER_REPEAT: f32 = 1.4;
// How far the soil's own light and dark are drawn out (D294).
const SOIL_CONTRAST: f32 = 1.7;
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
// from none to any way, or `PLAIN` for one plain sample.
fn hex_tiled(uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>, index: u32, turn: f32) -> Texel {
    if turn < 0.0 {
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
    } else if index < 8u {
        metres = ground.more_metres[index - 4u];
    } else {
        metres = ground.mountain_metres[index - 8u];
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

// The climate map round the camera at a point. Past its edge, the edge's answer: the
// climate changes slowly, over hundreds of metres.
fn climate_at(world: vec3<f32>) -> vec4<f32> {
    if ground.climate_on == 0u {
        return vec4<f32>(0.0);
    }
    let at = clamp((ground.climate_from_world * vec4<f32>(world, 1.0)).xy, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSampleLevel(climate_ground, climate_ground_sampler, at, 0.0);
}

// How far inside the climate map a point is: 1 well inside, falling to 0 at its edge.
fn climate_inside(world: vec3<f32>) -> f32 {
    let at = (ground.climate_from_world * vec4<f32>(world, 1.0)).xy;
    return clamp(min(min(at.x, at.y), min(1.0 - at.x, 1.0 - at.y)) * 10.0, 0.0, 1.0);
}

// How much of the broadleaves' leaves have fallen where a point is, 0 to 1.
fn fallen_leaves(world: vec3<f32>) -> f32 {
    if ground.climate_on == 0u || ground.fallen_layer < 0 {
        return 0.0;
    }
    let at = clamp((ground.climate_from_world * vec4<f32>(world, 1.0)).xy, vec2<f32>(0.0), vec2<f32>(1.0));
    let leaves = textureSampleLevel(climate_leaves, climate_leaves_sampler, at, ground.fallen_layer, 0.0);
    return 1.0 - 0.5 * (leaves.r + leaves.g);
}

// The heath at a point: the land map's two layers there, none outside it, fading at its edge.
// The map is a plane touching the planet at its middle, a point on it where the line from the
// centre meets it, as `terra_land::region` lays its grid.
struct HeathHere {
    // Lichen, dwarf shrubs, sedge, moss.
    plants: vec4<f32>,
    // The fellfield's stones, the wetness, how open the ground is to the heath.
    ground: vec4<f32>,
}

fn heath_here(world: vec3<f32>) -> HeathHere {
    var none = HeathHere(vec4<f32>(0.0), vec4<f32>(0.0));
    if ground.land_north.w < 0.5 {
        return none;
    }
    // Looked up a little way off, by noise at two scales, so the map's 10 m cells never show
    // as squares and the communities' edges wander as a real mosaic's do.
    let warp = (vec3<f32>(noise(world * 0.07 + 211.0), noise(world * 0.07 + 223.0), noise(world * 0.07 + 227.0)) - vec3<f32>(0.5)) * 16.0
        + (vec3<f32>(noise(world * 0.35 + 5.0), noise(world * 0.35 + 9.0), noise(world * 0.35 + 15.0)) - vec3<f32>(0.5)) * 4.0;
    let at = world + warp;
    let along = dot(at, ground.land_middle.xyz);
    if along <= 0.0 {
        return none;
    }
    let scale = ground.land_middle.w / along;
    let size = ground.land_east.w;
    let uv = vec2<f32>(
        dot(at, ground.land_east.xyz) * scale / size + 0.5,
        0.5 - dot(at, ground.land_north.xyz) * scale / size,
    );
    let inside = clamp(min(min(uv.x, uv.y), min(1.0 - uv.x, 1.0 - uv.y)) * 30.0, 0.0, 1.0);
    if inside <= 0.0 {
        return none;
    }
    return HeathHere(
        textureSampleLevel(land_map, land_sampler, uv, 0, 0.0) * inside,
        textureSampleLevel(land_map, land_sampler, uv, 1, 0.0) * inside,
    );
}

// How far a kind's leaves have turned near a point, from the climate's leaves map, 0 green to
// 1 its autumn colour; 0 with no map.
fn kind_turned(world: vec3<f32>, layer: i32) -> f32 {
    if ground.climate_on == 0u || layer < 0 {
        return 0.0;
    }
    let at = clamp((ground.climate_from_world * vec4<f32>(world, 1.0)).xy, vec2<f32>(0.0), vec2<f32>(1.0));
    let leaves = textureSampleLevel(climate_leaves, climate_leaves_sampler, at, layer, 0.0);
    return 0.5 * (leaves.b + leaves.a);
}

// The heath's colours, linear, measured off photographs of tundra in late summer and early
// autumn (research/2026-10-15-tundra-reference.md): pale Cladonia lichen; dwarf birch and
// crowberry green in summer; sedge green, and straw once turned; Sphagnum's yellow green;
// the fellfield's grey stones.
const LICHEN: vec3<f32> = vec3<f32>(0.4, 0.39, 0.27);
const SHRUB_GREEN: vec3<f32> = vec3<f32>(0.045, 0.06, 0.022);
const SEDGE_GREEN: vec3<f32> = vec3<f32>(0.1, 0.11, 0.045);
const SEDGE_STRAW: vec3<f32> = vec3<f32>(0.4, 0.31, 0.12);
const SPHAGNUM: vec3<f32> = vec3<f32>(0.16, 0.15, 0.04);
const FELL_STONE: vec3<f32> = vec3<f32>(0.17, 0.165, 0.15);

// The heath's colour as a flat tint, for the low preset and far off: its communities' colours
// by their shares.
fn heath_flat(h: HeathHere, shrub_colour: vec3<f32>, sedge_colour: vec3<f32>) -> vec3<f32> {
    let total = max(h.plants.x + h.plants.y + h.plants.z + h.plants.w + h.ground.x, 0.001);
    return (LICHEN * h.plants.x + shrub_colour * h.plants.y + sedge_colour * h.plants.z
        + SPHAGNUM * h.plants.w + FELL_STONE * h.ground.x) / total;
}

// Snow lying on ground facing up (climate.md, D327): a dusting shows the floor through in
// patches, its edge broken by noise, and some ten centimetres covers it; less under the
// crowns, which catch what falls. `up_facing` is how level the surface is.
fn snow_on(depth_m: f32, world: vec3<f32>, up_facing: f32, under: f32) -> f32 {
    let lying = depth_m * (1.0 - 0.6 * under);
    let edge = noise(world * 0.35 + 91.0) * 0.6 + noise(world * 1.9 + 23.0) * 0.4;
    let cover = smoothstep(0.0, 1.0, (lying - 0.004) / 0.09 + (edge - 0.5) * 0.9);
    return cover * smoothstep(0.55, 0.85, up_facing) * step(0.001, lying);
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

// How much of a fragment's colour its hollows keep in the sun (D294): 1 out of the sun, and
// less in the hollows the more of its light is the sun's.
@if(!PREPASS_PIPELINE)
fn micro_shadow(world: vec4<f32>, n: vec3<f32>, relief: vec3<f32>, height: f32, frag: vec4<f32>) -> f32 {
    if lights.n_directional_lights == 0u {
        return 1.0;
    }
    let L = lights.directional_lights[0].direction_to_light;
    let view_z = dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), world);
    let sunlit = fetch_directional_shadow(0u, world, n, view_z, frag.xy) * clamp(dot(n, L) * 4.0, 0.0, 1.0);
    let cavity = smoothstep(0.2, 0.7, height);
    let micro = clamp(abs(dot(relief, L)) + 2.0 * cavity * cavity - 1.0, 0.0, 1.0);
    return mix(1.0, micro, sunlit * SUN_SHARE);
}

// How much of the light on sunlit ground is the sun's, so how much of it the hollows lose.
const SUN_SHARE: f32 = 0.85;

// The low preset's ground (D307): what shipped games do on an integrated GPU. The layers'
// shares are worked out first, as a splat map holds them, and only the two that show most
// are sampled, blended by height (a "top two" splat, as Far Cry 5's and most open worlds'
// terrain keep a few layers a texel rather than blending all); each is projected along
// the one or two axes the surface faces most (biplanar mapping, Inigo Quilez, "Biplanar
// mapping", 2020) rather than three; and past LOW_TEXTURED_M the layers are drawn as their
// average colours, flat, which is what their smallest mips show there anyway.

// Textures fade to flat colour between these distances from the camera.
@if(GROUND_LOW)
const LOW_TEXTURED_M: vec2<f32> = vec2<f32>(45.0, 75.0);

// One projection of a layer: its colour, height and the surface's normal bent by its relief,
// along axis 0 (x), 1 (y) or 2 (z), as `layer` lays each.
@if(GROUND_LOW)
fn projected(axis: u32, index: u32, p: vec3<f32>, dx: vec3<f32>, dy: vec3<f32>, n: vec3<f32>) -> Layer {
    let side = select(vec3<f32>(-1.0), vec3<f32>(1.0), n >= vec3<f32>(0.0));
    if axis == 0u {
        let t = fetch(vec2<f32>(p.z * side.x, p.y), vec2<f32>(dx.z * side.x, dx.y), vec2<f32>(dy.z * side.x, dy.y), index);
        let r = vec2<f32>(t.relief.x * side.x, t.relief.y) * RELIEF;
        return Layer(t.colour, vec3<f32>(n.x, r.y + n.y, r.x + n.z), t.height);
    }
    if axis == 1u {
        let t = fetch(vec2<f32>(p.x * side.y, p.z), vec2<f32>(dx.x * side.y, dx.z), vec2<f32>(dy.x * side.y, dy.z), index);
        let r = vec2<f32>(t.relief.x * side.y, t.relief.y) * RELIEF;
        return Layer(t.colour, vec3<f32>(r.x + n.x, n.y, r.y + n.z), t.height);
    }
    let t = fetch(vec2<f32>(-p.x * side.z, p.y), vec2<f32>(-dx.x * side.z, dx.y), vec2<f32>(-dy.x * side.z, dy.y), index);
    let r = vec2<f32>(-t.relief.x * side.z, t.relief.y) * RELIEF;
    return Layer(t.colour, vec3<f32>(r.x + n.x, r.y + n.y, n.z), t.height);
}

// A layer, biplanar: along the axis the surface faces most, and along the next only where
// the surface turns toward it, within about 35 degrees of the diagonal between them, so
// most of the ground takes one projection and none takes three.
@if(GROUND_LOW)
fn biplanar(index: u32, scale: f32, tint: vec3<f32>, world: vec3<f32>, gx: vec3<f32>, gy: vec3<f32>, n: vec3<f32>) -> Layer {
    var metres = 1.0;
    if index < 4u {
        metres = ground.layer_metres[index];
    } else if index < 8u {
        metres = ground.more_metres[index - 4u];
    } else {
        metres = ground.mountain_metres[index - 8u];
    }
    let k = 1.0 / (metres * scale);
    let a = abs(n);
    var major = 2u;
    var next = select(1u, 0u, a.x > a.y);
    if a.x >= a.y && a.x >= a.z {
        major = 0u;
        next = select(2u, 1u, a.y > a.z);
    } else if a.y >= a.z {
        major = 1u;
        next = select(2u, 0u, a.x > a.z);
    }
    // Quilez's weights: each axis's share past the diagonal, 0.5773.
    let w_major = clamp((a[major] - 0.5773) / 0.4227, 0.0, 1.0);
    let w_next = clamp((a[next] - 0.5773) / 0.4227, 0.0, 1.0);
    var drawn = projected(major, index, world * k, gx * k, gy * k, n);
    if w_next > 0.0 {
        let other = projected(next, index, world * k, gx * k, gy * k, n);
        let share = w_next / max(w_major + w_next, 1.0e-4);
        drawn = Layer(mix(drawn.colour, other.colour, share), mix(drawn.normal, other.normal, share), mix(drawn.height, other.height, share));
    }
    let average = textureSampleLevel(ground_color, ground_sampler, vec2<f32>(0.5), index, 16.0).rgb;
    return Layer(drawn.colour * tint / max(average, vec3<f32>(0.02)), normalize(drawn.normal), drawn.height);
}

// The layers the low preset may draw, in the order they lie, bottom first, as the high
// preset lays them one over another.
@if(GROUND_LOW)
const LOW_LAYERS: u32 = 12u;

// The floor at the low preset: each layer's share where it shows, then the two that show
// most sampled and settled by height, near the camera; flat colours farther off. `amounts`
// are how much of each lies over what is under it, and `tints` its colour.
@if(GROUND_LOW)
fn low_floor(amounts: array<f32, 12>, tint_of: array<vec3<f32>, 12>, world: vec3<f32>, n: vec3<f32>) -> Layer {
    var tints = tint_of;
    let gx = dpdx(world);
    let gy = dpdy(world);
    // What shows of each, from the top down: its own amount of what the layers over it leave.
    var shares: array<f32, 12>;
    var left = 1.0;
    for (var k = 0u; k < LOW_LAYERS; k++) {
        let i = LOW_LAYERS - 1u - k;
        shares[i] = amounts[i] * left;
        left *= 1.0 - amounts[i];
    }
    var flat_colour = vec3<f32>(0.0);
    var first = 0u;
    var second = 0u;
    var most = -1.0;
    var next_most = -1.0;
    for (var i = 0u; i < LOW_LAYERS; i++) {
        flat_colour += tints[i] * shares[i];
        if shares[i] > most {
            second = first;
            next_most = most;
            first = i;
            most = shares[i];
        } else if shares[i] > next_most {
            second = i;
            next_most = shares[i];
        }
    }
    let flat_layer = Layer(flat_colour, n, 0.5);
    let textured = 1.0 - smoothstep(LOW_TEXTURED_M.x, LOW_TEXTURED_M.y, distance(world, view.world_position));
    if textured <= 0.0 {
        return flat_layer;
    }
    var drawn: Layer;
    if next_most > 0.02 {
        // The two as if they were all there is: the floor's litter, needles, grass and moss
        // settled by height, the rest laid over, as the high preset lays each.
        let under = min(first, second);
        let top = max(first, second);
        let lower = low_layer(under, tints[under], world, gx, gy, n);
        let upper = low_layer(top, tints[top], world, gx, gy, n);
        let amount = shares[top] / (most + next_most);
        if top == 2u || top == 3u || top == 5u || top == 6u {
            drawn = settle(lower, upper, amount);
        } else {
            drawn = over(lower, upper, amount);
        }
    } else {
        drawn = low_layer(first, tints[first], world, gx, gy, n);
    }
    return over(flat_layer, drawn, textured);
}

// Layer `i` of LOW_LAYERS: which picture, at what size, moved by how much so no two show the
// same place.
@if(GROUND_LOW)
fn low_layer(i: u32, tint: vec3<f32>, world: vec3<f32>, gx: vec3<f32>, gy: vec3<f32>, n: vec3<f32>) -> Layer {
    var index = array<u32, 12>(SOIL, ROCK, LITTER, NEEDLES, ROOTS, GRASS, MOSS, ROCK, SAND, ROCK, SCREE, GRAVEL);
    var scale = array<f32, 12>(1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.6, 1.0, 1.0, 1.0, 1.0);
    var offset = array<f32, 12>(0.0, 37.0, 41.0, 29.0, 3.0, 11.0, 5.0, 0.0, 0.0, 37.0, 19.0, 23.0);
    var drawn = biplanar(index[i], scale[i], tint, world + offset[i], gx, gy, n);
    if i == 0u {
        // Trodden soil's light and dark drawn out, as the high preset's (D294).
        drawn.colour = tint * pow(max(drawn.colour / max(tint, vec3<f32>(0.001)), vec3<f32>(0.0)), vec3<f32>(SOIL_CONTRAST));
    }
    if i == 2u {
        // The fallen leaves' reds and yellows toned toward the tint, as the high preset's.
        let light = dot(drawn.colour, vec3<f32>(0.333)) / dot(tint, vec3<f32>(0.333));
        drawn.colour = mix(tint * light, drawn.colour, 0.3);
    }
    return drawn;
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
        var fresh = 0.0;
        var trodden = 0.0;
        if ground.cover_on != 0u {
            // The map is half a metre a texel: looked up a little way off, by noise, so what
            // it says frays and wanders rather than ending in smooth blobs.
            let warp = vec3<f32>(noise(world * 0.7 + 41.0), 0.0, noise(world * 0.7 + 83.0)) - vec3<f32>(0.5, 0.0, 0.5);
            // Finer fraying, but not at the low preset.
@if(GROUND_LOW)
            let fine = vec3<f32>(0.0);
@else
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
            trodden = more.a * inside;
            // After the fall the broadleaves' leaves lie fresh under them, over the grass too,
            // warm brown through the winter until the spring's growth comes through (D327).
            fresh = fallen_leaves(world) * smoothstep(0.15, 0.6, more.b * inside) * (1.0 - needles);
            litter = max(litter, fresh);
            grows.r = min(grows.r, 1.0 - fresh);
        }
        // What the land's rules lay on the ground (D322, terrain.rs `lying`): scree and gravel,
        // bare rock, snow, and how close the woods stand, frayed by noise so edges wander.
        // The first texture coordinate's x is the loose stones (1 scree, 2 gravel), its y the
        // snow's depth in metres; the second's x the bare rock, or below 0 the woods.
        var scree = 0.0;
        var gravel = 0.0;
        var bare = 0.0;
        var snow = 0.0;
        var woods = 0.0;
@if(VERTEX_UVS_A)
        {
            let loose = in.uv.x + (blotch - 0.5) * 0.6;
            scree = smoothstep(0.3, 0.7, 1.0 - abs(loose - 1.0));
            gravel = smoothstep(0.3, 0.7, loose - 1.0);
            // Snow lies on what faces up, its edge broken, thinning off the steep.
            let up_facing = smoothstep(0.5, 0.82, flatness + (fleck - 0.5) * 0.12);
            // Its edge broken a metre or two by noise, as a patch of old snow's is, so the
            // vertices' depths never show as triangles.
            let edge = noise(world * 0.35 + 91.0) * 0.6 + noise(world * 1.9 + 23.0) * 0.4;
            snow = smoothstep(0.03, 0.16, in.uv.y + (edge - 0.5) * 0.22 + (fleck - 0.5) * 0.04) * up_facing;
        }
@if(VERTEX_UVS_B)
        {
            bare = smoothstep(0.25, 0.6, in.uv_b.x + (fleck - 0.5) * 0.3 + (blotch - 0.5) * 0.3);
            woods = clamp(-in.uv_b.x, 0.0, 1.0);
        }
        let snow_tint = vec3<f32>(0.7, 0.74, 0.8) * (0.92 + 0.12 * fleck);
        // Far off, past where its trees are drawn, woods are the ground's own colour: a dark
        // conifer canopy (the far crowns' colour, linear), so a forested slope reads forested to
        // the horizon (docs/demos.md §1); nearer, the cards stand on it and it is darker
        // under them a little.
        let far_off = distance(world, view.world_position);
        let canopy = vec3<f32>(0.018, 0.036, 0.016) * (0.85 + 0.3 * blotch);
        let under_woods = woods * mix(0.25, 1.0, smoothstep(3200.0, 4400.0, far_off)) * smoothstep(300.0, 700.0, far_off);
        let granite = ground.steep.rgb * (0.8 + 0.4 * blotch);
        let scree_tint = ground.steep.rgb * 1.25 * (0.85 + 0.3 * fleck);
        // Washed stones of the gully's own rock, grey, a little browner than the cliffs.
        let gravel_tint = mix(ground.steep.rgb, ground.sand.rgb, 0.1) * (0.85 + 0.3 * blotch);
@if(GROUND_LOW) {
        // The same layers and shares as below, drawn as `low_floor` says.
        let n = normalize(in.world_normal);
        let flat_ground = 1.0 - steep;
        let litter_colour = mix(vec3<f32>(0.068, 0.054, 0.036), FRESH_LITTER, fresh);
        let needle_colour = vec3<f32>(0.054, 0.043, 0.031);
        let leaves = smoothstep(0.2, 0.75, litter + (blotch - 0.5) * 0.45) * flat_ground;
        let needly = smoothstep(0.25, 0.7, needles + (blotch - 0.5) * 0.4) * flat_ground;
        let rooty = smoothstep(0.3, 0.75, roots + (fleck - 0.5) * 0.35 + (blotch - 0.5) * 0.3) * flat_ground;
        let heath = heath_here(world);
        let grass = smoothstep(0.38, 0.62, grows.r + (blotch - 0.5) * 0.5) * ground.cover * flat_ground * (1.0 - heath.ground.z);
        let lush = smoothstep(0.2, 0.8, grows.g + (blotch - 0.5) * 0.45);
        let mossy = smoothstep(0.25, 0.75, moss + (blotch - 0.5) * 0.5) * flat_ground;
        let rock = smoothstep(0.2, 0.4, grows.b);
        let ore = smoothstep(0.7, 0.9, grows.b);
        let above_sea = from_centre - ground.sea_radius_m;
        let undug = 1.0 - smoothstep(0.1, 0.3, grows.b);
        let sandy = (1.0 - smoothstep(0.35, 1.5, above_sea + (blotch - 0.5) * 0.9)) * ground.cover * undug;
        let amounts = array<f32, 12>(1.0, steep, leaves, needly, rooty, grass, mossy, rock, sandy, bare, scree, gravel);
        let tints = array<vec3<f32>, 12>(
            // Damp, packed patches darker and dry crumbs lighter, as the high preset's (D294).
            mix(mix(colour, mix(litter_colour, needle_colour, needles), max(litter, needles)), TRODDEN, smoothstep(0.08, 0.55, trodden)) * (0.9 + 0.2 * blotch)
                * mix(1.18, 0.62, smoothstep(0.3, 0.75, noise(world * 0.6 + 31.0) * 0.6 + noise(world * 2.1 + 47.0) * 0.4)),
            ground.steep.rgb * (0.85 + 0.3 * blotch),
            litter_colour * (0.85 + 0.3 * fleck),
            needle_colour * (0.85 + 0.3 * fleck),
            vec3<f32>(0.08, 0.058, 0.04) * (0.8 + 0.4 * blotch),
            mix(ground.grass_dry.rgb, ground.grass_lush.rgb, lush) * (0.92 + 0.16 * fleck),
            vec3<f32>(0.056, 0.08, 0.026) * (0.8 + 0.4 * fleck),
            mix(vec3<f32>(0.12, 0.1, 0.08) * (0.85 + 0.3 * blotch), vec3<f32>(0.24, 0.08, 0.04) * (0.8 + 0.4 * fleck), ore),
            ground.sand.rgb,
            granite,
            scree_tint,
            gravel_tint,
        );
        var ground_is = low_floor(amounts, tints, world, n);
        // The heath past the trees, as its flat colours, over the soil it grows on.
        if heath.ground.z > 0.0 {
            let shrub_colour = mix(SHRUB_GREEN, ground.heath_autumn.rgb * 0.45, kind_turned(world, ground.heath_layer));
            let sedge_colour = mix(SEDGE_GREEN, SEDGE_STRAW, smoothstep(0.0, 0.5, kind_turned(world, ground.grass_layer)));
            let tint = heath_flat(heath, shrub_colour, sedge_colour) * (0.85 + 0.3 * blotch);
            ground_is.colour = mix(ground_is.colour, tint, heath.ground.z * (1.0 - steep) * 0.9);
        }
        // The blades' shade and the broad swells, over all but dug rock and sand.
        let shaded = (1.0 - 0.6 * smoothstep(0.1, 0.7, blades)) * (0.86 + 0.28 * swell);
        let hollow = smoothstep(0.0, 0.6, ground_is.height);
        colour = ground_is.colour * mix(0.8, 1.0, hollow) * mix(shaded, 1.0, 1.0 - (1.0 - rock) * (1.0 - sandy));
        colour *= micro_shadow(in.world_position, n, ground_is.normal, ground_is.height, in.position);
        pbr_input.diffuse_occlusion *= vec3<f32>(mix(0.6, 1.0, hollow));
        // The shore, only near the sea.
        if above_sea < 2.0 {
            let lap = sin(globals.time * 0.9 + noise(world * 0.12) * 12.0) * 0.06;
            colour *= mix(0.72, 1.0, smoothstep(0.05, 0.3, above_sea - lap) * undug + (1.0 - undug));
            colour = mix(colour, ground.sea_deep.rgb, smoothstep(0.0, 4.5, -above_sea) * 0.92 * undug);
            let foam_width = 0.07 + 0.05 * noise(world * 1.7 + globals.time * 0.25);
            let foam = 1.0 - smoothstep(0.0, foam_width, abs(above_sea - lap - 0.02));
            colour = mix(colour, vec3<f32>(1.0, 0.99, 0.96), foam * 0.85 * ground.cover * undug);
        }
        colour = mix(colour, snow_tint, snow);
        colour = mix(colour, canopy, under_woods * (1.0 - snow));
@if(VERTEX_UVS_B)
        {
            colour = lake_shore(colour, in.uv_b.y, fleck);
        }
        pbr_input.N = ground_is.normal;
} @else {
        let n = normalize(in.world_normal);
        let gx = dpdx(world);
        let gy = dpdy(world);
        let flat_ground = 1.0 - steep;
        // Under broadleaves the floor is their fallen leaves, brown with paler and darker
        // ones; under conifers, needles, a warmer red-brown. Both a dark umber, as Sons of
        // the Forest's floor is, not lit orange (D293).
        let litter_colour = mix(vec3<f32>(0.068, 0.054, 0.036), FRESH_LITTER, fresh);
        let needle_colour = vec3<f32>(0.054, 0.043, 0.031);
        let leaves = smoothstep(0.2, 0.75, litter + (blotch - 0.5) * 0.45) * flat_ground;
        let needly = smoothstep(0.25, 0.7, needles + (blotch - 0.5) * 0.4) * flat_ground;
        // Soil, and rock on steep ground, where the floor's litter does not cover it all.
        var ground_is = Layer(colour, n, 0.5);
        if max(leaves, needly) < 0.999 {
            let soil_tint = mix(colour, mix(litter_colour, needle_colour, needles), max(litter, needles)) * (0.9 + 0.2 * blotch);
            ground_is = layer(SOIL, 1.0, soil_tint, world, gx, gy, n, 1.0);
            // Trodden soil is not one tone (D294): its scan's light and dark drawn out, damp,
            // packed patches darker and dry crumbs lighter, a metre or two across and less, and
            // small stones worked up out of it, so a path in the sun is not flat peach.
            let crumbs = ground_is.colour / max(soil_tint, vec3<f32>(0.001));
            ground_is.colour = soil_tint * pow(max(crumbs, vec3<f32>(0.0)), vec3<f32>(SOIL_CONTRAST));
            let damp = noise(world * 0.6 + 31.0) * 0.6 + noise(world * 2.1 + 47.0) * 0.4;
            ground_is.colour *= mix(1.18, 0.62, smoothstep(0.3, 0.75, damp));
            let stony = smoothstep(0.4, 0.85, noise(world * 0.45 + 59.0)) * 0.5;
            if stony > 0.0 {
                let stone_tint = vec3<f32>(0.11, 0.1, 0.085) * (0.75 + 0.5 * fleck);
                ground_is = settle(ground_is, layer(ROCK, 0.22, stone_tint, world + 71.0, gx, gy, n, 1.0), stony);
            }
            if steep > 0.0 {
                let rock_tint = ground.steep.rgb * (0.85 + 0.3 * blotch);
                ground_is = over(ground_is, layer(ROCK, 1.0, rock_tint, world + 37.0, gx, gy, n, 0.15), steep);
            }
        }
        // A settlement's trodden ways (TownCover): packed earth of its own colour, its scan's
        // grain kept but not its hue, stones worked up out of it, mud lying dark in its
        // hollows, and straw dropped about its edges and in the yards.
        let worn = smoothstep(0.08, 0.55, trodden + (blotch - 0.5) * 0.35) * flat_ground;
        if worn > 0.0 {
            let packed = TRODDEN * (0.82 + 0.36 * blotch) * (0.9 + 0.2 * swell);
            var earth = layer(SOIL, 0.7, packed, world + 13.0, gx, gy, n, 1.0);
            let grain = dot(earth.colour, vec3<f32>(0.333)) / dot(packed, vec3<f32>(0.333));
            earth.colour = packed * pow(max(grain, 0.0), 1.25);
            let stones = smoothstep(0.5, 0.85, noise(world * 0.7 + 91.0) * 0.7 + fleck * 0.3) * 0.8;
            if stones > 0.0 {
                earth = settle(earth, layer(GRAVEL, 0.45, packed * 1.2, world + 19.0, gx, gy, n, 1.0), stones);
            }
            let wet = smoothstep(0.58, 0.78, noise(world * 0.28 + 7.0) * 0.75 + noise(world * 1.9 + 3.0) * 0.25) * smoothstep(0.6, 0.9, trodden);
            earth.colour *= mix(1.0, 0.45, wet);
            ground_is = over(ground_is, earth, worn);
            let straw = smoothstep(0.62, 0.8, noise(world * 1.3 + 61.0) * 0.6 + fleck * 0.4) * (1.0 - smoothstep(0.55, 0.95, trodden)) * worn;
            if straw > 0.0 {
                ground_is = settle(ground_is, layer(LITTER, 0.6, STRAW * (0.8 + 0.4 * fleck), world + 67.0, gx, gy, n, 1.0), straw);
            }
        }
        // Fallen leaves, laid on the soil leaf by leaf as they thicken.
        if leaves > 0.0 {
            let leaf_tint = litter_colour * (0.85 + 0.3 * fleck);
            var fallen = layer(LITTER, 1.0, leaf_tint, world + 41.0, gx, gy, n, 1.0);
            // The scan's reds and yellows toned down toward the tint, keeping its light and dark.
            let light = dot(fallen.colour, vec3<f32>(0.333)) / dot(leaf_tint, vec3<f32>(0.333));
            fallen.colour = mix(leaf_tint * light, fallen.colour, 0.3);
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

        // Past the trees the heath takes the ground from the meadow's grass (terra_land::heath).
        let heath = heath_here(world);
        // Grass sits in the hollows first, and gives out where the floor stands higher.
        let grass = smoothstep(0.38, 0.62, grows.r + (blotch - 0.5) * 0.5) * ground.cover * flat_ground * (1.0 - heath.ground.z);
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

        // The heath past the trees (docs/demos.md §2): its communities a mosaic, each where
        // its share is most, their edges frayed a metre or two by noise so no line shows; the
        // fellfield's stones under the rest, then lichen, moss, sedge and the dwarf shrubs on
        // top, as they stand.
        if heath.ground.z > 0.0 {
            let mosaic = (noise(world * 0.31 + 101.0) - 0.5) * 0.55 + (noise(world * 1.3 + 103.0) - 0.5) * 0.3
                + (fleck - 0.5) * 0.15;
            let stones = smoothstep(0.2, 0.6, heath.ground.x + mosaic);
            if stones > 0.0 {
                ground_is = over(ground_is, layer(GRAVEL, 0.8, FELL_STONE * (0.8 + 0.4 * blotch), world + 61.0, gx, gy, n, 1.0), stones);
            }
            let lichen = smoothstep(0.15, 0.5, heath.plants.x + mosaic) * flat_ground;
            if lichen > 0.0 {
                // Cladonia's pale cushions over the moss's relief, a little greener and greyer
                // from cushion to cushion.
                let tint = LICHEN * mix(vec3<f32>(0.92, 0.95, 0.9), vec3<f32>(1.06, 1.03, 1.08), fleck);
                ground_is = settle(ground_is, layer(MOSS, 0.7, tint, world + 67.0, gx, gy, n, PLAIN), lichen);
            }
            // Moss carpets the edges of pools and lakes, soaked a metre or so up from the water.
            var by_water = 0.0;
@if(VERTEX_UVS_B)
            {
                by_water = (1.0 - smoothstep(0.1, 1.1 + 0.6 * fleck, in.uv_b.y)) * step(-0.05, in.uv_b.y);
            }
            let carpet = smoothstep(0.3, 0.65, max(heath.plants.w, by_water * heath.ground.z) + mosaic) * flat_ground;
            if carpet > 0.0 {
                let tint = mix(SPHAGNUM, vec3<f32>(0.16, 0.05, 0.03), smoothstep(0.55, 0.85, noise(world * 0.5 + 7.0)) * 0.6);
                ground_is = settle(ground_is, layer(MOSS, 1.0, tint, world + 71.0, gx, gy, n, PLAIN), carpet);
            }
            let sedge = smoothstep(0.15, 0.5, heath.plants.z + mosaic) * flat_ground;
            if sedge > 0.0 {
                let tint = mix(SEDGE_GREEN, SEDGE_STRAW, smoothstep(0.0, 0.5, kind_turned(world, ground.grass_layer))) * (0.85 + 0.3 * fleck);
                ground_is = settle(ground_is, layer(GRASS, 0.8, tint, world + 73.0, gx, gy, n, 1.0), sedge);
            }
            let shrub = smoothstep(0.12, 0.45, heath.plants.y + mosaic) * flat_ground;
            if shrub > 0.0 {
                // Dwarf birch and bilberry turning crimson and rust, crowberry staying dark
                // green among them, in patches a hand or two across.
                let crowberry = smoothstep(0.45, 0.7, noise(world * 1.3 + 31.0));
                let autumn = ground.heath_autumn.rgb * 0.6 * mix(vec3<f32>(1.1, 0.75, 0.8), vec3<f32>(0.9, 1.1, 1.0), fleck);
                let leaves = mix(mix(SHRUB_GREEN, autumn, kind_turned(world, ground.heath_layer)), SHRUB_GREEN * 0.8, crowberry);
                ground_is = settle(ground_is, layer(LITTER, 0.6, leaves, world + 79.0, gx, gy, n, 1.0), shrub);
            }
            // Wet ground darker and glossier, as wet soil and moss are (Lagarde).
            let soaked = smoothstep(0.55, 0.95, heath.ground.y) * heath.ground.z;
            ground_is.colour *= mix(1.0, 0.72, soaked);
        }

        // The ground under thick grass is in the blades' own shade, so what shows between
        // them is dark, as their roots are, rather than a lawn.
        ground_is.colour *= 1.0 - 0.6 * smoothstep(0.1, 0.7, blades);
        ground_is.colour *= 0.86 + 0.28 * swell;

        // The mountains' bare rock, their scree under its cliffs and gravel in gullies' beds,
        // over whatever grew: nothing grows through them.
        if bare > 0.0 {
            ground_is = over(ground_is, layer(ROCK, 1.0, granite, world + 37.0, gx, gy, n, 0.15), bare);
        }
        if scree > 0.0 {
            ground_is = over(ground_is, layer(SCREE, 1.0, scree_tint, world + 19.0, gx, gy, n, 1.0), scree);
        }
        if gravel > 0.0 {
            ground_is = over(ground_is, layer(GRAVEL, 1.0, gravel_tint, world + 23.0, gx, gy, n, 1.0), gravel);
        }
        if snow > 0.0 {
            ground_is = over(ground_is, layer(SNOW_LAYER, 1.0, snow_tint, world + 13.0, gx, gy, n, 1.0), snow);
        }

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
        // In the sun the hollows are shadowed by what stands round them, needle by needle and
        // clod by clod, which the sky's light (darkened above) does not show: micro-shadowing
        // from the cavity, as Uncharted 4 does it (Brinck and Maximov, "The Technical Art of
        // Uncharted 4", 2016), on the direct light alone (D294). Without it the sunlit floor
        // read as flat peach sand beside the shaded floor's detail.
        colour *= micro_shadow(in.world_position, n, ground_is.normal, ground_is.height, in.position);
        pbr_input.diffuse_occlusion *= vec3<f32>(mix(0.6, 1.0, hollow));
        colour *= mix(0.72, 1.0, smoothstep(0.05, 0.3, above_sea - lap) * undug + (1.0 - undug));
        colour = mix(colour, ground.sea_deep.rgb, smoothstep(0.0, 4.5, -above_sea) * 0.92 * undug);
        let foam_width = 0.07 + 0.05 * noise(world * 1.7 + globals.time * 0.25);
        let foam = 1.0 - smoothstep(0.0, foam_width, abs(above_sea - lap - 0.02));
        colour = mix(colour, vec3<f32>(1.0, 0.99, 0.96), foam * 0.85 * ground.cover * undug);
        colour = mix(colour, canopy, under_woods * (1.0 - snow));
@if(VERTEX_UVS_B)
        {
            colour = lake_shore(colour, in.uv_b.y, fleck);
            pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.35, wet_band(in.uv_b.y, fleck));
        }
        pbr_input.N = ground_is.normal;
}
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
@if(!PREPASS_PIPELINE) {
    // What the weather has left (climate.md, D327): wet ground darker and glossier, rime in a
    // hard frost, and snow lying on what faces up.
    let climate = climate_at(world);
    let level = dot(normalize(in.world_normal), up);
    // Where the land's rules carry the snow on the ground's own vertices out to the far land
    // (D335), they draw it there; the climate map's, whose edge's answer reaches on past it,
    // fades out before its edge, or it smeared the edge's snow white over every far ridge.
    var map_snow = climate.r;
@if(VERTEX_UVS_A)
    {
        map_snow *= climate_inside(world);
    }
    let snow = snow_on(map_snow * SNOW_FULL_M, world, level, canopy_over(world));
    let wet = max(climate.g * 0.6, climate.b) * (1.0 - snow);
    colour *= mix(1.0, 0.6, wet);
    pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.5, wet * 0.7);
    let rime = climate.a * (1.0 - snow) * smoothstep(0.6, 0.9, level) * smoothstep(0.3, 0.7, noise(world * 4.3 + 61.0));
    colour = mix(colour, RIME, rime * 0.5);
    let sparkle = 0.94 + 0.12 * noise(world * 9.0 + 3.0);
    colour = mix(colour, SNOW * sparkle, snow);
    pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.8, snow);
    pbr_input.N = normalize(mix(pbr_input.N, normalize(in.world_normal), snow));
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
