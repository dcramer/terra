// The exposure the tonemapper applied to the frame, kept for materials drawn in the next
// (crates/terra_world/src/adaptation.rs): the view's colour grading exposure once Bevy's auto
// exposure has added its correction, and the camera's exposure.

import bevy_render::view::View;

@group(0) @binding(0) var<storage, read> view: View;
@group(0) @binding(1) var<storage, read_write> adapted: vec4<f32>;

@compute @workgroup_size(1)
fn main() {
    adapted = vec4(view.color_grading.exposure, view.exposure, 0.0, 0.0);
}
