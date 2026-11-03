import package::{
    atmosphere::{
        bindings::settings,
        functions::{
            sample_density_lut, sample_local_inscattering, uv_to_ray_direction, get_view_position,
            MIDPOINT_RATIO, MIN_EXTINCTION, ABSORPTION_DENSITY, SCATTERING_DENSITY,
        },
    }
};


@group(0) @binding(13) var aerial_view_lut_out: texture_storage_3d<rgba16float, write>;

@compute
@workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) idx: vec3<u32>) {
    if any(idx.xy > settings.aerial_view_lut_size.xy) { return; }

    let uv = (vec2<f32>(idx.xy) + 0.5) / vec2<f32>(settings.aerial_view_lut_size.xy);
    let ray_dir = uv_to_ray_direction(uv);
    let world_pos = get_view_position();

    let r = length(world_pos);
    let t_max = settings.aerial_view_lut_max_distance;

    var prev_t = 0.0;
    var total_inscattering = vec3(0.0);
    var throughput = vec3(1.0);

    for (var slice_i: u32 = 0; slice_i < settings.aerial_view_lut_size.z; slice_i++) {
        for (var step_i: u32 = 0; step_i < settings.aerial_view_lut_samples; step_i++) {
            let t_i = t_max * (f32(slice_i) + ((f32(step_i) + MIDPOINT_RATIO) / f32(settings.aerial_view_lut_samples))) / f32(settings.aerial_view_lut_size.z);
            let dt = (t_i - prev_t);
            prev_t = t_i;

            let sample_pos = world_pos + ray_dir * t_i;
            let local_r = length(sample_pos);
            let local_up = normalize(sample_pos);

            let absorption = sample_density_lut(local_r, ABSORPTION_DENSITY);
            let scattering = sample_density_lut(local_r, SCATTERING_DENSITY);
            let extinction = absorption + scattering;

            let sample_optical_depth = extinction * dt;
            let sample_transmittance = exp(-sample_optical_depth);

            // evaluate one segment of the integral
            var inscattering = sample_local_inscattering(scattering, ray_dir, sample_pos);

            // Analytical integration of the single scattering term in the radiance transfer equation
            let s_int = (inscattering - inscattering * sample_transmittance) / max(extinction, MIN_EXTINCTION);
            total_inscattering += throughput * s_int;

            throughput *= sample_transmittance;
            if all(throughput < vec3(0.001)) {
                break;
            }
        }

        // Store in log space to allow linear interpolation of exponential values between slices
        let log_inscattering = log(max(total_inscattering, vec3(1e-6)));
        textureStore(aerial_view_lut_out, vec3(vec2<u32>(idx.xy), slice_i), vec4(log_inscattering, 0.0));
    }
}

// Terra's patch (D615): `main`'s table with each column's work spread over a workgroup. `main`
// runs a thread a column (32x32 threads, four workgroups on a card of a hundred multiprocessors),
// each marching slices x samples in a row, latency-bound on the five table reads of every sample.
// Here a workgroup is a column. Its threads take SPREAD_SAMPLES samples at a time and work out
// each as `main` does with the step from the sample before (its extinction, transmittance,
// in-scattering and analytic segment, none of which depend on the light let through before it),
// into workgroup memory; then one thread runs `main`'s accumulation over them in order (the
// throughput, the sum and the stop once the throughput is all under 0.001, to the slice's end).
// A slice's first sample after a stop steps from the stopped sample, not the one before it, as
// in `main`, so that thread works its transmittance and segment out again from its step.
const SPREAD_THREADS: u32 = 64u;
const SPREAD_SAMPLES: u32 = 160u;
var<workgroup> spread_extinction: array<vec3<f32>, SPREAD_SAMPLES>;
var<workgroup> spread_inscattering: array<vec3<f32>, SPREAD_SAMPLES>;
var<workgroup> spread_transmittance: array<vec3<f32>, SPREAD_SAMPLES>;
var<workgroup> spread_segment: array<vec3<f32>, SPREAD_SAMPLES>;

