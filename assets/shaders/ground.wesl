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
// Bevy declares `fog` only for a camera with `DistanceFog` (as its pbr_functions guards it).
@if(!PREPASS_PIPELINE && DISTANCE_FOG) import bevy_pbr::render::mesh_view_bindings::fog;
import super::horizon::horizon_lit;

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
    // The same for the climate's maps (climate.rs, D327), and
    // the leaves map's layer for the broadleaves whose leaves fall on the floor, or -1.
    climate_from_world: mat4x4<f32>,
    climate_on: u32,
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
    // The land's horizon round the camera (horizon.wesl, D416), laid out as the land map.
    horizon_middle: vec4<f32>,
    horizon_east: vec4<f32>,
    horizon_north: vec4<f32>,
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
// One sampler for every map below, all linear and clamped at their edge, read at their top
// level: a stage may bind 16 samplers on some cards (Apple's), and Bevy's view and the standard
// material take most of them. Godot shares its samplers the same way, one a filter and repeat
// mode for any number of textures (`material_samplers` in its forward renderers' scene
// shaders, MIT).
@group(constants::MATERIAL_BIND_GROUP) @binding(105)
var map_sampler: sampler;
// And more of it: fallen needles in red, duff and roots in green, how much canopy stands
// over in blue, and how trodden the ground is in alpha (a settlement's ways).
@group(constants::MATERIAL_BIND_GROUP) @binding(106)
var cover_more: texture_2d<f32>;
// And a third: how near a trunk, log or rock stands, in red, and whether a path runs there in
// green.
@group(constants::MATERIAL_BIND_GROUP) @binding(114)
var cover_stood: texture_2d<f32>;
// What the climate leaves round the camera, a 32 m cell a texel (climate.rs): snow's depth in
// red, its brightest SNOW_FULL_M; the ground's wetness in green; rain falling now in blue;
// frost in alpha. And each kind's leaves, a layer a kind: how many it has early and late in
// its year in red and green, how far turned in blue and alpha.
@group(constants::MATERIAL_BIND_GROUP) @binding(108)
var climate_ground: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(110)
var climate_leaves: texture_2d_array<f32>;
// The heath past the trees (terra_land::heath), a 10 m cell a texel over the land round its
// middle: lichen, dwarf shrubs, sedge and moss in the first layer; the fellfield's stones,
// the wetness and how open the ground is to the heath in the second.
@group(constants::MATERIAL_BIND_GROUP) @binding(112)
var land_map: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(115)
var horizon_map: texture_2d_array<f32>;

// Snow this deep is the climate map's brightest (`climate::SNOW_FULL_M`).
const SNOW_FULL_M: f32 = 0.5;
// The penumbra of a range's far shadow (D416), radians either side of its horizon: the sun's
// half degree, and the map's step of 0.18 degrees.
const HORIZON_SOFT: f32 = 0.008;
// Lying snow, linear: short of fresh snow's albedo of 0.9, settled and dusted with what falls
// from the trees, and a little cool: the sun's warm light on it read peach. No bluer than it
// is green, or a low sun's red and the sky's blue on it make it magenta (the tundra's review).
// At 0.7 the forest's snow under a deck read grey beside the haze (weather round 2).
const SNOW: vec3<f32> = vec3<f32>(0.87, 0.88, 0.88);
// Fresh fallen oak leaves, linear: a warm mid brown, lighter and redder than the floor's
// old umber litter.
const FRESH_LITTER: vec3<f32> = vec3<f32>(0.13, 0.072, 0.03);
// A settlement's trodden ways, linear: packed earth, grey-brown and dusty, as a market town's
// streets are in Kingdom Come's and The Witcher's frames, not the planet's warm soil; and the
// straw dropped about their edges.
const TRODDEN: vec3<f32> = vec3<f32>(0.15, 0.132, 0.108);
// Ground trodden lightly, a new settlement's plots and ways (D381): turned earth still brown
// with its soil, not yet ground to a street's grey dust. The town's ground keeps to earth
// colours: red over blue under 1.5, or the low sun turns an empty plot to a red-orange
// patch (at 0.105, 0.08, 0.055, red over blue 1.9: settlement r19).
const TURNED: vec3<f32> = vec3<f32>(0.11, 0.096, 0.077);
// Straw on the ground, no brighter than the sunlit earth by more than half: at twice it (0.34,
// 0.27, 0.13) every fleck stood out from high as yellow confetti on the plots (settlement r19).
const STRAW: vec3<f32> = vec3<f32>(0.2, 0.17, 0.105);
// A settlement's paved streets and square (D350), linear: grey-brown setts of the local stone,
// dulled by the dirt ground into them; Kingdom Come's sunlit streets measure about 0.25 sRGB,
// near grey, where clean light stone (0.15 linear) stood at twice that in our frames.
const SETTS: vec3<f32> = vec3<f32>(0.092, 0.088, 0.08);
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
const COBBLES: u32 = 11u;

// How wet the ground is at so many metres above a lake's water (D322): soaked at the waterline
// and drying over a hand or two above it, its edge broken by `fleck`; by a lowland lake, whose
// meadow comes down to its water, over a few centimetres. On a flat bank a hand or two of
// height was a band of darkened, sky-mirroring ground tens of metres wide round the water,
// the slate ring at the settlement's shore.
fn wet_band(above: f32, fleck: f32, lowland: f32) -> f32 {
    let reach = mix(0.3 + 0.25 * fleck, 0.05 + 0.03 * fleck, lowland);
    return (1.0 - smoothstep(0.0, reach, above)) * step(-0.3, above);
}

// The ground by a lake (D322): darker where wet above the water, as wet stone and soil are
// (Lagarde: wet ground darkens and smooths the dry, its lighting the same), and under it
// darkened toward the water's colour with depth, the bed fading into the deep. `above` is how
// far over the nearest lake's water the ground stands, below 0 under it.
// A lowland lake's bed, `lowland` 1, is silt under stained water, olive brown, gone dark a
// metre or two down; a mountain lake's, 0, granite under clear green water.
fn lake_shore(colour: vec3<f32>, above: f32, fleck: f32, lowland: f32) -> vec3<f32> {
    let wet = wet_band(above, fleck, lowland);
    let under = smoothstep(0.0, mix(5.0, 1.8, lowland), -above);
    let damp = colour * mix(1.0, 0.55, wet);
    let bed = mix(vec3<f32>(0.012, 0.03, 0.026), vec3<f32>(0.01, 0.011, 0.006), lowland);
    return mix(damp, bed, under * mix(0.75, 0.92, lowland));
}

