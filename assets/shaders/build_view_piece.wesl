// What a building's meshes not drawn by the construction materials' arrays (a kit's modelled
// pieces from their atlas, what is drawn plain, the next piece lit, the baked far detail and
// hulls) are drawn in while a debug view is on (D385, build/views.rs), and every mesh of a
// building in the detail view: the same view as build.wesl draws, from what Bevy's
// StandardMaterial makes of the mesh and its textures.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::view,
};
import super::build_views::{Seen, shown, LIGHT_ALONE, GREY_CARD};

@if(SCREEN_SPACE_AMBIENT_OCCLUSION)
import bevy_pbr::render::mesh_view_bindings::screen_space_ambient_occlusion_texture;
@if(SCREEN_SPACE_AMBIENT_OCCLUSION)
import bevy_pbr::ssao::utils::ssao_multibounce;

// Field order must match PieceView in build/views.rs.
struct PieceView {
    // x the view, y texels across a copy of its texture (0 none), z the screen pixels a
    // texel should cover a metre off.
    view: vec4<f32>,
    // The colour the view gives it whole.
    flat: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> piece: PieceView;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    // The light view: an 18% grey lit as it is, as the grey card beside it, cut out as drawn.
    if u32(piece.view.x + 0.5) == LIGHT_ALONE {
        _ = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);
        pbr_input.material.base_color = vec4(vec3(GREY_CARD), 1.0);
        pbr_input.material.emissive = vec4(0.0);
        var lit: FragmentOutput;
        lit.color = apply_pbr_lighting(pbr_input);
        lit.color = main_pass_post_lighting_processing(pbr_input, lit.color);
        return lit;
    }
    var seen: Seen;
    seen.view = u32(piece.view.x + 0.5);
    var uv = vec2(0.0);
@if(VERTEX_UVS_A) {
    uv = in.uv;
}
    seen.uv = uv;
    seen.uv_dx = dpdx(uv);
    seen.uv_dy = dpdy(uv);
    seen.position = in.world_position.xyz;
    seen.p_dx = dpdx(in.world_position.xyz);
    seen.p_dy = dpdy(in.world_position.xyz);
    seen.pixel = in.position.xy;
    seen.side = piece.view.y;
    seen.aim = piece.view.z;
    let geometric = normalize(select(-in.world_normal, in.world_normal, is_front));
    seen.geometric = geometric;
    seen.mapped = pbr_input.N;
@if(VERTEX_TANGENTS) {
    let t = in.world_tangent.xyz - geometric * dot(in.world_tangent.xyz, geometric);
    if dot(t, t) > 1e-8 {
        seen.tangent = normalize(t);
        seen.bitangent = cross(geometric, seen.tangent) * sign(in.world_tangent.w);
        seen.framed = 1.0;
    }
}
    // Its corners' colours are its baked shade only where it has a texture: far off they
    // are its texture itself.
    var corners = 1.0;
@if(VERTEX_COLORS) {
    if piece.view.y > 0.0 {
        corners = min(dot(in.color.rgb, vec3(0.2126, 0.7152, 0.0722)), 1.0);
    }
}
    seen.baked = corners;
    // Bevy takes the least of the texture's occlusion and the screen's; the screen's is
    // read again to tell them apart.
    let occlusion = pbr_input.diffuse_occlusion.r;
    var screen = 1.0;
@if(SCREEN_SPACE_AMBIENT_OCCLUSION) {
    let ssao = textureLoad(screen_space_ambient_occlusion_texture, vec2<i32>(in.position.xy - view.viewport.xy), 0i).r;
    screen = ssao_multibounce(ssao, pbr_input.material.base_color.rgb).r;
}
    seen.screen_occlusion = screen;
    seen.texture_occlusion = select(occlusion, 1.0, screen <= occlusion + 1e-4);
    seen.albedo = clamp(dot(pbr_input.material.base_color.rgb, vec3(0.2126, 0.7152, 0.0722)) * 2.0, 0.0, 1.0);
    seen.flat = piece.flat.rgb;
    // Cut out where its texture is, as it is drawn in depth.
    _ = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);
    var out: FragmentOutput;
    out.color = vec4(shown(seen), 1.0);
    return out;
}
