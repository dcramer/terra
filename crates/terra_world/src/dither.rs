//! Crossfades between levels of detail that temporal anti-aliasing can resolve, for
//! everything Bevy draws with its standard material: buildings' sections, rocks, scans and
//! props (D214, D240). Bevy dithers a crossfade (`VisibilityRange`) in the same 4×4 pattern
//! every frame, which TAA cannot blend and shows as a crawling screen door. Its standard
//! shaders' dither, in `bevy_pbr/render/pbr_functions.wesl` (MIT OR Apache-2.0, by the Bevy
//! contributors), is given Jimenez's interleaved gradient noise shifted every frame
//! instead (Call of Duty: Advanced Warfare), as Unreal dithers its crossfades and as
//! `shaders/foliage_fragment.wesl` already does: the main pass, the prepass and the
//! shadows all use that one function, so they agree.
//!
//! The library is patched where it is loaded, as an asset: the two lines that pick the
//! threshold from Bevy's fixed pattern are swapped for the moving one. If Bevy's source no
//! longer has them, the patch is not made and says so, and the fixed pattern stays.

use bevy::prelude::*;

/// Where Bevy keeps the library with the dither.
const LIBRARY: &str = "embedded://bevy_pbr/render/pbr_functions.wesl";

/// Bevy's threshold, from its fixed 4×4 pattern.
const FIXED: &str = "    let coords = vec2<u32>(floor(frag_coord.xy)) % 4u;
    let threshold = i32((DITHER_THRESHOLD_MAP[coords.y] >> (coords.x * 8)) & 0xff);";

/// The threshold from interleaved gradient noise, shifted every frame, 0 to 15.
const MOVING: &str = "    let shifted = frag_coord.xy + 5.588238 * f32(dither_frame() % 64u);
    let threshold = i32(fract(52.9829189 * fract(dot(shifted, vec2<f32>(0.06711056, 0.00583715)))) * 16.0);";

/// The frame's number, from the view's globals: the prepass and shadows have them at
/// another binding from the main pass's.
const FRAME: &str = "
@if(PREPASS_PIPELINE) @group(0) @binding(1) var<uniform> dither_globals: bevy_render::globals::Globals;
@if(PREPASS_PIPELINE) fn dither_frame() -> u32 { return dither_globals.frame_count; }
@if(!PREPASS_PIPELINE) fn dither_frame() -> u32 { return globals.frame_count; }
";

pub fn plugin(app: &mut App) {
    super::shader_patch::patch_library(
        app,
        LIBRARY,
        patched,
        "Bevy's crossfade dither was not found: crossfades keep its fixed pattern, which TAA cannot resolve",
    );
}

/// The library with the moving threshold in place of the fixed one, if it has it.
fn patched(source: &str) -> Option<String> {
    super::shader_patch::swap_once(source, FIXED, MOVING).map(|source| source + FRAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_fixed_threshold_is_swapped() {
        let library = format!("fn a() {{}}\nfn dither() {{\n{FIXED}\n    use(threshold);\n}}\n");
        let out = patched(&library).unwrap();
        assert!(out.contains("dither_frame()") && !out.contains("DITHER_THRESHOLD_MAP"));
        assert!(out.starts_with("fn a() {}\nfn dither() {\n"));
        assert!(out.contains("    use(threshold);\n}\n") && out.ends_with(FRAME));
        // Not there, or there twice: left alone.
        assert!(patched("fn a() {}").is_none());
        assert!(patched(&format!("{library}{library}")).is_none());
    }
}