// How much the ground is a lowland lake's shore rather than a mountain lake's, from its
// height over the sea (as `lakes::MOUNTAIN_LAKE_M` and flora's `MOUNTAIN_SHORE_M`).
fn lowland_at(above_sea: f32) -> f32 {
    return 1.0 - smoothstep(250.0, 500.0, above_sea);
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
// How far a patch of a settlement's setts is turned, at most, as a share of a full turn.
const SETT_TURN: f32 = 0.06;

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
    // x and y only: a cooked normal map is BC5, its two components (D188); z is made from
    // them, stored as the map would have it.
    let xy = textureSampleGrad(ground_normal, ground_sampler, uv, index, ddx, ddy).xy * 2.0 - 1.0;
    return Texel(colour, xy, sqrt(max(1.0 - dot(xy, xy), 0.0)) * 0.5 + 0.5);
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

// How near a trunk, log or rock stands, 1 against it to 0 two metres off, as the cover's
// third map says (`Growing::stands`).
fn stands_at(world: vec3<f32>) -> f32 {
    if ground.cover_on == 0u {
        return 0.0;
    }
    let at = (ground.cover_from_world * vec4<f32>(world, 1.0)).xy;
    let inside = clamp(min(min(at.x, at.y), min(1.0 - at.x, 1.0 - at.y)) * 20.0, 0.0, 1.0);
    return textureSampleLevel(cover_stood, map_sampler, at, 0.0).r * inside;
}

// How much a path runs here, 0 off it to 1 on it, as the cover's third map says.
fn worn_at(world: vec3<f32>) -> f32 {
    if ground.cover_on == 0u {
        return 0.0;
    }
    let at = (ground.cover_from_world * vec4<f32>(world, 1.0)).xy;
    let inside = clamp(min(min(at.x, at.y), min(1.0 - at.x, 1.0 - at.y)) * 20.0, 0.0, 1.0);
    return textureSampleLevel(cover_stood, map_sampler, at, 0.0).g * inside;
}

// How much of a canopy stands over a place, as the cover map says.
fn canopy_over(world: vec3<f32>) -> f32 {
    if ground.cover_on == 0u {
        return 0.0;
    }
    let at = (ground.cover_from_world * vec4<f32>(world, 1.0)).xy;
    let inside = clamp(min(min(at.x, at.y), min(1.0 - at.x, 1.0 - at.y)) * 20.0, 0.0, 1.0);
    return textureSampleLevel(cover_more, map_sampler, at, 0.0).b * inside * 0.85;
}

// The climate map round the camera at a point. Past its edge, the edge's answer: the
// climate changes slowly, over hundreds of metres.
fn climate_at(world: vec3<f32>) -> vec4<f32> {
    if ground.climate_on == 0u {
        return vec4<f32>(0.0);
    }
    let at = clamp((ground.climate_from_world * vec4<f32>(world, 1.0)).xy, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSampleLevel(climate_ground, map_sampler, at, 0.0);
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
    let leaves = textureSampleLevel(climate_leaves, map_sampler, at, ground.fallen_layer, 0.0);
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
        textureSampleLevel(land_map, map_sampler, uv, 0, 0.0) * inside,
        textureSampleLevel(land_map, map_sampler, uv, 1, 0.0) * inside,
    );
}

// How far a kind's leaves have turned near a point, from the climate's leaves map, 0 green to
// 1 its autumn colour; 0 with no map.
fn kind_turned(world: vec3<f32>, layer: i32) -> f32 {
    if ground.climate_on == 0u || layer < 0 {
        return 0.0;
    }
    let at = clamp((ground.climate_from_world * vec4<f32>(world, 1.0)).xy, vec2<f32>(0.0), vec2<f32>(1.0));
    let leaves = textureSampleLevel(climate_leaves, map_sampler, at, layer, 0.0);
    return 0.5 * (leaves.b + leaves.a);
}

// The heath's colours, linear, measured off photographs of tundra in late summer and early
// autumn (research/2026-10-15-tundra-reference.md): pale Cladonia lichen; dwarf birch and
// crowberry and the lichen carpet's dark olive; sedge green, and wet straw once turned;
// Sphagnum's gold; the fellfield's grey stones. The dwarf shrubs' autumn is their year's.
// The lichen and shrub heath are speckled close to, the sedge smooth.
const LICHEN: vec3<f32> = vec3<f32>(0.45, 0.38, 0.23);
const SHRUB_GREEN: vec3<f32> = vec3<f32>(0.09, 0.085, 0.045);
const SEDGE_GREEN: vec3<f32> = vec3<f32>(0.1, 0.11, 0.045);
const SEDGE_STRAW: vec3<f32> = vec3<f32>(0.22, 0.13, 0.04);
const SPHAGNUM: vec3<f32> = vec3<f32>(0.23, 0.13, 0.006);
const FELL_STONE: vec3<f32> = vec3<f32>(0.17, 0.165, 0.15);

// A layer laid in `tint`'s own hue, keeping only its texture's light and dark: a scanned moss
// or grass swapped to a heath's colour by its average alone keeps every pixel's departure from
// that average in hue too, and a moss's greener pixels, divided by its dark average's little
// red and blue, turned a lichen lawn-green.
fn toned(l: Layer, tint: vec3<f32>) -> Layer {
    let luma = vec3<f32>(0.2126, 0.7152, 0.0722);
    let light = clamp(dot(l.colour, luma) / max(dot(tint, luma), 0.001), 0.25, 2.2);
    return Layer(tint * light, l.normal, l.height);
}

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
// How much less snow lies under the crowns than in the open: a conifer stand intercepts a
// third to a half of a fall (Hedstrom and Pomeroy 1998), but the floor still reads white; at
// 0.6 the forest's floor showed through as dark hatching (weather round 7).
const SNOW_UNDER_CROWNS: f32 = 0.35;
// How deep, metres, snow lies before it covers the floor whole, its edge's noise aside.
const SNOW_COVERS_M: f32 = 0.06;
// How level, the normal's dot with up, ground must be for snow to start to lie, and to lie
// whole: a bank of 50 degrees holds snow; without, banks along the path stood black.
const SNOW_SLOPE: vec2<f32> = vec2<f32>(0.35, 0.7);
// A tree well's shade at its deepest, its sky half hidden: cooler and darker, bluest on the
// side the trunk keeps the sun from (winter round 12).
const WELL_SHADE: vec3<f32> = vec3<f32>(0.74, 0.82, 0.95);
const WELL_OCCLUDES: f32 = 0.35;
// How far a well and a rim tilt the snow, by how the nearness changes across a metre.
const WELL_LEAN: f32 = 2.2;
const RIM_LEAN: f32 = 1.1;
// Wind ripples on open snow: how many a metre, and how far they tilt it (winter round 12).
const RIPPLE_PER_M: f32 = 3.2;
// A trodden path's snow: packed down into a trough, greyer and darker than the drifts beside
// it, broken by footprints, and how far its sides tilt the snow.
const TRODDEN_SNOW: vec3<f32> = vec3<f32>(0.66, 0.68, 0.72);
const TROUGH_LEAN: f32 = 1.8;
// The bank the walkers' feet push up either side of a path (winter round 14; the judge: "an
// even-width dark ribbon with no churned edge"): how far out it reaches, metres, how far its
// outer face tilts the snow, and how much brighter its fresh, broken snow is.
const BANK_M: f32 = 0.9;
const BANK_LEAN: f32 = 1.4;
const BANK_LIFT: f32 = 0.06;
const RIPPLE_LEAN: f32 = 0.35;
// Glints in lying snow: cells this many a metre, and the share of them a crystal turned to
// flash (winter round 10).
const GLINT_PER_M: f32 = 65.0;
const GLINT_SHARE: f32 = 0.04;
fn snow_on(depth_m: f32, world: vec3<f32>, up_facing: f32, under: f32) -> f32 {
    let lying = depth_m * (1.0 - SNOW_UNDER_CROWNS * under);
    let edge = noise(world * 0.35 + 91.0) * 0.6 + noise(world * 1.9 + 23.0) * 0.4;
    let cover = smoothstep(0.0, 1.0, (lying - 0.004) / SNOW_COVERS_M + (edge - 0.5) * 0.9);
    return cover * smoothstep(SNOW_SLOPE.x, SNOW_SLOPE.y, up_facing) * step(0.001, lying);
}

// A meadow's colour seen from high (D381): drifts of drier and lusher grass and darker
// hollows a few tens of metres across, as a real meadow's sward varies, so past where its
// blades are drawn the ground is not one flat olive.
fn meadow_tones(world: vec3<f32>, lush: f32) -> vec3<f32> {
    let drift = noise(world * 0.021 + 17.0) * 0.6 + noise(world * 0.07 + 29.0) * 0.4;
    let l = clamp(lush + (drift - 0.5) * 0.8, 0.0, 1.0);
    let shade = mix(0.78, 1.14, noise(world * 0.045 + 41.0));
    // Nearer the eye's scale from high: tussocks and trodden lines a few metres across mottle
    // the sward, a darker clump of rush or bramble stands here and there, and drier stalks gone
    // to seed pale whole patches. Without them, from a few hundred metres the drifts alone
    // read as one olive felt (settlement film, 4-20 s).
    let mottle = mix(0.8, 1.16, noise(world * 0.31 + 53.0) * 0.65 + noise(world * 0.9 + 59.0) * 0.35);
    let clump = smoothstep(0.62, 0.76, noise(world * 0.13 + 61.0)) * 0.55;
    let seeded = smoothstep(0.55, 0.85, noise(world * 0.035 + 67.0)) * 0.45;
    let sward = mix(ground.grass_dry.rgb, ground.grass_lush.rgb, l);
    let stalks = mix(sward, ground.grass_dry.rgb * vec3<f32>(1.3, 1.2, 0.85), seeded);
    return mix(stalks, ground.grass_lush.rgb * vec3<f32>(0.62, 0.7, 0.55), clump) * shade * mottle;
}

// Stripes down the fall line round a point, laid cell by cell as the ground's gullies are
// (D394; terra_voxel mountains.rs `Range::stripes`, after Runevision's erosion filter): each cell
// of `STRETCH` times `spacing` metres has a point of its own, and a stripe's phase is measured
// across the fall from the nearest points, so a fall turning between triangles moves it a
// little, never by a wavelength, and each stripe runs on down the fall for a cell (D395). Gives
// -1 down a gully's line to 1 on the rib between, and how fast that changes across the fall, a
// metre. `which` gives each set its own points.
const STRETCH: f32 = 2.5;
// The shallowest a gully is cut, as a share of the deepest, and how the depths between skew
// toward the shallow (a power over 1), so a few stand out deep.
const GULLY_LEAST: f32 = 0.35;
const GULLY_SKEW: f32 = 1.3;
// A far wall's strata (mountains round 29): seen from kilometres, a wall's mesh is too coarse
// to measure the 50 degrees its tiers are drawn past, and its tiers' tones alone sank into the
// haze, so the faces read as sheets (round 28's review: "the rock reads as sheets"). Past
// FAR_STRATA_M the tiers stand on gentler rock too, from the steeper flatness to the gentler,
// on more of the faces (FAR_BANDED, the least of the banding noise's share), and each standing
// tier's riser foot, under the bench below it, is drawn as a ledge's shadow line: darker by
// LEDGE_LINE, at least LEDGE_LINE_PX pixels of height wide however far, as a granite wall's
// overhangs and ledges read from across a valley in the Tatras' and the Dolomites' photographs.
// Gives the flatness the tiers fade out over, the least banding, and the shade at tier_f.
const FAR_STRATA_M: vec2<f32> = vec2<f32>(1200.0, 3500.0);
const FAR_STRATA_FLAT: vec2<f32> = vec2<f32>(0.74, 0.88);
const FAR_BANDED: f32 = 0.65;
const LEDGE_LINE: f32 = 0.5;
const LEDGE_LINE_PX: f32 = 1.5;
fn far_strata(far_off: f32, tier_f: f32, height_px: f32) -> vec4<f32> {
    let far = smoothstep(FAR_STRATA_M.x, FAR_STRATA_M.y, far_off);
    let flat_from = mix(0.55, FAR_STRATA_FLAT.x, far);
    let flat_to = mix(0.7, FAR_STRATA_FLAT.y, far);
    // The line's width as a share of the tier (95 m), at least so many pixels of height.
    let wide = max(0.06, LEDGE_LINE_PX * height_px / 95.0);
    let line = (1.0 - smoothstep(wide * 0.6, wide, tier_f)) * far;
    return vec4<f32>(flat_from, flat_to, FAR_BANDED * far, 1.0 - (1.0 - LEDGE_LINE) * line);
}

fn fall_stripes(world: vec3<f32>, across: vec3<f32>, spacing: f32, which: f32) -> vec2<f32> {
    let cell = world / (spacing * STRETCH);
    let base = floor(cell);
    var weights = 0.0;
    var cosine = 0.0;
    var sine = 0.0;
    for (var i = 0; i < 27; i++) {
        let c = base + vec3<f32>(f32(i % 3 - 1), f32((i / 3) % 3 - 1), f32(i / 9 - 1));
        let jitter = vec3<f32>(hash(c + which * 7.0), hash(c + which * 7.0 + 19.0), hash(c + which * 7.0 + 43.0));
        let d = cell - (c + jitter);
        let far = dot(d, d);
        if far < 1.0 {
            let w = (1.0 - far) * (1.0 - far) * (1.0 - far);
            let phase = dot(d, across) * 6.2831853 * STRETCH;
            // Each gully between two ribs cut to its own depth, a few deep and most shallow, as
            // `Range::stripes_by` cuts the ground's: cut alike, a cell's stripes were a comb of
            // one width and depth (the r3 review). Whole at the ribs, so it never jumps.
            let gully = floor(dot(d, across) * STRETCH);
            let deep = GULLY_LEAST + (1.0 - GULLY_LEAST) * pow(hash(c * 1.7 + vec3<f32>(3.1, 5.3, 7.7) * gully + which * 11.0), GULLY_SKEW);
            weights += w;
            cosine += w * (1.0 - deep * (1.0 - cos(phase)));
            sine += w * deep * sin(phase);
        }
    }
    return vec2<f32>((cosine + 0.02) / (weights + 0.02), -(sine / (weights + 0.02)) * 6.2831853 / spacing);
}

/// How rock facets turn the normal at a point, and how near it lies to a crack between two:
/// a jittered lattice of sites `size` metres apart, each its own plane turned across the
/// ground up to about 25 degrees, blended smoothly near their borders (Quilez's smooth
/// Voronoi, as Gaea's and World Machine's "facet" are cellular noise) so a face reads as
/// slabs shaded each by its own lean rather than flat-shaded polygons; `w` is 1 on the crack
/// between the nearest two (the second-nearest's distance less the nearest's), 0 inside.
fn facet_turn(world: vec3<f32>, up: vec3<f32>, size: f32) -> vec4<f32> {
    let cell = world / size;
    let base = floor(cell);
    var nearest = 9.0;
    var second = 9.0;
    var turn = vec3<f32>(0.0);
    var weights = 0.0;
    for (var i = 0; i < 27; i++) {
        let c = base + vec3<f32>(f32(i % 3 - 1), f32((i / 3) % 3 - 1), f32(i / 9 - 1));
        let j = vec3<f32>(hash(c + 3.3), hash(c + 17.9), hash(c + 29.1));
        let d = cell - (c + j);
        let far = dot(d, d);
        if far < nearest {
            second = nearest;
            nearest = far;
        } else if far < second {
            second = far;
        }
        let w = exp(-20.0 * far);
        turn += w * (vec3<f32>(hash(c + 41.7), hash(c + 53.3), hash(c + 67.1)) - 0.5);
        weights += w;
    }
    turn /= max(weights, 1e-4);
    let flat = turn - up * dot(turn, up);
    let crack = 1.0 - smoothstep(0.0, 0.04, sqrt(second) - sqrt(nearest));
    return vec4<f32>(flat * 1.4, crack);
}

// A wall's rock broken as granite and limestone break (Hoek and Bray's joint sets, the Odle's
// and the Tatras' faces): bedding planes tilted off level, a stratum `spacing` thick, and two
// upright joint sets across them, so the face is cut into blocks. Each block's exposed face is
// a fracture plane turned its own way with a hard edge (unlike `facet_turn`'s blended cells),
// the joints between them dark, and each stratum's top a ledge where snow or turf lies. `soft`
// is how wide an edge is, as a share of a block, from the pixel's footprint.
struct Joints {
    turn: vec3<f32>,
    crack: f32,
    ledge: f32,
    // How much of the face here lies in the shadow its own beds and joints cast.
    shadow: f32,
    // How deep in an open joint or under an open bed's lip the face lies, seeing less sky.
    recess: f32,
}

// A range's rock between its faults (D426): one block of strata a kilometre and a half
// across, its bedding dipping 35 to 70 degrees a way of its own, so its beds cross a face's
// height diagonally instead of ringing it; the faults, wavering lines between blocks, offset
// them. Measured from the block's centre in a frame fixed for the block: a frame turned with
// the position, dotted with the position itself, a planet's radius long, gave back the height
// above the centre, and every bed and tier ran as a contour line (the r8 review's layer cake).
struct Bedding {
    local: vec3<f32>,
    // The beds' normal, and the two level ways the upright joints face: along the beds'
    // level line (`strike`) and down their dip (`dip_way`).
    bed: vec3<f32>,
    strike: vec3<f32>,
    dip_way: vec3<f32>,
    key: vec3<f32>,
}

const FAULT_BLOCK_M: f32 = 1400.0;

// How dark a range seven kilometres off stands, its snow and rock, against near: at 0.78 the
// horns under the skyline stood brighter than the sky behind them (round 11).
const FAR_DIM: f32 = 0.62;
// The sunlit faces' warm ochre-grey, and how far their far dimming goes instead (round 14).
const LIT_ROCK: vec3<f32> = vec3<f32>(1.24, 1.05, 0.78);
const LIT_FAR_WALL: f32 = 0.78;
const LIT_FAR_DIM: f32 = 0.72;
// Past a few kilometres the lit faces' lift gives way to the air again (aerial perspective on the
// lit side, round 15): lifted to the horizon, the summits under the skyline stood as bright as
// the sky (the far gate fell from 0.111 to 0.053 in round 14).
const LIT_NEAR_FROM: f32 = 1200.0;
const LIT_NEAR_TO: f32 = 2500.0;
fn lit_near(far_off: f32) -> f32 {
    return 1.0 - smoothstep(LIT_NEAR_FROM, LIT_NEAR_TO, far_off);
}
// Aerial perspective (see the fragment's end): from how far, over how many metres it takes
// about two-thirds of the way to the air, and the air's colour against the haze's brightness.
// A clear day's air: 9 km to two-thirds is an extinction of 0.11 per km, Koschmieder's
// visibility 35 km, so a wall 3 km off keeps three-quarters of its light and a range at 14 km
// a fifth, receding in layers. At 2400 m, thickened so 14 km from 1.5 km showed some haze
// (round 13), a wall's foot kept a tenth of its light at 2 to 3 km and hid its rock (D434).
// At dusk the air keeps the thicker haze the developer prefers there (2026-10-22): the
// whole dusk's 2400 m, blended to the clear day's by the direct sun's brightness (`day`).
const AERIAL_FROM_M: f32 = 250.0;
const AERIAL_M: f32 = 9000.0;
const AERIAL_DUSK_M: f32 = 2400.0;
const AERIAL_AIR: vec3<f32> = vec3<f32>(0.5, 0.76, 1.32);
// How far the far snow's edge frays down the fall line (`fall_stripes`), against the slope.
const STREAKED: f32 = 0.6;
// How much more a steep wall's far snow breaks than its slope alone says (round 14).
const WALL_BARE: f32 = 0.3;
// How dark the rock's own cast shadows under its beds and in its joints stand (`jointing`).
const JOINT_SHADOW: f32 = 0.5;
const JOINT_RECESS: f32 = 0.35;
// How much of a far wall's folds, the sky its vertices see, is drawn in its rock's colour
// besides its sky light (D434).
const FOLD_TINT: f32 = 0.8;
const FOLD_LIFT: f32 = 1.15;
// A far snowfield's tint where it is turned from the sun: blue-grey, about half as bright.
const SHADED_SNOW: vec3<f32> = vec3<f32>(0.4, 0.5, 0.7);
// The direct light's exposed brightness under which the far snow's shading by the sun fades
// out, and over which it is whole: the dusk's rising moon is a few hundredths, the day's sun
// about one.
const DIRECT_FAINT: vec2<f32> = vec2<f32>(0.05, 0.3);
// How much brighter than the haze the aerial perspective's air stands at dusk.
const DUSK_AIR_LIFT: f32 = 1.8;
// Under a low sun still bright (`day` near 1), the air before a far range stands brighter
// and takes the hour's colour from the haze (`fog.base_color`, daylight.rs's low-sun keys):
// a long path through the lit air forward-scatters the low light, so a wall kilometres off
// lifts toward the sky behind it as the Tatras' and the Dolomites' do at golden hour. Lit by
// the noon sky's blue-grey alone, the settlement's far wall at 15 degrees stood one dark warm
// slab, eight levels over the forest under it (settlement r29). The sun's sine over the eye's
// horizon at which it begins and is whole, how much brighter, and how far toward the haze's hue.
const LOW_SUN: vec2<f32> = vec2<f32>(0.6, 0.2);
const LOW_SUN_AIR_LIFT: f32 = 2.6;
const LOW_SUN_HUE: f32 = 0.2;
// The low mist's heights over the eye between which it thins out, and over how many metres
// at its thickest it takes two-thirds of the way to the air, on top of the aerial perspective:
// thinned to a third in 140 m, it lay wholly behind the forest's wall and showed in no view
// (the r17 review); this way it lifts a range's lowest quarter or so seen over the trees.
// With the air, a valley's morning haze: visibility 11 km in it, so a wall's foot 3 km off
// keeps over a third of its light by day (D434; at 900 m it kept a sixtieth).
const FOOT_MIST_M: vec2<f32> = vec2<f32>(80.0, 750.0);
const FOOT_MIST_DEPTH_M: f32 = 4000.0;
const FOOT_MIST_DEPTH_DUSK_M: f32 = 900.0;
// Its share at dusk: whole, it lifted the lake's whole massif to one pale value, its rock
// as light as its snow (the r18 review).
const FOOT_MIST_DUSK: f32 = 0.3;
// How much of the way to the air the ground `past` metres beyond AERIAL_FROM_M and `over_eye`
// metres over the eye's height is taken, by day (`day` 1) or at dusk (0): the clear air and
// the low mist under it, both thinning from the dusk's depths to the day's as the sun brightens.
// terra_world's far_air.rs tests its rule on the CPU (D434).
fn air_through(past: f32, over_eye: f32, day: f32) -> f32 {
    let low_mist = (1.0 - smoothstep(FOOT_MIST_M.x, FOOT_MIST_M.y, over_eye)) / mix(FOOT_MIST_DEPTH_DUSK_M, FOOT_MIST_DEPTH_M, day) * mix(FOOT_MIST_DUSK, 1.0, day);
    return 1.0 - exp(-past / mix(AERIAL_DUSK_M, AERIAL_M, day) - past * low_mist);
}
// The heights above the sea over which a wall's fed snow goes from a low wall's to a horn's,
// as terra_world's terrain.rs `FED_FALLS`.
const FED_FALLS_M: vec2<f32> = vec2<f32>(1500.0, 2500.0);
// How wide a far snowfield's broken edge fades, and how much a fine grain frays it.
const BREAK_SOFT: vec2<f32> = vec2<f32>(0.4, 0.58);
const BREAK_GRAIN: f32 = 0.12;


fn bedding(world: vec3<f32>) -> Bedding {
    let warp = vec3<f32>(noise(world * 0.0007 + 301.0), noise(world * 0.0007 + 307.0), noise(world * 0.0007 + 311.0)) - 0.5;
    let key = floor((world + warp * 900.0) / FAULT_BLOCK_M);
    let centre = (key + 0.5) * FAULT_BLOCK_M;
    let up = normalize(centre);
    let raw = vec3<f32>(hash(key + 3.1), hash(key + 5.7), hash(key + 8.3)) - 0.5;
    let dip_way = normalize(raw - up * dot(raw, up) + vec3<f32>(1e-3, 0.0, 0.0));
    let dip = radians(35.0 + 35.0 * hash(key + 11.9));
    let bed = up * cos(dip) + dip_way * sin(dip);
    let strike = normalize(cross(up, dip_way));
    // Each fault throws its block's beds a share of a stratum and more against the next.
    let fault_throw = bed * (hash(key + 14.3) - 0.5) * 260.0;
    return Bedding(world - centre + fault_throw, bed, strike, dip_way, key);
}

fn jointing(world: vec3<f32>, up: vec3<f32>, spacing: f32, soft: f32, sun: vec3<f32>) -> Joints {
    // The beds tilted in their fault block (`bedding`); the upright joints cut across them in
    // two sets: one running on through the beds, so the wall stands in upright slabs as the
    // Odle's and the Matterhorn's do, the other staggered bed to bed as a wall's courses are.
    let b = bedding(world);
    // Each block jointed its own way (D426, round 10): its spacing, how far its sets turn off
    // the beds' strike and dip, and the warp of the rock: one spacing and frame through every
    // block, the near wall read as a quilt of equal square tiles (the r9 review).
    let s_bed = spacing * (0.7 + 0.6 * hash(b.key + 21.1));
    let s1 = spacing * (0.6 + 0.9 * hash(b.key + 23.3));
    let s2 = spacing * (1.1 + 1.3 * hash(b.key + 25.9));
    let swing = (hash(b.key + 27.7) - 0.5) * 1.1;
    let way1 = b.strike * cos(swing) + b.dip_way * sin(swing);
    let way2 = b.dip_way * cos(swing * 0.7 + 0.35) - b.strike * sin(swing * 0.7 + 0.35);
    // The rock warped tens of metres at a time, so a joint runs curved and a bed pinches.
    let warp = (vec3<f32>(noise(world * (0.11 / spacing) + 401.0), noise(world * (0.11 / spacing) + 409.0), noise(world * (0.11 / spacing) + 419.0)) - 0.5) * spacing * 1.2;
    let at = b.local + warp;
    let wobble = (noise(world * (0.9 / spacing) + 229.0) - 0.5) * 0.3;
    let tb = dot(at, b.bed) / s_bed + wobble + noise(world * (0.15 / spacing) + 231.0) * 0.8;
    let course = floor(tb);
    // The running set runs on through a few beds at a time, then steps, so its slabs stand at
    // heights of their own.
    let run = floor(tb / (2.0 + 3.0 * hash(b.key + 29.9)) + hash(b.key + 31.3));
    let t1 = dot(at, way1) / s1 + hash(vec3<f32>(run, 6.1, 2.7) + b.key * 7.0) * 5.0 + noise(world * (0.3 / spacing) + 233.0) * 0.9;
    let t2 = dot(at, way2) / s2 + hash(vec3<f32>(course, 4.9, 8.3) + b.key * 7.0) * 7.0 + noise(world * (0.25 / spacing) + 239.0) * 1.1;
    let cell = floor(vec3<f32>(tb, t1, t2)) + b.key * 13.0;
    let f = fract(vec3<f32>(tb, t1, t2));
    let h = vec3<f32>(hash(cell + 41.7), hash(cell + 53.3), hash(cell + 67.1)) - 0.5;
    // A block whose fracture face is turned up holds snow (or turf) on a lip along its top,
    // as wide as its own hash says, broken along the lip where the block's top is cut away,
    // the riser under it bare: one in four, so a wall is flecked, not banded.
    let holds = step(hash(cell + 9.1), 0.25);
    let lip = 0.55 + 0.3 * hash(cell + 13.7);
    let broken = smoothstep(0.4, 0.6, noise(world * (2.2 / spacing) + 241.0) + (hash(cell + 17.3) - 0.5) * 0.4);
    let ledge = holds * broken * smoothstep(lip - soft, lip + 0.04 + soft, f.x + (noise(world * (1.3 / spacing) + 243.0) - 0.5) * 0.3);
    // The joints open: the running set wide and dark, the staggered one narrower, a bed open
    // less often.
    let bed_open = step(0.75, hash(vec3<f32>(course, 3.3, 9.7) + b.key * 7.0));
    let bed_edge = mix(1.0, min(f.x, 1.0 - f.x) * 1.4, bed_open);
    let joint_edge = min(min(f.y, 1.0 - f.y) * 0.8, min(f.z, 1.0 - f.z) * 1.3) * 1.2;
    let crack = 1.0 - smoothstep(0.0, 0.025 + soft, min(bed_edge, joint_edge));
    // Each slab's face turned its own way, more across the wall than up or down it, so the
    // slabs read upright.
    // Turned by a smooth field more than block by block: a hard turn a block, each lit its own
    // tone, the wall read as a quilt of tiles (the r9 review), where only the joints should
    // show as lines.
    let sway = vec3<f32>(noise(world * (0.45 / spacing) + 431.0), noise(world * (0.45 / spacing) + 433.0), noise(world * (0.45 / spacing) + 439.0)) - 0.5;
    var turn = (sway - up * dot(sway, up)) * 1.1 + (h - up * dot(h, up)) * 0.25 + way1 * (h.y * 0.2);
    turn = mix(turn, b.bed * 0.8 + turn * 0.3, ledge);
    // The shadows the rock casts on itself (D426, round 13): each bed's lip overhangs the
    // riser of the bed under it, so a band under every open bed lies in shade, tilted with
    // the beds; and each upright joint of the running set is a recess whose wall turned from
    // the sun is dark, the shadow longer the more the sun comes across the wall. Shading by
    // the normal alone, the wall's beds and joints read as lines drawn on it (the r12 review).
    let under_lip = 1.0 - smoothstep(0.0, 0.16 + soft, 1.0 - f.x);
    let bed_shadow = max(bed_open, step(0.45, hash(vec3<f32>(course, 5.1, 1.3) + b.key * 7.0))) * under_lip;
    // Only the open joints, about two in five, each over its own run of beds: every joint's
    // shadow combed the walls into an even curtain of upright pinstripes (round 13).
    let across = dot(way1, sun);
    let from_joint = select(1.0 - f.y, f.y, across > 0.0);
    let joint_open = step(0.6, hash(vec3<f32>(floor(t1), run, 1.9) + b.key * 7.0));
    let joint_shadow = joint_open * (1.0 - smoothstep(0.0, 0.06 + 0.3 * abs(across) + soft, from_joint));
    let shadow = max(bed_shadow * (1.0 - ledge), joint_shadow) * smoothstep(-0.1, 0.3, dot(up, sun));
    // Recesses see less sky, so open joints and the undersides of open beds stand dark in the
    // shade too, where a shaded wall without them read as one smeared plane (the r13 review).
    let in_joint = joint_open * (1.0 - smoothstep(0.0, 0.1 + soft, min(f.y, 1.0 - f.y)));
    let recess = max(bed_open * under_lip * (1.0 - ledge), in_joint);
    return Joints(turn, crack, ledge, shadow, recess);
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
    // How much snow lies here, the far rock's or the weather's: a far wall's folds dim its
    // rock's sky light and not its snow's (D434).
    var snowed = 0.0;
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
            let map = textureSampleLevel(cover_map, map_sampler, at, 0.0);
            // The map's grass the most the land's may grow, or, where its cover sows a meadow
            // (`CoverSource::sows`), the ground's own over the land's bare patches.
            let sown = select(min(grows.r, map.r), map.r, ground.cover_on == 2u);
            grows.r = mix(grows.r, sown, inside);
            grows.g = mix(grows.g, map.g, inside);
            litter = map.b * inside;
            moss = map.a * inside;
            blades = map.r * inside;
            let more = textureSampleLevel(cover_more, map_sampler, at, 0.0);
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
        var scree_full = 0.0;
        var gravel = 0.0;
        var bare = 0.0;
        var snow = 0.0;
        var in_snow_m = 0.0;
        // How far into a wall's couloir, its avalanche snow's (terrain.rs `couloir_catch`).
        var in_couloir = 0.0;
        var woods = 0.0;
@if(VERTEX_UVS_A)
        {
            let loose = in.uv.x + (blotch - 0.5) * 0.6;
            scree = smoothstep(0.3, 0.7, 1.0 - abs(loose - 1.0));
            scree_full = scree;
            gravel = smoothstep(0.3, 0.7, loose - 1.0);
            // A lowland's loose stones thin to a quarter of a mountain's, so a lowland lake's
            // shingle (the land lays it round every lake) is a scatter in its meadow, not a
            // grey band; even where a far chunk keeps no height over the water (below).
            let low_loose = 1.0 - 0.75 * lowland_at(from_centre - ground.sea_radius_m);
            scree *= low_loose;
            gravel *= low_loose;
            // Snow lies on what faces up, its edge broken, thinning off the steep.
            let up_facing = smoothstep(0.4, 0.72, flatness + (fleck - 0.5) * 0.12);
            // Its edge broken a metre or two by noise, as a patch of old snow's is, so the
            // vertices' depths never show as triangles: the depth, laid on vertices a metre or
            // two apart and changing fast across a drift's edge, is only a share of where it
            // lies, the noise the rest. On a fell's face the patch thins out into fingers
            // round the ground's ribs rather than ending in a cut-out line.
            let edge = noise(world * 0.35 + 91.0) * 0.5 + noise(world * 1.9 + 23.0) * 0.3 + noise(world * 0.06 + 5.0) * 0.2;
            in_snow_m = in.uv.y;
            in_couloir = clamp(-in.uv.x, 0.0, 1.0);
            let deep = clamp((in.uv.y - 0.02) / 0.3, 0.0, 1.0);
            // On a fell's steeper faces thin snow keeps to the lees and gullies, the ribs
            // between bare: patchy by a noise tens of metres across, rather than an even cap.
            let ribs = noise(world * 0.018 + 51.0) * 0.6 + noise(world * 0.05 + 53.0) * 0.4;
            let shed = (1.0 - smoothstep(0.75, 0.95, flatness)) * (ribs - 0.45) * 1.4;
            let patches = (noise(world * 0.0032 + 71.0) * 0.65 + noise(world * 0.009 + 73.0) * 0.35 - 0.5) * 1.3 * (1.0 - smoothstep(0.75, 0.95, flatness));
            snow = smoothstep(0.3, 0.7, deep + (edge - 0.5) * 1.1 + (fleck - 0.5) * 0.12 - max(shed, 0.0) - max(patches, -0.15)) * up_facing;
        }
@if(VERTEX_UVS_B)
        {
            bare = smoothstep(0.25, 0.6, in.uv_b.x + (fleck - 0.5) * 0.3 + (blotch - 0.5) * 0.3);
            woods = clamp(-in.uv_b.x, 0.0, 1.0);
            // A lowland lake's banks are silt, soil and grass, not the shingle the land lays
            // round every lake: its gravel and scree give out a few metres over its water.
            let banks = lowland_at(from_centre - ground.sea_radius_m) * (1.0 - smoothstep(3.0, 7.0, in.uv_b.y));
            gravel *= 1.0 - banks;
            // Away from a lake a lowland's scree is a wall's talus apron (field.rs), shown whole:
            // thinned to a quarter, the forest seemed to run onto the wall (round 15).
            scree = mix(scree, scree_full, smoothstep(20.0, 45.0, in.uv_b.y));
            scree *= 1.0 - banks;
        }
        // Far off, under the haze, the snow and the rock stand darker than the sky (looks.ts's
        // "far under sky"): past a few kilometres the range is in its own faces' shade more than not.
        let snow_far = smoothstep(1000.0, 6000.0, distance(world, view.world_position));
        var snow_tint = vec3<f32>(0.71, 0.75, 0.76) * (0.92 + 0.12 * fleck);
        var snow_lit = 0.0;
        // Both darkenings stand in for a sun's light falling on some faces and not others, so
        // they go with the direct light's strength: at dusk, the moon barely risen, all of a
        // range is lit by the sky alone, and the shaded tint and the far dim took its snow
        // down to its rock's own value (lake film r16, "camouflage").
        var by_sun = 1.0;
        // The sunlit snow far less than the shaded (round 14): dimmed alike, it read grey.
        // A far face's snow turned from the sun is blue-grey, lit by the sky alone, and darker
        // than the sunlit snow by more than the light's own fall-off: the sky-lit shade the
        // references' snowfields keep, where ours stood one paper-white (the r12 review).
@if(!PREPASS_PIPELINE)
        {
            if lights.n_directional_lights > 0u {
                let light = lights.directional_lights[0];
                by_sun = smoothstep(DIRECT_FAINT.x, DIRECT_FAINT.y, dot(light.color.rgb * view.exposure, vec3<f32>(0.2126, 0.7152, 0.0722)));
                let sunward = dot(normalize(in.world_normal), light.direction_to_light);
                let shaded = smoothstep(0.5, 0.05, sunward) * smoothstep(300.0, 900.0, distance(world, view.world_position)) * by_sun;
                snow_tint *= mix(vec3<f32>(1.0), SHADED_SNOW, shaded);
                snow_lit = smoothstep(0.1, 0.6, sunward);
            }
        }
        snow_tint *= mix(1.0, mix(FAR_DIM * 0.9, LIT_FAR_DIM, snow_lit * lit_near(distance(world, view.world_position))), snow_far * by_sun);
        // Far off, past where its trees are drawn, woods are the ground's own colour: a dark
        // conifer canopy (the far crowns' colour, linear), so a forested slope reads forested to
        // the horizon (docs/demos.md §1); nearer, the cards stand on it and it is darker
        // under them a little.
        let far_off = distance(world, view.world_position);
        let canopy = vec3<f32>(0.018, 0.036, 0.016) * (0.85 + 0.3 * blotch);
        let under_woods = woods * mix(0.25, 1.0, smoothstep(3200.0, 4400.0, far_off)) * smoothstep(300.0, 700.0, far_off);
        // The mountains' granite: grey, as the Tatras' is, the steep ground's own brightness
        // without its earthy hue; darker streaks run down its faces where water and lichen
        // stain it, and broad tones a few hundred metres across, so a wall seen from across a
        // lake is not one flat colour.
        let face = normalize(in.world_normal);
        let level_face = face - up * dot(face, up);
        let fall = select(normalize(level_face), up.zxy, dot(level_face, level_face) < 1e-6);
        let across_face = normalize(cross(up, fall));
        // The streaks laid down the fall cell by cell, as the far ribs are (`fall_stripes`),
        // in patches: read as a noise of the distance across the fall and of `dot(world, up)`,
        // which is the distance from the centre, they were a noise of height alone, dark bands
        // at one height straight across every wall (D395).
        let wall = 1.0 - smoothstep(0.55, 0.85, flatness);
        // Only where a pixel spans a metre or two: past that the 23 m lines alias into evenly
        // spaced vertical seams down a far face (the r17 review).
        var streaks = 0.0;
        let streaks_seen = wall * (1.0 - smoothstep(1.2, 3.0, max(length(dpdx(world)), length(dpdy(world)))));
        if streaks_seen > 0.001 {
            let lines = fall_stripes(world, across_face, 23.0, 5.0).x;
            streaks = smoothstep(-0.25, -0.8, lines) * smoothstep(0.35, 0.65, noise(world * 0.012 + 7.0)) * streaks_seen;
        }
        let stained = 1.0 - 0.4 * streaks;
        let tones = 0.66 + 0.5 * (noise(world * 0.0016 + 53.0) * 0.6 + noise(world * 0.005 + 59.0) * 0.4);
        // Granite as the Tatras' walls are seen across a valley, a mid grey under the snow,
        // not the pale slab 1.3 times as bright read as.
        let grey = dot(ground.steep.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)) * 0.85;
        let granite = vec3<f32>(grey, grey, grey * 0.97) * (0.85 + 0.3 * blotch) * stained * tones;
        // Scree is the wall's rock broken small, and fresh: paler than the weathered face over
        // it, so a fan under a gully reads as a pale cone against the dark cliffs and the turf,
        // as the Tatras' and the Dolomites' do.
        let scree_tint = vec3<f32>(grey, grey, grey * 0.96) * 1.55 * (0.85 + 0.3 * fleck) * tones;
        // A wall is not one rock (the developers, 2026-10-17: "the face of the mountain needs a
        // bit of variety too"). Seen across a valley it stands in tiers: its rock in beds and
        // sheets of different thickness, each its own tone, the tops of some ledges that catch
        // the light and hold snow or turf, the bands following the height but warped and
        // tilted over kilometres, strong on some faces and faint on others; and its rock in
        // patches of different weathering, rust-stained, dark with lichen, and pale where it
        // fell away fresh. Drawn in the colour and the light, not the mesh: terraces cut into
        // the ground held water as ponds (research/2026-10-16-far-mountains.md).
        let above_sea_m = from_centre - ground.sea_radius_m;
        let rocky = max(bare, steep) * (1.0 - under_woods);
        let footprint_m = max(length(dpdx(world)), length(dpdy(world)));
        // How many metres of height a pixel spans: a far wall seen from below is foreshortened
        // across, but its tiers, which run by height, hold.
        let height_px = abs(dpdx(above_sea_m)) + abs(dpdy(above_sea_m));
        // The face's tint over the rock and steep ground it covers, applied after the layers:
        // a far wall's mesh is too coarse to measure 40 degrees, so it is drawn as the steep
        // layer more than as bare granite.
        var face_tint = vec3<f32>(1.0);
        var ledge = 0.0;
        var tier_tilt = 0.0;
        var tier_up = up;
        var joint_turn = vec3<f32>(0.0);
        var joint_crack = 0.0;
        var joint_shadow = 0.0;
        var joint_recess = 0.0;
        var sun_dir = up;
@if(!PREPASS_PIPELINE)
        {
            if lights.n_directional_lights > 0u {
                sun_dir = lights.directional_lights[0].direction_to_light;
            }
        }
        if rocky > 0.001 {
            // The beds folded, not laid level (the settlement r19 review: "a layer cake of hard
            // strata"): bent hundreds of metres over a couple of kilometres and a couple of
            // hundred over a few hundred metres, so a band climbs a wall at 15 to 35 degrees,
            // turns over and falls again, as the folded beds of Skellige's cliffs and the
            // Dolomites' faces do; wavered by metres over tens. A scalar fold, not a dip vector
            // dotted with the position: that one's gradient grew with the distance from the
            // planet's centre, a noise of tens of metres a metre.
            // Since D426's round 9 the tiers are the fault block's tilted beds (`bedding`),
            // folded a little: keyed to the height, folded or not, they ringed every horn.
            let fold = (noise(world * 0.0016 + 37.0) - 0.5) * 90.0 + (noise(world * 0.011 + 41.0) - 0.5) * 16.0;
            let tier_beds = bedding(world);
            tier_up = tier_beds.bed;
            let tier_at = dot(tier_beds.local, tier_beds.bed) + hash(tier_beds.key + 19.0) * 95.0 + fold + (noise(world * 0.0045 + 19.0) - 0.5) * 36.0;
            // Tiers of different heights, about 95 m on average: each a cliff band, its riser
            // standing steeper than the wall and darker, a bench on top of it facing the sky,
            // a fifth to a third of its height, that holds snow or turf. Drawn in the light,
            // as the riser's and the bench's normals, not in the tone alone: tone did not show
            // through the haze at 3 to 5 km, and the benches read as hairlines (the r3 review).
            // Only some tiers stand out, in runs of a few hundred metres along a wall, cut
            // where a couloir or a gully runs down through them, so no band runs a whole wall.
            let tiers = tier_at / 95.0;
            let tier_f = fract(tiers);
            let tier_r = hash(vec3<f32>(floor(tiers), 7.1, 5.3));
            let tier_b = hash(vec3<f32>(floor(tiers), 2.9, 13.1));
            let beds = tier_at / 29.0;
            let bed_r = hash(vec3<f32>(floor(beds), 3.7, 11.9));
            let banded = smoothstep(0.25, 0.55, noise(world * 0.0007 + 61.0) * 0.7 + noise(world * 0.003 + 67.0) * 0.3);
            let seen = 1.0 - smoothstep(18.0, 40.0, height_px);
            // Only on the cliffs, past about 50 degrees: on the gentler rock and its ledges the
            // tiers ruled every slope in contour lines (the r5 review).
            let far_tiers = far_strata(far_off, tier_f, height_px);
            var strength = max(banded, far_tiers.z) * seen * (1.0 - smoothstep(far_tiers.x, far_tiers.y, flatness));
            if strength > 0.001 {
                let couloir = fall_stripes(world, across_face, 150.0, 0.0).x;
                let gully = fall_stripes(world, across_face, 62.0, 1.0).x;
                strength *= smoothstep(-0.75, -0.3, couloir) * mix(1.0, smoothstep(-0.8, -0.2, gully), 0.75);
            }
            let runs = smoothstep(0.5, 0.62, noise(world * 0.0019 + 43.0) * 0.6 + noise(world * 0.0065 + 47.0) * 0.4);
            let standing = step(0.55, tier_r) * max(runs, far_tiers.z) * strength;
            let bench_from = 1.0 - (0.18 + 0.17 * tier_b);
            let bench = smoothstep(bench_from - 0.02, bench_from + 0.03, tier_f) * (1.0 - smoothstep(0.985, 1.0, tier_f));
            // How far the normal turns: the riser toward the level, steeper; the bench is shaded
            // as the wall round it, so no level line of light rules the face (the r5 review).
            tier_tilt = standing * rocky * -0.35 * (1.0 - bench);
            // Each tier its own tone and each bed within it a little, the riser's foot in the
            // corner under the bench below it in shade, the bench's lip catching the light.
            var band_tone = mix(1.0, mix(0.7, 1.15, tier_r) * mix(0.9, 1.06, bed_r), strength);
            band_tone *= mix(1.0, 0.85, (1.0 - smoothstep(0.0, 0.12, tier_f)) * standing);
            band_tone *= mix(1.0, far_tiers.w, standing);
            // The benches dusted with snow or turf only in patches: whole, they ruled the faces
            // with bright lines.
            ledge = 0.0;
            // The wall's joints and bedding (`jointing`, D426): strata 40 m thick drawn across
            // a valley, 16 m ones nearer, each while its blocks span pixels enough; only on the
            // walls, so a shoulder's snow and turf are left whole.
            let walled = rocky * (1.0 - smoothstep(0.62, 0.78, flatness));
            if walled > 0.001 {
                let big = jointing(world, up, 40.0, clamp(footprint_m / 40.0, 0.01, 0.3), sun_dir);
                let small_keep = 1.0 - smoothstep(2.0, 4.0, footprint_m);
                let small = jointing(world + 17.0, up, 16.0, clamp(footprint_m / 16.0, 0.01, 0.3), sun_dir);
                joint_turn = (big.turn + small.turn * 0.6 * small_keep) * walled;
                // Great blocks a hundred metres and more across, each face turned its own way and
                // their tops ledges, so a wall reads as slabs from across a valley, where the
                // 40 m set shows as a pinstripe. Their joints not drawn: warped at that size,
                // they netted the walls in wavy lines (round 12).
                let great = jointing(world + 53.0, up, 140.0, clamp(footprint_m / 140.0, 0.01, 0.3), sun_dir);
                joint_turn += great.turn * 0.9 * walled;
                joint_crack = max(big.crack, small.crack * small_keep) * walled;
                joint_shadow = max(max(big.shadow * 0.6, small.shadow * small_keep), great.shadow) * walled;
                joint_recess = max(big.recess * 0.8, small.recess * small_keep * 0.6) * walled;
                ledge = max(max(big.ledge, small.ledge * small_keep), great.ledge) * walled;
            }
            // Patches of weathering a few hundred metres across: iron-stained rust, dark
            // lichen and wet rock, and pale fresh scars where a slab fell away; and a far wall
            // a weathered mid grey, darker than the steep layer's fresh scan, as the Tatras'
            // granite and the Dolomites' shaded faces are.
            let patch_a = noise(world * 0.0019 + 83.0) * 0.65 + noise(world * 0.0061 + 89.0) * 0.35;
            let patch_b = noise(world * 0.0013 + 97.0) * 0.6 + noise(world * 0.0047 + 101.0) * 0.4;
            let patch_c = noise(world * 0.0037 + 107.0) * 0.7 + noise(world * 0.011 + 109.0) * 0.3;
            let rust = smoothstep(0.58, 0.78, patch_a);
            let lichen = smoothstep(0.5, 0.7, patch_b);
            let scar = smoothstep(0.64, 0.74, patch_c) * (1.0 - lichen);
            var weathered = mix(vec3<f32>(1.0), vec3<f32>(1.22, 0.98, 0.72), rust);
            weathered *= mix(vec3<f32>(1.0), vec3<f32>(0.4, 0.45, 0.4), lichen);
            weathered = mix(weathered, vec3<f32>(1.65, 1.62, 1.55), scar);
            // Dimmed from 1 km, so the horns under the skyline stand darker than the sky (round 11).
            // Faces by their aspect: those turned to the sun warmer, a pale ochre over the
            // grey, those turned from it cooler and bluer, as the Tatras' granite at morning
            // and Skellige's cliffs are, so a range reads by its faces, not its tiers.
            var aspect = vec3<f32>(1.0);
            var lit = 0.0;
@if(!PREPASS_PIPELINE)
            {
                if lights.n_directional_lights > 0u {
                    let sun = lights.directional_lights[0].direction_to_light;
                    // The block's own fracture face, so faces turned from the sun break dark.
                    let sunward = dot(normalize(face + joint_turn * 0.8), sun);
                    lit = smoothstep(0.05, 0.6, sunward);
                    aspect = mix(vec3<f32>(1.0), LIT_ROCK, lit);
                    // In the shade a face is lit by the sky and the sunlit rock round it, more
                    // the nearer it turns to the sun's side: its slabs and buttresses keep tones
                    // of their own there, where by the sky alone a shaded wall read as one
                    // smeared plane (the r13 review).
                    let bounce = mix(0.72, 1.18, smoothstep(-0.75, 0.1, sunward));
                    aspect *= mix(vec3<f32>(1.0), vec3<f32>(0.44, 0.5, 0.66) * bounce, smoothstep(0.2, -0.3, sunward));
                }
            }
            // Dimmed from 1 km, so the horns under the skyline stand darker than the sky (round 11);
            // the sunlit faces far less than the shade since the aerial perspective holds the far
            // gate (round 14): dimmed alike, the range read overcast against a sunny meadow.
            let lit_lift = lit * lit_near(far_off);
            let far_wall = mix(1.0, mix(0.44, LIT_FAR_WALL, lit_lift), smoothstep(300.0, 900.0, far_off)) * mix(1.0, mix(FAR_DIM, LIT_FAR_DIM, lit_lift), smoothstep(1000.0, 6000.0, far_off));
            face_tint = mix(vec3<f32>(1.0), weathered * band_tone * far_wall * aspect, rocky);
        }
        // Far off, a mountain's ground is meshed tens of metres a cell, too coarse for the
        // ribs and couloirs that split its walls (terra_voxel mountains.rs), so a wall a few
        // kilometres off read as smooth felt. Its rock and steep ground take their ribs in
        // the light instead: a detail normal running down the fall line, as games' far
        // terrain carries erosion detail in its normal maps, and darker in the gullies,
        // which see less sky; and its snow lies in the gullies, off the ribs, as a granite
        // wall's does in summer. It fades in past where the mesh holds the relief itself.
        // Its snowfields too: from across a valley a summer's snow lies in streaks down the
        // gullies, not in the round patches the far mesh's vertices give it.
        let ribbed = max(max(bare, steep), snow) * smoothstep(150.0, 450.0, far_off) * (1.0 - under_woods);
        var rib = 0.0;
        var rib_tilt = 0.0;
        // Dark wet streaks down the middle set of gullies on some faces, where water runs and
        // the rock stays wet and black with algae, as down the Dolomites' and Tatras' walls.
        var wet_far = 0.0;
        // How far down a great couloir's line a point lies, 0 off it to 1 down its middle.
        var fan = 0.0;
        // How white a wall's couloir lies with avalanche snow.
        var couloir_fill = 0.0;
        // How many metres of ground a pixel spans, for each set of ribs to fade out before a
        // pixel holds a whole rib and gully: finer, they sparkled as a grain over the wall.
        let footprint = max(length(dpdx(world)), length(dpdy(world)));
        var ledge_cut = 0.0;
        if ribbed > 0.0 {
            // Three sets of ribs, the great couloirs between a wall's buttresses split by
            // narrower gullies and those by runnels, each laid cell by cell as the ground's own
            // gullies are (`fall_stripes`), and cut deeper on some faces than others. Measured
            // as a distance across the fall from the planet's centre, as they were, their phase
            // followed the fragment's normal: a degree's turn between two triangles moved it by
            // half a kilometre, and each triangle of the far mesh drew its own set, a quilt of
            // chevrons across every wall.
            let sets = array<vec3<f32>, 3>(vec3<f32>(150.0, 0.5, 0.0), vec3<f32>(62.0, 0.32, 1.0), vec3<f32>(26.0, 0.05, 2.0));
            var weight = 0.0;
            var rise = 0.0;
            var couloirs = 1.0;
            var couloirs_drawn = 0.0;
            var gullies_far = 1.0;
            // The far mesh's cells, about 16 m at 3 km and 32 m at 6 km: a set of gullies at
            // least four or five of them across stands in the mesh itself, the ground's own
            // gullies (mountains.rs), with its snow lying in their beds (terrain.rs `lying`).
            // Drawn here too, a second set of other gullies the same size broke up the mesh's,
            // and the two together read as an egg-crate of round bumps capped with snow.
            let mesh_cell = far_off / 190.0;
            for (var k = 0; k < 3; k++) {
                let band = sets[k];
                let unheld = smoothstep(band.x * 0.2, band.x * 0.3, mesh_cell);
                // Each set strong on some faces and nearly gone from others, so one wall is cut
                // by broad couloirs, the next by close runnels, a third a smooth slab: drawn all
                // alike, every face was combed at one width (the r2 review).
                let patchy = 0.08 + 0.92 * smoothstep(0.3, 0.7, noise(world * 0.0017 + band.z * 13.0) * 0.7 + noise(world * 0.0051 + band.z * 7.0) * 0.3);
                let keep = band.y * unheld * patchy * (1.0 - smoothstep(band.x * 0.1, band.x * 0.25, footprint));
                if keep > 0.001 {
                    let stripes = fall_stripes(world, across_face, band.x, band.z);
                    rib += keep * stripes.x;
                    rise += keep * stripes.y;
                    weight += keep;
                    if k == 0 {
                        couloirs = stripes.x;
                        couloirs_drawn = keep / band.y;
                    }
                    if k == 1 {
                        gullies_far = stripes.x;
                    }
                }
            }
            rib /= max(weight, 0.001);
            let deep = 0.55 + 0.9 * smoothstep(0.3, 0.7, noise(world * 0.0021 + 17.0));
            // A gully's sides face across the slope: the ribs' slope across, up to about 35
            // degrees, tilts the normal toward or away from `across_face`.
            rib_tilt = clamp(rise * deep * 3.0, -0.45, 0.45) * ribbed;
            // A summer's snow on a wall lies in its great couloirs, in streaks down them, and
            // is gone from the buttresses between: past where the mesh holds them, in these.
            // The faces' ledges too (D426): level, the vertices' snow lay along every one of
            // them, ruling the range in white dashes, a layer cake (the settlement r19 review).
            // So far off, on a wall's rock, ledges and all (the land calls a stepped wall's ledges
            // bare, `Rise::wall`), thin snow keeps to the couloirs whether or not this set of
            // them is drawn here; a deep snowfield stays, and so do the summits' caps, which
            // are not wall (keyed by height, they were lost under the range's tops, r5).
            // Only on the walls past about 55 degrees (50 until round 12, which left 7 percent snow): keyed to bare rock alone, it took the snow
            // off the horns' gentler shoulders, saddles and cirque floors, which the land lays
            // in stone too, and the summits stood bare (the r6 review).
            // A deep snowfield is spared only on the gentler walls: on those past about 55 degrees
            // the horns' fed snow, metres deep, stood unbroken in white caps under the skyline,
            // brighter than the sky, where a summer's faces are two-thirds rock (D425, round 11).
            ledge_cut = ribbed * bare * (1.0 - smoothstep(0.4, 1.2, in_snow_m) * smoothstep(0.52, 0.66, flatness)) * (1.0 - smoothstep(0.5, 0.62, flatness));
            snow *= mix(1.0, smoothstep(0.35, -0.5, couloirs), max(ribbed * mix(0.75, 1.0, steep) * couloirs_drawn, 0.9 * ledge_cut));
            wet_far = smoothstep(-0.3, -0.85, gullies_far) * smoothstep(0.45, 0.7, noise(world * 0.0031 + 113.0)) * ribbed * rocky;
            fan = smoothstep(0.0, -0.55, couloirs + (blotch - 0.5) * 0.3) * couloirs_drawn;
            // Avalanche snow fills a wall's couloirs between its ribs (D426, round 9), where its
            // fed depth lies, whatever the cuts above took off the faces: the Matterhorn's and the
            // Tatras' couloirs white in summer between dark ribs.
            couloir_fill = smoothstep(-0.05, -0.5, couloirs + (blotch - 0.5) * 0.3) * couloirs_drawn * smoothstep(0.02, 0.25, in_snow_m) * smoothstep(0.12, 0.3, flatness) * 0.95;
        }
        // Under the walls, on the ground of 20 to 33 degrees their foot eases into, the
        // couloirs' lines run on as pale scree fans and talus, and between them alpine turf
        // grows below the bare heights: the Tatras' and the Odle's skirts of grey cones and
        // green between, where the faces met the forest bare (the r2 review).
        let skirt_slope = smoothstep(0.83, 0.87, flatness) * (1.0 - smoothstep(0.93, 0.97, flatness));
        let skirt = skirt_slope * smoothstep(300.0, 800.0, far_off) * (1.0 - woods) * step(0.001, ribbed);
        let talus = skirt * fan;
        // The faces' ledges hold no pale scree in lines either: it lies in the fans.
        scree *= 1.0 - 0.8 * ledge_cut * (1.0 - fan);
        scree = max(scree, smoothstep(0.2, 0.5, talus));
        let skirt_turf = skirt * (1.0 - fan) * smoothstep(0.35, 0.6, noise(world * 0.006 + 151.0) * 0.6 + noise(world * 0.02 + 157.0) * 0.4)
            * (1.0 - smoothstep(1900.0, 2300.0, above_sea_m + (blotch - 0.5) * 200.0));
        // Far snow is not a smooth cap: rock breaks through it in outcrops tens of metres
        // across, more the steeper the ground, and its edges are sharp, as a summer's snow on
        // the Tetons' and the Tatras' tops is. Near, the vertices' snow and its frayed edge
        // above hold it.
        let far_snow = smoothstep(250.0, 700.0, far_off);
        // How far the far snow's outcrops break a wall's couloir fill too (below).
        var far_broken = 0.0;
        if (snow > 0.0 || in_couloir > 0.0) && far_snow > 0.0 {
            // Drawn out down a wall, two and a half times as tall as wide, so its rock and snow
            // streak down the faces rather than lying in round spots over them (the r13 review);
            // four times ran in zebra stripes (r16); three times as tall, they cut gullies. A gentle
            // snowfield's outcrops stay round. Mostly the broad noise, so the snow lies in whole
            // fields, and keyed more by the slope: led by the noise of tens of metres, a face's
            // snow lay in round blobs with holes, read as a face (the r17 review).
            let drawn = world - up * dot(world, up) * 0.75 * (1.0 - smoothstep(0.6, 0.85, flatness));
            let outcrop = noise(drawn * 0.013 + 131.0) * 0.3 + noise(drawn * 0.041 + 137.0) * 0.25 + noise(drawn * 0.0045 + 139.0) * 0.45;
            // Fewer the higher and the deeper the snow lies, so the highest ground keeps the
            // most: cut alike, the steep summits came out barer than the lower tops behind
            // them, a snowline running backwards (the r3 review).
            // Held so far, the horns' fed snow, metres deep, broke nowhere and capped them in
            // solid white under the skyline, brighter than the sky: about two-fifths of a summit's
            // dome now breaks, two-thirds of a steep face (D425, round 11).
            let held = smoothstep(2000.0, 2500.0, above_sea_m) * 0.1 + smoothstep(0.3, 2.0, in_snow_m) * 0.05;
            // Round 12: by the slope far more than by the noise, so a gentle shoulder, col or
            // ledge keeps its snow whole bar a few outcrops and a wall past about 50 degrees
            // breaks all but its couloirs: at 0.8 the shoulders broke as much as the walls and
            // the range held snow on 7 percent of its rock against D425's third.
            // Its edges frayed down the fall line, rock breaking through on the ribs between
            // streaks: keyed to the slope alone, on the left range's stepped shoulders the snow
            // lay in even level bands along the steps (the r12 review).
            let streaks = fall_stripes(world, across_face, 70.0, 2.0).x;
            // A wall past about 55 degrees bare bar its couloirs and ledges (round 14).
            let wall_bare = (1.0 - smoothstep(0.5, 0.66, flatness)) * WALL_BARE;
            // Its edge frayed by a grain a few metres across and faded over more of the noise:
            // cut at one value over 0.04 of it, the streaks stood hard-edged as paint (the r16 review).
            let grain = (noise(world * 0.11 + 141.0) - 0.5) * BREAK_GRAIN;
            // A low wall's slope breaks it as round 16 did, under half as hard: its fed snow
            // lies in streaks on its benches and gullies, as terrain.rs feeds it (`FED_FALLS`);
            // broken as the horns' faces, the town's foothills lay bare and flat (settlement r28).
            let by_slope = mix(1.7, 3.0, smoothstep(FED_FALLS_M.x, FED_FALLS_M.y, above_sea_m));
            let breaks = smoothstep(BREAK_SOFT.x, BREAK_SOFT.y, (outcrop - 0.2) * 0.7 + (0.86 - flatness) * by_slope - held + (streaks - 0.55) * STREAKED + wall_bare + grain);
            snow *= 1.0 - breaks * far_snow;
            // The couloirs by the outcrops alone, about a third of them: keyed by the steepness
            // too, the walls' fill broke whole and the range stood bare (round 11).
            far_broken = smoothstep(0.56, 0.6, outcrop) * far_snow;
            // Its edge sharpened, a fade over a few metres rather than tens.
            snow = mix(snow, smoothstep(0.02, 0.5, snow), far_snow);
        }
        // Snow lies on a wall's ledges where it lies round them; elsewhere they hold turf below
        // the bare heights, the green benches between the Tatras' cliff bands.
        // Since D426's round 9 only some ledges, in patches, hold it: along every lip they ruled the
        // horns in zebra stripes (the r8 review); a wall's snow lies in its couloirs (`couloir_fill`).
        let ledge_snow = ledge * smoothstep(0.01, 0.06, in_snow_m) * smoothstep(0.36, 0.56, noise(world * 0.004 + 251.0));
        // The couloirs the land fills from the gullies above them (terrain.rs `couloir_catch`,
        // round 10), near and far, on any wall up to about 75 degrees: known by depth alone, the
        // tops' fed snow, as deep, capped every horn in one white sheet brighter than the sky.
        // Only on the walls: a gentler slope's deep snow is the vertices', broken by its outcrops
        // far off; filled here too, a far col lay as one smooth sheet brighter than the sky.
        let caught = smoothstep(0.25, 0.6, in_couloir + (blotch - 0.5) * 0.4 + (fleck - 0.5) * 0.1) * smoothstep(0.05, 0.3, in_snow_m) * smoothstep(0.18, 0.32, flatness) * (1.0 - smoothstep(0.6, 0.75, flatness)) * (1.0 - under_woods);
        // Broken far off by the same outcrops as the far snow: whole, the far horns' walls,
        // kilometres off, lay in broad white sheets brighter than the sky (round 11).
        couloir_fill = max(couloir_fill, caught * 0.97 * (1.0 - far_broken));
        snow = max(snow, couloir_fill);
        snow = max(snow, ledge_snow * 0.9);
        let ledge_turf = 0.25 * ledge * (1.0 - ledge_snow) * (1.0 - smoothstep(1900.0, 2300.0, above_sea_m + (blotch - 0.5) * 200.0));
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
            meadow_tones(world, lush) * (0.92 + 0.16 * fleck),
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
            // Seen from off, a shrub heath is its leaves and what shows between them, the
            // lichen, stones and dead stems: lighter and greyer than its leaves alone, which
            // left the middle distance a dark red trough under the near ground's blades.
            let shrub_far = mix(ground.heath_autumn.rgb * 0.6, LICHEN * 0.7, 0.35);
            let shrub_colour = mix(SHRUB_GREEN, shrub_far, kind_turned(world, ground.heath_layer));
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
            colour = lake_shore(colour, in.uv_b.y, fleck, lowland_at(from_centre - ground.sea_radius_m));
        }
        colour *= mix(vec3<f32>(1.0), face_tint, (1.0 - snow) * (1.0 - scree));
        if abs(tier_tilt) > 0.001 {
            ground_is.normal = normalize(ground_is.normal + tier_up * tier_tilt);
        }
        pbr_input.N = ground_is.normal;
        snowed = snow;
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
            let packed = mix(TURNED, TRODDEN, smoothstep(0.45, 0.8, trodden)) * (0.82 + 0.36 * blotch) * (0.9 + 0.2 * swell);
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
            // Paved where the cover says (a trodden way's middle, from 0.85): setts laid in
            // the packed earth, their edge broken, earth and mud between them where it thins.
            let paved = smoothstep(0.8, 0.95, trodden + (blotch - 0.5) * 0.12 + (fleck - 0.5) * 0.04) * flat_ground;
            if paved > 0.0 {
                // A lived-in paving, as Kingdom Come's and The Witcher's streets are: earth
                // trodden over it in broad patches and filling its joints everywhere, the stones'
                // tops showing through (`settle` lays them by height, so the earth stays low), not
                // a clean, regular pavement.
                let dirt = smoothstep(0.3, 0.75, noise(world * 0.16 + 23.0) * 0.7 + noise(world * 0.7 + 29.0) * 0.3);
                let shows = paved * mix(0.85, 0.35, dirt);
                // Two layings, offset and a little differently sized, in patches a few metres
                // across, so its two-metre repeat does not show as a grid; the stones each take
                // the tint's hue, not the scan's, a little lighter or darker by patch.
                let laid = SETTS * (0.8 + 0.4 * blotch) * (0.9 + 0.2 * swell) * (0.85 + 0.3 * noise(world * 0.45 + 13.0));
                // Laid in hexagon patches a metre or so across, each shifted and turned a little,
                // as a street relaid piece by piece over the years is: the scan's even grid
                // read as tiles from across the square (the town review, round 8).
                var setts = layer(COBBLES, 0.9, laid, world + 5.0, gx, gy, n, SETT_TURN);
                let other = layer(COBBLES, 0.83, laid, world + vec3<f32>(0.7, 0.0, 1.3), gx, gy, n, SETT_TURN);
                setts = over(setts, other, step(0.5, noise(world * 0.21 + 71.0)));
                // The scan's joints are deep and dark; worn setts in packed earth are nearly
                // flush, their joints full, so the tone and the relief are both softened.
                let tone = dot(setts.colour, vec3<f32>(0.333)) / dot(laid, vec3<f32>(0.333));
                setts.colour = laid * pow(max(tone, 0.0), 0.75);
                setts.normal = normalize(mix(n, setts.normal, 0.55));
                setts.colour *= mix(1.0, 0.6, wet);
                ground_is = settle(ground_is, setts, shows);
                // Where the dirt lies thickest, a film of trodden earth over the stones' tops
                // too, so they show only faintly through it, as in Kingdom Come's market.
                // Kingdom Come's market street is grey-brown trodden mud over its stones,
                // about 0.25 sRGB in the sun, the stones barely showing; ours had every sett
                // clean and lit at twice that. The mud is damp and dark, lies in broad patches
                // and fills the low ground between, its own fine grain and none of the
                // stones' relief.
                var mud = earth;
                mud.colour = earth.colour * 0.55;
                mud.normal = normalize(mix(n, earth.normal, 0.5));
                let film = paved * smoothstep(0.22, 0.5, dirt + (fleck - 0.5) * 0.15) * 0.92;
                ground_is = over(ground_is, mud, film);
                // Worn stone under dust is matt: the scan's gloss caught the low sun on each
                // sett's top as a white fleck, in patches, as if wet.
                pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.92, paved);
            }
            // Straw about a way's edges and the yards, and a little scattered over the paving.
            let straw = smoothstep(0.7, 0.86, noise(world * 1.3 + 61.0) * 0.6 + fleck * 0.4) * mix(1.0, 0.55, smoothstep(0.55, 0.95, trodden)) * worn;
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
            let tint = meadow_tones(world, lush) * (0.92 + 0.16 * fleck);
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
            // Three octaves of the noise, each on axes turned from the last, so no octave's
            // lattice shows as squares where a community's edge follows it.
            let turned_a = vec3<f32>(world.x * 0.8 + world.z * 0.6, world.y, world.z * 0.8 - world.x * 0.6);
            let turned_b = vec3<f32>(world.x * 0.36 - world.y * 0.48 + world.z * 0.8, world.y * 0.8 + world.x * 0.6, world.z * 0.6 - world.x * 0.64);
            let mosaic = (noise(turned_a * 0.27 + 101.0) - 0.5) * 0.42
                + (noise(turned_b * 0.71 + 103.0) - 0.5) * 0.26
                + (noise(world * 1.9 + 107.0) - 0.5) * 0.16
                + (fleck - 0.5) * 0.1;
            let stones = smoothstep(0.2, 0.6, heath.ground.x + mosaic);
            if stones > 0.0 {
                ground_is = over(ground_is, toned(layer(GRAVEL, 0.8, FELL_STONE * (0.8 + 0.4 * blotch), world + 61.0, gx, gy, n, 1.0), FELL_STONE * (0.8 + 0.4 * blotch)), stones);
            }
            let lichen = smoothstep(0.15, 0.5, heath.plants.x + mosaic) * flat_ground;
            if lichen > 0.0 {
                // Cladonia's pale cushions over the moss's relief, a little greener and greyer
                // from cushion to cushion.
                let tint = LICHEN * mix(vec3<f32>(0.92, 0.95, 0.9), vec3<f32>(1.06, 1.03, 1.08), fleck);
                ground_is = settle(ground_is, toned(layer(MOSS, 0.7, tint, world + 67.0, gx, gy, n, PLAIN), tint), lichen);
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
                ground_is = settle(ground_is, toned(layer(MOSS, 1.0, tint, world + 71.0, gx, gy, n, PLAIN), tint), carpet);
            }
            let sedge = smoothstep(0.15, 0.5, heath.plants.z + mosaic) * flat_ground;
            if sedge > 0.0 {
                // Some stands of it turn later than others: olive patches among the straw.
                let late = smoothstep(0.45, 0.75, noise(world * 0.13 + 41.0) * 0.7 + fleck * 0.3);
                let turned = smoothstep(0.0, 0.5, kind_turned(world, ground.grass_layer)) * (1.0 - 0.7 * late);
                let tint = mix(SEDGE_GREEN, SEDGE_STRAW, turned) * (0.85 + 0.3 * fleck);
                ground_is = settle(ground_is, toned(layer(GRASS, 0.8, tint, world + 73.0, gx, gy, n, 1.0), tint), sedge);
            }
            let shrub = smoothstep(0.12, 0.45, heath.plants.y + mosaic) * flat_ground;
            if shrub > 0.0 {
                // Dwarf birch and bilberry turning crimson and rust, crowberry staying dark
                // green among them, in patches a hand or two across.
                // Which shrub holds a patch, a metre to ten across: bearberry's crimson, dwarf
                // birch's rust and its gold, crowberry's olive, which stays green; so a shrub
                // heath seen from off is a mottle of them, as the photographs' is, not one rust.
                let which = noise(world * 0.21 + 5.0) * 0.55 + noise(world * 0.9 + 9.0) * 0.3 + fleck * 0.15;
                let crimson = vec3<f32>(0.17, 0.022, 0.01);
                let gold = vec3<f32>(0.3, 0.15, 0.025);
                var autumn = mix(crimson, ground.heath_autumn.rgb, smoothstep(0.3, 0.42, which));
                autumn = mix(autumn, gold, smoothstep(0.55, 0.68, which));
                autumn *= mix(vec3<f32>(1.05, 0.8, 0.8), vec3<f32>(0.95, 1.1, 1.0), fleck);
                let crowberry = smoothstep(0.4, 0.55, noise(world * 0.37 + 31.0) * 0.7 + fleck * 0.3);
                let olive = vec3<f32>(0.075, 0.068, 0.02);
                let leaves = mix(mix(SHRUB_GREEN, autumn, kind_turned(world, ground.heath_layer)), olive, crowberry * 0.85);
                ground_is = settle(ground_is, toned(layer(LITTER, 0.6, leaves, world + 79.0, gx, gy, n, 1.0), leaves), shrub);
            }
            // Wet ground darker and glossier, as wet soil and moss are (Lagarde).
            let soaked = smoothstep(0.55, 0.95, heath.ground.y) * heath.ground.z;
            ground_is.colour *= mix(1.0, 0.72, soaked);
            // Past the blades the plants they stand for still show as a mottle a metre or
            // two across, light and dark and of their colours, the way a heath seen from a
            // hundred metres is grained to the horizon: the layers' own scans minified to an
            // even paint there.
            let past_blades = smoothstep(25.0, 80.0, far_off) * flat_ground;
            if past_blades > 0.0 {
                // Clumps a hand to a half metre across, lit on top with dark gaps between, as a
                // heath's shrubs and tussocks stand seen from off; faded out where they are
                // under a few pixels, before they shimmer.
                let grain = noise(world * 2.4 + 77.0) * 0.65 + noise(world * 5.3 + 79.0) * 0.35;
                let clumps = mix(1.0, 0.45 + 1.0 * smoothstep(0.3, 0.7, grain), 1.0 - smoothstep(180.0, 320.0, far_off));
                let tuft = noise(world * 0.45 + 83.0) * 0.7 + grain * 0.3;
                var mottled = ground_is.colour * clumps;
                mottled = mix(mottled, LICHEN * (0.8 + 0.3 * grain), smoothstep(0.62, 0.8, tuft) * (0.3 + heath.plants.x));
                mottled = mix(mottled, vec3<f32>(0.075, 0.07, 0.022) * (0.8 + 0.4 * grain), smoothstep(0.66, 0.82, 1.0 - tuft) * 0.6);
                ground_is.colour = mix(ground_is.colour, mottled, past_blades * heath.ground.z);
            }
        }

        // The ground under thick grass is in the blades' own shade, so what shows between
        // them is dark, as their roots are, rather than a lawn. Less so farther off, where
        // the blades thin to a third of them (blades.rs `RINGS`) and the ground stands in for
        // the rest: darkened as if under them all, the heath's middle distance was a dark
        // trough under its lit near blades.
        ground_is.colour *= 1.0 - 0.6 * smoothstep(0.1, 0.7, blades) * mix(1.0, 0.35, smoothstep(18.0, 60.0, far_off));
        ground_is.colour *= 0.86 + 0.28 * swell;

        // The mountains' bare rock, their scree under its cliffs and gravel in gullies' beds,
        // over whatever grew: nothing grows through them.
        if bare > 0.0 {
            // The rock face's scan is a warm brown; its grain kept, its hue the granite's.
            var face_rock = layer(ROCK, 1.0, granite, world + 37.0, gx, gy, n, 0.15);
            let face_grey = dot(face_rock.colour, vec3<f32>(0.2126, 0.7152, 0.0722));
            face_rock.colour = mix(vec3<f32>(face_grey), face_rock.colour, 0.25);
            ground_is = over(ground_is, face_rock, bare);
        }
        if scree > 0.0 {
            ground_is = over(ground_is, layer(SCREE, 1.0, scree_tint, world + 19.0, gx, gy, n, 1.0), scree);
        }
        if gravel > 0.0 {
            ground_is = over(ground_is, layer(GRAVEL, 1.0, gravel_tint, world + 23.0, gx, gy, n, 1.0), gravel);
        }
        // A lake's shore from its shallows to where the trees start (terra_land::woods
        // SHORE_BARE_M): washed granite gravel and stones where it is gentle, the granite's
        // own slabs where it is steep, its edge frayed so it never runs as a contour.
