//! How a breath's fireball burns out once its stream stops, as `fire.wesl` draws it
//! (`smoke_left`, `ball_size`, the soot's albedo in `breath`): its flame gone within a
//! second, what is left dark soot that rises, spreads and thins, as a real fireball's does
//! (castle fall r26). Only the test reads it: the shader is what draws it.

/// Seconds: the smoke mixing into the air from the first to the second; the burning gas
/// gone to its soot by `BURN_S`; how far the smoke rises a second, in metres, and how much
/// bigger it is by the time it is gone.
const HOLD_S: f32 = 0.5;
const GONE_S: f32 = 4.5;
const BURN_S: f32 = 1.0;
const SPREAD: f32 = 1.1;
/// The soot's density, a share of the burning gas's.
const SOOT: f32 = 0.04;
/// The soot's albedo, fresh and aged.
const ALBEDO: f32 = 0.03;
const AGED_ALBEDO: f32 = 0.1;

use terra_gen::smoothstep;

/// What is left of the fireball's gas so long after the stream stops, a share of it burning.
fn left(since_off: f32) -> f32 {
    let burnt = 1.0 + (SOOT - 1.0) * smoothstep(0.0, BURN_S, since_off);
    let spread = spread(since_off);
    burnt / (spread * spread * spread) * (1.0 - smoothstep(HOLD_S, GONE_S, since_off))
}

/// How much bigger it is than it burned.
fn spread(since_off: f32) -> f32 {
    1.0 + SPREAD * (since_off / GONE_S).clamp(0.0, 1.0)
}

/// Its soot's albedo.
fn albedo(since_off: f32) -> f32 {
    ALBEDO + (AGED_ALBEDO - ALBEDO) * smoothstep(0.3, 1.5, since_off)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fraction of light through the ball at a share of its radius off its middle, so
    /// long after: Beer-Lambert through its gas, as the shader marches it (`VOLUME_SOOT_K`
    /// a metre at density 1, the gas thinner toward its edge and between its billows).
    fn through(since_off: f32, off: f32) -> f32 {
        let (k, radius) = (14.0, 2.4 * spread(since_off));
        let path = 2.0 * radius * (1.0 - off * off).max(0.0).sqrt();
        // Its edge's thinning and its rolls (`breath_gas`), at their middling values.
        let edge = 0.35 + 0.65 * (1.0 - smoothstep(0.3, 0.9, off));
        let density = left(since_off) * edge * 0.65;
        libm::expf(-k * density * path)
    }

    #[test]
    fn a_fireball_burns_out_in_a_second_to_dark_smoke_that_rises_spreads_and_thins() {
        // While it burns, and as its stream stops, it hides what is behind it.
        assert!(through(0.0, 0.0) < 0.01 && through(0.0, 0.5) < 0.01);
        // Within a second and a half its flame is gone: see through its outer half, and
        // its middle no more than a haze.
        assert!(through(1.5, 0.5) > 0.45, "outer half {}", through(1.5, 0.5));
        assert!(through(1.5, 0.0) > 0.25, "middle {}", through(1.5, 0.0));
        // Thinning all the while it hangs, never thickening again.
        let mut last = 0.0;
        for tenth in 0..=50 {
            let now = through(tenth as f32 / 10.0, 0.0);
            assert!(
                now >= last - 1.0e-6,
                "thickened at {} s",
                tenth as f32 / 10.0
            );
            last = now;
        }
        // Gone by the time it would be, bigger than it burned.
        assert!(left(GONE_S) < 1.0e-6);
        assert!(spread(GONE_S) > 1.5);
        // Dark smoke all its life, never a white cloud: soot's albedo is 0.2 to 0.3 at most
        // (Bond and Bergstrom 2006).
        for tenth in 0..=50 {
            assert!(albedo(tenth as f32 / 10.0) <= 0.2 + 1.0e-6);
        }
    }

    #[test]
    fn the_shader_burns_a_fireball_out_as_this_says() {
        let shader =
            std::fs::read_to_string(crate::data::assets().join("shaders/fire.wesl")).unwrap();
        for line in [
            format!("const SMOKE_HOLD: f32 = {HOLD_S:?};"),
            format!("const SMOKE_GONE: f32 = {GONE_S:?};"),
            format!("const SMOKE_SPREAD: f32 = {SPREAD:?};"),
            format!("const SMOKE_SOOT: f32 = {SOOT:?};"),
            format!("const SMOKE_BURN: f32 = {BURN_S:?};"),
            "const VOLUME_SOOT_K: f32 = 14.0;".to_string(),
            format!("const SOOT_ALBEDO: f32 = {ALBEDO:?};"),
            format!("const SOOT_AGED_ALBEDO: f32 = {AGED_ALBEDO:?};"),
            "b.smoke = select(0.0, was * smoke_left(b.since_off),".to_string(),
            "let burnt = mix(1.0, SMOKE_SOOT, smoothstep(0.0, SMOKE_BURN, since_off));".to_string(),
            "return burnt / (spread * spread * spread) * (1.0 - smoothstep(SMOKE_HOLD, SMOKE_GONE, since_off));".to_string(),
            "mix(SOOT_ALBEDO, SOOT_AGED_ALBEDO, smoothstep(0.3, 1.5, b.since_off))".to_string(),
            "b.smoke * (0.35 + 0.65 * (1.0 - smoothstep(0.3, 0.9, d)))".to_string(),
        ] {
            assert!(shader.contains(&line), "fire.wesl has no `{line}`");
        }
    }
}
