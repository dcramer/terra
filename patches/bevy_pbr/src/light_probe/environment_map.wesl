import package::light_probe::{light_probe_iterator_new, light_probe_iterator_next};
import package::render::mesh_view_bindings as bindings;
import package::render::mesh_view_bindings::light_probes;
import package::render::mesh_view_types::{
    LIGHT_PROBE_FLAG_AFFECTS_LIGHTMAPPED_MESH_DIFFUSE, LIGHT_PROBE_FLAG_PARALLAX_CORRECT
};
import package::render::pbr_lighting::{F_Schlick_vec, LightingInput, LayerLightingInput, LAYER_BASE, LAYER_CLEARCOAT};
import package::render::clustered_forward::ClusterableObjectIndexRanges;
import bevy_render::maths::quat_rotate;
@if(IRRADIANCE_VOLUME) import package::light_probe::irradiance_volume::irradiance_volume_light_held;

/// The maximum representable value in a 32-bit floating point number.
const FLOAT_MAX: f32 = 3.40282347e+38;

struct EnvironmentMapLight {
    diffuse: vec3<f32>,
    specular: vec3<f32>,
};

struct EnvironmentMapRadiances {
    irradiance: vec3<f32>,
    radiance: vec3<f32>,
}

struct MultiscatterResult {
    FssEss: vec3<f32>,
    FmsEms: vec3<f32>,
    Edss: vec3<f32>,
}

/// Computes the direction at which to sample the reflection probe.
///
/// * `light_from_world` is the matrix that transforms world space into light
///   probe space (a 1×1×1 cube centered on the origin).
///
/// * `parallax_correction_bounds` is the half-extents of the simulated
///   reflection boundaries used for parallax correction, in light probe space.
///   It's ignored if the `parallax_correct` parameter is false.
///
/// * `parallax_correct` is true if parallax correction is to be applied and
///   false otherwise.
fn compute_cubemap_sample_dir(
    world_ray_origin: vec3<f32>,
    world_ray_direction: vec3<f32>,
    light_from_world: mat4x4<f32>,
    parallax_correction_bounds: vec3<f32>,
    parallax_correct: bool,
    view_rotation: vec4<f32>,
) -> vec3<f32> {
    var sample_dir: vec3<f32>;

    // If we're supposed to parallax correct, then intersect with the light cube.
    if (parallax_correct) {
        // Compute the direction of the ray bouncing off the surface, in light
        // probe space.
        // Recall that light probe space is a 1×1×1 cube centered at the origin.
        let ray_origin = (light_from_world * vec4(world_ray_origin, 1.0)).xyz;
        let ray_direction = (light_from_world * vec4(world_ray_direction, 0.0)).xyz;

        // Solve for the intersection of that ray with each side of the cube.
        // Since our light probe is a 1×1×1 cube centered at the origin in light
        // probe space, the faces of the cube are at X = ±0.5, Y = ±0.5, and Z =
        // ±0.5.
        var t0 = (-parallax_correction_bounds - ray_origin) / ray_direction;
        var t1 = (parallax_correction_bounds - ray_origin) / ray_direction;

        // We're shooting the rays forward, so we need to rule out negative time
        // values. So, if t is negative, make it a large value so that we won't
        // choose it below.
        // We would use infinity here but WGSL forbids it:
        // https://github.com/gfx-rs/wgpu/issues/5515
        t0 = select(vec3(FLOAT_MAX), t0, t0 >= vec3(0.0));
        t1 = select(vec3(FLOAT_MAX), t1, t1 >= vec3(0.0));

        // Choose the minimum valid time value to find the intersection of the
        // first cube face.
        let t_min = min(t0, t1);
        let t = min(min(t_min.x, t_min.y), t_min.z);

        // Compute the sample direction. (It doesn't have to be normalized.)
        sample_dir = ray_origin + ray_direction * t;
    } else {
        // We treat the reflection as infinitely far away in the non-parallax
        // case, so the ray origin is irrelevant.
        sample_dir = (light_from_world * vec4(world_ray_direction, 0.0)).xyz;
    }

    // Rotating the world space ray direction by the environment map light view rotation,
    // it is equivalent to rotating the environment cubemap itself.
    sample_dir = quat_rotate(view_rotation, sample_dir);

    // Cubemaps are left-handed, so we negate the Z coordinate.
    sample_dir.z = -sample_dir.z;
    return sample_dir;
}