@if(VERTEX_UVS_B)
        {
            let above = in.uv_b.y + (blotch - 0.5) * 0.7 + (fleck - 0.5) * 0.15;
            let lowland = lowland_at(from_centre - ground.sea_radius_m);
            // A lowland lake's shore is soft: its meadow comes down to the water, a few
            // centimetres of wet mud at the line, and under the water one bed of silt however
            // deep, the brown-green of drowned leaves and algae, darker as it goes down. Mud,
            // not the sand scan, whose pebbles read as coarse grey shingle from above
            // ("textures of the beach are kinda chonky"), finer than the field's soil, as silt
            // is. The margin is a few centimetres of height, frayed by as little: on a flat
            // bank 3 to 30 cm over the water was a mud flat tens of metres wide with the
            // meadow's bumps standing out of it as islands, and where the bed gave way to the
            // meadow's own ground a metre down it showed through the water as olive patches
            // (measured with the ground tinted by its height over the water). Past 5 m, where
            // the lowland water hides its bed, the ground's own again, so land lower than a
            // lake beside it (below its outflow) is no mud flat.
            let fray = (blotch - 0.5) * 0.04 + (fleck - 0.5) * 0.02;
            let wet_above = in.uv_b.y + fray;
            let soft = (1.0 - smoothstep(0.0, 0.06, wet_above)) * smoothstep(-8.0, -5.0, wet_above) * lowland;
            if soft > 0.0 {
                let down = 1.0 - smoothstep(-2.5, 0.0, in.uv_b.y);
                // Wet, so darker than the meadow above it, not a pale tan rim: at 0.062 the
                // margin and its puddles read brighter than the grass and the water both.
                let mud = mix(vec3<f32>(0.032, 0.029, 0.017), vec3<f32>(0.022, 0.022, 0.012), down);
                let silt = mud * (0.95 + 0.1 * fleck) * mix(tones, 1.0, 0.5);
                ground_is = over(ground_is, layer(SOIL, 0.6, silt, world + 67.0, gx, gy, n, 1.0), soft);
            }
            let shore = (1.0 - smoothstep(0.9, 1.6, above)) * smoothstep(-6.0, -3.0, above) * (1.0 - lowland);
            if shore > 0.0 {
                // Granite washed by the lake, a warm grey: the blue sky over the open shore
                // lights it, and a neutral grey under it read violet.
                let pebbles = vec3<f32>(grey * 1.3, grey * 1.06, grey * 0.72) * 1.2 * (0.8 + 0.4 * fleck) * tones;
                ground_is = over(ground_is, layer(GRAVEL, 0.7, pebbles, world + 67.0, gx, gy, n, 1.0), shore * (1.0 - steep));
                ground_is = over(ground_is, layer(SCREE, 0.6, pebbles, world + 71.0, gx, gy, n, 1.0), shore * smoothstep(0.55, 0.8, blotch) * (1.0 - steep));
                var slab = layer(ROCK, 1.0, granite, world + 89.0, gx, gy, n, 0.15);
                slab.colour = mix(vec3<f32>(dot(slab.colour, vec3<f32>(0.2126, 0.7152, 0.0722))), slab.colour, 0.25);
                ground_is = over(ground_is, slab, shore * steep);
            }
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
            let lowland_bank = lowland_at(from_centre - ground.sea_radius_m);
            colour = lake_shore(colour, in.uv_b.y, fleck, lowland_bank);
            // Silt by a lowland lake, smoothed less than wet rock: at 0.35 its margin mirrored the
            // sky as a grey-violet rim round the water.
            pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, mix(0.35, 0.6, lowland_bank), wet_band(in.uv_b.y, fleck, lowland_bank));
        }
        if ribbed > 0.0 {
            // The rock broken into facets tens of metres across, each a plane turned its own way,
            // so a far wall reads by crisp light and shade between its buttresses and slabs, as
            // the Tatras' granite does (the r5 review's gap 4), rather than one smooth sheet.
            // Two sizes, slabs a couple of hundred metres across split by blocks of forty, the
            // finer fading out before a pixel holds a block: one size at 70 m read as a
            // decimated low-poly mesh (the r6 review). Steeper rock turns more: a slab of the
            // shoulders lies near its slope.
            let slabs = facet_turn(world, up, 190.0);
            let blocks = facet_turn(world + 37.0, up, 45.0);
            let fine = 1.0 - smoothstep(2.0, 5.0, footprint);
            let faceted = rocky * (1.0 - snow) * mix(0.5, 1.0, steep);
            let facets = (slabs.xyz * 0.8 + blocks.xyz * 0.32 * fine) * faceted;
            // Dark in some of the cracks between them, the fractures a face is seen to be made
            // of: dark round every slab, they drew a web of crazy paving over the walls.
            let cracked = smoothstep(0.5, 0.75, noise(world * 0.0045 + 191.0) * 0.7 + noise(world * 0.017 + 193.0) * 0.3);
            colour *= 1.0 - (0.3 * slabs.w + 0.15 * blocks.w * fine) * cracked * faceted * ribbed;
            // The gullies' ribs strong on some faces and faint on others: alike everywhere they
            // combed the walls into an even curtain of streaks (the r5 review).
            let combed = 0.3 + 0.7 * smoothstep(0.4, 0.7, noise(world * 0.0011 + 171.0));
            ground_is.normal = normalize(ground_is.normal - across_face * rib_tilt * combed + facets * ribbed + joint_turn * (1.0 - snow * 0.7));
            colour *= 1.0 - mix(0.55, 0.45, smoothstep(1500.0, 5000.0, far_off)) * joint_crack * (1.0 - snow);
            colour *= 1.0 - JOINT_SHADOW * joint_shadow * (1.0 - snow);
            colour *= 1.0 - JOINT_RECESS * joint_recess * (1.0 - snow);
            colour *= mix(1.0, mix(0.62, 1.08, rib * 0.5 + 0.5), ribbed * combed);
            colour *= 1.0 - 0.5 * wet_far * (1.0 - snow);
@if(VERTEX_COLORS)
            {
                // A far wall's folds (D434, terrain.rs `folded`): the sky its vertices see
                // dims its gullies' and its cliff bands' corners in the sky's light; drawn in
                // the rock's colour too, a share of it, so they show where the sun lights the
                // face as well, as Battlefield 3 put its terrain's occlusion in the colour.
                // Lifted a little where the wall is open, so its mean tone holds while its
                // folds darken.
                colour *= mix(1.0, in.color.a * FOLD_LIFT, FOLD_TINT * smoothstep(300.0, 900.0, far_off) * (1.0 - snow));
            }
        }
        colour *= mix(vec3<f32>(1.0), face_tint, (1.0 - snow) * (1.0 - scree));
        // A ledge's top faces the sky: it catches the light over the shaded face under it, and
        // holds turf or snow.
        if abs(tier_tilt) > 0.001 {
            ground_is.normal = normalize(ground_is.normal + tier_up * tier_tilt);
        }
        if ledge > 0.001 || skirt_turf > 0.001 {
            let turf = meadow_tones(world, 0.35) * 0.75 * (0.85 + 0.3 * blotch);
            colour *= 1.0 + 0.12 * ledge * (1.0 - snow);
            colour = mix(colour, turf, max(ledge_turf, skirt_turf) * (1.0 - snow) * 0.85);
        }
        pbr_input.N = ground_is.normal;
        snowed = snow;
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
        var grass_colour = meadow_tones(world, lush);
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
    snowed = max(snowed, snow);
    let wet = max(climate.g * 0.6, climate.b) * (1.0 - snow);
    colour *= mix(1.0, 0.6, wet);
    pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.5, wet * 0.7);
    let rime = climate.a * (1.0 - snow) * smoothstep(0.6, 0.9, level) * smoothstep(0.3, 0.7, noise(world * 4.3 + 61.0));
    colour = mix(colour, RIME, rime * 0.5);
    let sparkle = 0.94 + 0.12 * noise(world * 9.0 + 3.0);
    colour = mix(colour, SNOW * sparkle, snow);
    pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.8, snow);
    // The snow's own surface: drifted in soft swells a metre or two across, and grained finer.
    let geometric = normalize(in.world_normal);
    let tangent = normalize(cross(geometric, vec3<f32>(0.0, 0.0, 1.0)) + vec3<f32>(1e-4, 0.0, 0.0));
    let bitangent = cross(geometric, tangent);
    let swell = vec2<f32>(
        noise(world * 0.6 + tangent * 0.4 + 7.0) - noise(world * 0.6 - tangent * 0.4 + 7.0),
        noise(world * 0.6 + bitangent * 0.4 + 7.0) - noise(world * 0.6 - bitangent * 0.4 + 7.0),
    ) * 2.2 + vec2<f32>(noise(world * 5.0 + 3.0) - 0.5, noise(world * 5.0 + 19.0) - 0.5) * 0.25
        // And broad undulations five to ten metres across, so a low sun shades the floor in
        // long soft swells, not one even plane (winter round 9's judge).
        + vec2<f32>(
            noise(world * 0.15 + tangent * 1.5 + 41.0) - noise(world * 0.15 - tangent * 1.5 + 41.0),
            noise(world * 0.15 + bitangent * 1.5 + 41.0) - noise(world * 0.15 - bitangent * 1.5 + 41.0),
        ) * 0.9;
    // The broad swells' hollows hold a cooler shade, the sky over them a little hidden.
    let hollow = 1.0 - smoothstep(0.3, 0.7, noise(world * 0.15 + 41.0));
    colour = mix(colour, colour * vec3<f32>(0.9, 0.94, 1.0), hollow * snow * 0.6);
    // Where the floor lies open, the wind shapes it (winter round 12's judge: "a flat sheet"):
    // ripples across the wind, a gentle windward face and a steep lee, in patches; and long
    // sastrugi-like ridges along it, so a low light streaks the open snow. One prevailing way
    // over a place, as the ground is not told the weather's wind. Faded where a pixel holds a
    // ripple or more, which would only shimmer, and under the crowns, which still the air.
    let open = (1.0 - smoothstep(0.2, 0.6, canopy_over(world))) * snow;
    var ripple = vec2<f32>(0.0);
    if open > 0.01 {
        let way = normalize(tangent * 0.8 + bitangent * 0.6);
        let side = cross(geometric, way);
        let bend = noise(world * 0.3 + 23.0) * 2.4;
        let phase = fract(dot(world, way) * RIPPLE_PER_M + bend + 0.6 * noise(world * 1.3 + 5.0));
        let face = select(-3.0, 1.0, phase < 0.75) * smoothstep(0.0, 0.08, phase) * (1.0 - smoothstep(0.92, 1.0, phase));
        let patchy = smoothstep(0.35, 0.75, noise(world * 0.22 + 61.0));
        let seen = 1.0 - smoothstep(0.25, 0.6, length(fwidth(world)) * RIPPLE_PER_M);
        let q = world + way * dot(world, way) * -0.8 + side * dot(world, side) * 1.5;
        let ridge = noise(q * 0.9 + side * 0.25 + 77.0) - noise(q * 0.9 - side * 0.25 + 77.0);
        let r = way * (face * RIPPLE_LEAN * patchy * seen) + side * (ridge * 1.6);
        ripple = vec2<f32>(dot(r, tangent), dot(r, bitangent)) * open;
    }
    // Round a trunk's, log's or rock's foot the snow lies in a well, as far as the crown keeps
    // the snow off (`Stand::well`, sized to the tree), deepest at the bark and bluest on the
    // side its trunk keeps the sun from, its edge broken; and banks up in a soft rounded lip
    // beyond, lit where it faces the light. Its slope from how the nearness changes half a
    // metre either way: a well leans toward what stands, the lip's outer face away.
    let stands = stands_at(world) + (noise(world * 1.9 + 31.0) - 0.5) * 0.18;
    var lean = vec2<f32>(0.0);
    var well = 0.0;
    if stands > 0.02 && snow > 0.0 {
        let toward = vec2<f32>(
            stands_at(world + tangent * 0.5) - stands_at(world - tangent * 0.5),
            stands_at(world + bitangent * 0.5) - stands_at(world - bitangent * 0.5),
        );
        well = smoothstep(0.3, 1.0, stands);
        let rim = smoothstep(0.04, 0.2, stands) * (1.0 - smoothstep(0.22, 0.42, stands));
        lean = toward * (WELL_LEAN * well - RIM_LEAN * rim);
        var behind = 0.5;
        if lights.n_directional_lights > 0u && dot(toward, toward) > 1e-6 {
            let L = lights.directional_lights[0].direction_to_light;
            let level_toward = normalize(toward.x * tangent + toward.y * bitangent);
            behind = 0.5 + 0.5 * dot(level_toward, normalize(L - geometric * dot(L, geometric)));
        }
        let deep = well * well * (0.6 + 0.5 * behind);
        colour = mix(colour, colour * WELL_SHADE, clamp(deep, 0.0, 1.0) * snow);
        colour *= 1.0 + 0.05 * rim * snow;
        pbr_input.diffuse_occlusion *= vec3<f32>(1.0 - WELL_OCCLUDES * deep * snow);
    }
    // Along a path the snow is trodden into a trough a hand or two deep, packed grey and
    // broken by footprints, its sides falling in from the untrodden snow beside it, the
    // ground's dark showing through where it is worn most (winter round 12's judge: "no
    // trail trough"; the Red Dead 2 trail's). The path's edge wanders a little.
    let worn_raw = worn_at(world);
    var trough = vec2<f32>(0.0);
    if worn_raw > 0.01 && snow > 0.0 {
        // Its width wanders over metres, and its edge is churned, kicked snow and prints
        // breaking it, not a clean line.
        let wide = (noise(world * 0.23 + 13.0) - 0.5) * 0.45;
        let churn = (noise(world * 3.7 + 29.0) - 0.5) * 0.35 + (noise(world * 11.0 + 5.0) - 0.5) * 0.15;
        let worn = smoothstep(0.15, 0.85, worn_raw + (noise(world * 0.9 + 47.0) - 0.5) * 0.35 + wide + churn);
        let prints = smoothstep(0.45, 0.7, noise(world * 3.1 + 19.0) * 0.65 + noise(world * 8.0 + 3.0) * 0.35);
        let packed = TRODDEN_SNOW * (0.9 + 0.12 * noise(world * 5.0 + 71.0)) * mix(1.0, 0.82, prints);
        colour = mix(colour, packed, worn * snow);
        // Down to the earth here and there in the middle of it.
        let bare = smoothstep(0.75, 0.95, worn * (0.6 + 0.5 * prints)) * 0.35;
        colour = mix(colour, colour * vec3<f32>(0.45, 0.42, 0.4), bare * snow);
        let sides = vec2<f32>(
            worn_at(world + tangent * 0.4) - worn_at(world - tangent * 0.4),
            worn_at(world + bitangent * 0.4) - worn_at(world - bitangent * 0.4),
        );
        trough = sides * TROUGH_LEAN * snow;
        ripple *= 1.0 - worn;
        pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.65, worn * snow);
    }
    // Beside it, the bank the feet pushed up: where the path is a stride off but not here,
    // its outer face leaning away from the path, its snow broken and a little brighter.
    if snow > 0.0 && ground.cover_on != 0u {
        let out = vec4<f32>(
            worn_at(world + tangent * BANK_M),
            worn_at(world - tangent * BANK_M),
            worn_at(world + bitangent * BANK_M),
            worn_at(world - bitangent * BANK_M),
        );
        let near_path = max(max(out.x, out.y), max(out.z, out.w));
        let bank = smoothstep(0.15, 0.55, near_path) * (1.0 - smoothstep(0.05, 0.4, worn_raw))
            * (0.6 + 0.4 * noise(world * 2.3 + 37.0)) * snow;
        trough -= vec2<f32>(out.x - out.y, out.z - out.w) * BANK_LEAN * bank;
        colour *= 1.0 + BANK_LIFT * bank;
    }
    var drifted = normalize(geometric + tangent * (swell.x + lean.x + ripple.x + trough.x) + bitangent * (swell.y + lean.y + ripple.y + trough.y));
    // Glints: a crystal here and there, a facet a centimetre and a half across turned its own
    // way, mirror-smooth, so where it turns the sun to the eye it flashes, and in shade or
    // under an overcast it does not (Bevy's own sun specular and shadow); its sky reflection
    // kept to the snow's. Faded out where a pixel holds several, which would only shimmer.
    let cell = floor(world * GLINT_PER_M);
    let resolved = 1.0 - smoothstep(0.6, 1.4, length(fwidth(world)) * GLINT_PER_M);
    let glint = step(1.0 - GLINT_SHARE, hash(cell + 0.5)) * resolved * snow * (1.0 - well);
    if glint > 0.0 {
        let tilt = vec3<f32>(hash(cell + 1.7), hash(cell + 3.1), hash(cell + 5.3)) - vec3<f32>(0.5);
        drifted = normalize(mix(drifted, normalize(geometric + tilt * 1.4), glint));
        pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.06, glint);
        pbr_input.material.reflectance = mix(pbr_input.material.reflectance, vec3<f32>(1.0), glint);
        pbr_input.specular_occlusion *= 1.0 - 0.8 * glint;
    }
    pbr_input.N = normalize(mix(pbr_input.N, drifted, snow));
}
    pbr_input.material.base_color = vec4<f32>(colour, 1.0);
    // The vertex's alpha is how much of the sky it sees: the sky's light is dimmed by it,
    // so the bottom of a dug hole is dark rather than lit as open ground. Past a few hundred
    // metres it is mostly a far wall's folds (terrain.rs `folded`), and the snow they hold is
    // left its sky: dimmed, a couloir's snow went grey and the low rock lost its contrast
    // (D434, mountains r23).
