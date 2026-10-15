// The foliage's main-pass fragment shader: Bevy 0.20's standard one (bevy_pbr
// src/render/pbr.wesl, MIT OR Apache-2.0, by the Bevy contributors), cut to what foliage
// uses, with one change: the crossfade between levels of detail dithers in a pattern that
// moves every frame, so temporal anti-aliasing blends it into a fade (D214). Bevy's own
// pattern is the same 4x4 grid every frame, which TAA cannot resolve and shows as a
// crawling screen door. The pattern is Jimenez's interleaved gradient noise (Call of Duty:
// Advanced Warfare), shifted each frame, as Unreal dithers its foliage's crossfades.

import bevy_pbr::render::pbr_bindings;
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::{PbrInput, STANDARD_MATERIAL_FLAGS_UNLIT_BIT},
    mesh_functions,
    mesh_view_bindings::{globals, view, lights},
    shadows::fetch_directional_shadow,
    clustered_forward::{view_fragment_cluster_index, unpack_clusterable_object_index_ranges},
};
@if(IRRADIANCE_VOLUME) import bevy_pbr::light_probe::irradiance_volume::irradiance_volume_light;
@if(IRRADIANCE_VOLUME && ENVIRONMENT_MAP) import bevy_pbr::render::mesh_view_bindings::{light_probes, environment_map_sampler};
@if(IRRADIANCE_VOLUME && ENVIRONMENT_MAP && MULTIPLE_LIGHT_PROBES_IN_ARRAY) import bevy_pbr::render::mesh_view_bindings::diffuse_environment_maps;
@if(IRRADIANCE_VOLUME && ENVIRONMENT_MAP && !MULTIPLE_LIGHT_PROBES_IN_ARRAY) import bevy_pbr::render::mesh_view_bindings::diffuse_environment_map;
@if(IRRADIANCE_VOLUME && ENVIRONMENT_MAP) import bevy_render::maths::quat_rotate;

// How much of the open sky's light reaches a point from the side `N` faces, 0 to 1: the sky
// occlusion volume's light there (D270) against the open sky's, as the environment map gives
// it. Under the crowns it is a fraction; in the open, all of it.
@if(IRRADIANCE_VOLUME && ENVIRONMENT_MAP)
fn sky_seen(world: vec3<f32>, N: vec3<f32>, frag_coord: vec4<f32>) -> f32 {
    let view_z = dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), vec4<f32>(world, 1.0));
    let orthographic = view.clip_from_view[3].w == 1.0;
    var ranges = unpack_clusterable_object_index_ranges(
        view_fragment_cluster_index(frag_coord.xy, view_z, orthographic),
    );
    let seen = irradiance_volume_light(world, N, &ranges);
    var d = quat_rotate(light_probes.view_rotation, N);
    d.z = -d.z;
    var open = vec3<f32>(0.0);
@if(MULTIPLE_LIGHT_PROBES_IN_ARRAY) {
    if light_probes.view_cubemap_index >= 0 {
        open = textureSampleLevel(diffuse_environment_maps[light_probes.view_cubemap_index], environment_map_sampler, d, 0.0).rgb;
    }
} @else {
    open = textureSampleLevel(diffuse_environment_map, environment_map_sampler, d, 0.0).rgb;
}
    let luma = vec3<f32>(0.2126, 0.7152, 0.0722);
    let open_light = dot(open, luma) * light_probes.intensity_for_view;
    if open_light <= 1.0e-6 {
        return 1.0;
    }
    return clamp(dot(seen, luma) / open_light, 0.0, 1.0);
}

// Bark and leaves reflect the sky only where they see the sky (D289, D292): the environment
// map's specular light is the open sky's, which no crown hides, so limbs up among the leaves,
// seen from below, and glossy leaves deep in the undergrowth shone slate-blue with a sky they
// cannot see. Specular occlusion from how much of the
// sky the point sees, as Frostbite's from ambient occlusion (Lagarde and de Rousiers, "Moving
// Frostbite to Physically Based Rendering", 2014, section 4.10.2), the visibility here the
// sky occlusion volume's, as Unreal's sky light occlusion dims its sky's reflections as well.
@if(IRRADIANCE_VOLUME && ENVIRONMENT_MAP)
fn sky_specular(input: PbrInput, frag_coord: vec4<f32>) -> f32 {
    let seen = sky_seen(input.world_position.xyz, input.N, frag_coord);
    let n_dot_v = max(dot(input.N, input.V), 1.0e-4);
    let roughness = input.material.perceptual_roughness * input.material.perceptual_roughness;
    return clamp(pow(n_dot_v + seen, exp2(-16.0 * roughness - 1.0)) - 1.0 + seen, 0.0, 1.0);
}

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
// than all but the outermost going dark (D264). A little: leaves in another crown's shadow
// pass on a few hundredths of the sun, so they read dark against those a fleck lights (D269).
// Its glow follows its own colour, before its underside was darkened.
fn lit_through(input: PbrInput, colour: vec3<f32>, sky: f32, frag_coord: vec4<f32>) -> vec3<f32> {
    if wind.leaf.z <= 0.0 || lights.n_directional_lights == 0u {
        return vec3<f32>(0.0);
    }
    let sun = &lights.directional_lights[0];
    let L = (*sun).direction_to_light;
    let V = input.V;
    let toward = clamp(dot(-V, L), 0.0, 1.0);
    let forward = 0.75 * pow(toward, 6.0) + 0.25 * toward * toward;
    // Each leaf at its own tilt (D269), so some glow brighter than their neighbours.
    let behind = clamp(dot(-input.N, L), 0.0, 1.0);
    // Mostly toward the sun, little from any angle on its far side: at a fifth, every leaf
    // facing away from the sun glowed, and the crowns along the top of the picture read lime,
    // 14% of its top third against Sons of the Forest's 2% (D300).
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
        * reached * deep * wind.leaf.z * (0.8 * forward + 0.05 * behind);
}

