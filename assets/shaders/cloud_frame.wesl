// What the clouds' march and its temporal resolve (cloud_volume.wesl, cloud_resolve.wesl) know of
// this frame and the last (D617). Field order must match `Frame` in sky/volume/render.rs.
struct Frame {
    // The view's turn and projection only, unjittered, from the picture's clip space to a way in
    // the world: a point at the eye's place but at infinity, so no 30 km translation costs f32
    // its precision.
    world_from_clip: mat4x4<f32>,
    // Last frame's, the other way: a way in the world to its clip space.
    clip_from_world_before: mat4x4<f32>,
    // xy the march's offset inside its pixel this frame, in pixels; z 1 when there is no history
    // to blend (the first frame, or a new picture size).
    jitter: vec4<f32>,
}
