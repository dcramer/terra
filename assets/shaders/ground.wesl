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
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> ground: Ground;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var ground_color: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var ground_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var ground_normal: texture_2d_array<f32>;

// The layers of the texture array, in the order planet.rs gives them.
const SOIL: u32 = 0u;
const GRASS: u32 = 1u;
const ROCK: u32 = 2u;
const SAND: u32 = 3u;

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
    let k = 1.0 / (ground.layer_metres[index] * scale);
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

// `under` with `top` laid on it, `amount` of the way, colour and relief alike.
fn over(under: Layer, top: Layer, amount: f32) -> Layer {
    return Layer(mix(under.colour, top.colour, amount), normalize(mix(under.normal, top.normal, amount)));
}

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
    let steep = 1.0 - smoothstep(0.93, 0.99, flatness);

@if(!PREPASS_PIPELINE)
    if ground.cover > 0.0 && ground.textured != 0u {
        // The same patches as the flat ground's, below, in textured layers.
        let blotch = noise(world * 0.23) * 0.6 + noise(world * 0.9 + 17.0) * 0.4;
        let fleck = noise(world * 3.1 + 5.0);
@if(VERTEX_COLORS)
        let grows = in.color;
@else
        let grows = vec4<f32>(0.0);
        let n = normalize(in.world_normal);
        let gx = dpdx(world);
        let gy = dpdy(world);
        var ground_is = layer(SOIL, 1.0, colour * (0.9 + 0.2 * blotch), world, gx, gy, n);
        if steep > 0.0 {
            ground_is = over(ground_is, layer(ROCK, 1.0, ground.steep.rgb, world, gx, gy, n), steep);
        }

        // Grass sits in the soil's hollows first: where the soil is lighter it is higher,
        // and the grass gives out there.
        let soil_high = dot(ground_is.colour / max(colour, vec3<f32>(0.02)), vec3<f32>(0.33)) - 1.0;
        let grass = smoothstep(0.38, 0.62, grows.r + (blotch - 0.5) * 0.5 - soil_high * 0.35) * ground.cover * (1.0 - steep);
        if grass > 0.0 {
            let lush = smoothstep(0.2, 0.8, grows.g + (blotch - 0.5) * 0.45);
            let tint = mix(ground.grass_dry.rgb, ground.grass_lush.rgb, lush) * (0.92 + 0.16 * fleck);
            // Two sizes of the same grass, so its repeat does not show across a field.
            let near = layer(GRASS, 1.0, tint, world, gx, gy, n);
            let far = layer(GRASS, 3.7, tint, world + 11.0, gx, gy, n);
            ground_is = over(ground_is, over(near, far, 0.25 + 0.4 * blotch), grass);
        }

        let rock = smoothstep(0.2, 0.4, grows.b);
        if rock > 0.0 {
            let ore = smoothstep(0.7, 0.9, grows.b);
            let stone_colour = vec3<f32>(0.56, 0.49, 0.42) * (0.85 + 0.3 * blotch);
            let ore_colour = vec3<f32>(0.55, 0.3, 0.2) * (0.8 + 0.4 * fleck);
            ground_is = over(ground_is, layer(ROCK, 0.6, mix(stone_colour, ore_colour, ore), world, gx, gy, n), rock);
        }

        let above_sea = from_centre - ground.sea_radius_m;
        let lap = sin(globals.time * 0.9 + noise(world * 0.12) * 12.0) * 0.06;
        let sandy = (1.0 - smoothstep(0.35, 1.5, above_sea + (blotch - 0.5) * 0.9)) * ground.cover;
        if sandy > 0.0 {
            ground_is = over(ground_is, layer(SAND, 1.0, ground.sand.rgb, world, gx, gy, n), sandy);
        }
        colour = ground_is.colour;
        colour *= mix(0.72, 1.0, smoothstep(0.05, 0.3, above_sea - lap));
        colour = mix(colour, ground.sea_deep.rgb, smoothstep(0.0, 4.5, -above_sea) * 0.92);
        let foam_width = 0.07 + 0.05 * noise(world * 1.7 + globals.time * 0.25);
        let foam = 1.0 - smoothstep(0.0, foam_width, abs(above_sea - lap - 0.02));
        colour = mix(colour, vec3<f32>(1.0, 0.99, 0.96), foam * 0.85 * ground.cover);
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
        let stone_colour = vec3<f32>(0.56, 0.49, 0.42) * (0.85 + 0.3 * blotch);
        let ore_colour = vec3<f32>(0.55, 0.3, 0.2) * (0.8 + 0.4 * fleck);
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