// Terra's patch (D485, D336): how much of the view's environment map, the open sky's, a
// fragment mirrors toward `direction`, 0 to 1: the light the irradiance volumes holding it
// give from that side against the open sky's diffuse light from it, mixed in by the volumes'
// weight, so 1 outside every volume and through a volume's falloff. Bevy 0.20 occludes the
// sky's diffuse light by a volume and not its reflection, so leather, skin and stone under a
// canopy or in a den mirrored the whole open sky. The reflection's occlusion is derived from
// the diffuse light's visibility, as Frostbite's specular occlusion is (Lagarde and de
// Rousiers, "Moving Frostbite to Physically Based Rendering 3.0", 2014, section 4.10.2) and
// as Unreal's sky light occlusion dims its sky's reflections with its diffuse light. Per
// fragment, so it does not change with where the camera stands. A reflection probe's own
// map (a room's) is not scaled: it holds what the room sees.
/// The view's environment map's diffuse light toward a direction in the map's own frame,
/// before its brightness (Terra's patch, D485).
@if(IRRADIANCE_VOLUME && MULTIPLE_LIGHT_PROBES_IN_ARRAY)
fn view_diffuse_toward(in_map: vec3<f32>) -> vec3<f32> {
    // Cubemaps are left-handed, so we negate the Z coordinate.
    return textureSampleLevel(
        bindings::diffuse_environment_maps[light_probes.view_cubemap_index],
        bindings::environment_map_sampler,
        vec3<f32>(in_map.xy, -in_map.z),
        0.0).rgb;
}

// Terra's patch (D485): what an irradiance volume's light is scaled by at a normal, so a
// cell holding the open sky lights a surface exactly as the view's environment map does.
// A volume's cell is an ambient cube (Mitchell, McTaggart and Green, "Shading in Valve's
// Source Engine", SIGGRAPH 2006: six sides, blended by the squares of the normal's parts);
// the map's diffuse light is the sky's spherical harmonics. The two hold the same light and
// agree only for a normal along an axis: off it the cube read 2 to 6% brighter or dimmer by
// direction, a step in every surface's shade where a volume began. Shipped engines keep the
// sky's light in one form and let the volume say only how much of it arrives: Unreal's
// volumetric lightmap and distance field ambient occlusion give a bent normal and an
// occlusion the sky light's own harmonics are read with ("Volumetric Lightmaps", "Distance
// Field Ambient Occlusion", Unreal Engine documentation), and Far Cry 4 multiplies the
// sky's harmonics by a baked sky occlusion (Stefanov, "Global Illumination in Far Cry 4",
// GDC 2015). Here the volume is not baked again: its light is divided by what an open cell
// holds toward the normal and multiplied by the map's light there, which is the share of
// the sky that arrives, in the cube's terms, times the sky's own light.
//
// What an open cell holds: the app fills a side with the map's light toward the six axes of
// the map's own frame, blended by the squares of the side's parts in that frame (Terra's
// `sky_occlusion::world_sides`), and the shader blends three sides by the normal's. Both
// are done again here from the map's six face middles. Each colour apart, so an open cell's
// hue is the map's too; held within a half and two, and left alone where the map is dark.
@if(IRRADIANCE_VOLUME && MULTIPLE_LIGHT_PROBES_IN_ARRAY)
fn sky_light_over_its_cube(N: vec3<f32>) -> vec3<f32> {
    if (light_probes.view_cubemap_index < 0) {
        return vec3<f32>(1.0);
    }
    let rotation = light_probes.view_rotation;
    let toward = view_diffuse_toward(quat_rotate(rotation, N));
    let x_pos = view_diffuse_toward(vec3<f32>(1.0, 0.0, 0.0));
    let x_neg = view_diffuse_toward(vec3<f32>(-1.0, 0.0, 0.0));
    let y_pos = view_diffuse_toward(vec3<f32>(0.0, 1.0, 0.0));
    let y_neg = view_diffuse_toward(vec3<f32>(0.0, -1.0, 0.0));
    let z_pos = view_diffuse_toward(vec3<f32>(0.0, 0.0, 1.0));
    let z_neg = view_diffuse_toward(vec3<f32>(0.0, 0.0, -1.0));
    let facing = sign(N);
    let NN = N * N;
    var cube = vec3<f32>(0.0);
    for (var i = 0u; i < 3u; i += 1u) {
        var side = vec3<f32>(0.0);
        side[i] = facing[i];
        let in_map = quat_rotate(rotation, side);
        let pos = max(in_map, vec3<f32>(0.0));
        let neg = max(-in_map, vec3<f32>(0.0));
        cube += NN[i] * (x_pos * pos.x * pos.x + x_neg * neg.x * neg.x
            + y_pos * pos.y * pos.y + y_neg * neg.y * neg.y
            + z_pos * pos.z * pos.z + z_neg * neg.z * neg.z);
    }
    let scale = clamp(toward / max(cube, vec3<f32>(1.0e-6)), vec3<f32>(0.5), vec3<f32>(2.0));
    return select(scale, vec3<f32>(1.0), cube <= vec3<f32>(1.0e-6));
}

