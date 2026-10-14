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
fn lit_through(input: PbrInput, sky: f32, frag_coord: vec4<f32>) -> vec3<f32> {
    if wind.leaf.z <= 0.0 || lights.n_directional_lights == 0u {
        return vec3<f32>(0.0);
    }
    let sun = &lights.directional_lights[0];
    let L = (*sun).direction_to_light;
    let V = input.V;
    let forward = pow(clamp(dot(-V, L), 0.0, 1.0), 6.0);
    let behind = clamp(dot(-input.world_normal, L), 0.0, 1.0);
    let view_z = dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), input.world_position);
    let shadow = fetch_directional_shadow(0u, input.world_position, input.world_normal, view_z, frag_coord.xy);
    let deep = sky * sky;
    return input.material.base_color.rgb * THROUGH * (*sun).color.rgb * view.exposure
        * shadow * deep * wind.leaf.z * (0.8 * forward + 0.12 * behind);
}

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
    let wet = 0.85 * facing_up * facing_up + 0.55 * low * (0.35 + 0.65 * damp_side) + 0.6 * crevice;
    let moss = smoothstep(0.5, 0.85, wet + 0.35 * (fine - 0.5) - 0.05) * share;
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
// screen, since its normal leans out of the crown instead.
fn edge_fade(frag_coord: vec4<f32>, world: vec3<f32>) {
    let face = normalize(cross(dpdx(world), dpdy(world)));
    let towards = normalize(view.world_position - world);
    let keep = smoothstep(0.08, 0.3, abs(dot(face, towards)));
    if noise_at(frag_coord) > keep {
        discard;
    }
}

// Leaves and twigs right at the camera fade out, a pixel at a time and a different pixel
// each frame, so TAA blends them away rather than the camera pushing through them (D216).
fn near_fade(frag_coord: vec4<f32>, world: vec3<f32>) {
    let near = smoothstep(0.5, 1.6, distance(world, view.world_position));
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

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
@if(VISIBILITY_RANGE_DITHER)
    temporal_dither(in.position, in.visibility_range_dither);

    near_fade(in.position, in.world_position.xyz);
    if wind.bark < 0.5 {
        edge_fade(in.position, in.world_position.xyz);
    }

    var pbr_input = pbr_input_from_standard_material(in, is_front);
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
            out.color = vec4<f32>(out.color.rgb + lit_through(pbr_input, in.color.a, in.position), out.color.a);
        }
}
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
