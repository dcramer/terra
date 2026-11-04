// Terra's patch (D616): temporal upscaling. The main pass was drawn in the top-left
// `upscale.render_size` pixels of its textures (`MainPassResolutionOverride`), jittered; this
// pass accumulates it into a history at the view's full size, as AMD's FidelityFX Super
// Resolution 2 does, in one pass: each shown pixel takes the drawn samples near it, weighted
// by a Lanczos kernel narrowed by the scale (FSR 2's `ComputeUpsampledColorAndWeight`), and
// blends them into its reprojected history by that weight, the history first clamped to the
// samples' box (`RectifyHistory`, `ComputeBaseAccumulationWeight`, `Accumulate`). The kernel
// reads the 3x3 drawn pixels round the one the shown pixel falls in, as FSR 2.2 does: it loads
// a 4x4 block, [-2, 1] or [-1, 2] by the jitter's side, but flips it so its weights and box
// take only offsets -1 to 1 (`bFlipRow`, `bFlipCol`). Left out
// for now: FSR 2's depth-clip pass, locks, luma instability, reactive masks and the
// accumulation cut a clamped history takes without the last two (`RectifyHistory`); the motion
// vector is dilated to the nearest depth of the 3x3 drawn pixels, as FSR 2's reconstruct pass
// does.
//
// Ported from FidelityFX Super Resolution 2.2, ffx_fsr2_upsample.h, ffx_fsr2_accumulate.h and
// ffx_fsr2_common.h (https://github.com/GPUOpen-Effects/FidelityFX-FSR2), under this notice:
//
// Copyright (c) 2022-2023 Advanced Micro Devices, Inc. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

@group(0) @binding(0) var view_target: texture_2d<f32>;
@group(0) @binding(1) var history: texture_2d<f32>;
@group(0) @binding(2) var motion_vectors: texture_2d<f32>;
@group(0) @binding(3) var depth: texture_depth_2d;
@group(0) @binding(4) var nearest_sampler: sampler;
@group(0) @binding(5) var linear_sampler: sampler;

struct Upscale {
    // The size the main pass was drawn at, in pixels.
    render_size: vec2<f32>,
    // This frame's jitter (`TemporalJitter::offset`), in drawn pixels.
    jitter: vec2<f32>,
};
@group(0) @binding(6) var<uniform> upscale: Upscale;

struct Output {
    @location(0) view_target: vec4<f32>,
    @location(1) history: vec4<f32>,
};

const EPSILON: f32 = 1e-3;
// ffx_fsr2_common.h
const UPSAMPLE_LANCZOS_WEIGHT_SCALE: f32 = 1.0 / 12.0;
const MAX_ACCUMULATION_LANCZOS_WEIGHT: f32 = 1.0;

// As Bevy's taa.wesl: the reversible tonemap the history is kept in (Karis 2014; AMD's
// "optimized reversible tonemapper for resolve") and Playdead's YCoCg (MIT).
fn max3(x: vec3<f32>) -> f32 { return max(x.r, max(x.g, x.b)); }
fn tonemap(color: vec3<f32>) -> vec3<f32> { return color / (max3(color) + 1.0); }
fn reverse_tonemap(color: vec3<f32>) -> vec3<f32> { return color / max(1.0 - max3(color), 1e-4); }

fn rgb_to_ycocg(rgb: vec3<f32>) -> vec3<f32> {
    let y = (rgb.r / 4.0) + (rgb.g / 2.0) + (rgb.b / 4.0);
    let co = (rgb.r / 2.0) - (rgb.b / 2.0);
    let cg = (-rgb.r / 4.0) + (rgb.g / 2.0) - (rgb.b / 4.0);
    return vec3(y, co, cg);
}

fn ycocg_to_rgb(ycocg: vec3<f32>) -> vec3<f32> {
    let r = ycocg.x + ycocg.y - ycocg.z;
    let g = ycocg.x + ycocg.z;
    let b = ycocg.x - ycocg.y - ycocg.z;
    return saturate(vec3(r, g, b));
}

