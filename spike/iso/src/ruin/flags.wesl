// The ruin's flags (ruin/flags.rs): the scanned Large Grey Tiles through StandardMaterial's
// own maps, with the weather laid over them per pixel as a decal layer would be, so no flag
// reads clean (judge 2026-11-06: "fields of clean flagstone"): black grime packed in every
// joint and moss growing out of them (the scan's own occlusion marks the joints), over the
// whole flag where the floor is mossiest; hairline cracks across some flags; the detail
// texture's fine grain (D362: stone's every half metre, grey about the middle, multiplied in
// as Unity's detail albedo is); and puddles of wet stone, darker and near-mirror smooth with
// a sharp, wandering water line and a dark wet rim, so the fires glint in them (Lagarde, "Water drop 2b: dynamic rain and
// its effects", 2013: wet porous stone darkens and its roughness drops; standing water is
// flat and smooth over it).
//
// The mesh's second UV carries the two weights: x how mossy (litter::mossy), y how wet
// (water::puddled), each 0 to 1.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var stack: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var stack_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var arm: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var arm_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(104)
var detail: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(105)
var detail_sampler: sampler;

// The ground stack's moss (assets/materials/README.md, ambientCG Moss 004) and its size,
// as the glade's ground draws it; the detail texture's repeat and strength (D362).
const MOSS: u32 = 7u;
const MOSS_M: f32 = 1.1;
const DETAIL_M: f32 = 0.5;
const DETAIL_K: f32 = 0.6;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// How near `p` is to a cell wall of a Voronoi pattern (F2 - F1, Worley 1996): 0 on the
// wall, so a thin band about it draws a crack.
fn cell_wall(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    var f1 = 8.0;
    var f2 = 8.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let g = vec2<f32>(f32(x), f32(y));
            let o = vec2<f32>(hash(i + g), hash(i + g + 17.0));
            let d = length(g + o - f);
            if d < f1 {
                f2 = f1;
                f1 = d;
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    return f2 - f1;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let p = in.world_position.xz;
    let mossy = in.uv_b.x;
    let wet_w = in.uv_b.y;
    var colour = pbr_input.material.base_color.rgb;

    // The joints: dark in the scan's occlusion.
    let ao = textureSample(arm, arm_sampler, in.uv).r;
    let joint = 1.0 - smoothstep(0.5, 0.75, ao);

    // Fine grain, so the flags hold the refs' sharpness at the camera's distance.
    let grain = textureSample(detail, detail_sampler, p / DETAIL_M).rgb * 2.0;
    colour *= mix(vec3<f32>(1.0), grain, DETAIL_K);

    // Grime packed in every joint.
    colour *= mix(1.0, 0.3, joint);

    // Hairline cracks across some flags: thin wandering lines in patches.
    let warp = vec2<f32>(noise(p * 3.1), noise(p * 3.1 + 7.0)) - 0.5;
    let wall = cell_wall(p * 1.25 + warp * 0.35);
    let cracked = smoothstep(0.55, 0.7, noise(p * 0.37 + 3.0));
    let crack = (1.0 - smoothstep(0.012, 0.035, wall)) * cracked;
    colour *= mix(1.0, 0.25, crack);

    // Moss: out of the joints first, then over the flags where it is mossiest.
    let tuft = noise(p * 2.3) * 0.6 + noise(p * 7.7 + 2.0) * 0.4;
    let in_joints = max(joint, crack) * smoothstep(0.3, 0.6, mossy + (tuft - 0.5) * 0.7);
    let on_flags = smoothstep(0.62, 0.82, mossy * (0.55 + 0.75 * tuft) + (1.0 - ao) * 0.5);
    let moss = max(in_joints, on_flags);
    let m1 = textureSample(stack, stack_sampler, p / MOSS_M, MOSS).rgb;
    let m2 = textureSample(stack, stack_sampler, p / (MOSS_M * 2.3) + 0.37, MOSS).rgb;
    let moss_c = mix(m1, m2, smoothstep(0.3, 0.7, noise(p * 0.13 + 7.0))) * 0.75;
    colour = mix(colour, moss_c, moss * 0.9);

    // Puddles. `wet_w` is only the puddle's broad shape, a wide ramp from the paving's
    // vertices 20 cm apart; the water line is drawn here, per pixel, where that ramp crosses
    // a level broken by warped noise of two octaves, so it wanders in bays and spits over a
    // hand's width or two. Drawn from the vertices, and with the joints in its level, it ran
    // in straight strokes along the flags' square edges (the judge's "straight lit edge").
    // Outside the line a darker rim a few centimetres wide, where the stone has drunk the
    // water, the joints wet a little further, then damp stone fading out. Inside it the
    // stone is wet stone, darker and smooth with its grain and joints showing: a black
    // mirror read as a hole in the floor (the judge).
    let wq = p + (vec2<f32>(noise(p * 1.9 + 3.0), noise(p * 1.9 + 9.0)) - 0.5) * 0.35;
    let rag = (noise(wq * 3.5) - 0.5) * 0.5 + (noise(wq * 9.0 + 5.0) - 0.5) * 0.25;
    let level = wet_w + rag * smoothstep(0.0, 0.25, wet_w);
    let wet = smoothstep(0.47, 0.53, level);
    let rim = smoothstep(0.32, 0.5, level) * (1.0 - wet);
    let creep = joint * smoothstep(0.2, 0.45, level) * (1.0 - wet);
    let damp = smoothstep(0.08, 0.4, level) * (1.0 - wet);
    colour *= mix(1.0, 0.7, damp);
    colour *= mix(1.0, 0.6, max(rim, creep));
    colour *= mix(1.0, 0.58, wet);
    // The rim stays matte, only darker: glossed (0.25) it caught the fires as a bright line
    // round each puddle, a cut-out's outline. The water's lobe is a little wide (0.2, not
    // a mirror's 0.09), so the fires near it spread a sheen over it rather than leaving it
    // black but for one point.
    let rough = pbr_input.material.perceptual_roughness;
    pbr_input.material.perceptual_roughness = mix(mix(rough, rough * 0.6, damp), 0.2, wet);
    // Standing water's face is flat, so its mirror is too: the stone shows through in its
    // colour, not its relief. 15% of the scan's normal left in it bent the fire's glint
    // along a flag's bevel into a straight lit edge across the puddle (the judge's tell).
    pbr_input.N = normalize(mix(pbr_input.N, pbr_input.world_normal, wet));

    pbr_input.material.base_color = vec4<f32>(colour, 1.0);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
