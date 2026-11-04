// The clouds' temporal resolve (D617): blends the march's picture (cloud_volume.wesl), each frame
// marched at its own offset inside the pixel, into a history of its own, reprojected to where
// each way in the world was last frame, so a heap's edge is the average over its pixel rather
// than one ray's inside-or-out, and the main pass's TAA or upscaler sees a still picture.
//
// As shipped engines resolve their clouds before laying them over the scene: Schneider, "The
// real-time volumetric cloudscapes of Horizon Zero Dawn" (SIGGRAPH 2015) and "Nubis: authoring
// real-time volumetric cloudscapes" (2017), which march a part of the pixels a frame and
// reproject the rest from a history, the clouds taken as at infinity so a turn of the camera
// moves them and its walk does not; and Unreal's volumetric cloud, which traces at a fraction of
// the view with a sub-pixel offset a frame and reconstructs into its own history before the
// composite (its docs, "Volumetric Cloud Component": the volumetric render target,
// r.VolumetricRenderTarget and its modes; ported from the description, not the code). The
// history is clamped to the current picture's 3x3 neighbourhood (Karis, "High quality temporal
// supersampling", SIGGRAPH 2014), so what moves or is uncovered shows at once.

import super::cloud_frame::Frame;

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var current: texture_2d<f32>;
@group(0) @binding(2) var history: texture_2d<f32>;
@group(0) @binding(3) var linear: sampler;
@group(0) @binding(4) var next: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var shown: texture_storage_2d<rgba16float, write>;

// How much of the current picture a frame takes: Karis 2014's and Bevy's TAA's tenth, an average
// over some twenty frames, longer than the march's sixteen offsets.
const BLEND: f32 = 0.1;

@compute @workgroup_size(8, 8, 1)
fn resolve(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(current);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let now = textureLoad(current, id.xy, 0);
    var lo = now;
    var hi = now;
    for (var k = 0u; k < 9u; k++) {
        let at = clamp(vec2<i32>(id.xy) + vec2<i32>(i32(k % 3u) - 1, i32(k / 3u) - 1), vec2<i32>(0), vec2<i32>(size) - 1);
        let c = textureLoad(current, at, 0);
        lo = min(lo, c);
        hi = max(hi, c);
    }
    var out = now;
    if frame.jitter.z < 0.5 {
        // The way through this pixel's middle, and where it was in last frame's picture.
        let uv = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(size);
        let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
        let near = frame.world_from_clip * vec4<f32>(ndc, 1.0, 1.0);
        let before = frame.clip_from_world_before * vec4<f32>(near.xyz / near.w, 0.0);
        if before.w > 0.0 {
            let was = before.xy / before.w;
            let uv_before = vec2<f32>(was.x * 0.5 + 0.5, 0.5 - was.y * 0.5);
            if all(uv_before >= vec2<f32>(0.0)) && all(uv_before <= vec2<f32>(1.0)) {
                let kept = clamp(textureSampleLevel(history, linear, uv_before, 0.0), lo, hi);
                out = mix(kept, now, BLEND);
            }
        }
    }
    textureStore(next, id.xy, out);
    textureStore(shown, id.xy, out);
}