// A drawn pixel's colour as the accumulation works in it: tonemapped (when the view is HDR),
// in YCoCg (FSR 2's prepared input colour).
fn prepared(pixel: vec2<i32>) -> vec3<f32> {
    var color = textureLoad(view_target, pixel, 0).rgb;
@if(TONEMAP)
    color = tonemap(color);
    return rgb_to_ycocg(color);
}

// ffx_fsr2_sample.h: FSR 1's approximation of Lanczos 2 from the squared distance.
fn lanczos2_approx_sq(x2_in: f32) -> f32 {
    let x2 = min(x2_in, 4.0);
    let a = (2.0 / 5.0) * x2 - 1.0;
    let b = (1.0 / 4.0) * x2 - 1.0;
    return ((25.0 / 16.0) * a * a - (25.0 / 16.0 - 1.0)) * (b * b);
}

// The history at `uv` with five taps of Catmull-Rom, as Bevy's taa.wesl samples it (Jimenez,
// "Filmic SMAA", 2016; MJP's gist).
fn sample_history(uv: vec2<f32>, texture_size: vec2<f32>) -> vec3<f32> {
    let texel_size = 1.0 / texture_size;
    let sample_position = uv * texture_size;
    let texel_center = floor(sample_position - 0.5) + 0.5;
    let f = sample_position - texel_center;
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let p0 = (texel_center - 1.0) * texel_size;
    let p3 = (texel_center + 2.0) * texel_size;
    let p12 = (texel_center + (w2 / w12)) * texel_size;
    var color = textureSampleLevel(history, linear_sampler, vec2(p12.x, p0.y), 0.0).rgb * w12.x * w0.y;
    color += textureSampleLevel(history, linear_sampler, vec2(p0.x, p12.y), 0.0).rgb * w0.x * w12.y;
    color += textureSampleLevel(history, linear_sampler, vec2(p12.x, p12.y), 0.0).rgb * w12.x * w12.y;
    color += textureSampleLevel(history, linear_sampler, vec2(p3.x, p12.y), 0.0).rgb * w3.x * w12.y;
    color += textureSampleLevel(history, linear_sampler, vec2(p12.x, p3.y), 0.0).rgb * w12.x * w3.y;
    return max(color, vec3(0.0));
}

@fragment
fn taa(@location(0) uv: vec2<f32>) -> Output {
    let display_size = vec2<f32>(textureDimensions(history));
    let render_size = upscale.render_size;
    let last = vec2<i32>(render_size) - 1;
    // The shown pixel's middle in drawn pixels (FSR 2's fSrcOutputPos), and the drawn pixel it
    // falls in.
    let source = uv * render_size;
    let centre = clamp(vec2<i32>(floor(source)), vec2(0), last);

    // The motion of the nearest surface among the 3x3 drawn pixels (reversed depth: larger is
    // nearer), so an edge's history moves with what is in front.
    var nearest = centre;
    var nearest_depth = textureLoad(depth, centre, 0);
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let at = clamp(centre + vec2(x, y), vec2(0), last);
            let d = textureLoad(depth, at, 0);
            if d > nearest_depth {
                nearest_depth = d;
                nearest = at;
            }
        }
    }
    let motion = textureLoad(motion_vectors, nearest, 0).rg;
    let history_uv = uv - motion;
    let velocity = length(motion * display_size);
    var new_sample = any(saturate(history_uv) != history_uv);
