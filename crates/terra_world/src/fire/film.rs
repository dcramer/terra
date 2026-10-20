//! The hearths' and torches' flames through a film's shoulder, the rest of the picture through
//! the camera's own (docs/fire.md, "The tonemapper"; D415). KhronosPbrNeutral mixes whatever
//! is past its white toward white in every channel alike, so a flame with no blue gained it
//! there and read salmon; the grade's `post_saturation` after it floored its blue at a fifth of
//! its brightness. Film takes each channel on its own: the red runs out first, then the green,
//! so a flame goes orange, yellow, then white, as night photographs of a campfire do (their
//! brightest tenth B/R 0.03 to 0.15). Frostbite and The Witcher 3 tonemap their fire apart so.
//!
//! The flames say where they are in the HDR picture's alpha (`fire.wesl`, `FIRE_MASK`): their
//! mask draw adds to it past 1, which nothing else of ours writes, and TAA, bloom and the
//! volumetric fog keep it to the tonemapper; what is drawn blended over a flame (its smoke)
//! takes its share back. Patched into Bevy's `tone_mapping` (D184), for the tonemapping pass
//! only: a material tonemapped in its own shader has its own alpha there.

use bevy::prelude::*;

/// Where Bevy keeps the library with the tonemapping.
const LIBRARY: &str = "embedded://bevy_core_pipeline/tonemapping.wesl";

