//! The ground shader's far strata (`assets/shaders/ground.wesl`'s `far_strata`, mountains round
//! 29) on the CPU, with the shader's constants read from it, so its rule is tested without a
//! GPU: a wall kilometres off, whose coarse mesh measures gentler than the 50 degrees its tiers
//! were drawn past, still stands in tiers on most of its faces, and each standing tier's foot
//! is a ledge's shadow line at least a pixel of height wide however far; near, nothing changes.

use crate::far_air::constant;
use terra_gen::smoothstep;

/// The shader's constants that `far_strata` reads.
struct Strata {
    from_m: [f32; 2],
    flat: [f32; 2],
    banded: f32,
    line: f32,
    line_px: f32,
    gain: f32,
    bench: f32,
    least: f32,
}

/// The share of the light the eye gets from a far wall that is its rock's own, the rest the
/// air's, measured on the lake's wall at dusk in round 30's diagnostic still: a third through
/// the air, the rock dimmed to 0.37 as a far wall, its tiers' 20% showing as 2 to 4%.
const ROCK_SHARE: f32 = 1.0 / 6.0;

/// The tiers' own height in the shader, in metres, and the flatness they fade over near.
const TIER_M: f32 = 95.0;
const NEAR_FLAT: [f32; 2] = [0.55, 0.7];

impl Strata {
    fn shader() -> Self {
        let source =
            std::fs::read_to_string(crate::data::assets().join("shaders/ground.wesl")).unwrap();
        let pair = |name| {
            let v = constant(&source, name);
            [v[0], v[1]]
        };
        Self {
            from_m: pair("FAR_STRATA_M"),
            flat: pair("FAR_STRATA_FLAT"),
            banded: constant(&source, "FAR_BANDED")[0],
            line: constant(&source, "LEDGE_LINE")[0],
            line_px: constant(&source, "LEDGE_LINE_PX")[0],
            gain: constant(&source, "FAR_TONE_GAIN")[0],
            bench: constant(&source, "FAR_BENCH")[0],
            least: constant(&source, "FAR_TONE_LEAST")[0],
        }
    }

    /// `far_tier_tone`, line for line: a far tier's tone from its near one, on a bench's top.
    fn far_tone(&self, tone: f32, bench: f32) -> f32 {
        (1.0 + (tone - 1.0) * self.gain).max(self.least) * (1.0 + (self.bench - 1.0) * bench)
    }

    /// `far_strata`, line for line: the flatness the tiers fade over, the least banding, and
    /// the shade at `tier_f` of the way up a tier.
    fn at(&self, far_off: f32, tier_f: f32, height_px: f32) -> ([f32; 2], f32, f32) {
        let far = smoothstep(self.from_m[0], self.from_m[1], far_off);
        let mix = |a: f32, b: f32| a + (b - a) * far;
        let wide = (self.line_px * height_px / TIER_M).max(0.06);
        let line = (1.0 - smoothstep(wide * 0.6, wide, tier_f)) * far;
        (
            [
                mix(NEAR_FLAT[0], self.flat[0]),
                mix(NEAR_FLAT[1], self.flat[1]),
            ],
            self.banded * far,
            1.0 - (1.0 - self.line) * line,
        )
    }

    /// How strongly a wall of `slope_deg` stands in tiers `far_off` metres away, with the
    /// banding noise at its least (`strength` in the shader, `seen` 1).
    fn strength(&self, far_off: f32, slope_deg: f32) -> f32 {
        let (flat, banded, _) = self.at(far_off, 0.5, 1.0);
        let flatness = libm::cosf(slope_deg.to_radians());
        banded * (1.0 - smoothstep(flat[0], flat[1], flatness))
    }

