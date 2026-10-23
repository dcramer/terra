//! The hearths' and torches' flames through a film's shoulder, the rest of the picture through
//! the camera's own (docs/fire.md, "The tonemapper"; D415). KhronosPbrNeutral mixes whatever
//! is past its white toward white in every channel alike, so a flame with no blue gained it
//! there and read salmon; the grade's `post_saturation` after it floored its blue at a fifth of
//! its brightness. Film takes each channel on its own: the red runs out first, then the green,
//! so a flame goes orange, yellow, then white, as night photographs of a campfire do (their
//! brightest tenth B/R 0.03 to 0.15). Frostbite and The Witcher 3 tonemap their fire apart so.
//!
//! The flames say where they are in the HDR picture's alpha (`fire.wesl`, `FIRE_MASK`), as the
//! glowing parts of what burns do for their glow's share (`glow`): their
//! mask draw adds to it past 1, which nothing else of ours writes, and TAA, bloom and the
//! volumetric fog keep it to the tonemapper; what is drawn blended over a flame (its smoke)
//! takes its share back. Patched into Bevy's `tone_mapping` (D184), for the tonemapping pass
//! only: a material tonemapped in its own shader has its own alpha there.

use bevy::prelude::*;

/// Where Bevy keeps the library with the tonemapping.
const LIBRARY: &str = "embedded://bevy_core_pipeline/tonemapping.wesl";

/// Where Bevy's sectional grading starts, the colour white-balanced.
const BEFORE: &str = "@if(SECTIONAL_COLOR_GRADING) {
    color = sectional_color_grading(color, &color_grading);";