/// The view's environment map's diffuse light toward a world direction, at the view's
/// brightness: what `sky_light_over_its_cube` brings an open cell's light to, for a shader
/// that reads a volume's light itself (a leaf's light from above, shaded snow's tint) to
/// fall back on where no volume holds the fragment, or none is in the picture (Terra's
/// patch, D485). Nothing where the view has no map.
@if(ENVIRONMENT_MAP && MULTIPLE_LIGHT_PROBES_IN_ARRAY)
fn sky_light_toward(N: vec3<f32>) -> vec3<f32> {
    if (light_probes.view_cubemap_index < 0) {
        return vec3<f32>(0.0);
    }
    let in_map = quat_rotate(light_probes.view_rotation, N);
    // Cubemaps are left-handed, so we negate the Z coordinate.
    return textureSampleLevel(
        bindings::diffuse_environment_maps[light_probes.view_cubemap_index],
        bindings::environment_map_sampler,
        vec3<f32>(in_map.xy, -in_map.z),
        0.0).rgb * light_probes.intensity_for_view;
}

@if(IRRADIANCE_VOLUME && MULTIPLE_LIGHT_PROBES_IN_ARRAY)
fn sky_reflection_seen(
    world_position: vec3<f32>,
    direction: vec3<f32>,
    clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>,
) -> f32 {
    if (light_probes.view_cubemap_index < 0) {
        return 1.0;
    }
    let held = irradiance_volume_light_held(world_position, direction, clusterable_object_index_ranges);
    let weight = min(held.w, 1.0);
    if (weight <= 0.0) {
        return 1.0;
    }
    var sample_dir = quat_rotate(light_probes.view_rotation, direction);
    sample_dir.z = -sample_dir.z;
    let luma = vec3<f32>(0.2126, 0.7152, 0.0722);
    let open = dot(textureSampleLevel(
        bindings::diffuse_environment_maps[light_probes.view_cubemap_index],
        bindings::environment_map_sampler,
        sample_dir,
        0.0).rgb, luma) * light_probes.intensity_for_view;
    if (open <= 1.0e-6) {
        return 1.0;
    }
    return mix(1.0, saturate(dot(held.rgb, luma) / open), weight);
}

// Define two versions of this function, one for the case in which there are
// multiple light probes and one for the case in which only the view light probe
// is present.

@if(MULTIPLE_LIGHT_PROBES_IN_ARRAY)

