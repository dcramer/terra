// The kit's laid floor (kit/laid.rs, D634): a scan of laid units through StandardMaterial's
// own maps, with the weather laid over it per pixel as a decal layer would be, so no unit
// reads clean (judge 2026-11-06: "fields of clean flagstone"): black grime packed in every
// joint and moss growing out of them (the scan's own occlusion marks the joints), over the
// whole unit where the floor is mossiest; hairline cracks across some units; the detail
// texture's fine grain (D362: stone's every half metre, grey about the middle, multiplied in
// as Unity's detail albedo is); and puddles of wet stone, darker and near-mirror smooth with
// a sharp water line, so lights glint in them (Lagarde, "Water drop 2b: dynamic rain and
// its effects", 2013: wet porous stone darkens and its roughness drops; standing water is
// flat and smooth over it).
//
// Where moss and water lie are the layout's maps, read by `source`: the mesh's second UV's
// x and y and its colour's alpha, each 0 to 1.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

struct Params {
    look: vec4<f32>,
    masks: vec4<f32>,
    vary: vec4<f32>,
    courses: array<vec4<f32>, 4>,
    joints: array<vec4<f32>, 4>,
    shape: vec4<f32>,
    wear: vec4<f32>,
    mark_at: array<vec4<f32>, 24>,
    mark_turn: array<vec4<f32>, 24>,
    mark_rep: array<vec4<f32>, 24>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(106)
var<uniform> laid: Params;
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

// The ground stack's moss (assets/materials/README.md, ambientCG Moss 004).
const MOSS: u32 = 7u;

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

// Which unit a point of the scan lies in (kit/laid.rs's `unit`, tested there): the tile,
// the course and the unit along it, a unit cut by the tile's edge counted where it starts.
fn unit(uv: vec2<f32>) -> vec4<f32> {
    var tile = floor(uv);
    let f = uv - tile;
    let n = u32(laid.shape.x);
    var r = n - 1u;
    var found = false;
    for (var k = 0u; k < n; k++) {
        if f.y >= laid.courses[k].x {
            r = k;
            found = true;
        }
    }
    if !found { tile.y -= 1.0; }
    let a = vec4<f32>(laid.courses[r].yzw, laid.joints[r].x);
    let b = vec3<f32>(laid.joints[r].yzw);
    var c = dot(step(a, vec4<f32>(f.x)), vec4<f32>(1.0)) + dot(step(b, vec3<f32>(f.x)), vec3<f32>(1.0));
    if c < 0.5 {
        tile.x -= 1.0;
        c = dot(step(a, vec4<f32>(1.5)), vec4<f32>(1.0)) + dot(step(b, vec3<f32>(1.5)), vec3<f32>(1.0));
    }
    return vec4<f32>(tile, f32(r), c);
}

// The `i`th joint across course `r` (u; 2 past its last).
fn joint_u(r: u32, i: u32) -> f32 {
    if i < 3u {
        return laid.courses[r][i + 1u];
    }
    return laid.joints[r][i - 3u];
}

// A unit as it has worn (`wear`): the piece of it a point lies in, its new joints and
// their chipped arrises, its tilt, and where it is gone to soil.
struct Flag {
    key: vec2<f32>,
    joint: f32,
    bevel: f32,
    tilt: vec3<f32>,
    gone: f32,
    rim: f32,
}

// Per-cell variation as Substance Designer's Tile Generator gives each tile (random size by
// splitting, luminance, tilt) and texture bombing's random per cell (Glanville, GPU Gems
// ch. 20): each unit of the scan, found by `unit`, hashed to decide whether it was split in
// two (and a half again, so the flags vary in size), broken across with its far side sunk
// and tilted its own way, or lost a corner or the whole to soil; more of them by the walls'
// feet, where the walls fell on them. Distances in metres (`uv_m` the metres a uv unit).
fn flag(uv: vec2<f32>, uv_m: f32, p: vec2<f32>, feet: f32) -> Flag {
    let u = unit(uv);
    let r = u32(u.z);
    let c = u32(u.w + 0.5);
    let n = u32(laid.shape.x);
    var nj = 1u;
    for (var i = 0u; i < 7u; i++) {
        if joint_u(r, i) < 1.5 {
            nj = i + 1u;
        }
    }
    let x0 = joint_u(r, max(c, 1u) - 1u);
    var x1 = joint_u(r, 0u) + 1.0;
    if c < nj {
        x1 = joint_u(r, c);
    }
    let y0 = laid.courses[r].x;
    var y1 = laid.courses[0].x + 1.0;
    if r + 1u < n {
        y1 = laid.courses[r + 1u].x;
    }
    let f = uv - u.xy;
    var q = (f - vec2<f32>(x0, y0)) * uv_m;
    var sz = (vec2<f32>(x1, y1) - vec2<f32>(x0, y0)) * uv_m;
    var key = u.xy * vec2<f32>(5.31, 7.17) + u.zw * vec2<f32>(1.73, 3.11) + 0.5;
    let w = laid.wear;
    let worn = 1.0 + feet * 1.5;

    // Split across its longer side, a third to two thirds along, and a half split again.
    var seam = 8.0;
    for (var k = 0; k < 2; k++) {
        let fk = f32(k);
        if hash(key + 31.0 + fk * 13.0) < w.x * select(1.0, 0.55, k == 1) {
            let t = 0.36 + 0.28 * hash(key + 37.0 + fk * 5.0);
            if sz.x >= sz.y {
                let s = sz.x * t;
                seam = min(seam, abs(q.x - s));
                if q.x >= s {
                    q.x -= s;
                    sz.x -= s;
                    key += vec2<f32>(53.0, 0.0);
                } else {
                    sz.x = s;
                    key += vec2<f32>(0.0, 29.0);
                }
            } else {
                let s = sz.y * t;
                seam = min(seam, abs(q.y - s));
                if q.y >= s {
                    q.y -= s;
                    sz.y -= s;
                    key += vec2<f32>(41.0, 0.0);
                } else {
                    sz.y = s;
                    key += vec2<f32>(0.0, 67.0);
                }
            }
        }
    }
    seam += (noise(p * 37.0) - 0.5) * 0.006;
    var joint = 1.0 - smoothstep(0.004, 0.012, seam);
    var bevel = 1.0 - smoothstep(0.004, 0.05, seam);

    // Broken across: a wandering crack, the far side sunk and tilted its own way.
    var sunk = 0.0;
    if hash(key + 71.0) < w.z * worn {
        let ca = hash(key + 73.0) * 6.2832;
        let c0 = sz * (0.3 + 0.4 * vec2<f32>(hash(key + 79.0), hash(key + 83.0)));
        let d = dot(q - c0, vec2<f32>(cos(ca), sin(ca))) + (noise(p * 6.0 + key) - 0.5) * 0.08
            + (noise(p * 23.0) - 0.5) * 0.015;
        joint = max(joint, 1.0 - smoothstep(0.002, 0.008, abs(d)));
        bevel = max(bevel, 0.7 * (1.0 - smoothstep(0.002, 0.03, abs(d))));
        if d > 0.0 {
            key += vec2<f32>(97.0, 89.0);
            sunk = 1.0;
        }
    }

    // Tilted: settled out of true, so it takes the light its own way; a sunk piece more.
    let a = hash(key + 67.0) * 6.2832;
    let tl = w.y * hash(key + 61.0) * hash(key + 61.0) * (1.0 + sunk);
    let tilt = vec3<f32>(cos(a), 0.0, sin(a)) * tl;

    // Gone: the whole piece, or a corner broken off it, its edge ragged.
    let ragged = (noise(p * 8.0) - 0.5) * 0.1 + (noise(p * 29.0) - 0.5) * 0.03;
    var depth = -1.0;
    if hash(key + 91.0) < w.w * worn {
        let e = min(min(q.x, sz.x - q.x), min(q.y, sz.y - q.y));
        depth = e + ragged - 0.05;
    } else if hash(key + 101.0) < w.z * worn * 0.6 {
        let ck = u32(hash(key + 103.0) * 3.99);
        let cp = vec2<f32>(select(0.0, sz.x, (ck & 1u) == 1u), select(0.0, sz.y, (ck & 2u) == 2u));
        let rr = min(sz.x, sz.y) * (0.25 + 0.25 * hash(key + 107.0));
        depth = rr - length(q - cp) + ragged;
    }
    let gone = smoothstep(-0.006, 0.006, depth);
    let rim = gone * (1.0 - smoothstep(0.0, 0.07, depth));
    return Flag(key, joint, bevel, tilt, gone, rim);
}

// How far `p` lies outside mark `k`'s nearest repeat (negative inside), in metres, and
// where it lies in that repeat's own frame (kit/laid.rs's `mark_edge`, tested there).
fn mark_edge(k: u32, p: vec2<f32>) -> vec3<f32> {
    let a = laid.mark_at[k];
    let b = laid.mark_turn[k];
    let c = laid.mark_rep[k];
    let d = p - a.xy;
    var q = vec2<f32>(d.x * b.x + d.y * b.y, -d.x * b.y + d.y * b.x);
    let half = fract(c.xy);
    q -= clamp(floor(q / b.zw - half + 0.5) + half, -c.xy, c.xy) * b.zw;
    let e = abs(q) - a.zw;
    return vec3<f32>(max(e.x, e.y), q);
}

// A mask's weight at the fragment (kit/laid.rs's `source`).
fn source(s: f32, in: VertexOutput) -> f32 {
    switch u32(s) {
        case 0u: { return in.uv_b.x; }
        case 1u: { return in.uv_b.y; }
        case 2u: { return in.color.a; }
        case 8u: { return 1.0; }
        default: { return 0.0; }
    }
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let p = in.world_position.xz;
    var mossy = source(laid.masks.x, in);
    let wet_w = source(laid.masks.y, in);
    var colour = pbr_input.material.base_color.rgb;

    // The joints: dark in the scan's occlusion.
    let ao = textureSample(arm, arm_sampler, in.uv).r;
    var joint = 1.0 - smoothstep(0.5, 0.75, ao);

    // Fine grain, so the units hold the refs' sharpness at the camera's distance.
    let grain = textureSample(detail, detail_sampler, p / laid.look.z).rgb * 2.0;
    colour *= mix(vec3<f32>(1.0), grain, laid.look.w);

    // Each unit its own stone: its value, warmth and roughness wander (quarried and worn
    // apart), the step hidden in the joint; and worn its own way (`wear`, biome.rs `Wear`):
    // split, tilted, broken across, a corner or the whole gone to soil.
    var rough_k = 1.0;
    let feet = source(laid.masks.w, in);
    let uv_m = length(dpdx(p)) / max(length(dpdx(in.uv)), 1e-6);
    var tilt_n = vec3<f32>(0.0);
    var gone = 0.0;
    var gone_rim = 0.0;
    var joint_all = joint;
    if laid.shape.x > 0.5 {
        let f = flag(in.uv, uv_m, p, feet);
        joint_all = max(joint, f.joint);
        let key = f.key;
        let inner = 1.0 - joint_all;
        // A third of the units far from the rest, an old court's mixed stone.
        let odd = select(1.0, mix(0.62, 1.3, hash(key + 5.0)), hash(key + 3.0) < 0.3 * step(0.001, laid.wear.x + laid.wear.z));
        colour *= (1.0 + laid.vary.x * (hash(key) * 2.0 - 1.0)) * mix(1.0, odd, inner);
        colour *= mix(vec3<f32>(1.0), vec3<f32>(1.06, 1.0, 0.92), hash(key + 11.0) * inner * laid.vary.x * 2.0);
        rough_k = 1.0 + laid.vary.y * (hash(key + 23.0) * 2.0 - 1.0) * inner;
        // Each piece's edge chipped and its arris worn: a soft shade along its new joints.
        colour *= 1.0 - 0.22 * f.bevel;
        tilt_n = f.tilt;
        gone = f.gone;
        gone_rim = f.rim;
    }
    joint = joint_all;

    // Grime packed in every joint, and in a band at the walls' feet.
    colour *= mix(1.0, laid.look.x, joint);
    colour *= mix(1.0, 0.55, feet * (0.5 + 0.5 * noise(p * 1.9 + 4.0)));

    // Soil blown into drifts at two scales, thickest in the joints and at the walls; grit a
    // couple of centimetres across strewn over all, lighter and darker.
    var dirty = 0.0;
    if laid.vary.z > 0.0 {
        let d = noise(p * 0.55 + 31.0) * 0.55 + noise(p * 1.9 + 5.0) * 0.3 + noise(p * 6.5) * 0.15;
        dirty = smoothstep(0.5, 0.78, d + feet * 0.35 + joint * 0.25) * laid.vary.z;
        let soil = vec3<f32>(0.115, 0.095, 0.075) * (0.7 + 0.6 * noise(p * 23.0));
        colour = mix(colour, soil, dirty);
        let cell = floor(p * 45.0);
        let speck = smoothstep(0.86, 0.94, hash(cell + 3.0)) * laid.vary.z * laid.shape.z;
        colour *= mix(1.0, mix(0.55, 1.45, hash(cell + 9.0)), speck);
    }

    // Sand and soil washed into the joints over the grime and the drifts (which would bury
    // it before), patchy along them and grainy, so an old court's joints read as earth, not
    // shadow (shape.w, 0: the dark joints alone).
    if laid.shape.w > 0.0 {
        let fill = smoothstep(0.05, 0.45, joint) * smoothstep(0.2, 0.6, noise(p * 2.3 + 13.0) + 0.3);
        let grains = mix(0.55, 1.45, hash(floor(p * 70.0) + 17.0));
        let sand = vec3<f32>(0.4, 0.31, 0.2) * grains * (0.8 + 0.4 * noise(p * 9.0));
        colour = mix(colour, sand, fill * laid.shape.w);
    }

    // Hairline cracks across some units: thin wandering lines in patches.
    let warp = vec2<f32>(noise(p * 3.1), noise(p * 3.1 + 7.0)) - 0.5;
    let wall = cell_wall(p * 1.25 + warp * 0.35);
    let cracked = smoothstep(0.55, 0.7, noise(p * 0.37 + 3.0));
    let crack = (1.0 - smoothstep(0.012, 0.035, wall)) * cracked;
    colour *= mix(1.0, laid.look.y, crack);

    // The marks, in their order: a patch's fresh darker surface sealed at its edge in a
    // tar band; a stain (tyres' rubber, dripped oil) darker and smoother, soft-edged and
    // streaked along its length; paint, crisp-edged, worn through in flecks and patches,
    // the surface's grain showing in it.
    var paint = 0.0;
    var mark_rough = 1.0;
    let grain_v = dot(grain, vec3<f32>(0.333));
    for (var k = 0u; k < u32(laid.shape.y); k++) {
        let m = mark_edge(k, p);
        let row = laid.mark_rep[k];
        let kind = u32(row.z + 0.5);
        let amount = row.w;
        let seed = f32(k) * 7.3;
        if kind == 3u {
            let inside = (1.0 - smoothstep(-0.004, 0.004, m.x)) * amount;
            colour *= mix(1.0, 0.68, inside);
            mark_rough *= mix(1.0, 0.85, inside);
            let band = 1.0 - smoothstep(0.02, 0.045, abs(m.x + 0.01) + (noise(m.yz * 9.0 + seed) - 0.5) * 0.03);
            colour *= mix(1.0, 0.3, band * amount);
            mark_rough *= mix(1.0, 0.45, band * amount);
        } else if kind == 2u {
            let streak = noise(m.yz * vec2<f32>(0.35, 4.5) + seed) * 0.6 + noise(m.yz * vec2<f32>(1.6, 11.0) + seed) * 0.4;
            let s = (1.0 - smoothstep(-0.12, 0.08, m.x)) * smoothstep(0.25, 0.7, streak) * amount;
            colour *= 1.0 - 0.5 * s;
            mark_rough *= 1.0 - 0.4 * s;
        } else {
            // Worn in broad soft patches, their edges frayed by the surface's own grain
            // (the aggregate's high points lose their paint first), never by a value
            // noise's grid thresholded sharp (it read as blocks, round 261107-0109). Read
            // in the mark's frame before its repeats, so each stripe of a zebra and each
            // dash wears its own way (in the repeat's frame all seven wore alike).
            let turn = laid.mark_turn[k].xy;
            let d = p - laid.mark_at[k].xy;
            let u = vec2<f32>(dot(d, turn), dot(d, vec2<f32>(-turn.y, turn.x)));
            let wear = noise(u * vec2<f32>(0.7, 1.8) + seed) * 0.65
                + noise(u * vec2<f32>(3.1, 5.3) + seed) * 0.2
                + (grain_v - 1.0) * 0.3 + 0.075;
            let cover = (1.0 - smoothstep(-0.003, 0.003, m.x)) * smoothstep(0.05, 0.25, wear - (1.0 - amount));
            let tint = select(vec3<f32>(0.6, 0.6, 0.58), vec3<f32>(0.62, 0.45, 0.07), kind == 1u);
            colour = mix(colour, tint * mix(1.0, grain_v, 0.6), cover);
            paint = max(paint, cover);
        }
    }

    // Moss: out of the joints first, then over the units where it is mossiest.
    let tuft = noise(p * 2.3) * 0.6 + noise(p * 7.7 + 2.0) * 0.4;
    mossy = min(mossy + feet * 0.4, 1.0);
    let in_joints = max(joint, crack) * smoothstep(0.3, 0.6, mossy + (tuft - 0.5) * 0.7);
    let on_flags = smoothstep(0.62, 0.82, mossy * (0.55 + 0.75 * tuft) + (1.0 - ao) * 0.5);
    let moss = max(in_joints, on_flags) * step(0.5, 8.5 - laid.masks.x);
    let m1 = textureSample(stack, stack_sampler, p / laid.masks.z, MOSS).rgb;
    let m2 = textureSample(stack, stack_sampler, p / (laid.masks.z * 2.3) + 0.37, MOSS).rgb;
    let moss_c = mix(m1, m2, smoothstep(0.3, 0.7, noise(p * 0.13 + 7.0))) * 0.75;
    colour = mix(colour, moss_c, moss * 0.9);

    // Where a unit or its corner is gone: the soil it was bedded in a few centimetres down,
    // grit and moss in it, shadowed under the broken rim.
    if gone > 0.0 {
        let s = noise(p * 4.0 + 19.0) * 0.6 + noise(p * 17.0) * 0.4;
        var earth = vec3<f32>(0.085, 0.07, 0.055) * (0.7 + 0.6 * s);
        earth *= mix(1.0, mix(0.6, 1.5, hash(floor(p * 60.0) + 7.0)), 0.5);
        let weeds = smoothstep(0.45, 0.7, noise(p * 3.3 + 2.0) * 0.7 + mossy * 0.5);
        earth = mix(earth, moss_c * 0.8, weeds * 0.8);
        earth *= 1.0 - 0.6 * gone_rim;
        colour = mix(colour, earth, gone);
    }

    // Puddles: the water line sharp and a little ragged, the water creeping out along the
    // joints; damp stone a hand's width round it.
    let rag = (noise(p * 6.0) - 0.5) * 0.12 + joint * 0.12;
    let wet = smoothstep(0.47, 0.53, wet_w + rag) * step(0.5, 8.5 - laid.masks.y);
    let damp = smoothstep(0.15, 0.45, wet_w + rag) * (1.0 - wet) * step(0.5, 8.5 - laid.masks.y);
    colour *= mix(1.0, 0.62, damp);
    colour *= mix(1.0, 0.38, wet);
    // Broad patches of damp stone, darker and glossier.
    let dp = noise(p * 0.27 + 41.0) * 0.65 + noise(p * 1.1 + 13.0) * 0.35;
    let damp_patch = smoothstep(0.58, 0.72, dp) * laid.vary.w * (1.0 - dirty);
    colour *= mix(1.0, 0.7, damp_patch);
    var rough = clamp(pbr_input.material.perceptual_roughness * rough_k, 0.05, 1.0);
    rough = mix(rough, 1.0, dirty) * mix(1.0, 0.55, damp_patch) * mark_rough;
    rough = mix(mix(rough, 0.55, paint), 1.0, gone);
    pbr_input.material.perceptual_roughness = mix(mix(rough, rough * 0.6, damp), 0.09, wet);
    // Each unit tilted its own way; standing water is flat: the units' relief shows only
    // faintly through it; soil has none of the stone's.
    pbr_input.N = normalize(mix(normalize(pbr_input.N + tilt_n), pbr_input.world_normal, max(wet * 0.85, gone * 0.7)));

    pbr_input.material.base_color = vec4<f32>(colour, 1.0);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