// The sky's light through a leaf (D272): what falls on its upper face, as the sky occlusion
// volume holds it there (D270), little deep under the canopy and most at its top, passed on
// in the leaf's own colour, yellower. Green light passes through a leaf as readily as it is
// reflected (a third to a half of it), so seen from below, a crown out of the sun glows a deep
// green from the sky above it rather than going black, and the leaves in the sun glow brightly
// (`lit_through`). Bevy's own diffuse transmission took the open sky, unshaded by the crowns,
// and the colour after its underside was darkened, so it gave almost nothing.
fn sky_through(input: PbrInput, colour: vec3<f32>, frag_coord: vec4<f32>) -> vec3<f32> {
    var light = vec3<f32>(0.0);
@if(IRRADIANCE_VOLUME) {
    let world = input.world_position.xyz;
    let up = normalize(world);
    let view_z = dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), input.world_position);
    let orthographic = view.clip_from_view[3].w == 1.0;
    var ranges = unpack_clusterable_object_index_ranges(
        view_fragment_cluster_index(frag_coord.xy, view_z, orthographic),
    );
    let above = irradiance_volume_light(world, up, &ranges);
    // Seen from below, a leaf shows the light it passes on; from above, what it reflects.
    let seen = 0.25 + 0.75 * smoothstep(0.1, -0.6, dot(input.V, up));
    light = colour * THROUGH * above * view.exposure * SKY_THROUGH * wind.leaf.z * seen;
}
    return light;
}

// How much of the light on a leaf's upper face it passes on, for its colour (D272).
const SKY_THROUGH: f32 = 1.2;

// How much of the sun reaches the inside of a crown through the leaves above (D264).
const LAYERS: f32 = 0.06;

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

// How much of the painted sky's light from below a crown's lower face gets (D264, D286).
const FLOOR_LIGHT: f32 = 0.2;

// Sky light under a canopy is dimmed by the leaves above and warmed by what they and the
// ground bounce: what a surface that sees none of the sky is lit by instead, as a share of
// the sky's light (D216, D286). Crysis bakes the same into its trees' vertices.
const BOUNCE: vec3<f32> = vec3<f32>(0.28, 0.3, 0.2);

// Field order must match FoliageExtension in grown/mod.rs and lush.rs.
// What the climate does to foliage (grown/mod.rs's FoliageClimate, D327): where its maps are
// round the camera, whether there are any and which painting, how wide the card atlas's cells
// are, which kind's layer of the leaves map each cell's card is (-1 none), and each layer's
// autumn colour, linear.
struct FoliageClimate {
    from_world: mat4x4<f32>,
    on: u32,
    painting: u32,
    across: f32,
    layers: array<vec4<i32>, 4>,
    autumn: array<vec4<f32>, 8>,
}

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
    // For leaves: how far the atlas's normal map turns each leaf from the crown's normal
    // (D269); 0 for none.
    relief: f32,
    climate: FoliageClimate,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> wind: Wind;
// The card atlas's normal map: each leaf's relief and tilt, in OpenGL's convention (D269), and
// in its alpha each pixel's roughness (D292).
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var leaf_normals: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var leaf_normals_sampler: sampler;
// What the climate leaves round the camera (climate.rs, D327): snow's depth in red, its
// brightest SNOW_FULL_M; the ground's wetness in green; rain now in blue; frost in alpha. And
// each kind's leaves: how many early and late in red and green, how far turned in blue and
// alpha.
@group(constants::MATERIAL_BIND_GROUP) @binding(104)
var climate_ground: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(105)
var climate_ground_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(106)
var climate_leaves: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(107)
var climate_leaves_sampler: sampler;

// As ground.wesl's.
const SNOW_FULL_M: f32 = 0.5;
const SNOW: vec3<f32> = vec3<f32>(0.68, 0.72, 0.8);
// How bright a turned leaf is against the green one it was: dead leaves are duller and darker
// than living ones; at a quarter brighter oaks and bracken went pastel orange and salmon.
const AUTUMN_LIGHT: f32 = 0.8;