fn compute_radiances(
    input: LayerLightingInput,
    clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>,
    world_position: vec3<f32>,
    found_diffuse_indirect: bool,
) -> EnvironmentMapRadiances {
    // Unpack.
    let N = input.N;
    let R = input.R;
    let perceptual_roughness = input.perceptual_roughness;
    let roughness = input.roughness;

    var radiances: EnvironmentMapRadiances;

    // Find all reflection probes that contain the fragment. We're going to
    // accumulate all the radiance and irradiance from them in a weighted sum.
    var iterator = light_probe_iterator_new(
        world_position,
        /*is_irradiance_volume=*/ false,
        clusterable_object_index_ranges,
    );

    var total_weight = 0.0;
    radiances.irradiance = vec3(0.0);
    radiances.radiance = vec3(0.0);

    while (true) {
        var query_result = light_probe_iterator_next(&iterator);
        var view_rotation = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        // Terra's patch (D485, D336): the share of the view's map this fragment mirrors.
        var sky_seen = 1.0;

        // If we reached the end of the light probe list, and we didn't find
        // enough reflection probes to reach a weight of 1.0, use the view
        // environment map if applicable. This allows for e.g. nice transitions
        // between the interior of a building and the outdoor environment map.
        if (query_result.texture_index < 0 && total_weight < 0.9999) {
            query_result.texture_index = light_probes.view_cubemap_index;
            query_result.intensity = light_probes.intensity_for_view;
            view_rotation = light_probes.view_rotation;
            query_result.light_from_world = mat4x4(
                vec4(1.0, 0.0, 0.0, 0.0),
                vec4(0.0, 1.0, 0.0, 0.0),
                vec4(0.0, 0.0, 1.0, 0.0),
                vec4(0.0, 0.0, 0.0, 1.0)
            );
            query_result.parallax_correction_bounds = vec3(0.0);
            if light_probes.view_environment_map_affects_lightmapped_mesh_diffuse != 0u {
                query_result.flags = LIGHT_PROBE_FLAG_AFFECTS_LIGHTMAPPED_MESH_DIFFUSE;
            } else {
                query_result.flags = 0u;
            }
            query_result.weight = 1.0 - total_weight;
            @if(IRRADIANCE_VOLUME)
            sky_seen = sky_reflection_seen(
                world_position,
                radiance_sample_direction(N, R, roughness),
                clusterable_object_index_ranges,
            );
        }

        // If we reached the end, we're done.
        if (query_result.texture_index < 0) {
            break;
        }

        // Split-sum approximation for image based lighting: https://cdn2.unrealengine.com/Resources/files/2013SiggraphPresentationsNotes-26915738.pdf
        let radiance_level = perceptual_roughness * f32(textureNumLevels(
            bindings::specular_environment_maps[query_result.texture_index]) - 1u);

        // If we're lightmapped, and we shouldn't accumulate diffuse light from the
        // environment map, note that.
        var enable_diffuse = !found_diffuse_indirect;
        @if(LIGHTMAP)
        enable_diffuse = enable_diffuse &&
            (query_result.flags & LIGHT_PROBE_FLAG_AFFECTS_LIGHTMAPPED_MESH_DIFFUSE) != 0u;

        let parallax_correct = (query_result.flags & LIGHT_PROBE_FLAG_PARALLAX_CORRECT) != 0u;

        if (enable_diffuse) {
            let irradiance_sample_dir = compute_cubemap_sample_dir(
                world_position,
                N,
                query_result.light_from_world,
                query_result.parallax_correction_bounds,
                parallax_correct,
                view_rotation,
            );
            radiances.irradiance += textureSampleLevel(
                bindings::diffuse_environment_maps[query_result.texture_index],
                bindings::environment_map_sampler,
                irradiance_sample_dir,
                0.0).rgb * query_result.intensity * query_result.weight;
        }

        var radiance_sample_dir = radiance_sample_direction(N, R, roughness);
        radiance_sample_dir = compute_cubemap_sample_dir(
            world_position,
            radiance_sample_dir,
            query_result.light_from_world,
            query_result.parallax_correction_bounds,
            parallax_correct,
            view_rotation,
        );
        radiances.radiance +=
            textureSampleLevel(
                bindings::specular_environment_maps[query_result.texture_index],
                bindings::environment_map_sampler,
                radiance_sample_dir,
                radiance_level).rgb * query_result.intensity * query_result.weight * sky_seen;

        total_weight += query_result.weight;
    }

    if (total_weight != 0.0) {
        radiances.irradiance /= total_weight;
        radiances.radiance /= total_weight;
    }

    return radiances;
}

