// Rocks of the forest floor (D246): their scanned colour, relief and roughness, with moss
// grown over what faces the sky, as it does on every stone left lying in a damp wood. Where
// it grows is the surface's own facing, its relief included, so moss fills the tops and
// the ledges and gives out down the sides in a ragged edge, broken into patches by noise in
// the world.
//
// The moss is a scan (D294): ambientCG's Moss 004, the ground's moss layer, projected from
// three sides at two sizes, its colour, relief and height, heaped into cushions a hand
// across that bulge and catch the light, its edge given out tuft by tuft by its own height
// rather than a smooth line, darker in its hollows and shadowed there in the sun, with a
// dark lip on the stone where it overhangs. Before, it was one green, flat as felt.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::{view, lights},
    shadows::fetch_directional_shadow,
};

// Field order must match MossExtension in lush/floor.rs.
struct Moss {
    // Its colour in rgb, and how much grows in a.
    colour: vec4<f32>,
}

// How much of the ground round the camera snow covers in x (climate::SnowCover), and in y 1
// for a plant's scan (a fern's): no moss or lichen on it, and it withers under the snow.
struct RockSnow {
    cover: vec4<f32>,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(104)
var<uniform> rock_snow: RockSnow;
// As ground.wesl's lying snow.
const SNOW: vec3<f32> = vec3<f32>(0.87, 0.88, 0.88);
// A fern's fronds dead in winter, linear: bracken's rust-brown gone dull and dark, as the
// fronds lie in a snowy wood (winter round 13's judge: "the fern clump is dark summer green").
const WITHERED: vec3<f32> = vec3<f32>(0.16, 0.075, 0.03);
// How bright the withered frond is against the green it was, and how much of a clump's
// fronds wither (the rest a dark green, as a hard fern's or a sheltered one's stay).
const WITHERED_LIGHT: f32 = 0.85;
const WITHERS: f32 = 0.85;

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> moss: Moss;
// The ground's layers (planet.rs), of which the moss is one.
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var ground_color: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var ground_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var ground_normal: texture_2d_array<f32>;

// The ground's moss layer, and how many metres its picture covers (planet.rs).
const MOSS_LAYER: u32 = 7u;
const MOSS_M: f32 = 0.7;
// How much more the moss's relief bends the light than its normal map says, as the ground's.
const RELIEF: f32 = 1.6;
// How high the cushions heap, in metres.
const CUSHION_M: f32 = 0.035;

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

// The moss as sampled at a place: its colour as a share of its average, its relief turned
// into the world's axes, and its height.
struct Texel {
    colour: vec3<f32>,
    normal: vec3<f32>,
    height: f32,
}

// The moss projected from the three sides the surface faces (triplanar), its picture
// `metres` across, its relief added to `n` as seen along each axis (the "UDN" blend).
fn moss_at(world: vec3<f32>, gx: vec3<f32>, gy: vec3<f32>, n: vec3<f32>, metres: f32) -> Texel {
    let k = 1.0 / metres;
    let p = world * k;
    let dx = gx * k;
    let dy = gy * k;
    var w = pow(abs(n), vec3<f32>(4.0));
    w /= w.x + w.y + w.z;
    let side = sign(n);
    let a = textureSampleGrad(ground_color, ground_sampler, vec2<f32>(p.z * side.x, p.y), MOSS_LAYER, vec2<f32>(dx.z * side.x, dx.y), vec2<f32>(dy.z * side.x, dy.y));
    let b = textureSampleGrad(ground_color, ground_sampler, vec2<f32>(p.x * side.y, p.z), MOSS_LAYER, vec2<f32>(dx.x * side.y, dx.z), vec2<f32>(dy.x * side.y, dy.z));
    let c = textureSampleGrad(ground_color, ground_sampler, vec2<f32>(-p.x * side.z, p.y), MOSS_LAYER, vec2<f32>(-dx.x * side.z, dx.y), vec2<f32>(-dy.x * side.z, dy.y));
    let na = textureSampleGrad(ground_normal, ground_sampler, vec2<f32>(p.z * side.x, p.y), MOSS_LAYER, vec2<f32>(dx.z * side.x, dx.y), vec2<f32>(dy.z * side.x, dy.y)).xyz;
    let nb = textureSampleGrad(ground_normal, ground_sampler, vec2<f32>(p.x * side.y, p.z), MOSS_LAYER, vec2<f32>(dx.x * side.y, dx.z), vec2<f32>(dy.x * side.y, dy.z)).xyz;
    let nc = textureSampleGrad(ground_normal, ground_sampler, vec2<f32>(-p.x * side.z, p.y), MOSS_LAYER, vec2<f32>(-dx.x * side.z, dx.y), vec2<f32>(-dy.x * side.z, dy.y)).xyz;
    var tx = (na.xy * 2.0 - 1.0) * RELIEF;
    var ty = (nb.xy * 2.0 - 1.0) * RELIEF;
    var tz = (nc.xy * 2.0 - 1.0) * RELIEF;
    tx.x *= side.x;
    ty.x *= side.y;
    tz.x *= -side.z;
    let nx = vec3<f32>(n.x, tx.y + n.y, tx.x + n.z);
    let ny = vec3<f32>(ty.x + n.x, n.y, ty.y + n.z);
    let nz = vec3<f32>(tz.x + n.x, tz.y + n.y, n.z);
    let average = textureSampleLevel(ground_color, ground_sampler, vec2<f32>(0.5), MOSS_LAYER, 16.0).rgb;
    let colour = (a.rgb * w.x + b.rgb * w.y + c.rgb * w.z) / max(average, vec3<f32>(0.02));
    return Texel(colour, normalize(nx * w.x + ny * w.y + nz * w.z), na.z * w.x + nb.z * w.y + nc.z * w.z);
}

// A surface's normal bent by a height over it, from how the height and the position change
// across the screen: Mikkelsen's "Bump Mapping Unparametrized Surfaces on the GPU" (2010).
// The derivatives are taken where every pixel takes them, and passed in.
fn bumped(n: vec3<f32>, gx: vec3<f32>, gy: vec3<f32>, hx: f32, hy: f32) -> vec3<f32> {
    let r1 = cross(gy, n);
    let r2 = cross(n, gx);
    let det = dot(gx, r1);
    let grad = sign(det) * (hx * r1 + hy * r2);
    let bent = abs(det) * n - grad;
    let length2 = dot(bent, bent);
    if length2 < 1.0e-20 {
        return n;
    }
    return bent * inverseSqrt(length2);
}

// How much of the moss's colour its hollows keep in the sun, as the ground's (D294).
fn micro_shadow(world: vec4<f32>, n: vec3<f32>, relief: vec3<f32>, cavity: f32, frag: vec4<f32>) -> f32 {
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
    let micro = clamp(abs(dot(relief, L)) + 2.0 * cavity * cavity - 1.0, 0.0, 1.0);
    return mix(1.0, micro, sunlit * 0.85);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let world = in.world_position.xyz;
    // The planet's centre is the origin, so up is away from it.
    let up = normalize(world);
    let gx = dpdx(world);
    let gy = dpdy(world);
    let geometric = normalize(in.world_normal);
    // Cushions a hand across and smaller, heaped on each other, their height in metres.
    let heap = noise(world * 6.5 + 5.0) * 0.6 + noise(world * 17.0) * 0.4;
    let heap_m = heap * CUSHION_M;
    let hx = dpdx(heap_m);
    let hy = dpdy(heap_m);
    let fine = moss_at(world, gx, gy, geometric, MOSS_M);
    let broad = moss_at(world + 13.0, gx, gy, geometric, MOSS_M * 2.7);
    let height = clamp(0.6 * fine.height + 0.4 * broad.height + (heap - 0.5) * 0.7, 0.0, 1.0);

    let facing = dot(pbr_input.N, up);
    let patches = noise(world * 1.6) * 0.5 + noise(world * 4.7 + 11.0) * 0.3 + noise(world * 17.0 + 3.0) * 0.2;
    // Moss on the tops and ledges in patches; bare stone down the sides, where it shows its
    // own relief and grain. Its edge gives out by its own height, the low tufts first.
    let reach = facing + (patches - 0.5) * 1.4 + (height - 0.5) * 0.6;
    let grows = smoothstep(0.52, 0.66, reach) * moss.colour.a;
    // Lichen on the bare stone, in pale grey-green crusts and specks (D268).
    let crust = noise(world * 7.0 + 23.0) * 0.6 + noise(world * 29.0 + 9.0) * 0.4;
    let plant = rock_snow.cover.y;
    let lichen = smoothstep(0.6, 0.74, crust) * (1.0 - grows) * 0.6 * (1.0 - plant);
    if lichen > 0.0 {
        let base = pbr_input.material.base_color.rgb;
        let pale = vec3<f32>(0.16, 0.17, 0.12) * (0.8 + 0.4 * noise(world * 61.0));
        pbr_input.material.base_color = vec4<f32>(mix(base, pale, lichen), pbr_input.material.base_color.a);
    }
    // Where the moss overhangs the stone, the stone under its lip is in its shade.
    let lip = smoothstep(0.38, 0.52, reach) * (1.0 - grows) * moss.colour.a;
    pbr_input.material.base_color = vec4<f32>(pbr_input.material.base_color.rgb * (1.0 - 0.45 * lip), pbr_input.material.base_color.a);
    if grows > 0.0 {
        let base = pbr_input.material.base_color.rgb;
        // The scan's own greens and browns, the broad picture's light and dark over the fine,
        // yellower and browner where it is thin.
        let thin = 1.0 - smoothstep(0.35, 0.9, grows);
        let broad_light = dot(broad.colour, vec3<f32>(0.333));
        var colour = moss.colour.rgb * fine.colour * mix(1.0, broad_light, 0.6)
            * mix(vec3<f32>(1.0), vec3<f32>(1.2, 1.0, 0.6), thin);
        // Darker down in its hollows, between the cushions.
        let cavity = smoothstep(0.15, 0.75, height);
        colour *= mix(0.45, 1.1, cavity);
        // Its cushions bulge and its strands catch the light.
        let n = bumped(geometric, gx, gy, hx, hy);
        let relief = normalize(fine.normal - geometric + broad.normal - geometric + n * 1.0);
        colour *= micro_shadow(in.world_position, geometric, relief, cavity, in.position);
        pbr_input.material.base_color = vec4<f32>(mix(base, colour, grows), pbr_input.material.base_color.a);
        pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 1.0, grows);
        pbr_input.material.reflectance = mix(pbr_input.material.reflectance, vec3<f32>(0.1), grows);
        pbr_input.N = normalize(mix(pbr_input.N, relief, grows));
        pbr_input.diffuse_occlusion *= vec3<f32>(mix(1.0, mix(0.55, 1.0, cavity), grows));
    }
    // Snow caps the tops over the moss, as it lies on the ground beside them: more of the
    // rock's shoulders the more lies, with an edge, since a cap has a thickness; smooth and
    // white over the stone's grain (Unreal's and Ghost of Tsushima's world-normal snow).
    let cover = rock_snow.cover.x;
    if cover > 0.0 && plant > 0.5 {
        // A plant: its fronds withered brown through the winter, frond by frond, and snow
        // lying in clumps on the faces turned up to the sky, as on a bough's sprays (D419).
        let base = pbr_input.material.base_color.rgb;
        let grey = dot(base, vec3<f32>(0.2126, 0.7152, 0.0722));
        let tone = WITHERED * grey / dot(WITHERED, vec3<f32>(0.2126, 0.7152, 0.0722)) * WITHERED_LIGHT;
        let frond = noise(world * 2.2 + 17.0);
        let withered = smoothstep(1.0 - WITHERS - 0.1, 1.0 - WITHERS + 0.1, frond) * smoothstep(0.05, 0.4, cover);
        var colour = mix(base * 0.7, tone, withered);
        var face_up = 0.0;
        let across = cross(gx, gy);
        if dot(across, across) > 1.0e-24 {
            let face = across * inverseSqrt(dot(across, across));
            face_up = dot(face * select(-1.0, 1.0, dot(face, view.world_position.xyz - world) >= 0.0), up);
        }
        let load = smoothstep(0.15, 0.6, face_up) * smoothstep(0.05, 0.4, cover);
        let clump = noise(world * 7.0 + 31.0) * 0.6 + noise(world * 19.0 + 7.0) * 0.4;
        let lies = smoothstep(-0.08, 0.08, load - (1.0 - clump) * 0.9) * smoothstep(0.0, 0.15, load);
        colour = mix(colour, SNOW * (0.92 + 0.08 * noise(world * 9.0 + 31.0)), lies);
        pbr_input.material.base_color = vec4<f32>(colour, pbr_input.material.base_color.a);
        pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.8, max(lies, withered));
        pbr_input.material.reflectance = mix(pbr_input.material.reflectance, vec3<f32>(0.5), lies);
        pbr_input.diffuse_occlusion = mix(pbr_input.diffuse_occlusion, vec3<f32>(1.0), lies * 0.6);
    } else if cover > 0.0 {
        let level = dot(geometric, up) + (patches - 0.5) * 0.35 + (heap - 0.5) * 0.2;
        let cap = smoothstep(0.62 - 0.3 * cover, 0.7 - 0.3 * cover, level) * smoothstep(0.05, 0.4, cover);
        let snow_colour = SNOW * (0.94 + 0.06 * noise(world * 9.0 + 31.0));
        pbr_input.material.base_color = vec4<f32>(mix(pbr_input.material.base_color.rgb, snow_colour, cap), pbr_input.material.base_color.a);
        pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.75, cap);
        pbr_input.material.reflectance = mix(pbr_input.material.reflectance, vec3<f32>(0.5), cap);
        pbr_input.N = normalize(mix(pbr_input.N, geometric, cap * 0.8));
        pbr_input.diffuse_occlusion = mix(pbr_input.diffuse_occlusion, vec3<f32>(1.0), cap);
    }
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