@if(RESET)
    new_sample = true;

    // ffx_fsr2_upsample.h, ComputeUpsampledColorAndWeight. Bevy's `TemporalJitter` moves what
    // is drawn by minus its offset, so the drawn pixel at `i + 0.5` shows what stands at
    // `i + 0.5 + jitter` unjittered (FSR 2's projection moves it the other way, so it
    // subtracts its jitter here).
    let base_offset = vec2<f32>(centre) + 0.5 + upscale.jitter - source;
    let kernel_reactive = select(0.0, 1.0, new_sample);
    let kernel_bias_max = min(1.99, display_size.x / render_size.x) * (1.0 - kernel_reactive);
    let kernel_bias_min = max(1.0, (1.0 + kernel_bias_max) * 0.3);
    let kernel_bias = mix(kernel_bias_max, kernel_bias_min, kernel_reactive);
    let curve = mix(-2.0, -3.0, saturate(velocity / 50.0));

    var color_sum = vec3(0.0);
    var weight_sum = 0.0;
    var box_sum = vec3(0.0);
    var box_square = vec3(0.0);
    var box_weight = 0.0;
    var box_min = vec3(1.0e9);
    var box_max = vec3(-1.0e9);
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let at = centre + vec2(x, y);
            let on_screen = all(at >= vec2(0)) && all(at <= last);
            let color = prepared(clamp(at, vec2(0), last));
            let offset = base_offset + vec2(f32(x), f32(y));
            let biased = offset * kernel_bias;
            let weight = select(0.0, lanczos2_approx_sq(dot(biased, biased)), on_screen);
            color_sum += color * weight;
            weight_sum += weight;
            // The rectification box: FSR 2's RectificationBoxAddSample.
            let w = exp(curve * dot(offset, offset));
            box_sum += color * w;
            box_square += color * color * w;
            box_weight += w;
            box_min = min(box_min, color);
            box_max = max(box_max, color);
        }
    }
    box_weight = select(1.0, box_weight, abs(box_weight) > EPSILON);
    let box_centre = box_sum / box_weight;
    let box_vec = sqrt(abs(box_square / box_weight - box_centre * box_centre));
    var upsampled = box_centre;
    var upsampled_weight = 0.0;
    if weight_sum > EPSILON {
        // Deringing: Lanczos's negative lobes kept within the samples.
        upsampled = clamp(color_sum / weight_sum, box_min, box_max);
        upsampled_weight = weight_sum * UPSAMPLE_LANCZOS_WEIGHT_SCALE;
    }

    var color = upsampled;
    let moving = select(0.0, 1.0, saturate(velocity * 10.0) >= 1.0);
    if !new_sample {
        var history_color = rgb_to_ycocg(sample_history(history_uv, display_size));
        let in_motion_last_frame = textureSampleLevel(history, nearest_sampler, history_uv, 0.0).a > 0.5;

        // ffx_fsr2_accumulate.h, ComputeBaseAccumulationWeight (no reactive mask, no depth clip).
        var accumulation = MAX_ACCUMULATION_LANCZOS_WEIGHT;
        let in_motion = max(select(0.0, 1.0, in_motion_last_frame), saturate(velocity * 10.0));
        accumulation = min(accumulation, mix(accumulation, upsampled_weight * 10.0, in_motion));
        accumulation = min(accumulation, mix(accumulation, upsampled_weight, saturate(velocity / 20.0)));

        // RectifyHistory: the box widened by the scale while still, the samples' own spread
        // when moving, within the samples' extremes. FSR 2 also cuts the accumulation to 0.1
        // where it clamps, unless a lock or its luma instability (a pixel flickering frame to
        // frame) keeps the history; without those two, the cut let one frame's dither through
        // wherever a drawn frame's noise left the box: contact shadows' speckle, on about a
        // tenth of the drawn pixels and rarely in the same place twice, stood as dark dots on
        // the lake's rock (play-3080 j6). So the history is clamped, as Bevy's native TAA
        // clamps it, and keeps its weight; motion still lowers it (above).
        let scale = render_size / display_size;
        let influence = min(20.0, pow(1.0 / (scale.x * scale.y), 3.0));
        let box_scale = mix(influence, 1.0, saturate(velocity / 20.0));
        let rect_min = max(box_min, box_centre - box_vec * box_scale);
        let rect_max = min(box_max, box_centre + box_vec * box_scale);
        history_color = clamp(history_color, rect_min, rect_max);

        // Accumulate.
        accumulation = max(EPSILON, accumulation + upsampled_weight);
        color = mix(history_color, upsampled, upsampled_weight / accumulation);
    }

    var rgb = ycocg_to_rgb(color);
    var out: Output;
    out.history = vec4(rgb, moving);
@if(TONEMAP)
    rgb = reverse_tonemap(rgb);
    out.view_target = vec4(rgb, textureLoad(view_target, centre, 0).a);
    return out;
}