@if(VERTEX_COLORS)
    {
        let snow_kept = snowed * smoothstep(300.0, 900.0, distance(world, view.world_position));
        pbr_input.diffuse_occlusion *= vec3<f32>(mix(in.color.a, 1.0, snow_kept));
    }
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
    // Past the shadow map's last cascade, the sun is taken off where it stands under the
    // land's horizon (D416), faded in over that cascade's last fifth, where the cascade still
    // shades and the two agree: a range's evening shadow across a valley kilometres off.
    if lights.n_directional_lights > 0u {
        let sun = lights.directional_lights[0];
        let view_z = dot(vec4<f32>(view.view_from_world[0].z, view.view_from_world[1].z, view.view_from_world[2].z, view.view_from_world[3].z), in.world_position);
        let far = sun.cascades[max(sun.num_cascades, 1u) - 1u].far_bound;
        let fade = smoothstep(0.8 * far, far, -view_z);
        if fade > 0.0 {
            let lit = horizon_lit(horizon_map, map_sampler, ground.horizon_middle, ground.horizon_east, ground.horizon_north, in.world_position.xyz, sun.direction_to_light, HORIZON_SOFT);
            pbr_input.directional_shadow_factor = mix(1.0, lit, fade);
        }
    }
    out.color = apply_pbr_lighting(pbr_input);
    // Aerial perspective: past a kilometre and a half a range fades toward the air's blue-grey
    // and loses its contrast, its snow dimming and its shade lifting, as the references' far
    // ranges do and as a physical atmosphere's in-scatter does (Bevy's `Atmosphere`, real air
    // on a world this size, is too clean to show it at 5 km). The air is as bright as the open
    // air's haze (`daylight::haze_for`, which follows the hour and the weather) in the sky's
    // blue-grey: a few kilometres of in-scatter, short of the sky's own endless path, so a
    // range under the skyline stays darker than the sky behind it (D426, round 13). The
    // distance haze (`DistanceFog`) still follows, for the kilometres beyond.
    // And a mist lying low over the valley's floor, thinning with height over the eye's own,
    // so a range's foot stands paler than its crest and apart from the forest under it, as the
    // references' walls rise out of their valleys' haze: a peak set straight on the treeline
    // read as a billboard (lake film r16). A height fog, as Unreal's, but whole to near the eye's
    // height and thinning out over a few hundred metres above it.
    // Without distance fog there is no haze to fade toward.
    @if(DISTANCE_FOG) {
        let past = max(distance(world, view.world_position) - AERIAL_FROM_M, 0.0);
        let over_eye = length(world) - length(view.world_position);
        var haze_light = dot(fog.base_color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        var day = 1.0;
        var low = 0.0;
        // At dusk the air before a range is lit by the whole bright west, more than the dimmed
        // haze says: a range across the lake stood as dark as the treeline under it (lake film r16).
        if lights.n_directional_lights > 0u {
            let direct = dot(lights.directional_lights[0].color.rgb * view.exposure, vec3<f32>(0.2126, 0.7152, 0.0722));
            day = smoothstep(DIRECT_FAINT.x, DIRECT_FAINT.y, direct);
            haze_light *= mix(DUSK_AIR_LIFT, 1.0, day);
            let sun_up = dot(lights.directional_lights[0].direction_to_light, normalize(view.world_position));
            low = smoothstep(LOW_SUN.x, LOW_SUN.y, sun_up) * day;
        }
        let haze_hue = fog.base_color.rgb / max(dot(fog.base_color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), 1e-4);
        let air = haze_light * mix(1.0, LOW_SUN_AIR_LIFT, low) * mix(AERIAL_AIR, haze_hue, LOW_SUN_HUE * low);
        let through = air_through(past, over_eye, day);
        out.color = vec4<f32>(mix(out.color.rgb, air, through), out.color.a);
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
}
    return out;
}
