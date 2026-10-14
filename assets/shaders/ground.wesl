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

struct Layer {
    colour: vec3<f32>,
    normal: vec3<f32>,
}

// Where a surface faces along each axis, how much each axis's projection counts: sharply
// favouring the nearest, so the blend between them is narrow.
fn facing(n: vec3<f32>) -> vec3<f32> {
    let w = pow(abs(n), vec3<f32>(4.0));
    return w / (w.x + w.y + w.z);
}

// One layer, projected along each axis the surface faces and blended, its colour with its
// average swapped for `tint`, and the surface's normal bent by its relief. The position's
// gradients are given so that this can be called where only some pixels need it.
fn layer(index: u32, scale: f32, tint: vec3<f32>, world: vec3<f32>, gx: vec3<f32>, gy: vec3<f32>, n: vec3<f32>) -> Layer {
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
    var tx = vec3<f32>(0.0, 0.0, 1.0);
    var ty = vec3<f32>(0.0, 0.0, 1.0);
    var tz = vec3<f32>(0.0, 0.0, 1.0);
    // Each projection mirrored on the far side, so the relief does not turn inside out there.
    if w.x > 0.01 {
        let uv = vec2<f32>(p.z * side.x, p.y);
        let ddx = vec2<f32>(dx.z * side.x, dx.y);
        let ddy = vec2<f32>(dy.z * side.x, dy.y);
        colour += textureSampleGrad(ground_color, ground_sampler, uv, index, ddx, ddy).rgb * w.x;
        tx = textureSampleGrad(ground_normal, ground_sampler, uv, index, ddx, ddy).xyz * 2.0 - 1.0;
        tx.x *= side.x;
    }
    if w.y > 0.01 {
        let uv = vec2<f32>(p.x * side.y, p.z);
        let ddx = vec2<f32>(dx.x * side.y, dx.z);
        let ddy = vec2<f32>(dy.x * side.y, dy.z);
        colour += textureSampleGrad(ground_color, ground_sampler, uv, index, ddx, ddy).rgb * w.y;
        ty = textureSampleGrad(ground_normal, ground_sampler, uv, index, ddx, ddy).xyz * 2.0 - 1.0;
        ty.x *= side.y;
    }
    if w.z > 0.01 {
        let uv = vec2<f32>(-p.x * side.z, p.y);
        let ddx = vec2<f32>(-dx.x * side.z, dx.y);
        let ddy = vec2<f32>(-dy.x * side.z, dy.y);
        colour += textureSampleGrad(ground_color, ground_sampler, uv, index, ddx, ddy).rgb * w.z;
        tz = textureSampleGrad(ground_normal, ground_sampler, uv, index, ddx, ddy).xyz * 2.0 - 1.0;
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
    return Layer(colour * tint / max(average, vec3<f32>(0.02)), normal);
}

// A layer at two sizes, the second `scale` times the first and laid over it `amount` of
// the way, so its repeat does not show across a field; at one only, at the low preset.
fn sizes(index: u32, scale: f32, offset: f32, tint: vec3<f32>, world: vec3<f32>, gx: vec3<f32>, gy: vec3<f32>, n: vec3<f32>, amount: f32) -> Layer {
    let first = layer(index, 1.0, tint, world, gx, gy, n);
    if ground.one_size != 0u {
        return first;
    }
    return over(first, layer(index, scale, tint, world + offset, gx, gy, n), amount);
}

// `under` with `top` laid on it, `amount` of the way, colour and relief alike.
fn over(under: Layer, top: Layer, amount: f32) -> Layer {
    return Layer(mix(under.colour, top.colour, amount), normalize(mix(under.normal, top.normal, amount)));
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
            let at = (ground.cover_from_world * vec4<f32>(world, 1.0)).xy;
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
        // Under trees the soil is fallen leaves, browner than the open ground's; under
        // conifers, needles, a little redder.
        let litter_colour = vec3<f32>(0.09, 0.07, 0.048);
        let needle_colour = vec3<f32>(0.066, 0.055, 0.043);
        // Soil and rock each at two sizes, blended in patches, so their repeat does not
        // show across a field or a hillside.
        let soil_tint = mix(colour, mix(litter_colour, needle_colour, needles), max(litter, needles)) * (0.9 + 0.2 * blotch);
        var ground_is = sizes(SOIL, 2.9, 23.0, soil_tint, world, gx, gy, n, 0.2 + 0.45 * blotch);
        if steep > 0.0 {
            let rock_tint = ground.steep.rgb * (0.85 + 0.3 * blotch);
            let cliff = sizes(ROCK, 3.3, 37.0, rock_tint, world, gx, gy, n, 0.3 + 0.4 * fleck);
            ground_is = over(ground_is, cliff, steep);
        }
        // Under trees the ground is fallen leaves, in soft-edged drifts.
        let leaves = smoothstep(0.2, 0.75, litter + (blotch - 0.5) * 0.45) * (1.0 - steep);
        if leaves > 0.0 {
            let leaf_tint = litter_colour * (0.85 + 0.3 * fleck);
            let fallen = sizes(LITTER, 2.6, 41.0, leaf_tint, world, gx, gy, n, 0.2 + 0.4 * blotch);
            ground_is = over(ground_is, fallen, leaves);
        }
        // Under pines and spruces, their needles, drifting over the leaves at the edges.
        let needly = smoothstep(0.25, 0.7, needles + (blotch - 0.5) * 0.4) * (1.0 - steep);
        if needly > 0.0 {
            let needle_tint = needle_colour * (0.85 + 0.3 * fleck);
            let fallen = sizes(NEEDLES, 2.3, 29.0, needle_tint, world, gx, gy, n, 0.2 + 0.4 * blotch);
            ground_is = over(ground_is, fallen, needly);
        }
        // Round a tree's foot and along a path's edge, dark duff and the roots in it, in
        // broken patches.
        let rooty = smoothstep(0.3, 0.75, roots + (fleck - 0.5) * 0.35 + (blotch - 0.5) * 0.3) * (1.0 - steep);
        if rooty > 0.0 {
            let duff = vec3<f32>(0.066, 0.05, 0.037) * (0.8 + 0.4 * blotch);
            ground_is = over(ground_is, layer(ROOTS, 1.0, duff, world + 3.0, gx, gy, n), rooty);
        }

        // Grass sits in the soil's hollows first: where the soil is lighter it is higher,
        // and the grass gives out there.
        let soil_high = dot(ground_is.colour / max(colour, vec3<f32>(0.02)), vec3<f32>(0.33)) - 1.0;
        let grass = smoothstep(0.38, 0.62, grows.r + (blotch - 0.5) * 0.5 - soil_high * 0.35) * ground.cover * (1.0 - steep);
        if grass > 0.0 {
            let lush = smoothstep(0.2, 0.8, grows.g + (blotch - 0.5) * 0.45);
            let tint = mix(ground.grass_dry.rgb, ground.grass_lush.rgb, lush) * (0.92 + 0.16 * fleck);
            // Two sizes of the same grass, so its repeat does not show across a field.
            let lawn = sizes(GRASS, 3.7, 11.0, tint, world, gx, gy, n, 0.25 + 0.4 * blotch);
            ground_is = over(ground_is, lawn, grass);
        }

        let mossy = smoothstep(0.25, 0.75, moss + (blotch - 0.5) * 0.5) * (1.0 - steep);
        if mossy > 0.0 {
            // Moss: the grass's texture, small and deep green, in soft patches.
            let moss_colour = vec3<f32>(0.042, 0.062, 0.02) * (0.8 + 0.4 * fleck);
            ground_is = over(ground_is, layer(GRASS, 0.55, moss_colour, world + 5.0, gx, gy, n), mossy);
        }

        // The ground under thick grass is in the blades' own shade, so what shows between
        // them is dark, as their roots are, rather than a lawn.
        ground_is.colour *= 1.0 - 0.6 * smoothstep(0.1, 0.7, blades);

        let rock = smoothstep(0.2, 0.4, grows.b);
        if rock > 0.0 {
            let ore = smoothstep(0.7, 0.9, grows.b);
            // Linear colours, as the renderer works: a mid grey-brown stone, a rusty ore.
            let stone_colour = vec3<f32>(0.12, 0.1, 0.08) * (0.85 + 0.3 * blotch);
            let ore_colour = vec3<f32>(0.24, 0.08, 0.04) * (0.8 + 0.4 * fleck);
            ground_is = over(ground_is, layer(ROCK, 0.6, mix(stone_colour, ore_colour, ore), world, gx, gy, n), rock);
        }

        let above_sea = from_centre - ground.sea_radius_m;
        let lap = sin(globals.time * 0.9 + noise(world * 0.12) * 12.0) * 0.06;
        // The shore is the sea's: ground dug down to its level inland is not a beach.
        let undug = 1.0 - smoothstep(0.1, 0.3, grows.b);
        let sandy = (1.0 - smoothstep(0.35, 1.5, above_sea + (blotch - 0.5) * 0.9)) * ground.cover * undug;
        if sandy > 0.0 {
            ground_is = over(ground_is, layer(SAND, 1.0, ground.sand.rgb, world, gx, gy, n), sandy);
        }
        colour = ground_is.colour;
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
