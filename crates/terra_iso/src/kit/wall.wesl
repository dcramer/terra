// A front's plaster weathered (kit/facade.rs `Wall`, iso-kit j42): the module's own plaster
// and paint, its value and stains varied by a scanned wall laid over it in world space at
// its true size, dirt washed down from each storey's string course and each sill in
// streaks of uneven length, and grime splashed up its foot, as rain weathers a city's
// rendered fronts (and as Unreal's and Unity's facade shaders add them over a trim sheet).
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
};

// x the scan's metres, y its contrast (1 its own), z the streaks' strength, w the foot's.
@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> look: vec4<f32>;
// x the storey's height, y the longest streak, z the sills' height over the storey's
// floor, w the sills' streaks' share of the string course's (all metres but w).
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var<uniform> storey: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var scan: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var scan_sampler: sampler;

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// Value noise along a line, `k` its seed.
fn noise(x: f32, k: f32) -> f32 {
    let i = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(hash(vec2<f32>(i, k)), hash(vec2<f32>(i + 1.0, k)), u);
}

// Streaks hanging `d` metres under their source at `along` metres: thin runs about 9 cm
// apart, each its own length (out to `longest`) and darkness, most missing, over a wide
// grey wash just under the source.
fn streaks(along: f32, d: f32, longest: f32, k: f32) -> f32 {
    let cell = along / 0.09;
    let i = floor(cell);
    let r = hash(vec2<f32>(i, k));
    let len = mix(0.25, longest, r * r);
    let on = step(0.45, hash(vec2<f32>(i, k + 7.0)));
    let across = 1.0 - abs(fract(cell) * 2.0 - 1.0);
    let thin = smoothstep(0.25, 0.85, across) * (0.5 + 0.5 * hash(vec2<f32>(i, k + 3.0)));
    let run = on * thin * (1.0 - smoothstep(0.0, len, d)) * step(0.0, d);
    let wash = 0.45 * noise(along / 0.7, k + 11.0) * (1.0 - smoothstep(0.0, 0.9, d)) * step(0.0, d);
    return clamp(run + wash, 0.0, 1.0);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);
    let w = in.world_position.xyz;
    let n = normalize(in.world_normal);
    // Along the face: x on a front facing the street, z on a reveal or a pier's end.
    let along = select(w.x, w.z, abs(n.x) > abs(n.z));

    // The scan: its colour over its own whole-tile mean (its last mip), so the paint keeps
    // its tint and takes the scan's stains, pocks and runs.
    let uv = vec2<f32>(along, -w.y) / look.x;
    let s = textureSample(scan, scan_sampler, uv).rgb;
    let mean = textureSampleLevel(scan, scan_sampler, uv, 12.0).rgb;
    let detail = clamp(s / max(mean, vec3<f32>(0.02)), vec3<f32>(0.0), vec3<f32>(2.0));
    var colour = pbr_input.material.base_color.rgb * mix(vec3<f32>(1.0), detail, look.y);

    // Streaks under each storey's string course (its top) and its sills; none on the
    // street storey's (its shop fronts and doors), whose dirt is the foot's.
    let h = storey.x;
    let level = floor(w.y / h);
    let t = w.y - level * h;
    let under_top = h - t;
    let under_sill = storey.z - t;
    var dirt = streaks(along, under_top, storey.y, level);
    dirt = max(dirt, storey.w * streaks(along + 0.37, under_sill, storey.y * 0.6, level + 31.0));
    dirt *= look.z * step(h * 0.98, w.y);

    // The foot: splash-back and soot, darkest at the pavement, its top edge ragged.
    let top = 0.55 + 0.45 * noise(along / 0.8, 5.0) + 0.15 * noise(along / 0.15, 9.0);
    let foot = look.w * (1.0 - smoothstep(0.05, top, w.y));

    let grime = vec3<f32>(0.42, 0.37, 0.3);
    colour *= mix(vec3<f32>(1.0), grime * 0.75, dirt);
    colour *= mix(vec3<f32>(1.0), grime * 0.55, foot);
    pbr_input.material.base_color = vec4<f32>(colour, pbr_input.material.base_color.a);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
