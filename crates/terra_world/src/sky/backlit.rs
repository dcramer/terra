//! The far range's ridge after sunset (`assets/shaders/sky.wesl`'s `backlit_ridge`, D426) on
//! the CPU, line for line, so its rule is tested without a GPU: a range stands dark in layers
//! under the sky, toward the set sun as a silhouette with a thin bright rim on its crests,
//! away from it blue-grey in the earth's shadow.

use super::dusk::{mix, mixf};
use bevy::math::Vec3;

#[allow(clippy::too_many_arguments)]
fn backlit_ridge(
    seen: Vec3,
    behind: Vec3,
    cool: Vec3,
    far: f32,
    snowy: f32,
    grain: f32,
    rim: f32,
    dusk: f32,
    toward: f32,
) -> Vec3 {
    let shade =
        mixf(0.18, 0.85, libm::powf(far, 1.25)) * (0.9 + 0.2 * grain) * mixf(1.0, 1.3, snowy);
    let silhouette = mix(mix(behind, cool, 0.35), behind, 0.35 * toward) * shade;
    let edge = behind * mixf(1.5, 2.0, snowy);
    mix(mix(seen, silhouette, dusk), edge, rim * toward * dusk)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUMA: Vec3 = Vec3::new(0.2126, 0.7152, 0.0722);

    /// A linear colour's luma as shown, sRGB 0-255.
    fn shown(linear: Vec3) -> f32 {
        let luma = linear.dot(LUMA);
        bevy::color::Srgba::from(bevy::color::LinearRgba::rgb(luma, luma, luma)).red * 255.0
    }

    /// The linear luminance shown as this sRGB value.
    fn linear(srgb: f32) -> f32 {
        let v = srgb / 255.0;
        bevy::color::LinearRgba::from(bevy::color::Srgba::rgb(v, v, v)).red
    }

    #[test]
    fn toward_the_set_sun_a_range_is_a_layered_silhouette_under_the_glow_with_a_bright_rim() {
        // The lake's dusk sky behind the range, an amber glow shown at 112, and the land before
        // it as pale as the glow (the r19 render: 12 percent under the sky).
        let amber = Vec3::new(1.25, 0.98, 0.7);
        let behind = amber * (linear(112.0) / amber.dot(LUMA));
        let cool = Vec3::new(0.35, 0.5, 0.79).normalize() * behind.length();
        let sky = shown(behind);
        let pale = behind * 0.88;
        let ridge = |far: f32, snowy: f32, rim: f32| {
            backlit_ridge(pale, behind, cool, far, snowy, 0.5, rim, 1.0, 1.0)
        };
        // The nearer ridges at most 0.6 of the sky, each nearer one darker by 15.
        let layers: Vec<f32> = (0..4)
            .map(|k| shown(ridge(k as f32 / 3.0, 0.0, 0.0)))
            .collect();
        assert!(
            layers[0] <= 0.6 * sky && layers[1] <= 0.6 * sky,
            "{layers:?} under {sky}"
        );
        for pair in layers.windows(2) {
            assert!(pair[1] - pair[0] >= 15.0, "layers {layers:?}");
        }
        assert!(
            layers[3] < sky,
            "the farthest {layers:?} under the sky {sky}"
        );
        // Its snow a little lighter than its rock, still a silhouette.
        let snow = shown(ridge(0.0, 1.0, 0.0));
        assert!(
            snow > layers[0] + 5.0 && snow <= 0.6 * sky,
            "snow {snow} rock {layers:?}"
        );
        // The rim 20 over the sky beside it.
        let rim = shown(ridge(0.0, 0.0, 1.0));
        assert!(rim >= sky + 20.0, "rim {rim} by the sky {sky}");
        // Away from the set sun, as dark, bluer than toward it, and no rim.
        let away = backlit_ridge(pale, behind, cool, 0.0, 0.0, 0.5, 1.0, 1.0, 0.0);
        let toward = ridge(0.0, 0.0, 0.0);
        assert!(shown(away) <= 0.6 * sky, "away {away}");
        assert!(
            away.z / away.x > toward.z / toward.x,
            "away {away} toward {toward}"
        );
        // Under a blue-grey horizon, the earth's shadow's blue-grey, not the zenith's blue:
        // tinted by it whole, the lake's range stood royal blue (round 20's first render).
        let horizon = Vec3::new(0.86, 0.97, 1.15) * (linear(112.0) / 0.95);
        let deep_blue = Vec3::new(0.35, 0.5, 0.79).normalize() * horizon.length();
        let shadowed = backlit_ridge(horizon, horizon, deep_blue, 0.0, 0.0, 0.5, 0.0, 1.0, 0.0);
        let s = bevy::color::Srgba::from(bevy::color::LinearRgba::rgb(
            shadowed.x, shadowed.y, shadowed.z,
        ));
        assert!(s.blue < 1.6 * s.red, "away under a pale horizon {s:?}");
        // By day, nothing changes.
        assert_eq!(
            backlit_ridge(pale, behind, cool, 0.0, 0.0, 0.5, 1.0, 0.0, 1.0),
            pale
        );
    }

    #[test]
    fn the_shader_draws_the_rule_tested_here() {
        let shader =
            std::fs::read_to_string(crate::data::assets().join("shaders/sky.wesl")).unwrap();
        let here = include_str!("backlit.rs");
        let numbers = super::super::dusk::tests::numbers;
        assert_eq!(
            numbers(&shader, "backlit_ridge"),
            numbers(here, "backlit_ridge")
        );
    }
}
