// The foliage's main-pass fragment shader: Bevy 0.20's standard one (bevy_pbr
// src/render/pbr.wesl, MIT OR Apache-2.0, by the Bevy contributors), cut to what foliage
// uses, with one change: the crossfade between levels of detail dithers in a pattern that
// moves every frame, so temporal anti-aliasing blends it into a fade (D214). Bevy's own
// pattern is the same 4x4 grid every frame, which TAA cannot resolve and shows as a
// crawling screen door. The pattern is Jimenez's interleaved gradient noise (Call of Duty:
// Advanced Warfare), shifted each frame, as Unreal dithers its foliage's crossfades.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::{PbrInput, STANDARD_MATERIAL_FLAGS_UNLIT_BIT},
    mesh_view_bindings::{globals, view, lights},
    shadows::fetch_directional_shadow,
};

// What colour a leaf passes light on in, lit through: yellower than it reflects, as
// chlorophyll passes green and yellow and keeps the red and blue (D247).
const THROUGH: vec3<f32> = vec3<f32>(1.0, 1.0, 0.45);

// A leaf lit from behind (D247), as Crysis and SpeedTree light theirs: the sun through it,
// brightest looking toward the sun, a little from any angle on its far side, only where the
// sun reaches the leaf (its shadow), and less deep in the crown (`sky`, its occlusion).
// Bevy's own diffuse transmission is lit the same from any angle, so a crown seen from below
// read as flat; this gives it the glow of lit-through leaves.
//
// A leaf deep in a crown is lit through the leaves above it as well as in flecks where the
// sun finds a gap, each layer passing on a little (`LAYERS` of the sun at a crown's inside),
// so from under the canopy every leaf glows a little and those in a fleck brightly, rather
// than all but the outermost going dark (D264). Its glow follows its own colour, before its
// underside was darkened.
fn lit_through(input: PbrInput, colour: vec3<f32>, sky: f32, frag_coord: vec4<f32>) -> vec3<f32> {
    if wind.leaf.z <= 0.0 || lights.n_directional_lights == 0u {
        return vec3<f32>(0.0);
    }
    let sun = &lights.directional_lights[0];
    let L = (*sun).direction_to_light;
    let V = input.V;
    let toward = clamp(dot(-V, L), 0.0, 1.0);
    let forward = 0.75 * pow(toward, 6.0) + 0.25 * toward * toward;
    let behind = clamp(dot(-input.world_normal, L), 0.0, 1.0);
    let view_z = dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), input.world_position);
    let shadow = fetch_directional_shadow(0u, input.world_position, input.world_normal, view_z, frag_coord.xy);
    let reached = mix(LAYERS * sky, 1.0, shadow);
    let deep = sky * sky;
    return colour * THROUGH * (*sun).color.rgb * view.exposure
        * reached * deep * wind.leaf.z * (0.8 * forward + 0.2 * behind);
}

// How much of the sun reaches the inside of a crown through the leaves above (D264).
const LAYERS: f32 = 0.2;

// How much a leaf's underside, seen from inside or below its crown, reflects of what its
// outer face would (D264).
const UNDERSIDE: f32 = 0.55;

// A crown's cards have normals leaning out of it, so from inside or below it a card shows
// its far side: the underside of its leaves, facing into the crown's shade. It reflects
// less, so the layers deeper in read darker than the outer ones, and has no sheen: at a
// normal turned from the eye, Bevy's Fresnel reflected the whole sky off it, which turned
// boughs seen from inside grey-blue (D264).
fn underside(input: ptr<function, PbrInput>) {
    let facing = dot((*input).world_normal, (*input).V);
    let under = smoothstep(0.15, -0.35, facing);
    let c = (*input).material.base_color;
    (*input).material.base_color = vec4<f32>(c.rgb * mix(1.0, UNDERSIDE, under), c.a);
    (*input).specular_occlusion *= 1.0 - under;
    (*input).material.reflectance *= 1.0 - under;
    // An underside faces into the crown, which hides the sky from it: it is lit by what the
    // leaves round it pass on and bounce, green, not blue, so a spruce's shade is not teal.
    (*input).diffuse_occlusion *= mix(vec3<f32>(1.0), BOUNCE, under);
    // A crown's lower face looks down on the forest floor, which lies in the canopy's shade,
    // not on the sunlit ground the painted sky gives every underside: from below, its leaves
    // are dark but where the sun comes through them.
    let up = normalize((*input).world_position.xyz);
    let down = smoothstep(0.2, -0.7, dot((*input).world_normal, up));
    (*input).diffuse_occlusion *= mix(1.0, FLOOR_LIGHT, down);
}