// Where a plant stands in its kind's year, 0 early to 1 late: foliage.wesl's `hash`, so its
// leaves fall and turn alike.
fn plant_share(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn climate_place(world: vec3<f32>) -> vec2<f32> {
    return clamp((wind.climate.from_world * vec4<f32>(world, 1.0)).xy, vec2<f32>(0.0), vec2<f32>(1.0));
}

// Which kind's layer of the leaves map a card is, by its atlas cell (foliage.wesl's).
fn card_layer(uv: vec2<f32>) -> i32 {
    let cell = vec2<i32>(floor(uv / wind.climate.across));
    if any(cell < vec2<i32>(0)) || any(cell > vec2<i32>(3)) {
        return -1;
    }
    let index = cell.y * 4 + cell.x;
    return wind.climate.layers[index / 4][index % 4];
}

// Smooth noise in clumps a hand or two across, stretched to about uniform from 0 to 1: where a
// plant's leaves have fallen, those left lie in rounded clumps, not cubes. The same in the main
// pass and the prepass.
fn clump_hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn clumps(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(clump_hash(i), clump_hash(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(clump_hash(i + vec3<f32>(0.0, 1.0, 0.0)), clump_hash(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(clump_hash(i + vec3<f32>(0.0, 0.0, 1.0)), clump_hash(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(clump_hash(i + vec3<f32>(0.0, 1.0, 1.0)), clump_hash(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    let n = mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
    return clamp((n - 0.2) / 0.6, 0.0, 1.0);
}

// Leaves its kind's year says have fallen, on a plant whose cards carry no rank to fall by, as
// a bush's or a fern's do not (foliage.wesl's `fallen` takes the crowns', and marks them
// `ranked`, its second texture coordinate's y far below nothing): gone in clumps a hand or
// two across (`clumps`), so it thins and stands bare (D327). The same in the
// main pass and the prepass, so depth and colour agree on what is left.
fn bare(uv: vec2<f32>, ranked: bool, world: vec3<f32>) {
    if wind.climate.on == 0u || wind.bark > 0.5 || ranked {
        return;
    }
    let layer = card_layer(uv);
    if layer < 0 {
        return;
    }
    // Taken where the leaf is, not where its plant stands, which a pass may not be told: a
    // bush's leaves agree, being near each other, and a patch of bushes a few metres across
    // turns as one plant.
    let year = textureSampleLevel(climate_leaves, climate_leaves_sampler, climate_place(world), layer, 0.0);
    let on = mix(year.r, year.g, plant_share(floor(world / 3.0) * 1.7 + 0.3));
    if on < 0.999 && clumps(world * 2.5 + 0.37) > on {
        discard;
    }
}

// The season on a plant (climate.md, D327): its leaves turned their kind's colour, the plant
// between its kind's early and late by where it stands, each leaf a little apart; snow lying
// on what faces up, on needles and the tops of limbs while it is cold enough to stay there;
// and wet bark darker and glossier.
fn seasoned(input: ptr<function, PbrInput>, uv: vec2<f32>, world: vec3<f32>, instance: u32) {
    if wind.climate.on == 0u {
        return;
    }
    let origin = mesh_functions::get_world_from_local(instance)[3].xyz;
    let at = climate_place(world);
    var colour = (*input).material.base_color.rgb;
    if wind.bark < 0.5 {
        let layer = card_layer(uv);
        if layer >= 0 {
            let year = textureSampleLevel(climate_leaves, climate_leaves_sampler, climate_place(origin), layer, 0.0);
            let plant = mix(year.b, year.a, plant_share(origin * 1.7 + 0.3));
            let leaf = clamp(plant * (0.7 + 0.6 * clumps(world * 3.0 + 11.0)), 0.0, 1.0);
            let grey = dot(colour, vec3<f32>(0.2126, 0.7152, 0.0722));
            let autumn = wind.climate.autumn[layer].rgb;
            let tone = autumn * grey / max(dot(autumn, vec3<f32>(0.2126, 0.7152, 0.0722)), 1e-4) * AUTUMN_LIGHT;
            colour = mix(colour, tone, leaf);
        }
    }
    let weather = textureSampleLevel(climate_ground, climate_ground_sampler, at, 0.0);
    if wind.bark > 0.5 {
        let wet = max(weather.b, weather.g * 0.35);
        colour *= mix(1.0, 0.6, wet);
        (*input).material.perceptual_roughness = mix((*input).material.perceptual_roughness, 0.55, wet * 0.6);
    }
    // Snow lies on branches through a snowy spell, most in a frost; a thaw, the rain or a
    // day above freezing takes it off, as the ground's snow going takes it.
    let lying = smoothstep(0.02, 0.12, weather.r * SNOW_FULL_M) * (0.55 + 0.45 * weather.a) * (1.0 - weather.b);
    let up = dot((*input).N, normalize(world));
    let snow = lying * smoothstep(0.3, 0.75, up) * smoothstep(0.25, 0.6, wander(world * 1.3));
    colour = mix(colour, SNOW, snow * 0.92);
    (*input).material.base_color = vec4<f32>(colour, (*input).material.base_color.a);
}

// Each leaf turned to its own angle (D269): the atlas's normal map, laid on the card's own
// face, leans the crown's normal (which leans out of the whole crown, so it still shades as
// one mass) as far as each leaf is tilted, so leaves catch the sun one by one. Cards carry no
// tangents of their own (theirs say how they drop out, D223), so the frame comes from how
// the world position and the texture place change across the screen: Schüler's cotangent
// frame ("Normal Mapping Without Precomputed Tangents", 2013), which is right from either
// side of the card.
fn leaf_relief(input: ptr<function, PbrInput>, uv: vec2<f32>, world: vec3<f32>, n: vec2<f32>) {
    let dp1 = dpdx(world);
    let dp2 = dpdy(world);
    let duv1 = dpdx(uv);
    let duv2 = dpdy(uv);
    // Where a card is seen edge-on, or so far off that a pixel spans next to none of it, the
    // frame is lost: no relief there, rather than a normal of nothing, which shaded black.
    let across = cross(dp1, dp2);
    if dot(across, across) < 1.0e-24 {
        return;
    }
    let face = across * inverseSqrt(dot(across, across));
    let dp2perp = cross(dp2, face);
    let dp1perp = cross(face, dp1);
    let t = dp2perp * duv1.x + dp1perp * duv2.x;
    let b = dp2perp * duv1.y + dp1perp * duv2.y;
    let scale = inverseSqrt(max(max(dot(t, t), dot(b, b)), 1.0e-20));
    // Green is up the picture, toward a smaller v.
    let lean = (n.x * t - n.y * b) * scale;
    let leant = (*input).N + lean * wind.relief;
    // Kept only if it is a direction: a frame of nothing gives no number at all.
    let length2 = dot(leant, leant);
    if length2 > 1.0e-8 && length2 < 1.0e8 {
        let crown = (*input).N;
        (*input).N = leant * inverseSqrt(length2);
        // A leaf, or half of a folded one, turned from the crown's outward face sees less of
        // the open side the crown's light comes from, so it is a little darker in the sky's
        // light too, not only in the sun's (D292): in shade, where only that light falls, each
        // leaf then still reads apart from the next, as a bent normal's occlusion has it.
        let facing = clamp(dot((*input).N, crown), 0.0, 1.0);
        (*input).diffuse_occlusion *= mix(1.0, facing * facing, LEAF_TILT_SHADE);
    }
}

// How much a leaf turned from its crown's face is darkened in the sky's light (D292).
const LEAF_TILT_SHADE: f32 = 0.7;

// Each leaf's own gloss (D292): the atlas's roughness, kept in its normal map's alpha, the
// waxy upper face of an oak or holly leaf glossier than its veins, a hazel's softer, every
// underside matte; so the sun glints off leaves one by one as they face it. Widened where
// the leaves' normals turn faster across the screen than a pixel can hold, so a crown farther
// off is a soft sheen, not glitter: Tokuyoshi and Kaplanyan's specular anti-aliasing
// ("Improved Geometric Specular Antialiasing", 2019), as Godot does it (MIT, by Juan
// Linietsky, Ariel Manzur and the Godot contributors; scene_forward_clustered.glsl), with
// its defaults.
fn leaf_gloss(input: ptr<function, PbrInput>, rough: f32) {
    let n = (*input).N;
    let dndu = dpdx(n);
    let dndv = dpdy(n);
    let variance = GLOSS_AA_SPREAD * (dot(dndu, dndu) + dot(dndv, dndv));
    let kernel = min(2.0 * variance, GLOSS_AA_LIMIT);
    let alpha = rough * rough;
    (*input).material.perceptual_roughness = sqrt(clamp(alpha + kernel, 0.0, 1.0));
}

// How much of the sky a glossy leaf reflects, of what the environment map gives (D292). A
// leaf's sheen is the sun's glint off it; the sky's, at a leaf's gloss, turned the leaves in
// shade at the forest's edge to grey ghosts, a leaf seldom being one smooth plane facing the
// sky as a card's normal map has it.
const LEAF_SKY_SHEEN: f32 = 0.15;

// Godot's roughness limiter: the screen-space variance taken, and the most it adds.
const GLOSS_AA_SPREAD: f32 = 0.25;
const GLOSS_AA_LIMIT: f32 = 0.18;

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
//
// Moss has a body of its own (D294): cushions a few centimetres across that bulge and catch
// the light, strands of lighter and darker green in them, darker down between, yellower where
// it is thin.
//
// A band of moss round the foot of some trunks (D309), `band` the share of the tree's height
// it reaches up to, `rise` the point's (0 for none): on all sides, thickest on the damp one,
// its top edge given out in tongues and streaks up the furrows, as Sons of the Forest's
// mossy boles are; and the rest of a trunk's moss as before.
fn weather(input: ptr<function, PbrInput>, world: vec3<f32>, sky: f32, share: f32, band: f32, rise: f32) {
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
    // On what faces up, a root flare's broad top, in cushions with bark between, not a coat.
    let cushions = mix(1.0, smoothstep(0.35, 0.65, wander(world * 1.6 + 3.0) + 0.4 * (fine - 0.5)), facing_up);
    let tongues = band * (0.75 + 0.5 * (fine - 0.5) + 0.35 * damp_side + 0.4 * crevice);
    let banded = smoothstep(0.0, 0.25 * band, tongues - rise) * step(0.0001, band);
    let moss = max(smoothstep(0.42, 0.8, wet + 0.35 * (fine - 0.5) - 0.05) * share * cushions,
        banded * smoothstep(0.15, 0.45, fine + 0.3 * damp_side + 0.3 * crevice) * cushions);
    // Cushions and the strands in them, their height in metres.
    let heap = wander(world * 16.0 + 2.0) * 0.6 + wander(world * 41.0) * 0.4;
    let hx = dpdx(heap * MOSS_CUSHION_M);
    let hy = dpdy(heap * MOSS_CUSHION_M);
    let strands = wander(world * vec3<f32>(90.0, 140.0, 90.0) + 17.0);
    let thin = 1.0 - smoothstep(0.3, 0.75, moss);
    let moss_colour = vec3<f32>(0.1, 0.13, 0.04) * (0.7 + 0.5 * wander(world * 6.0))
        * mix(0.45, 1.15, heap) * (0.8 + 0.4 * strands)
        * mix(vec3<f32>(1.0), vec3<f32>(1.25, 1.05, 0.6), thin);
    // A band's moss is thick and lit, so brighter than the thin moss up a trunk's damp side.
    colour = mix(colour, moss_colour * mix(1.0, MOSS_BAND_LIGHT, banded), moss * 0.9);
    let cushioned = bumped((*input).N, dpdx(world), dpdy(world), hx, hy);
    (*input).N = normalize(mix((*input).N, cushioned, moss));
    (*input).diffuse_occlusion *= vec3<f32>(mix(1.0, mix(0.6, 1.0, heap), moss));
    // Where the foot meets the floor, soil and leaf mould over the bark between the moss, so
    // the tree goes down into the ground rather than standing on it.
    let soiled = low * facing_up * (1.0 - moss) * smoothstep(0.3, 0.7, wander(world * 2.3 + 5.0) + 0.3 * (fine - 0.5) + 0.2);
    colour = mix(colour, vec3<f32>(0.075, 0.052, 0.033), soiled * 0.75);
    colour *= mix(1.0, 0.72, low * (1.0 - moss));
    (*input).material.base_color = vec4<f32>(colour, (*input).material.base_color.a);
    (*input).material.perceptual_roughness = mix((*input).material.perceptual_roughness, 1.0, moss);
}

// How high the moss's cushions heap on bark, in metres (D294).
const MOSS_CUSHION_M: f32 = 0.012;

// A moss band's height in its tag: the share of the tree's height times this (lush.rs,
// `MOSS_BAND_SCALE`).
const MOSS_BAND_SCALE: f32 = 1020.0;
// How much lighter a band's moss is than a trunk's thin moss (D309).
const MOSS_BAND_LIGHT: f32 = 2.2;

// Bark's furrows and the faces of its plates turned from the sun shadowed by the plates round
// them in direct light (D309): micro-shadowing, as HDRP does it (after Brinck and Maximov's
// Uncharted 4 talk, 2016) and as the sunlit floor is (D294), from the bark's occlusion as the
// cavity and its relief-mapped normal, on the sun's share of its light alone. Without it a
// sunlit trunk's furrows were lit as its plates and its relief read as a fine print.
fn bark_micro_shadow(input: ptr<function, PbrInput>, frag: vec4<f32>) {
    if lights.n_directional_lights == 0u {
        return;
    }
    let L = lights.directional_lights[0].direction_to_light;
    let world = (*input).world_position;
    let view_z = dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), world);
    let n = (*input).world_normal;
    let sunlit = fetch_directional_shadow(0u, world, n, view_z, frag.xy) * clamp(dot(n, L) * 3.0, 0.0, 1.0);
    let ao = (*input).diffuse_occlusion.r;
    let aperture = 2.0 * ao * ao;
    let micro = clamp(abs(dot((*input).N, L)) + aperture - 1.0, 0.0, 1.0);
    let keep = mix(1.0, micro, sunlit * BARK_SUN_SHARE);
    let c = (*input).material.base_color;
    (*input).material.base_color = vec4<f32>(c.rgb * keep, c.a);
    // Out of the sun the furrows see less of the sky than the scan's occlusion says, the plates
    // standing a few centimetres over them (D309): in the shade, which most bark is in, its
    // relief read only as a fine print.
    (*input).diffuse_occlusion *= vec3<f32>(mix(1.0, ao, BARK_CAVITY));
}

// How much of a sunlit trunk's light is the sun's, so how much of it its furrows lose (D309).
const BARK_SUN_SHARE: f32 = 0.8;
// How much more the bark's furrows are occluded from the sky than its scan's occlusion map
// gives, as a share of it (D309).
const BARK_CAVITY: f32 = 0.5;

// A surface's normal bent by a height over it, from how the height and the position change
// across the screen: Mikkelsen's "Bump Mapping Unparametrized Surfaces on the GPU" (2010).
fn bumped(n: vec3<f32>, gx: vec3<f32>, gy: vec3<f32>, hx: f32, hy: f32) -> vec3<f32> {
    let r1 = cross(gy, n);
    let r2 = cross(n, gx);
    let det = dot(gx, r1);
    let bent = abs(det) * n - sign(det) * (hx * r1 + hy * r2);
    let length2 = dot(bent, bent);
    if length2 < 1.0e-20 {
        return n;
    }
    return bent * inverseSqrt(length2);
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

// As foliage.wesl's: the camera's eye and the figure it follows (D271).
struct Parting {
    eye: vec4<f32>,
    subject: vec4<f32>,
    bodies: array<vec4<f32>, 32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var<storage, read> parting: Parting;

// A third-person camera's view kept clear (D271, D274), with camera-obstruction fades as
// Unreal games dither theirs. A plant between the camera and the figure it follows, or round
// the lens, fades as a whole, stems and leaves alike, by as much as its mesh's tag says
// (terra_world's parting.rs eases it over a few tenths of a second), rather than a hole cut
// through it; and leaves at the lens fade out over a short band, so few are ever partly
// gone at once. Only in the main camera's own view, so the prepass agrees and shadows keep
// them. Bark by its plant's fade, and only within a stride of the lens, which slides round
// trunks (D285): a trunk dithered through farther off reads as a hole in something solid.
fn clear_view(frag_coord: vec4<f32>, world: vec3<f32>, instance: u32, leaf: bool) {
    let eye = parting.eye;
    if eye.w < 0.5 || distance(view.world_position, eye.xyz) > 0.01 {
        return;
    }
    var keep = 1.0 - f32(mesh_functions::get_tag(instance) & 0xffu) / 255.0;
    if leaf {
        let lens = min(LENS_M, distance(parting.subject.xyz, eye.xyz) * 0.6);
        keep = min(keep, smoothstep(lens - LENS_BAND_M, lens, distance(world, eye.xyz)));
    } else {
        // Bark the lens slides past, as it does round a trunk, dithers out within a stride
        // of it rather than sweeping across the screen as a smear TAA cannot resolve (D289).
        keep = min(keep, smoothstep(BARK_LENS_M - LENS_BAND_M, BARK_LENS_M, distance(world, eye.xyz)));
    }
    if keep < 1.0 && noise_at(frag_coord) > keep {
        discard;
    }
}

// Leaves are whole from this far from a third-person lens, and gone this much nearer.
const LENS_M: f32 = 1.4;
const LENS_BAND_M: f32 = 0.35;
// Bark is whole from this far from a third-person lens (D289).
const BARK_LENS_M: f32 = 0.8;

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
    return projected_at(in, is_front, u, v, vec2<f32>(0.0));
}

// As `projected`, the picture moved by `offset` metres along `u` and `v`.
@if(VERTEX_UVS_A && VERTEX_TANGENTS)
fn projected_at(in: VertexOutput, is_front: bool, u: vec3<f32>, v: vec3<f32>, offset: vec2<f32>) -> PbrInput {
    var side = in;
    let p = in.world_position.xyz;
    side.uv = vec2<f32>(dot(p, u), dot(p, v)) + offset;
    let n = normalize(in.world_normal);
    let t = normalize(u - n * dot(n, u) + v * 1.0e-4);
    let w = select(-1.0, 1.0, dot(cross(n, t), v) >= 0.0);
    side.world_tangent = vec4<f32>(t, w);
    return flattened(pbr_input_from_standard_material(side, is_front), side.uv);
}

// A bark scan with its broad light and dark taken out (D294): its colour over the picture's
// own at an eighth of a repeat, times its mean, so what is left is the plates, furrows and
// knots. A scan's broad stripes (Bark Brown 02's pale band, its green one) repeated round a
// trunk and moved row by row read as a plaid of squares; the bark's broad variation is the
// world's (`weather`), not the picture's.
fn flattened(p: PbrInput, uv: vec2<f32>) -> PbrInput {
    var out = p;
@if(!BINDLESS) {
    let at = (pbr_bindings::material.uv_transform * vec3<f32>(uv, 1.0)).xy;
    let levels = f32(textureNumLevels(pbr_bindings::base_color_texture));
    let broad = textureSampleLevel(pbr_bindings::base_color_texture, pbr_bindings::base_color_sampler, at, levels - 4.0).rgb;
    let mean = textureSampleLevel(pbr_bindings::base_color_texture, pbr_bindings::base_color_sampler, at, levels - 1.0).rgb;
    let even = pow(mean / max(broad, vec3<f32>(0.004)), vec3<f32>(FLATTEN));
    out.material.base_color = vec4<f32>(p.material.base_color.rgb * even, p.material.base_color.a);
}
    return out;
}

// How much of a bark scan's broad light and dark is taken out (D294).
const FLATTEN: f32 = 0.9;

fn hash2(p: vec2<f32>) -> vec2<f32> {
    return fract(sin(vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)))) * 43758.5453);
}

// The two blended in `unrepeated`, by how far along the bark the point is, in repeats.
fn bark_blend(a: PbrInput, b: PbrInput, k: f32) -> PbrInput {
    var out = a;
    out.material.base_color = mix(a.material.base_color, b.material.base_color, k);
    out.material.perceptual_roughness = mix(a.material.perceptual_roughness, b.material.perceptual_roughness, k);
    out.diffuse_occlusion = mix(a.diffuse_occlusion, b.diffuse_occlusion, k);
    out.specular_occlusion = mix(a.specular_occlusion, b.specular_occlusion, k);
    out.N = normalize(mix(a.N, b.N, k));
    return out;
}

// The two blended in `unrepeated` keeping the bark's contrast (D294): each one's difference
// from the picture's mean, weighed and scaled back up by the root of the weights' squares,
// Heitz and Neyret's variance-preserving blend ("High-Performance By-Example Noise using a
// Histogram-Preserving Blending Operator", HPG 2018). A plain fade halved the contrast over
// the middle of every repeat, which read as bands up a trunk a repeat apart, the plaid of a
// mid-ground trunk. Its relief kept the same way.
fn bark_kept(a: PbrInput, b: PbrInput, k: f32, mean: vec3<f32>) -> PbrInput {
    var out = bark_blend(a, b, k);
    let wa = 1.0 - k;
    let wb = k;
    let gain = inverseSqrt(max(wa * wa + wb * wb, 1.0e-4));
    let colour = mean + ((a.material.base_color.rgb - mean) * wa + (b.material.base_color.rgb - mean) * wb) * gain;
    out.material.base_color = vec4<f32>(max(colour, vec3<f32>(0.0)), out.material.base_color.a);
    let n = a.world_normal;
    out.N = normalize(n + ((a.N - n) * wa + (b.N - n) * wb) * gain);
    return out;
}

// A grown trunk's bark without its picture's repeats showing (D264): up a tall trunk one
// scan repeated every two metres read as bands, a pine's orange plates as tape. Each repeat
// up the limb is the picture from a place of its own, and where two meet they blend over
// the middle of a repeat each, as Quilez's texture-repetition and Unreal's texture variation
// work, so no two repeats in a column are the same and no seam shows; keeping the bark's
// contrast where they meet (`bark_kept`, D294).
@if(VERTEX_UVS_A)
fn unrepeated(in: VertexOutput, is_front: bool) -> PbrInput {
    let along = in.uv.y;
    var a = in;
    var b = in;
    let cell_a = floor(along);
    let cell_b = floor(along + 0.5);
    a.uv = in.uv + hash2(vec2<f32>(cell_a, 7.0));
    b.uv = in.uv + hash2(vec2<f32>(cell_b, 19.0));
    let k = smoothstep(0.3, 0.7, abs(fract(along) * 2.0 - 1.0));
    let pa = flattened(pbr_input_from_standard_material(a, is_front), a.uv);
    let pb = flattened(pbr_input_from_standard_material(b, is_front), b.uv);
    var mean = pa.material.base_color.rgb;
@if(!BINDLESS) {
    mean = textureSampleLevel(pbr_bindings::base_color_texture, pbr_bindings::base_color_sampler, vec2<f32>(0.5), 16.0).rgb
        * pbr_bindings::material.base_color.rgb;
@if(VERTEX_COLORS) {
    mean *= in.color.rgb;
}
}
    // The blend, not the plain lookup: returning that left every grown trunk's bark one
    // picture repeated up it, unflattened (D309).
    return bark_kept(pa, pb, k, mean);
}

// One side of a triplanar projection without its repeat showing (D294): hex tiling, as
// ground.wesl's, from Morten Mikkelsen's "Practical Real-Time Hex-Tiling" (JCGT 11(2), 2022;
// github.com/mmikk/hextile-demo, MIT licence, copyright (c) 2022 mmikk). Each hexagon, about
// `HEX_M` across, is a copy of the bark moved at random (not turned: furrows run up a trunk),
// and every point blends the three nearest, weighted toward the nearest and the lighter, so
// the copies meet between plates rather than across them. Bevy's material samples with the
// screen's own derivatives, so each of the three is the copy of one class of the lattice's
// corners (a three-colouring, every triangle one of each): a slot's copy changes only where
// its weight is none, and no seam of blurred mips shows.
@if(VERTEX_UVS_A && VERTEX_TANGENTS)
fn hex_projected(in: VertexOutput, is_front: bool, u: vec3<f32>, v: vec3<f32>) -> PbrInput {
    let p = in.world_position.xyz;
    let st = vec2<f32>(dot(p, u), dot(p, v)) / HEX_M;
    let skewed = vec2<f32>(st.x - 0.57735027 * st.y, 1.15470054 * st.y);
    let base = floor(skewed);
    let f = fract(skewed);
    let z = 1.0 - f.x - f.y;
    let s = step(0.0, -z);
    let s2 = 2.0 * s - 1.0;
    let w = vec3<f32>(-z * s2, s - f.y * s2, s - f.x * s2);
    var corners = array<vec2<f32>, 3>(base + vec2<f32>(s, s), base + vec2<f32>(s, 1.0 - s), base + vec2<f32>(1.0 - s, s));
    var barycentric = array<f32, 3>(w.x, w.y, w.z);
    var slots: array<PbrInput, 3>;
    var weights = vec3<f32>(0.0);
    for (var k = 0; k < 3; k++) {
        // The corner of class k, and its weight.
        var corner = corners[0];
        var weight = barycentric[0];
        for (var c = 0; c < 3; c++) {
            let d = i32(corners[c].x) - i32(corners[c].y);
            if ((d % 3) + 3) % 3 == k {
                corner = corners[c];
                weight = barycentric[c];
            }
        }
        slots[k] = projected_at(in, is_front, u, v, hash2(corner) * 17.0);
        let light = dot(slots[k].material.base_color.rgb, vec3<f32>(0.333));
        weights[k] = mix(1.0, light * 3.0, 0.5) * pow(weight, 7.0);
    }
    weights /= max(weights.x + weights.y + weights.z, 1.0e-6);
    var out = slots[0];
    out.material.base_color = slots[0].material.base_color * weights.x + slots[1].material.base_color * weights.y + slots[2].material.base_color * weights.z;
    out.material.perceptual_roughness = slots[0].material.perceptual_roughness * weights.x + slots[1].material.perceptual_roughness * weights.y + slots[2].material.perceptual_roughness * weights.z;
    out.diffuse_occlusion = slots[0].diffuse_occlusion * weights.x + slots[1].diffuse_occlusion * weights.y + slots[2].diffuse_occlusion * weights.z;
    out.specular_occlusion = slots[0].specular_occlusion * weights.x + slots[1].specular_occlusion * weights.y + slots[2].specular_occlusion * weights.z;
    out.N = normalize(slots[0].N * weights.x + slots[1].N * weights.y + slots[2].N * weights.z);
    return out;
}

// How far across the hexagons of a modelled trunk's bark are, in metres (D294).
const HEX_M: f32 = 0.75;

// A modelled trunk's bark (D264). Its own texture coordinates are an atlas made for a
// texture it no longer has, so the bark scan is laid on by where each point is, from three
// sides (triplanar), each weighted by how squarely the surface faces it, as terrain and rock
// shaders do: the same bark, at the same scale and relief, as a grown trunk's, without the
// seams a tiling picture would show along the model's own islands. The trees stand near
// enough to the start that the world's y is their up, so the furrows run up the trunk. The
// two sides a trunk mostly faces are hex-tiled, so its picture's repeat does not show as a
// plaid of squares (D294).
@if(VERTEX_UVS_A && VERTEX_TANGENTS)
fn triplanar(in: VertexOutput, is_front: bool) -> PbrInput {
    let n = normalize(in.world_normal);
    var w = pow(abs(n), vec3<f32>(6.0));
    w = w / (w.x + w.y + w.z);
    let x = vec3<f32>(1.0, 0.0, 0.0);
    let y = vec3<f32>(0.0, 1.0, 0.0);
    let z = vec3<f32>(0.0, 0.0, 1.0);
    let a = hex_projected(in, is_front, z, y);
    let b = projected(in, is_front, x, z);
    let c = hex_projected(in, is_front, x, y);
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
    clear_view(in.position, in.world_position.xyz, in.instance_index, wind.bark < 0.5);
@if(VERTEX_UVS_A && VERTEX_UVS_B)
    bare(in.uv, in.uv_b.y < -500.0, in.world_position.xyz);
@if(VERTEX_UVS_A && !VERTEX_UVS_B)
    bare(in.uv, false, in.world_position.xyz);

    var pbr_input: PbrInput;
@if(VERTEX_UVS_A && VERTEX_TANGENTS && !VERTEX_UVS_B) {
    // A modelled trunk: grown wood always has wind weights, a model none (D264).
    if wind.bark > 0.5 {
        pbr_input = triplanar(in, is_front);
    } else {
        pbr_input = pbr_input_from_standard_material(in, is_front);
    }
} @elif(VERTEX_UVS_A && VERTEX_UVS_B) {
    if wind.bark > 0.5 {
        pbr_input = unrepeated(in, is_front);
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
@if(VERTEX_UVS_A) {
    if wind.bark < 0.5 && wind.relief > 0.0 {
        let surface = textureSampleBias(leaf_normals, leaf_normals_sampler, in.uv, view.mip_bias);
        leaf_relief(&pbr_input, in.uv, in.world_position.xyz, surface.xy * 2.0 - 1.0);
        leaf_gloss(&pbr_input, surface.a);
    }
}
@if(VERTEX_COLORS) {
    // Alpha is how much of the sky this point sees.
    let sky = in.color.a;
    // Bevy takes a vertex's colour, alpha too, into its base colour, which the alpha test
    // then cuts by; the prepass does not, so where a leaf sees little sky it was cut here and
    // kept there, and stood black against the sky in its own shape (D272). Its alpha is the
    // picture's again, so both passes cut the same leaf.
    if wind.bark < 0.5 {
        let a = pbr_input.material.base_color.a;
        pbr_input.material.base_color.a = min(a / max(sky, 0.05), 1.0);
    }
    if wind.bark > 0.5 {
        // A grown trunk's band of moss, if it has one, from its mesh's tag (D309): how high
        // it reaches, as a share of the tree's height, in its second byte; its height there
        // from how far up the tree it is, which its sway's bend gives as its square.
        var band = 0.0;
        var rise = 1.0;
@if(VERTEX_UVS_B) {
        band = f32((mesh_functions::get_tag(in.instance_index) >> 8u) & 0xffu) / MOSS_BAND_SCALE;
        rise = sqrt(max(in.uv_b.x, 0.0));
}
        // 1 is bark with all its moss; up to 2, bark with less (D235).
        weather(&pbr_input, in.world_position.xyz, sky, clamp(2.0 - wind.bark, 0.0, 1.0), band, rise);
        bark_micro_shadow(&pbr_input, in.position);
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
    // in the sky's light, down to leaf.w (D247); the outermost leaves, which see most of the
    // sky, not at all, so a crown's skin stays bright over its dark inside (D269).
    if wind.bark < 0.5 && wind.leaf.w > 0.0 {
        let inside = mix(wind.leaf.w, 1.0, smoothstep(0.25, 0.8, sky));
        pbr_input.material.base_color = vec4<f32>(pbr_input.material.base_color.rgb * inside, pbr_input.material.base_color.a);
    }
}
@if(VERTEX_UVS_A)
    seasoned(&pbr_input, in.uv, in.world_position.xyz, in.instance_index);
@if(IRRADIANCE_VOLUME && ENVIRONMENT_MAP) {
    pbr_input.specular_occlusion *= sky_specular(pbr_input, in.position);
    if wind.bark < 0.5 {
        pbr_input.specular_occlusion *= LEAF_SKY_SHEEN;
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
            let through = lit_through(pbr_input, leaf_colour, in.color.a, in.position)
                + sky_through(pbr_input, leaf_colour, in.position);
            out.color = vec4<f32>(out.color.rgb + through, out.color.a);
        }
}
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
