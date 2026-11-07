// The spike's rim (rim.rs): the standard material, with a fresnel term of the hour's sky
// added to its emission, so a figure's edge catches the light.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::view,
    view_transformations::{depth_ndc_to_view_z, direction_world_to_view},
};
@if(DEPTH_PREPASS) import bevy_pbr::prepass::utils::prepass_depth;

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> rim: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var canopy_map: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var canopy_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var<uniform> lift: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(104)
var scans: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(105)
var scans_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(106)
var<uniform> stone: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(107)
var<uniform> moss: vec4<f32>;

// How much of a fighter's band this fragment keeps: whole where it is the body's outline,
// none inside it. The mesh normal alone cannot tell: every leaf of a crown and every fold
// of Tripo's bark grazes the view somewhere, so the band glittered over the treants and
// the tree monster as hard white specks (the panel's "chrome glitter", uc-gaps j1). As
// the toon renderers' screen-space rim does (miHoYo's depth-offset rim, Unite 2017; docs
// only), the depth prepass is read a few pixels out along the normal's screen direction:
// an outline has the ground metres behind it there, a leaf or fold another of the same
// body's surfaces a hand's breadth away.
fn outline(frag: vec4<f32>, n: vec3<f32>) -> f32 {
    var share = 1.0;
    @if(DEPTH_PREPASS) {
        let s = direction_world_to_view(n).xy;
        let len = length(s);
        if len > 0.0001 {
            // 6 px at 1080 lines, past the band's own width (2 to 3 px at 960, iso-art j30),
            // so its inner side still finds the ground beyond the outline.
            let offset = vec2<f32>(s.x, -s.y) / len * 6.0 * view.viewport.w / 1080.0;
            let lo = view.viewport.xy;
            let at = clamp(frag.xy + offset, lo, lo + view.viewport.zw - 1.0);
            let behind = depth_ndc_to_view_z(prepass_depth(vec4<f32>(at, 0.0, 1.0), 0u));
            let here = depth_ndc_to_view_z(frag.z);
            share = smoothstep(0.15, 0.5, here - behind);
        }
    }
    return share;
}

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(127.1, 311.7));
    return fract((q.x + q.y) * 43758.5453 + dot(q, q + 19.19));
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

// A stone's dressing (rim.rs `Stone`): the ground's moss scan laid from above over what
// faces up, broken into patches; lichen's pale blotches on its sides; soil grime and the
// ground's occlusion where it meets its floor, darker underneath. As Megascans' and
// Unreal's world-aligned moss blend over a rock asset, by the mesh's own normal.
struct Dressed {
    colour: vec3<f32>,
    // How much rougher (moss and soil are matte).
    matte: f32,
    occlusion: f32,
}