// How much of the painted sky's light from below a crown's lower face gets (D264).
const FLOOR_LIGHT: f32 = 0.4;

// Sky light under a canopy is dimmed by the leaves above and warmed by what they and the
// ground bounce: what a surface that sees none of the sky is lit by instead, as a share of
// the sky's light (D216). Crysis bakes the same into its trees' vertices.
const BOUNCE: vec3<f32> = vec3<f32>(0.55, 0.6, 0.4);

// Field order must match FoliageExtension in grown/mod.rs and lush.rs.
struct Wind {
    direction: vec3<f32>,
    strength: f32,
    bark: f32,
    cards: vec3<f32>,
    // A shadow stand-in shrinks to its middles between x and y and grows from them between
    // z and w (D234).
    shrink: vec4<f32>,
    // For leaves: how much their colour is greyed and darkened, how brightly they glow lit
    // through, and how dark the inside of a crown goes at least (D247).
    leaf: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> wind: Wind;

fn hash3(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3<f32>(127.1, 311.7, 74.7))) * 43758.5453);
}

// Smooth noise in the world, 0 to 1, for weathering that wanders over a trunk.
fn wander(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// Bark weathered as real trunks are (D218): each tree a little lighter or darker, greener or
// greyer, by where it stands; moss where water runs and sits, on the side facing up and
// toward the foot, in patches; and the foot darker, where it stays wet. `sky` is how much of
// the sky the point sees, least low on the trunk; `share`, how much moss this bark grows
// (1 for an oak's, less for a pine's). Moss runs in streaks down the bark, as water does,
// its edges broken by finer patches rather than smooth blobs (D235).
fn weather(input: ptr<function, PbrInput>, world: vec3<f32>, sky: f32, share: f32) {
    let up = normalize(world);
    let facing_up = clamp(dot((*input).N, up), 0.0, 1.0);
    let low = 1.0 - smoothstep(0.3, 0.45, sky);
    var colour = (*input).material.base_color.rgb;
    colour *= 0.82 + 0.36 * wander(world * 0.21);
    // Moss grows where the bark faces the sky and stays damp (D247): on the tops of limbs
    // and of the roots, in the furrows the bark's own occlusion map finds, and toward the
    // foot on the trunk's damp side, its edge broken by fine noise rather than laid on in
    // patches or a band. The damp side is the same for every tree, as moss favours north.
    let damp_side = clamp(dot((*input).N, normalize(cross(up, vec3<f32>(0.0, 0.0, 1.0)))) * 0.5 + 0.5, 0.0, 1.0);
    let crevice = 1.0 - (*input).diffuse_occlusion.r;
    let streaked = world - up * dot(world, up) * 0.7;
    let fine = wander(streaked * 4.1 + 13.0) * 0.6 + wander(world * 11.0 + 7.0) * 0.4;
    let wet = 0.85 * facing_up * facing_up + 0.8 * low * (0.3 + 0.7 * damp_side) + 0.6 * crevice;
    let moss = smoothstep(0.42, 0.8, wet + 0.35 * (fine - 0.5) - 0.05) * share;
    colour = mix(colour, vec3<f32>(0.1, 0.13, 0.04) * (0.75 + 0.5 * wander(world * 6.0)), moss * 0.85);
    colour *= mix(1.0, 0.72, low * (1.0 - moss));
    (*input).material.base_color = vec4<f32>(colour, (*input).material.base_color.a);
    (*input).material.perceptual_roughness = mix((*input).material.perceptual_roughness, 1.0, moss);
}

fn noise_at(frag_coord: vec4<f32>) -> f32 {
    let shifted = frag_coord.xy + 5.588238 * f32(globals.frame_count % 64u);
    return fract(52.9829189 * fract(dot(shifted, vec2<f32>(0.06711056, 0.00583715))));
}

// A card seen edge-on is a smear of stretched leaves: it fades out a pixel at a time as it
// turns edge-on to the eye, as SpeedTree's leaf cards do, the cards behind it filling in
// (D235). Its face is the triangle's own, from how the world position changes across the
// screen, since its normal leans out of the crown instead. Only nearly edge-on: fading from
// further round left a bough by the camera a see-through grey smear once TAA blended it
// (D264).
fn edge_fade(frag_coord: vec4<f32>, world: vec3<f32>) {
    let face = normalize(cross(dpdx(world), dpdy(world)));
    let towards = normalize(view.world_position - world);
    let keep = smoothstep(0.04, 0.16, abs(dot(face, towards)));
    if noise_at(frag_coord) > keep {
        discard;
    }
}

// Leaves and twigs right at the camera fade out, a pixel at a time and a different pixel
// each frame, so TAA blends them away rather than the camera pushing through them (D216);
// within a stride of the eye, so a bough beside the camera stays solid (D264).
fn near_fade(frag_coord: vec4<f32>, world: vec3<f32>) {
    let near = smoothstep(0.3, 0.9, distance(world, view.world_position));
    if near < 1.0 && noise_at(frag_coord) > near {
        discard;
    }
}

// Discards this fragment if it is outside its crossfade's share at this pixel this frame.
// `dither` is Bevy's level: -16 to 0 fading in, 0 to 16 fading out.
fn temporal_dither(frag_coord: vec4<f32>, dither: i32) {
    if dither == 0 {
        return;
    }
    if dither <= -16 || dither >= 16 {
        discard;
    }
    let threshold = i32(noise_at(frag_coord) * 16.0);
    if (dither >= 0 && dither + threshold >= 16) || (dither < 0 && 1 + dither + threshold <= 0) {
        discard;
    }
}

// One side of a triplanar projection: the surface read as if its picture were laid flat
// along `u` and `v` (world directions, in metres, which the material's uv_transform scales to
// its picture), with the tangent that goes with them, so its normal map and relief follow.
@if(VERTEX_UVS_A && VERTEX_TANGENTS)
fn projected(in: VertexOutput, is_front: bool, u: vec3<f32>, v: vec3<f32>) -> PbrInput {
    var side = in;
    let p = in.world_position.xyz;
    side.uv = vec2<f32>(dot(p, u), dot(p, v));
    let n = normalize(in.world_normal);
    let t = normalize(u - n * dot(n, u) + v * 1.0e-4);
    let w = select(-1.0, 1.0, dot(cross(n, t), v) >= 0.0);
    side.world_tangent = vec4<f32>(t, w);
    return pbr_input_from_standard_material(side, is_front);
}

// A modelled trunk's bark (D264). Its own texture coordinates are an atlas made for a
// texture it no longer has, so the bark scan is laid on by where each point is, from three
// sides (triplanar), each weighted by how squarely the surface faces it, as terrain and rock
// shaders do: the same bark, at the same scale and relief, as a grown trunk's, without the
// seams a tiling picture would show along the model's own islands. The trees stand near
// enough to the start that the world's y is their up, so the furrows run up the trunk.
@if(VERTEX_UVS_A && VERTEX_TANGENTS)
fn triplanar(in: VertexOutput, is_front: bool) -> PbrInput {
    let n = normalize(in.world_normal);
    var w = pow(abs(n), vec3<f32>(6.0));
    w = w / (w.x + w.y + w.z);
    let x = vec3<f32>(1.0, 0.0, 0.0);
    let y = vec3<f32>(0.0, 1.0, 0.0);
    let z = vec3<f32>(0.0, 0.0, 1.0);
    let a = projected(in, is_front, z, y);
    let b = projected(in, is_front, x, z);
    let c = projected(in, is_front, x, y);
    var out = a;
    out.material.base_color = a.material.base_color * w.x + b.material.base_color * w.y + c.material.base_color * w.z;
    out.material.perceptual_roughness = a.material.perceptual_roughness * w.x + b.material.perceptual_roughness * w.y + c.material.perceptual_roughness * w.z;
    out.diffuse_occlusion = a.diffuse_occlusion * w.x + b.diffuse_occlusion * w.y + c.diffuse_occlusion * w.z;
    out.specular_occlusion = a.specular_occlusion * w.x + b.specular_occlusion * w.y + c.specular_occlusion * w.z;
    out.N = normalize(a.N * w.x + b.N * w.y + c.N * w.z);
    return out;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
@if(VISIBILITY_RANGE_DITHER)
    temporal_dither(in.position, in.visibility_range_dither);

    near_fade(in.position, in.world_position.xyz);
    if wind.bark < 0.5 {
        edge_fade(in.position, in.world_position.xyz);
    }

    var pbr_input: PbrInput;
@if(VERTEX_UVS_A && VERTEX_TANGENTS && !VERTEX_UVS_B) {
    // A modelled trunk: grown wood always has wind weights, a model none (D264).
    if wind.bark > 0.5 {
        pbr_input = triplanar(in, is_front);
    } else {
        pbr_input = pbr_input_from_standard_material(in, is_front);
    }
} @else {
    pbr_input = pbr_input_from_standard_material(in, is_front);
}
    if wind.bark > 0.5 {
        // The bark's relief read deeper than its scan's normal map gives, so its furrows and
        // plates catch the light as a real trunk's do rather than reading as fur (D247).
        pbr_input.N = normalize(pbr_input.world_normal + (pbr_input.N - pbr_input.world_normal) * 1.8);
    }
@if(VERTEX_COLORS) {
    // Alpha is how much of the sky this point sees.
    let sky = in.color.a;
    if wind.bark > 0.5 {
        // 1 is bark with all its moss; up to 2, bark with less (D235).
        weather(&pbr_input, in.world_position.xyz, sky, clamp(2.0 - wind.bark, 0.0, 1.0));
    }
    if wind.bark < 0.5 && (wind.leaf.x > 0.0 || wind.leaf.y > 0.0) {
        // Leaves greyed and darkened toward a forest's, as the pictures are brighter and
        // greener than leaves in shade read (D247).
        let c = pbr_input.material.base_color.rgb;
        let grey = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
        pbr_input.material.base_color = vec4<f32>(mix(c, vec3<f32>(grey), wind.leaf.x) * (1.0 - wind.leaf.y), pbr_input.material.base_color.a);
    }
    pbr_input.diffuse_occlusion *= mix(BOUNCE, vec3<f32>(1.0), sky);
    pbr_input.specular_occlusion *= mix(0.35, 1.0, sky);
    // The inside of a crown, which little light reaches, darkened in the sun too, not only
    // in the sky's light, down to leaf.w (D247).
    if wind.bark < 0.5 && wind.leaf.w > 0.0 {
        let inside = mix(wind.leaf.w, 1.0, smoothstep(0.3, 0.95, sky));
        pbr_input.material.base_color = vec4<f32>(pbr_input.material.base_color.rgb * inside, pbr_input.material.base_color.a);
    }
}
    let leaf_colour = pbr_input.material.base_color.rgb;
    if wind.bark < 0.5 && wind.leaf.z > 0.0 {
        underside(&pbr_input);
    }
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );

    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
@if(VERTEX_COLORS) {
        if wind.bark < 0.5 {
            out.color = vec4<f32>(out.color.rgb + lit_through(pbr_input, leaf_colour, in.color.a, in.position), out.color.a);
        }
}
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
