// The spike's rim (rim.rs): the standard material, with a fresnel term of the hour's sky
// added to its emission, so a figure's edge catches the light.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> rim: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101)
var canopy_map: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102)
var canopy_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103)
var<uniform> lift: vec4<f32>;

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
        edge = (1.0 - smoothstep(lift.y, lift.z, facing)) * sky * lift.w;
    } else {
        // Brighter where the edge faces up, as a sky's light would.
        let up = 0.55 + 0.45 * clamp(n.y, 0.0, 1.0);
        edge = pow(1.0 - facing, rim.w) * up;
    }
    // Under the oaks the crowns hide the sky: its fill dimmed as the ground's is there
    // (ground.wesl), and the rim with it (rim.rs's canopy map, 128 m about the middle),
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