    /// How many metres of a tier's height its foot's shadow line darkens by a fifth or more.
    fn line_m(&self, far_off: f32, height_px: f32) -> f32 {
        (0..1000)
            .filter(|&i| self.at(far_off, (i as f32 + 0.5) / 1000.0, height_px).2 < 0.8)
            .count() as f32
            / 1000.0
            * TIER_M
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How many metres of height a pixel spans on a wall so far off, at 1080 rows over a
    /// vertical field of view of about 50 degrees.
    fn px_m(far_m: f32) -> f32 {
        far_m * 50f32.to_radians() / 1080.0
    }

    #[test]
    fn a_far_wall_stands_in_tiers_with_ledge_lines() {
        let s = Strata::shader();
        for far_m in [4000.0, 8000.0, 20000.0] {
            for slope in [40.0, 45.0, 60.0] {
                let strength = s.strength(far_m, slope);
                assert!(
                    strength >= 0.5,
                    "a {slope}° wall {far_m} m off stands in tiers only {strength:.2}"
                );
            }
            let px = px_m(far_m);
            let line = s.line_m(far_m, px);
            assert!(
                line >= px && line <= TIER_M * 0.25,
                "{far_m} m off, a ledge line {line:.1} m wide against a pixel's {px:.1} m"
            );
            let dark = s.at(far_m, 0.0, px).2;
            assert!(
                dark <= 0.7,
                "{far_m} m off, a ledge line shades only to {dark:.2}"
            );
        }
    }

    #[test]
    fn a_near_wall_keeps_its_own_tiers() {
        let s = Strata::shader();
        let (flat, banded, shade) = s.at(500.0, 0.01, px_m(500.0));
        assert_eq!(flat, NEAR_FLAT);
        assert_eq!(banded, 0.0);
        assert_eq!(shade, 1.0);
        // A gentle slope near stays untiered, as the r5 review asked: no contour lines.
        assert_eq!(s.strength(500.0, 38.0), 0.0);
    }

    #[test]
    fn a_far_wall_s_tiers_part_through_the_haze_and_keep_its_tone() {
        // A light tier's bench over a dark tier's riser: near, their tones as drawn; far, as
        // the eye gets them through the air (ROCK_SHARE), they part by a seventh of the light
        // at least, where the near tones parted by under a twentieth (round 30's diagnostic).
        let s = Strata::shader();
        let (light, dark) = (1.1, 0.75);
        let parted = |a: f32, b: f32| (a - b) / ((a + b) * 0.5) * ROCK_SHARE;
        let near = parted(light, dark);
        let far = parted(s.far_tone(light, 1.0), s.far_tone(dark, 0.0));
        assert!(near < 0.07, "near, tiers part by {near:.3}");
        assert!(
            far > 1.0 / 7.0,
            "far, tiers part by {far:.3} through the haze"
        );
        // Over every tier's tone and bed (the shader's tier_r, bed_r) and a bench a quarter
        // of a tier, the far wall's mean stays within a tenth of its near tone, so the wall
        // stands as light or dark against the sky as before.
        let (mut near_sum, mut far_sum, mut n) = (0.0, 0.0, 0.0);
        for r in 0..=20 {
            for b in 0..=20 {
                let tone = (0.7 + 0.45 * r as f32 / 20.0) * (0.9 + 0.16 * b as f32 / 20.0);
                near_sum += tone;
                far_sum += 0.75 * s.far_tone(tone, 0.0) + 0.25 * s.far_tone(tone, 1.0);
                n += 1.0;
            }
        }
        let kept = far_sum / near_sum;
        assert!(
            (kept - 1.0).abs() < 0.1,
            "a far wall's mean tone {:.2} of its near {:.2}",
            far_sum / n,
            near_sum / n
        );
    }

    #[test]
    fn the_twin_draws_the_shader_rule() {
        let source =
            std::fs::read_to_string(crate::data::assets().join("shaders/ground.wesl")).unwrap();
        let start = source.find("fn far_strata(").unwrap();
        let body = &source[start..start + source[start..].find("\n}").unwrap()];
        for read in [
            "smoothstep(FAR_STRATA_M.x, FAR_STRATA_M.y, far_off)",
            "mix(0.55, FAR_STRATA_FLAT.x, far)",
            "mix(0.7, FAR_STRATA_FLAT.y, far)",
            "max(0.06, LEDGE_LINE_PX * height_px / 95.0)",
            "(1.0 - smoothstep(wide * 0.6, wide, tier_f)) * far",
            "FAR_BANDED * far, 1.0 - (1.0 - LEDGE_LINE) * line",
        ] {
            assert!(body.contains(read), "far_strata no longer reads {read}");
        }
        for used in [
            "let tier_f = fract(tiers);",
            "let tiers = tier_at / 95.0;",
            "var strength = max(banded, far_tiers.z) * seen * (1.0 - smoothstep(far_tiers.x, far_tiers.y, flatness));",
            "band_tone *= mix(1.0, far_tiers.w, standing);",
            "return max(1.0 + (tone - 1.0) * FAR_TONE_GAIN, FAR_TONE_LEAST) * mix(1.0, FAR_BENCH, bench);",
            "band_tone = mix(band_tone, far_tier_tone(band_tone, bench * standing), smoothstep(FAR_STRATA_M.x, FAR_STRATA_M.y, far_off));",
        ] {
            assert!(source.contains(used), "the tiers no longer read {used}");
        }
    }
}