// `n` is the mesh's normal, `relief` the normal map's (pbr_input.N): the masks follow the
// scan's own relief, not the 700-triangle mesh's facets, whose straight edges cut the moss
// into flat sheets and the lichen into pale triangles (uc-gaps fault c, "rocks read as clay
// or plastic"). Megascans' and Unreal's world-aligned blends take the pixel's normal so.
fn dress(base: vec3<f32>, world: vec3<f32>, n: vec3<f32>, relief: vec3<f32>) -> Dressed {
    // Cavity from the normal map: where the scan's relief turns well off the mesh's
    // surface it is a crack or a hollow's wall, darkened in the albedo as a cavity map is
    // multiplied into a scan's base colour; in full sun the stone then keeps its relief
    // instead of reading as one smooth clay tone.
    let cavity = mix(0.62, 1.0, smoothstep(0.6, 0.97, dot(relief, n)));
    let own = base * 0.8 * cavity;
    let m = textureSample(scans, scans_sampler, world.xz / stone.w, u32(stone.z)).rgb;
    let grain = dot(m, vec3<f32>(0.3, 0.55, 0.15));
    let patches = noise(world.xz * 2.6 + world.y * 1.3) * 0.65 + noise(world.xz * 0.8 + 7.0) * 0.35;
    let ny = mix(n.y, relief.y, 0.75);
    let up = smoothstep(0.2, 0.8, ny);
    let cover = smoothstep(0.45, 0.62, up * moss.a + (patches - 0.5) * 0.7 + (grain - 0.25) * 0.6);
    var c = mix(own, m * moss.rgb * mix(0.75, 1.0, cavity), cover);
    // Lichen: grey-green blotches a hand across on the sides and shoulders, paler than the
    // stone by about twice, not a fixed pale grey: at (0.34, 0.36, 0.29) they stood six to
    // eight times the scans' 0.04-0.06 and read as white stickers in the sun.
    let side = smoothstep(-0.1, 0.25, ny) * (1.0 - cover);
    let lichen = smoothstep(0.68, 0.76, noise(world.xz * 7.0 + world.y * 5.0)) * side;
    let value = dot(own, vec3<f32>(0.3, 0.55, 0.15));
    c = mix(c, vec3<f32>(0.95, 1.0, 0.8) * min(value * 2.2, 0.16), lichen * 0.8);
    // The soil splashed and grown up its foot, and the ground's occlusion there.
    let over = world.y - stone.y;
    let foot = 1.0 - smoothstep(-0.02, 0.28, over + (patches - 0.5) * 0.12);
    c = mix(c, c * vec3<f32>(0.42, 0.36, 0.28), foot * 0.85);
    let under = mix(0.55, 1.0, smoothstep(-0.6, 0.15, n.y));
    let contact = mix(0.45, 1.0, smoothstep(-0.05, 0.3, over));
    return Dressed(c, max(cover, foot), under * contact);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);
    // The fresnel of the mesh's own normal, not the normal map's: the map's fur and folds
    // graze the view all over a hunched body seen from above, frosting it whole instead of
    // edging it (iso-art j28).
    var n = normalize(in.world_normal);
    if !is_front {
        n = -n;
    }
    if stone.x > 0.0 {
        let d = dress(pbr_input.material.base_color.rgb, in.world_position.xyz, n, pbr_input.N);
        pbr_input.material.base_color = vec4<f32>(d.colour, pbr_input.material.base_color.a);
        pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 1.0, d.matte);
        pbr_input.diffuse_occlusion *= vec3<f32>(d.occlusion);
        pbr_input.specular_occlusion *= d.occlusion;
    }
    let facing = clamp(dot(n, normalize(pbr_input.V)), 0.0, 1.0);
    var edge: f32;
    if lift.z > 0.0 {
        // A fighter's: a hard band on the silhouette only (facing under `lift.z`, whole
        // under `lift.y`), not a fresnel's tail: at 70 px a figure the tail of a power-3
        // curve is half its body, a wash of cold light on every surface that faces up
        // (iso-art j30's judges). Masked by the normal's height, as Dota 2's rim keeps to
        // its world-space top-down mask: a moon over the fight edges the sides and the
        // upper outline, never a top (at the ARPG's 55 degrees a normal over about 0.6 up
        // faces the camera; a canopy's leaves are all such) nor an underside.
        let sky = smoothstep(-0.35, 0.1, n.y) * (1.0 - smoothstep(0.5, 0.8, n.y));
        edge = (1.0 - smoothstep(lift.y, lift.z, facing)) * sky * lift.w * outline(in.position, n);
    } else {
        // Brighter where the edge faces up, as a sky's light would.
        let up = 0.55 + 0.45 * clamp(n.y, 0.0, 1.0);
        edge = pow(1.0 - facing, rim.w) * up;
    }
    // Under the oaks the crowns hide the sky: its fill dimmed as the ground's is there
    // (kit/ground.wesl), and the rim with it (rim.rs's canopy map, 128 m about the middle),
    // but for the share `lift.x` a fighter's rim keeps (the ARPGs' lift, not the sky's).
    let open = textureSample(canopy_map, canopy_sampler, in.world_position.xz / 128.0 + 0.5).r;
    pbr_input.diffuse_occlusion *= vec3<f32>(open * open);
    pbr_input.specular_occlusion *= open;
    let kept = mix(open, 1.0, lift.x);
    pbr_input.material.emissive = vec4<f32>(pbr_input.material.emissive.rgb * open + rim.rgb * edge * kept, pbr_input.material.emissive.a);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