/// Where Bevy's tonemapping starts, the colour graded.
const BEFORE: &str = "    // tone_mapping
@if(TONEMAP_METHOD_LINEAR) {";

/// The graded colour kept for the flames.
const BEFORE_KEPT: &str = "    let graded = color;
    // tone_mapping
@if(TONEMAP_METHOD_LINEAR) {";

/// Bevy's grade after its tonemapping.
const AFTER: &str = "    color = saturation(color, color_grading.post_saturation);";

/// The flames through the film, ungraded after it.
const AFTER_FLAMES: &str = "    var post_saturation = color_grading.post_saturation;
@if(!TONEMAP_IN_SHADER) {
    // A flame's share, from its mask past 1 in the alpha (crates/terra_world/src/fire/film.rs).
    let flame = clamp(in.a - 1.0, 0.0, 1.0);
    color = mix(color, terra_flame_film(graded), flame);
    post_saturation = mix(post_saturation, 1.0, flame);
}
    color = saturation(color, post_saturation);";

/// Bevy's tonemapping function, which the film is put before.
const ENTRY: &str = "fn tone_mapping(in: vec4<f32>, in_color_grading: ColorGrading) -> vec4<f32> {";

/// How much of each channel's light the other two catch, as a film's layers catch some of
/// each other's: what takes a flame's yellow to white once it is bright enough. At none its
/// cores were (255, 255, 27), at 0.02 (255, 255, 132), as the photographs' brightest tenth
/// (254, 233, 141); more and its middle loses its orange.
pub(super) const CROSSTALK: f32 = 0.02;

/// Where Bevy keeps its depth of field's shader.
const DOF: &str = "embedded://bevy_post_process/dof/dof.wesl";

/// Its passes' ends, each of which writes an alpha of 1 over the picture: a film's depth of
/// field (`cine`, a shot's aperture) wiped the flames' mask before the tonemapper, so a filmed
/// campfire went through the camera's curve and read milky pink (fire round 9; the stills,
/// which have no depth of field, kept it). Each pass keeps the alpha it was given at its pixel.
const DOF_ENDS: [(&str, &str); 4] = [
    (
        "    return gaussian_blur(color_texture_a, color_texture_sampler, in.position, coc, vec2(1.0, 0.0));",
        "    return vec4(gaussian_blur(color_texture_a, color_texture_sampler, in.position, coc, vec2(1.0, 0.0)).rgb, terra_kept_alpha(in.position));",
    ),
    (
        "    return gaussian_blur(color_texture_a, color_texture_sampler, in.position, coc, vec2(0.0, 1.0));",
        "    return vec4(gaussian_blur(color_texture_a, color_texture_sampler, in.position, coc, vec2(0.0, 1.0)).rgb, terra_kept_alpha(in.position));",
    ),
    (
        "    output.output_1 = mix(vertical, diagonal, 0.5);",
        "    output.output_1 = mix(vertical, diagonal, 0.5);
    output.output_0.a = terra_kept_alpha(in.position);
    output.output_1.a = output.output_0.a;",
    ),
    (
        "    return mix(output_0, output_1, 0.5);",
        "    return vec4(mix(output_0, output_1, 0.5).rgb, terra_kept_alpha(in.position));",
    ),
];

/// The alpha the pass was given at its pixel: in the bokeh's second pass its first's, kept.
const KEPT_ALPHA: &str = "
fn terra_kept_alpha(position: vec4<f32>) -> f32 {
    return textureLoad(color_texture_a, vec2<i32>(floor(position.xy)), 0).a;
}
";

pub(super) fn plugin(app: &mut App) {
    crate::shader_patch::patch_library(
        app,
        LIBRARY,
        patched,
        "Bevy's tonemapping was not found where the flames' film goes: fire is tonemapped as the rest, and its hottest goes salmon",
    );
    crate::shader_patch::patch_library(
        app,
        DOF,
        dof_patched,
        "Bevy's depth of field's ends were not found: a film's depth of field wipes the flames' mask, and its campfires go pink",
    );
}

/// The depth of field keeping the alpha it was given, if it has each end once.
fn dof_patched(source: &str) -> Option<String> {
    let mut source = source.replace("\r\n", "\n");
    for (from, to) in DOF_ENDS {
        source = crate::shader_patch::swap_once(&source, from, to)?;
    }
    Some(source + KEPT_ALPHA)
}

/// The film for a graded colour, as the tonemapper draws a flame.
#[cfg(test)]
pub(super) fn film(color: Vec3) -> Vec3 {
    let c = color.max(Vec3::ZERO);
    let caught = Vec3::new(c.y + c.z, c.x + c.z, c.x + c.y) * 0.5 * CROSSTALK;
    let x = c + caught;
    Vec3::new(
        crate::hable::hable(x.x),
        crate::hable::hable(x.y),
        crate::hable::hable(x.z),
    )
    .min(Vec3::ONE)
}

/// The film in WGSL, as `film`.
fn wgsl() -> String {
    let curve = crate::hable::wgsl("terra_flame_curve");
    format!(
        "{curve}

fn terra_flame_film(color: vec3<f32>) -> vec3<f32> {{
    let c = max(color, vec3(0.0));
    let caught = vec3(c.g + c.b, c.r + c.b, c.r + c.g) * {half:?};
    return min(terra_flame_curve(c + caught), vec3(1.0));
}}

{ENTRY}",
        half = 0.5 * CROSSTALK,
    )
}

/// The library with the flames through the film, if it has each line once.
fn patched(source: &str) -> Option<String> {
    let source = crate::shader_patch::swap_once(source, BEFORE, BEFORE_KEPT)?;
    let source = crate::shader_patch::swap_once(&source, AFTER, AFTER_FLAMES)?;
    crate::shader_patch::swap_once(&source, ENTRY, &wgsl())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// sRGB's encoding of a linear channel, 0 to 255.
    fn srgb(v: f32) -> f32 {
        let v = v.clamp(0.0, 1.0);
        255.0
            * if v <= 0.003_130_8 {
                12.92 * v
            } else {
                1.055 * libm::powf(v, 1.0 / 2.4) - 0.055
            }
    }

    /// KhronosPbrNeutral, as Bevy's tonemapping library has it.
    fn neutral(c: Vec3) -> Vec3 {
        let low = c.min_element();
        let offset = if low < 0.08 {
            low - 6.25 * low * low
        } else {
            0.04
        };
        let o = c - Vec3::splat(offset);
        let peak = o.max_element();
        if peak < 0.76 {
            return o;
        }
        let d = 0.24;
        let top = 1.0 - d * d / (peak + d - 0.76);
        let g = 1.0 - 1.0 / (0.15 * (peak - top) + 1.0);
        (o * (top / peak)).lerp(Vec3::splat(top), g)
    }

    /// A hearth's hot texel on the screen's scale, as the fire shader gives it (the salmon
    /// trace, ~/.cache/terra-review/fire/HANDOFF.md): the ramp's orange with no blue.
    const FLAME: Vec3 = Vec3::new(12.64, 6.46, 0.013);

    #[test]
    fn a_flame_goes_orange_then_yellow_then_white() {
        let shown = |k: f32| film(FLAME * k).map(srgb);
        // Neutral takes the hot texel pink-white: its blue past 0.6 of its red.
        let pink = neutral(FLAME).map(srgb);
        assert!(pink.z / pink.x > 0.6, "{pink}");
        // The film keeps it yellow, its blue under 0.6 of its red as the photographs' cores.
        let core = shown(1.0);
        assert!(core.z / core.x < 0.6 && core.y / core.x > 0.85, "{core}");
        // Dimmer, the green falls behind the red and the blue stays dark: orange.
        let dim = shown(0.03);
        assert!(dim.y / dim.x < 0.85 && dim.z / dim.x < 0.15, "{dim}");
        // Brighter, every channel rises: yellow on to white, never back to red.
        let steps: Vec<Vec3> = (1..40).map(|i| shown(0.02 * i as f32 * i as f32)).collect();
        for pair in steps.windows(2) {
            assert!(pair[1].cmpge(pair[0] - Vec3::splat(0.01)).all(), "{pair:?}");
            assert!(pair[1].y / pair[1].x >= pair[0].y / pair[0].x - 1e-3);
        }
    }

    #[test]
    fn the_film_goes_in_once_for_the_tonemapping_pass_only() {
        let library = format!(
            "fn saturation() {{}}\n{ENTRY}\n    var color = in.rgb;\n{BEFORE}\n}}\n{AFTER}\n    return vec4(color, in.a);\n}}\n"
        );
        let out = patched(&library).unwrap();
        assert!(out.contains("let graded = color;") && out.contains("terra_flame_film(graded)"));
        assert!(out.contains("@if(!TONEMAP_IN_SHADER)"));
        assert!(out.matches(ENTRY).count() == 1 && out.contains("fn terra_flame_curve("));
        // The film is defined before the function that calls it.
        assert!(out.find("fn terra_flame_film").unwrap() < out.find(ENTRY).unwrap());
        assert!(out.contains(&format!("{:?}", 0.5 * CROSSTALK)));
        assert!(patched("fn a() {}").is_none());
        assert!(patched(&format!("{library}{library}")).is_none());
    }

    #[test]
    fn the_depth_of_field_keeps_the_flames_mask_at_every_end() {
        let source: String = DOF_ENDS
            .iter()
            .map(|(from, _)| format!("{from}\n"))
            .collect();
        let out = dof_patched(&source).unwrap();
        // Every pass's end writes the alpha it was given, none Bevy's.
        assert_eq!(out.matches("terra_kept_alpha(in.position)").count(), 4);
        assert!(!out.contains("vec2(1.0, 0.0));\n") && !out.contains("output_1, 0.5);\n"));
        assert!(out.contains("fn terra_kept_alpha("));
        assert!(dof_patched(&source[1..]).is_none());
    }
}