@else   // MULTIPLE_LIGHT_PROBES_IN_ARRAY

fn compute_radiances(
    input: LayerLightingInput,
    clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>,
    world_position: vec3<f32>,
    found_diffuse_indirect: bool,
) -> EnvironmentMapRadiances {
    // Unpack.
    let N = input.N;
    let R = input.R;
    let perceptual_roughness = input.perceptual_roughness;
    let roughness = input.roughness;

    var radiances: EnvironmentMapRadiances;

    // If we have no light probe, bail.
    if (light_probes.view_cubemap_index < 0) {
        radiances.irradiance = vec3(0.0);
        radiances.radiance = vec3(0.0);
        return radiances;
    }

    // Split-sum approximation for image based lighting: https://cdn2.unrealengine.com/Resources/files/2013SiggraphPresentationsNotes-26915738.pdf
    // Technically we could use textureNumLevels(specular_environment_map) - 1 here, but we use a uniform
    // because textureNumLevels() does not work on WebGL2
    let radiance_level = perceptual_roughness * f32(light_probes.smallest_specular_mip_level_for_view);

    let intensity = light_probes.intensity_for_view;

    // If we're lightmapped, and we shouldn't accumulate diffuse light from the
    // environment map, note that.
    var enable_diffuse = !found_diffuse_indirect;
    @if(LIGHTMAP)
    enable_diffuse = enable_diffuse &&
        light_probes.view_environment_map_affects_lightmapped_mesh_diffuse;

    if (enable_diffuse) {
        var irradiance_sample_dir = N;
        // Rotating the world space ray direction by the environment map light view rotation,
        // it is equivalent to rotating the environment cubemap itself.
        irradiance_sample_dir = quat_rotate(light_probes.view_rotation, irradiance_sample_dir);
        // Cube maps are left-handed so we negate the z coordinate.
        irradiance_sample_dir.z = -irradiance_sample_dir.z;
        radiances.irradiance = textureSampleLevel(
            bindings::diffuse_environment_map,
            bindings::environment_map_sampler,
            irradiance_sample_dir,
            0.0).rgb * intensity;
    }

    var radiance_sample_dir = radiance_sample_direction(N, R, roughness);
    // Rotating the world space ray direction by the environment map light view rotation,
    // it is equivalent to rotating the environment cubemap itself.
    radiance_sample_dir = quat_rotate(light_probes.view_rotation, radiance_sample_dir);
    // Cube maps are left-handed so we negate the z coordinate.
    radiance_sample_dir.z = -radiance_sample_dir.z;
    radiances.radiance = textureSampleLevel(
        bindings::specular_environment_map,
        bindings::environment_map_sampler,
        radiance_sample_dir,
        radiance_level).rgb * intensity;

    return radiances;
}

@if(STANDARD_MATERIAL_CLEARCOAT)

/// Adds the environment map light from the clearcoat layer to that of the base
/// layer.
fn environment_map_light_clearcoat(
    out: ptr<function, EnvironmentMapLight>,
    input: ptr<function, LightingInput>,
    clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>,
    found_diffuse_indirect: bool,
) {
    // Unpack.
    let world_position = (*input).P;
    let clearcoat_NdotV = (*input).layers[LAYER_CLEARCOAT].NdotV;
    let clearcoat_strength = (*input).clearcoat_strength;

    // Calculate the Fresnel term `Fc` for the clearcoat layer.
    // 0.04 is a hardcoded value for F0 from the Filament spec.
    let clearcoat_F0 = vec3<f32>(0.04);
    let Fc = F_Schlick_vec(clearcoat_F0, 1.0, clearcoat_NdotV) * clearcoat_strength;
    let inv_Fc = 1.0 - Fc;

    let clearcoat_radiances = compute_radiances(
        (*input).layers[LAYER_CLEARCOAT],
        clusterable_object_index_ranges,
        world_position,
        found_diffuse_indirect,
    );

    // Composite the clearcoat layer on top of the existing one.
    // These formulas are from Filament:
    // <https://google.github.io/filament/Filament.md.html#lighting/imagebasedlights/clearcoat>
    (*out).diffuse *= inv_Fc;
    (*out).specular = (*out).specular * inv_Fc * inv_Fc + clearcoat_radiances.radiance * Fc;
}