/// The colour kept for the flames and glows before the sections' grade, exposed: the night's
/// grade fades the dim's colour, and a dull coal's glow is dim, so its red went pink (fire
/// round 31); a fire keeps its colour by night as an eye's cones see it (`daylight`).
const BEFORE_KEPT: &str = "    let graded = color * powsafe(vec3(2.0), color_grading.exposure);
@if(SECTIONAL_COLOR_GRADING) {
    color = sectional_color_grading(color, &color_grading);";

/// Where Bevy's tonemapping starts, the colour graded as the world's.
const WORLD: &str = "    // tone_mapping
@if(TONEMAP_METHOD_LINEAR) {";

/// The world's graded colour kept for the surfaces a breath lights.
const WORLD_KEPT: &str = "    let world = color;
    // tone_mapping
@if(TONEMAP_METHOD_LINEAR) {";

/// Bevy's grade after its tonemapping.
const AFTER: &str = "    color = saturation(color, color_grading.post_saturation);";

/// The flames through the film, ungraded after it.
pub(super) const AFTER_FLAMES: &str = "    var post_saturation = color_grading.post_saturation;
@if(!TONEMAP_IN_SHADER) {
    // A flame's or a glow's share, from its mask past 1 in the alpha
    // (crates/terra_world/src/fire/film.rs).
    let flame = clamp(in.a - 1.0, 0.0, 1.0);
    // A firelit surface's share, from its mask just under 1 (`LIT_SHARE`), and all of what
    // shows through a flame (its mask past 1): the camera's curve keeping its hue, every
    // channel scaled by its brightest one's shoulder. Taken through the camera's own, the
    // bright logs and coals seen through a thin flame read salmon (campfire roll round: the
    // pink pixels had 0.4 of the flame's share in the matte).
    let surface = select(0.0, clamp((1.0 - in.a) / LIT_SHARE, 0.0, 1.0), in.a > 1.0 - 2.0 * LIT_SHARE);
    let lit = select(surface, 1.0, flame > 0.0);
    color = mix(color, terra_lit_neutral(world), lit);
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

/// How far under 1 the alpha goes where a fire's light falls on a surface (`fire.wesl`'s
/// `lit_mask`, its one writer): a hearth's stones, logs and ground within 1.3 m of it
/// (453a6321) and the wall and ground a marched breath strikes (957c0c79). Its share, so
/// little that a flame's mask past 1 loses nothing by it, and only in a band just under 1, so
/// a sky's or anything's alpha of 0 is not counted firelit. KhronosPbrNeutral mixed their red
/// toward white past its shoulder and they read salmon (fire round 27, vol6's C).
pub(super) const LIT_SHARE: f32 = 0.05;

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

const LIT_SHARE: f32 = {lit:?};

// KhronosPbrNeutral (Bevy's `tonemapping_pbr_neutral`) with no desaturation toward white:
// the same below the shoulder; past it every channel is scaled by the brightest one's
// shoulder, so a firelit wall keeps its channels' ratio, its hue, however bright (the
// max-RGB, hue-preserving shoulder, as Frostbite's and Unreal's hue-keeping options).
fn terra_lit_neutral(color: vec3<f32>) -> vec3<f32> {{
    let start = 0.8 - 0.04;
    let m = min(color.r, min(color.g, color.b));
    let c = color - select(0.04, m - 6.25 * m * m, m < 0.08);
    let peak = max(c.r, max(c.g, c.b));
    let d = 1.0 - start;
    let top = 1.0 - d * d / (peak + d - start);
    return select(c, c * (top / peak), peak > start);
}}

fn terra_flame_film(color: vec3<f32>) -> vec3<f32> {{
    let c = max(color, vec3(0.0));
    let caught = vec3(c.g + c.b, c.r + c.b, c.r + c.g) * {half:?};
    return min(terra_flame_curve(c + caught), vec3(1.0));
}}

{ENTRY}",
        half = 0.5 * CROSSTALK,
        lit = LIT_SHARE,
    )
}

/// The library with the flames through the film, if it has each line once.
fn patched(source: &str) -> Option<String> {
    let source = crate::shader_patch::swap_once(source, BEFORE, BEFORE_KEPT)?;
    let source = crate::shader_patch::swap_once(&source, WORLD, WORLD_KEPT)?;
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

    /// HSV hue of an sRGB colour, degrees.
    fn hue(c: Vec3) -> f32 {
        let (hi, lo) = (c.max_element(), c.min_element());
        if hi - lo < 1e-6 {
            return 0.0;
        }
        let h = if hi == c.x {
            (c.y - c.z) / (hi - lo)
        } else if hi == c.y {
            2.0 + (c.z - c.x) / (hi - lo)
        } else {
            4.0 + (c.x - c.y) / (hi - lo)
        };
        (h * 60.0).rem_euclid(360.0)
    }

    #[test]
    fn a_dull_coal_keeps_its_orange_red_by_night() {
        // A dull coal's glow, linear: tools/props/coals.ts `glowColour(0.4)`, sRGB (107, 39, 3).
        let coal = Vec3::new(0.146, 0.0205, 0.0011);
        // The night's grade on the dim (daylight.rs: NIGHT_SATURATION 0.45 times
        // DIM_SATURATION 0.2) fades it to a pinkish grey, its blue near its green: why the
        // glow is kept out of it (`glow`, the mask).
        let luma = coal.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        let faded = (Vec3::splat(luma) + (coal - Vec3::splat(luma)) * 0.09).map(srgb);
        assert!(faded.z / faded.y > 0.9, "{faded}");
        // Through the film, ungraded, at every brightness a coal glows by night: orange-red,
        // its green past a quarter of its red and its blue under its green.
        for k in [0.3, 1.0, 3.0, 10.0] {
            let shown = film(coal * k).map(srgb);
            let h = hue(shown);
            assert!((5.0..35.0).contains(&h), "{k}: {shown} hue {h}");
            assert!(
                shown.y / shown.x > 0.25 && shown.z < shown.y,
                "{k}: {shown}"
            );
        }
    }

    /// `terra_lit_neutral`, the camera's curve with every channel scaled by the brightest
    /// one's shoulder.
    fn lit_neutral(c: Vec3) -> Vec3 {
        let low = c.min_element();
        let offset = if low < 0.08 {
            low - 6.25 * low * low
        } else {
            0.04
        };
        let o = c - Vec3::splat(offset);
        let peak = o.max_element();
        if peak <= 0.76 {
            return o;
        }
        let d = 0.24;
        o * ((1.0 - d * d / (peak + d - 0.76)) / peak)
    }

    /// Neutral's shoulder a channel at a time, the lit share's before the hue was kept.
    fn per_channel(c: Vec3) -> Vec3 {
        let low = c.min_element();
        let offset = if low < 0.08 {
            low - 6.25 * low * low
        } else {
            0.04
        };
        let d = 0.24;
        (c - Vec3::splat(offset)).map(|v| {
            if v > 0.76 {
                1.0 - d * d / (v + d - 0.76)
            } else {
                v
            }
        })
    }

    #[test]
    fn a_firelit_surface_keeps_its_hue_however_bright() {
        // The ground by a breath's strike before the tonemapper, from 1.6 to 5 times white
        // (~/.cache/terra-review/fire/breath/HANDOFF.md, round 10): orange.
        let strike = Vec3::new(5.19, 1.57, 0.10);
        for k in [0.3, 0.6, 1.0, 2.0] {
            let surface = strike * k;
            let low = surface.min_element();
            let offset = if low < 0.08 {
                low - 6.25 * low * low
            } else {
                0.04
            };
            let o = surface - Vec3::splat(offset);
            let lit = lit_neutral(surface);
            // The channels keep their ratio, in linear light, and none passes white.
            assert!((lit.y / lit.x - o.y / o.x).abs() < 1e-5, "{k}: {lit}");
            assert!((lit.z / lit.x - o.z / o.x).abs() < 1e-5, "{k}: {lit}");
            assert!(lit.max_element() <= 1.0, "{k}: {lit}");
            // On the screen orange, where a channel at a time ran the green out with the red
            // and the ground read lime-yellow (sRGB 251, 242, 129).
            let shown = lit.map(srgb);
            let yellow = per_channel(surface).map(srgb);
            assert!(
                hue(shown) < 35.0 && hue(yellow) > 40.0,
                "{k}: {shown} against {yellow}"
            );
        }
        // Brighter, never darker: the shoulder rises with the light.
        let steps: Vec<f32> = (1..60)
            .map(|i| lit_neutral(strike * 0.05 * i as f32).x)
            .collect();
        assert!(steps.windows(2).all(|p| p[1] >= p[0]), "{steps:?}");
    }

    #[test]
    fn a_firelit_surface_stays_orange_where_the_camera_mixes_it_toward_white() {
        // Before the tonemapper, red twice the shoulder, little green, almost no blue: a
        // campfire's stone face nearest its flames, and a wall's face by a breath's strike.
        for (what, surface) in [
            ("stone", Vec3::new(2.0, 0.42, 0.08)),
            ("wall", Vec3::new(2.0, 0.6, 0.1)),
        ] {
            let pale = neutral(surface).map(srgb);
            let lit = lit_neutral(surface).map(srgb);
            // The camera's mixes white in: its blue past 0.4 of its red.
            assert!(pale.z / pale.x > 0.4, "{what}: {pale}");
            // Through the hue-keeping shoulder it stays orange, as bright, its hue Neutral's
            // before the mix toward white.
            let h = hue(lit);
            assert!(
                lit.z / lit.x < 0.35 && lit.x >= pale.x - 1.0 && (15.0..40.0).contains(&h),
                "{what}: {lit} hue {h} against {pale}"
            );
        }
        // Under the shoulder, the camera's own curve, so the lit mask's edge never shows.
        for c in [
            Vec3::new(0.5, 0.3, 0.1),
            Vec3::new(0.05, 0.04, 0.03),
            Vec3::splat(0.7),
        ] {
            assert!(
                (lit_neutral(c) - neutral(c)).abs().max_element() < 1e-6,
                "{c}"
            );
        }
    }

    #[test]
    fn every_firelit_surface_writes_the_share_the_tonemapper_reads() {
        let shader = include_str!("../../../../assets/shaders/fire.wesl");
        // One writer, at the share the film reads.
        assert!(shader.contains(&format!("const LIT_SHARE: f32 = {LIT_SHARE:?};")));
        assert!(shader.contains("return -LIT_SHARE * saturate(lit);"));
        // A hearth's lit card and a marched breath both write through it: a stale copy of the
        // shader that drops either one's (b743f619 dropped the hearth's) fails here.
        let fragment = &shader[shader.rfind("fn fragment(").unwrap()..];
        let hearth = fragment.contains("out.color = vec4(0.0, 0.0, 0.0, lit_mask(lit));");
        let breath = fragment.contains("out.color = vec4(0.0, 0.0, 0.0, flame + lit_mask(lit));");
        assert!(hearth && breath, "hearth {hearth}, breath {breath}");
        // The hearth's card that carries it, drawn in its free card's place (j 1).
        assert!(
            shader.contains("out.life = vec4(0.0, heat, luck, 6.0);")
                && fragment.contains("in.uv_b.x > 5.5 && in.uv_b.x < 6.5")
        );
    }

    #[test]
    fn the_film_goes_in_once_for_the_tonemapping_pass_only() {
        let library = format!(
            "fn saturation() {{}}\n{ENTRY}\n    var color = in.rgb;\n{BEFORE}\n}}\n{WORLD}\n}}\n{AFTER}\n    return vec4(color, in.a);\n}}\n"
        );
        let out = patched(&library).unwrap();
        // The film takes the colour from before the sections' grade, exposed.
        assert!(
            out.contains("let graded = color * powsafe(")
                && out.contains("terra_flame_film(graded)")
        );
        assert!(out.find("let graded").unwrap() < out.find("sectional_color_grading(").unwrap());
        assert!(out.contains("@if(!TONEMAP_IN_SHADER)"));
        // A firelit surface takes the world's grade, then the shoulder a channel at a time.
        assert!(
            out.find("let world = color;").unwrap() > out.find("sectional_color_grading(").unwrap()
        );
        assert!(out.contains("terra_lit_neutral(world)") && out.contains("fn terra_lit_neutral("));
        // What shows through a flame keeps its hue too, under the flame's own film.
        assert!(out.contains("select(surface, 1.0, flame > 0.0)"));
        assert!(
            out.find("terra_lit_neutral(world), lit)").unwrap()
                < out.find("terra_flame_film(graded), flame)").unwrap()
        );
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
