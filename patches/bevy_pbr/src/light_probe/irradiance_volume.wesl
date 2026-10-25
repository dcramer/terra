import package::light_probe::{light_probe_iterator_new, light_probe_iterator_next};
import package::render::mesh_view_bindings::{
    irradiance_volumes,
    irradiance_volume,
    irradiance_volume_sampler,
    light_probes,
};
import package::render::clustered_forward::ClusterableObjectIndexRanges;

@if(LIGHTMAP) import package::render::mesh_view_types::LIGHT_PROBE_FLAG_AFFECTS_LIGHTMAPPED_MESH_DIFFUSE;

@if(IRRADIANCE_VOLUMES_ARE_USABLE)

/// See:
/// https://advances.realtimerendering.com/s2006/Mitchell-ShadingInValvesSourceEngine.pdf
/// Slide 28, "Ambient Cube Basis"
fn irradiance_volume_light(
    world_position: vec3<f32>,
    N: vec3<f32>,
    clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>,
) -> vec3<f32> {
    // Find all irradiance volumes that contain the fragment. We're going to
    // accumulate all the irradiance from them in a weighted sum.
    var iterator = light_probe_iterator_new(
        world_position,
        /*is_irradiance_volume=*/ true,
        clusterable_object_index_ranges,
    );

    var total_weight = 0.0;
    var total_irradiance = vec3(0.0);

    while (true) {
        var query_result = light_probe_iterator_next(&iterator);

        // If there was no irradiance volume found, we're done.
        if (query_result.texture_index < 0) {
            break;
        }

        // If we're lightmapped, and the irradiance volume contributes no diffuse
        // light, then bail out.
        @if(LIGHTMAP)
        if ((query_result.flags & LIGHT_PROBE_FLAG_AFFECTS_LIGHTMAPPED_MESH_DIFFUSE) == 0u) {
            continue;
        }

        @if(MULTIPLE_LIGHT_PROBES_IN_ARRAY)
        let irradiance_volume_texture = irradiance_volumes[query_result.texture_index];
        @else
        let irradiance_volume_texture = irradiance_volume;

        let atlas_resolution = vec3<f32>(textureDimensions(irradiance_volume_texture));
        let resolution = vec3<f32>(textureDimensions(irradiance_volume_texture) / vec3(1u, 2u, 3u));

        // Make sure to clamp to the edges to avoid texture bleed.
        var unit_pos = (query_result.light_from_world * vec4(world_position, 1.0f)).xyz;
        let stp = clamp((unit_pos + 0.5) * resolution, vec3(0.5f), resolution - vec3(0.5f));
        let uvw = stp / atlas_resolution;

        // The bottom half of each cube slice is the negative part, so choose it if applicable on each
        // slice.
        let neg_offset = select(vec3(0.0f), vec3(0.5f), N < vec3(0.0f));

        let uvw_x = uvw + vec3(0.0f, neg_offset.x, 0.0f);
        let uvw_y = uvw + vec3(0.0f, neg_offset.y, 1.0f / 3.0f);
        let uvw_z = uvw + vec3(0.0f, neg_offset.z, 2.0f / 3.0f);

        let rgb_x = textureSampleLevel(irradiance_volume_texture, irradiance_volume_sampler, uvw_x, 0.0).rgb;
        let rgb_y = textureSampleLevel(irradiance_volume_texture, irradiance_volume_sampler, uvw_y, 0.0).rgb;
        let rgb_z = textureSampleLevel(irradiance_volume_texture, irradiance_volume_sampler, uvw_z, 0.0).rgb;

        // Use Valve's formula to sample.
        let NN = N * N;
        total_irradiance += (rgb_x * NN.x + rgb_y * NN.y + rgb_z * NN.z) * query_result.intensity *
            query_result.weight;

        total_weight += query_result.weight;
    }

    if (total_weight != 0.0) {
        total_irradiance /= total_weight;
    }

    return total_irradiance;
}

// Terra's patch (D485): a reflection probe smaller than every irradiance volume holding a
// fragment takes its diffuse light from those volumes. Bevy 0.20 lets any irradiance volume
// win over every reflection probe's diffuse, so a sky volume over a forest lit the closed
// rooms of a dungeon dug under it, whatever their own probes said. Shipped engines let the
// smaller, more local probe win: Unreal's reflection captures (smaller captures override
// larger ones, "Reflection Environment", Unreal Engine documentation) and Godot 4's
// ReflectionProbe, whose ambient (`ambient_mode`) replaces the scene's inside its box
// (godotengine/godot, MIT). A probe is a unit cube in its own space, so how small it is
// is the determinant of its world-to-probe transform. Outside every reflection probe, or
// inside a volume smaller than the probe, nothing changes.

/// How much smaller than a unit cube a light probe is, from its world-to-probe transform:
/// larger for a smaller probe (Terra's patch, D485).
@if(IRRADIANCE_VOLUMES_ARE_USABLE)
fn light_probe_density(light_from_world: mat4x4<f32>) -> f32 {
    return abs(determinant(mat3x3<f32>(
        light_from_world[0].xyz,
        light_from_world[1].xyz,
        light_from_world[2].xyz,
    )));
}

/// Whether a reflection probe holding the fragment is smaller than every irradiance volume
/// holding it, so that probe's diffuse light is the fragment's (Terra's patch, D485).
@if(IRRADIANCE_VOLUMES_ARE_USABLE)
fn smaller_reflection_probe_holds(
    world_position: vec3<f32>,
    clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>,
) -> bool {
    var probes = light_probe_iterator_new(
        world_position,
        /*is_irradiance_volume=*/ false,
        clusterable_object_index_ranges,
    );
    var smallest_probe = 0.0;
    while (true) {
        let found = light_probe_iterator_next(&probes);
        if (found.texture_index < 0) {
            break;
        }
        smallest_probe = max(smallest_probe, light_probe_density(found.light_from_world));
    }
    if (smallest_probe == 0.0) {
        return false;
    }

    var volumes = light_probe_iterator_new(
        world_position,
        /*is_irradiance_volume=*/ true,
        clusterable_object_index_ranges,
    );
    while (true) {
        let found = light_probe_iterator_next(&volumes);
        if (found.texture_index < 0) {
            break;
        }
        if (light_probe_density(found.light_from_world) >= smallest_probe) {
            return false;
        }
    }
    return true;
}