/// Multiscattering approximation: https://www.jcgt.org/published/0008/01/03/paper.pdf
///
/// We initially used this (https://bruop.github.io/ibl) reference with Roughness Dependent
/// Fresnel, but it made fresnel very bright so we reverted to the "typical" fresnel term.
fn compute_multiscatter(
    F0: vec3<f32>,
    F_ab: vec2<f32>,
    Ems: f32,
    specular_occlusion: f32,
) -> MultiscatterResult {
    let FssEss = (F0 * F_ab.x + F_ab.y) * specular_occlusion;
    let Favg = F0 + (1.0 - F0) / 21.0;
    let FmsEms = FssEss * Favg / (1.0 - Ems * Favg) * Ems;
    let Edss = 1.0 - (FssEss + FmsEms);

    return MultiscatterResult(FssEss, FmsEms, Edss);
}

fn environment_map_light(
    input: ptr<function, LightingInput>,
    clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>,
    found_diffuse_indirect: bool,
) -> EnvironmentMapLight {
    // Unpack.
    let roughness = (*input).layers[LAYER_BASE].roughness;
    let diffuse_color = (*input).diffuse_color;
    let metallic = (*input).metallic;
    let NdotV = (*input).layers[LAYER_BASE].NdotV;
    let F_ab = (*input).F_ab;
    let F0_dielectric = (*input).F0_dielectric;
    let F0_metallic = (*input).F0_metallic;
    let world_position = (*input).P;

    var out: EnvironmentMapLight;

    let radiances = compute_radiances(
        (*input).layers[LAYER_BASE],
        clusterable_object_index_ranges,
        world_position,
        found_diffuse_indirect,
    );

    if (all(radiances.irradiance == vec3(0.0)) && all(radiances.radiance == vec3(0.0))) {
        out.diffuse = vec3(0.0);
        out.specular = vec3(0.0);
        return out;
    }

    // No real world material has specular values under 0.02, so we use this range as a
    // "pre-baked specular occlusion" that extinguishes the fresnel term, for artistic control.
    // See: https://google.github.io/filament/Filament.md.html#specularocclusion
    let F0_surface = mix(F0_dielectric, F0_metallic, metallic);
    let specular_occlusion = saturate(dot(F0_surface, vec3(50.0 * 0.33)));

    // Compute per-material (dielectric and metallic separately) then mix the results.
    // We can't use F0 directly as the multiscattering term is nonlinear.
    let Ems = 1.0 - (F_ab.x + F_ab.y);

    let ms_dielectric = compute_multiscatter(F0_dielectric, F_ab, Ems, specular_occlusion);
    let ms_metallic = compute_multiscatter(F0_metallic, F_ab, Ems, specular_occlusion);

    let FssEss = mix(ms_dielectric.FssEss, ms_metallic.FssEss, metallic);
    let FmsEms = mix(ms_dielectric.FmsEms, ms_metallic.FmsEms, metallic);
    let kD = diffuse_color * ms_dielectric.Edss;

    if (!found_diffuse_indirect) {
        out.diffuse = (FmsEms + kD) * radiances.irradiance;
    } else {
        out.diffuse = vec3(0.0);
    }

    out.specular = FssEss * radiances.radiance;

    @if(STANDARD_MATERIAL_CLEARCOAT)
    environment_map_light_clearcoat(
        &out,
        input,
        clusterable_object_index_ranges,
        found_diffuse_indirect,
    );

    return out;
}

/// "Moving Frostbite to Physically Based Rendering 3.0", listing 22
/// https://seblagarde.wordpress.com/wp-content/uploads/2015/07/course_notes_moving_frostbite_to_pbr_v32.pdf#page=70
fn radiance_sample_direction(N: vec3<f32>, R: vec3<f32>, roughness: f32) -> vec3<f32> {
    let smoothness = saturate(1.0 - roughness);
    let lerp_factor = smoothness * (sqrt(smoothness) + roughness);
    return mix(N, R, lerp_factor);
}
