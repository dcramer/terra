//! The ground shader's own air (`assets/shaders/ground.wesl`'s `air_through`, D434) on the
//! CPU, with the shader's constants read from it, so its rule is tested without a GPU: a far
//! wall's foot keeps a third of its own light at 2 to 3 km by day, as clear air leaves most of
//! it, and the ranges beyond still recede in layers; at dusk, with the sun faint, the air
//! keeps the thicker haze the developer chose there (2026-10-22). Bevy's distance haze follows it; on the
//! mountain world it leaves 97% at 3 km (`daylight::haze_for`), so it is left out here.

/// The shader's constants that `air_through` reads.
struct Air {
    from_m: f32,
    aerial_m: [f32; 2],
    mist_m: [f32; 2],
    mist_depth_m: [f32; 2],
    mist_dusk: f32,
}

impl Air {
    fn shader() -> Self {
        let source =
            std::fs::read_to_string(crate::data::assets().join("shaders/ground.wesl")).unwrap();
        let mist = constant(&source, "FOOT_MIST_M");
        Self {
            from_m: constant(&source, "AERIAL_FROM_M")[0],
            aerial_m: [
                constant(&source, "AERIAL_DUSK_M")[0],
                constant(&source, "AERIAL_M")[0],
            ],
            mist_m: [mist[0], mist[1]],
            mist_depth_m: [
                constant(&source, "FOOT_MIST_DEPTH_DUSK_M")[0],
                constant(&source, "FOOT_MIST_DEPTH_M")[0],
            ],
            mist_dusk: constant(&source, "FOOT_MIST_DUSK")[0],
        }
    }

    /// `air_through`, line for line.
    fn through(&self, past: f32, over_eye: f32, day: f32) -> f32 {
        let mix = |[dusk, day_m]: [f32; 2]| dusk + (day_m - dusk) * day;
        let low_mist = (1.0 - terra_gen::smoothstep(self.mist_m[0], self.mist_m[1], over_eye))
            / mix(self.mist_depth_m)
            * mix([self.mist_dusk, 1.0]);
        1.0 - libm::expf(-past / mix(self.aerial_m) - past * low_mist)
    }

    /// How much of its own light the ground `far_m` from the eye keeps.
    fn kept(&self, far_m: f32, over_eye: f32, day: f32) -> f32 {
        1.0 - self.through((far_m - self.from_m).max(0.0), over_eye, day)
    }
}

/// The numbers of a WGSL `const NAME: ... = ...;`.
pub(crate) fn constant(source: &str, name: &str) -> Vec<f32> {
    let start = source
        .find(&format!("const {name}:"))
        .unwrap_or_else(|| panic!("ground.wesl has no {name}"));
    let line = &source[start..start + source[start..].find(';').unwrap()];
    let value = &line[line.find('=').unwrap() + 1..];
    value
        .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .filter(|s| s.contains('.'))
        .map(|s| s.parse().unwrap())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Over the eye's height a wall's foot stands, and where the low mist has thinned out.
    const FOOT: [f32; 3] = [0.0, 40.0, 80.0];
    const CREST: f32 = 900.0;

    #[test]
    fn by_day_a_far_wall_keeps_a_third_of_its_light_and_far_ranges_recede() {
        let air = Air::shader();
        {
            let day = 1.0;
            for far_m in [2000.0, 3000.0] {
                for over in FOOT {
                    let kept = air.kept(far_m, over, day);
                    assert!(
                        kept >= 1.0 / 3.0,
                        "a wall's foot {far_m} m off, {over} m over the eye, day {day}, kept {kept:.3}"
                    );
                }
            }
            // Its foot stands in the valley's haze, paler than its crest, so a peak does not
            // read as a billboard set on the treeline (lake film r16).
            let (foot, crest) = (air.kept(3000.0, 0.0, day), air.kept(3000.0, CREST, day));
            assert!(
                foot < crest * 0.95,
                "foot {foot:.3} against crest {crest:.3}, day {day}"
            );
        }
        // Clear air over the mist: a range 3 km off keeps most of its light, as real air
        // leaves 70 to 85% (visibility 20 to 50 km), and each range farther back is hazier,
        // 14 km of it half gone at least, so the far ranges stand in layers (round 13).
        let clear = air.kept(3000.0, CREST, 1.0);
        assert!(
            (0.6..=0.9).contains(&clear),
            "3 km of clear air kept {clear:.3}"
        );
        let ranges = [3000.0, 6000.0, 10_000.0, 14_000.0].map(|m| air.kept(m, CREST, 1.0));
        for pair in ranges.windows(2) {
            assert!(pair[1] < pair[0] * 0.8, "ranges {ranges:?} do not recede");
        }
        assert!(ranges[3] <= 0.5, "14 km of air kept {:.3}", ranges[3]);
    }

    #[test]
    fn at_dusk_the_air_keeps_its_haze_and_clears_as_the_sun_brightens() {
        let air = Air::shader();
        // With the sun faint, a wall's foot across the lake stands deep in the haze, as the
        // developer chose over the clear air at dusk (2026-10-22): a sixth of its light at most
        // at 3 km and under three-tenths at 2 km, where the clear day's air leaves over a third.
        for (far_m, most) in [(2000.0, 0.3), (3000.0, 1.0 / 6.0)] {
            for over in FOOT {
                let kept = air.kept(far_m, over, 0.0);
                assert!(
                    kept <= most,
                    "at dusk a wall's foot {far_m} m off, {over} m over the eye, kept {kept:.3}"
                );
            }
        }
        // Its foot still paler than its crest, so it rises out of the valley's haze.
        let (foot, crest) = (air.kept(3000.0, 0.0, 0.0), air.kept(3000.0, CREST, 0.0));
        assert!(
            foot < crest * 0.95,
            "dusk foot {foot:.3} against crest {crest:.3}"
        );
        // And the air clears steadily as the sun brightens, at every distance and height.
        for far_m in [1000.0, 3000.0, 10_000.0] {
            for over in [0.0, CREST] {
                let kept: Vec<f32> = (0..=10)
                    .map(|i| air.kept(far_m, over, i as f32 / 10.0))
                    .collect();
                assert!(
                    kept.windows(2).all(|w| w[1] > w[0]),
                    "{far_m} m, {over} m over: kept {kept:?} does not rise with the sun"
                );
            }
        }
    }

    #[test]
    fn the_twin_draws_the_shader_rule() {
        let source =
            std::fs::read_to_string(crate::data::assets().join("shaders/ground.wesl")).unwrap();
        let start = source.find("fn air_through(").unwrap();
        let body = &source[start..start + source[start..].find("\n}").unwrap()];
        for read in [
            "smoothstep(FOOT_MIST_M.x, FOOT_MIST_M.y, over_eye)",
            "/ mix(FOOT_MIST_DEPTH_DUSK_M, FOOT_MIST_DEPTH_M, day) * mix(FOOT_MIST_DUSK, 1.0, day)",
            "1.0 - exp(-past / mix(AERIAL_DUSK_M, AERIAL_M, day) - past * low_mist)",
        ] {
            assert!(body.contains(read), "air_through no longer reads {read}");
        }
        assert!(source.contains("let through = air_through(past, over_eye, day);"));
        assert!(source.contains(
            "let past = max(distance(world, view.world_position) - AERIAL_FROM_M, 0.0);"
        ));
    }
}