@compute
@workgroup_size(64, 1, 1)
fn main_spread(
    @builtin(workgroup_id) column: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    if any(column.xy > settings.aerial_view_lut_size.xy) { return; }

    let uv = (vec2<f32>(column.xy) + 0.5) / vec2<f32>(settings.aerial_view_lut_size.xy);
    let ray_dir = uv_to_ray_direction(uv);
    let world_pos = get_view_position();

    let t_max = settings.aerial_view_lut_max_distance;
    let samples = settings.aerial_view_lut_samples;
    let total = settings.aerial_view_lut_size.z * samples;

    // The accumulation's state, the first thread's alone.
    var total_inscattering = vec3(0.0);
    var throughput = vec3(1.0);
    var slice_i: u32 = 0u;
    var step_i: u32 = 0u;
    var stopped = false;
    // Where the last slice stopped, if it did: the next slice's first sample steps from there.
    var stopped_t = 0.0;
    var resumed = false;

    for (var base: u32 = 0u; base < total; base += SPREAD_SAMPLES) {
        let count = min(SPREAD_SAMPLES, total - base);
        for (var k = lane; k < count; k += SPREAD_THREADS) {
            let sample_i = base + k;
            let sample_slice = sample_i / samples;
            let sample_step = sample_i % samples;
            let t_i = t_max * (f32(sample_slice) + ((f32(sample_step) + MIDPOINT_RATIO) / f32(settings.aerial_view_lut_samples))) / f32(settings.aerial_view_lut_size.z);
            var prev_t = 0.0;
            if sample_i > 0u {
                let prev_slice = (sample_i - 1u) / samples;
                let prev_step = (sample_i - 1u) % samples;
                prev_t = t_max * (f32(prev_slice) + ((f32(prev_step) + MIDPOINT_RATIO) / f32(settings.aerial_view_lut_samples))) / f32(settings.aerial_view_lut_size.z);
            }
            let dt = (t_i - prev_t);

            let sample_pos = world_pos + ray_dir * t_i;
            let local_r = length(sample_pos);

            let absorption = sample_density_lut(local_r, ABSORPTION_DENSITY);
            let scattering = sample_density_lut(local_r, SCATTERING_DENSITY);
            let extinction = absorption + scattering;

            let sample_optical_depth = extinction * dt;
            let sample_transmittance = exp(-sample_optical_depth);

            var inscattering = sample_local_inscattering(scattering, ray_dir, sample_pos);

            let s_int = (inscattering - inscattering * sample_transmittance) / max(extinction, MIN_EXTINCTION);

            spread_extinction[k] = extinction;
            spread_inscattering[k] = inscattering;
            spread_transmittance[k] = sample_transmittance;
            spread_segment[k] = s_int;
        }
        workgroupBarrier();

        if lane == 0u {
            for (var k: u32 = 0u; k < count; k++) {
                if !stopped {
                    var sample_transmittance = spread_transmittance[k];
                    var s_int = spread_segment[k];
                    if resumed {
                        let t_i = t_max * (f32(slice_i) + ((f32(step_i) + MIDPOINT_RATIO) / f32(settings.aerial_view_lut_samples))) / f32(settings.aerial_view_lut_size.z);
                        let dt = (t_i - stopped_t);
                        let extinction = spread_extinction[k];
                        let sample_optical_depth = extinction * dt;
                        sample_transmittance = exp(-sample_optical_depth);
                        let inscattering = spread_inscattering[k];
                        s_int = (inscattering - inscattering * sample_transmittance) / max(extinction, MIN_EXTINCTION);
                        resumed = false;
                    }
                    total_inscattering += throughput * s_int;

                    throughput *= sample_transmittance;
                    if all(throughput < vec3(0.001)) {
                        stopped = true;
                        stopped_t = t_max * (f32(slice_i) + ((f32(step_i) + MIDPOINT_RATIO) / f32(settings.aerial_view_lut_samples))) / f32(settings.aerial_view_lut_size.z);
                    }
                }

                step_i++;
                if step_i == samples {
                    let log_inscattering = log(max(total_inscattering, vec3(1e-6)));
                    textureStore(aerial_view_lut_out, vec3(vec2<u32>(column.xy), slice_i), vec4(log_inscattering, 0.0));
                    step_i = 0u;
                    slice_i++;
                    resumed = stopped;
                    stopped = false;
                }
            }
        }
        workgroupBarrier();
    }
}
