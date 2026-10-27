// The haze in front of a fire's card, at the card's own depth (D553): the fires are drawn after
// Bevy's volumetric fog (crates/terra_world/src/fire/fogged.rs), which marched each of their
// pixels on to what stands behind them, so each card takes the haze between the eye and itself
// here, by the fog's own law (patches/bevy_pbr/src/volumetric_fog/volumetric_fog.wesl, MIT OR
// Apache-2.0): Beer's law through the haze's density picture, the sun's single scattering
// through the Henyey-Greenstein phase, and the camera's ambient as the fog takes it. Left to the
// fog, smoke 6 m off with the sky behind was fogged as if it stood at the sky, a hard band at
// the horizon (fire round t12). As Unreal fogs a translucent at its own depth; its CPU twin,
// tested, is `fogged::seen`.

import bevy_pbr::render::{
    mesh_view_bindings::{view, lights},
    mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_VOLUMETRIC_BIT,
    shadows::{get_cascade_index, world_to_directional_light_local},
    shadow_sampling::sample_shadow_map_hardware,
    view_transformations::position_world_to_view,
};
@if(ATMOSPHERE)
import bevy_pbr::render::pbr_lighting::sample_transmittance_lut;
@if(ATMOSPHERE)
import bevy_pbr::atmosphere::functions::{calculate_visible_sun_ratio, clamp_to_surface};
@if(ATMOSPHERE)
import bevy_pbr::render::mesh_view_bindings::atmosphere;

// The haze's fog volume and the camera's fog as the fog pass is told them. Must match
// `fogged::Haze` (crates/terra_world/src/fire/fogged.rs).
struct Haze {
    uvw_from_world: mat4x4<f32>,
    // rgb the fog's colour, w its density factor (0 with no haze).
    fog: vec4<f32>,
    // rgb the light's tint in it, w its light intensity.
    light: vec4<f32>,
    // rgb the camera's ambient times its intensity, w the radius of the sphere round its box.
    ambient: vec4<f32>,
    // x absorption, y scattering, z the phase's asymmetry.
    law: vec4<f32>,
    // xyz the density picture's offset.
    offset: vec4<f32>,
}

// Steps from the eye to the card: the haze's density changes over tens of metres, its mist's
// drifts over a few texels of 3.75 m.
const HAZE_STEPS: u32 = 4u;
const FRAC_4_PI: f32 = 0.07957747154594767;

fn henyey_greenstein(neg_LdotV: f32, g: f32) -> f32 {
    let denom = 1.0 + g * g - 2.0 * g * neg_LdotV;
    return FRAC_4_PI * (1.0 - g * g) / (denom * sqrt(denom));
}

// What the haze between the eye and `at` does to light from there: rgb what it scatters toward
// the eye, exposed as the fog's is; a the share of `at`'s light that comes through.
fn haze_seen(haze: Haze, density_texture: texture_3d<f32>, density_sampler: sampler, at: vec3<f32>) -> vec4<f32> {
    let density_factor = haze.fog.w;
    if density_factor <= 0.0 {
        return vec4(0.0, 0.0, 0.0, 1.0);
    }
    let extinction = haze.law.x + haze.law.y;
    let eye = view.world_position;
    // The fog's ray is as long as what it ends at stands deep in the view, marched along the
    // pixel's way that far.
    let reach = max(-position_world_to_view(at).z, 0.0);
    let way = normalize(at - eye + vec3(0.0, 1.0e-6, 0.0));
    let step = reach / f32(HAZE_STEPS);
    let from_uvw = (haze.uvw_from_world * vec4(eye, 1.0)).xyz;
    let step_uvw = mat3x3(haze.uvw_from_world[0].xyz, haze.uvw_from_world[1].xyz, haze.uvw_from_world[2].xyz) * (way * step);
    var through = 1.0;
    // The sun's share at each step before its colour and phase, summed.
    var lit = 0.0;
    for (var i = 0u; i < HAZE_STEPS; i = i + 1u) {
        let uvw = from_uvw + step_uvw * (f32(i) + 0.5);
        var density = 0.0;
        if all(uvw >= vec3(0.0)) && all(uvw <= vec3(1.0)) {
            density = density_factor * textureSampleLevel(density_texture, density_sampler, uvw + haze.offset.xyz, 0.0).r;
        }
        through *= exp(-step * density * extinction);
        lit += exp(-density * haze.ambient.w * extinction) * density * step * through;
    }
    // The fog's ambient, as Bevy's lays it: by the ray's length alone, with no density in it.
    var scattered = exp(-reach * extinction) * haze.ambient.rgb;
    // Each sun lit through its shadow at the ray's middle, the shafts' scale over a card's few
    // metres; the fog takes a shadow at every step.
    let middle = eye + way * reach * 0.5;
    let middle_z = position_world_to_view(middle).z;
    for (var i = 0u; i < lights.n_directional_lights; i = i + 1u) {
        let light = &lights.directional_lights[i];
        if ((*light).flags & DIRECTIONAL_LIGHT_FLAGS_VOLUMETRIC_BIT) == 0u {
            break;
        }
        let L = (*light).direction_to_light.xyz;
        let phase = henyey_greenstein(dot(normalize(L), way), haze.law.z);
        let cascade = get_cascade_index(i, middle_z);
        let local = world_to_directional_light_local(i, cascade, vec4(middle + (*light).shadow_depth_bias * L, 1.0));
        var seen = f32(local.w != 0.0);
        if seen != 0.0 {
            seen = sample_shadow_map_hardware(local.xy, local.z, i32((*light).depth_texture_base_index + cascade));
        }
        var sun = (*light).color.rgb * phase * haze.fog.rgb * haze.light.rgb * haze.law.y * haze.light.w * view.exposure;
@if(ATMOSPHERE)
        {
            let P_as = (atmosphere.world_to_atmosphere * vec4(middle, 1.0)).xyz;
            let P_clamped = clamp_to_surface(atmosphere, P_as);
            let r = length(P_clamped);
            let mu_light = dot(L, normalize(P_clamped));
            sun *= sample_transmittance_lut(r, mu_light) * calculate_visible_sun_ratio(atmosphere, r, mu_light, (*light).sun_disk_angular_size);
        }
        scattered += sun * seen * lit;
    }
    return vec4(scattered, through);
}

// A premultiplied colour (the flames') at `at` behind the haze: its light dimmed, and the haze's
// own light over as much of the pixel as it covers.
fn hazed_premultiplied(color: vec4<f32>, seen: vec4<f32>) -> vec4<f32> {
    return vec4(color.rgb * seen.a + seen.rgb * color.a, color.a);
}

// A colour blended by its alpha (the smoke's) behind the haze.
fn hazed_blended(color: vec4<f32>, seen: vec4<f32>) -> vec4<f32> {
    return vec4(color.rgb * seen.a + seen.rgb, color.a);
}
