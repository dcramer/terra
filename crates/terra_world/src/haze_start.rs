//! The haze fades in over the first metres out from the eye, as Unreal's Exponential Height
//! Fog does with its Start Distance: what stands nearest keeps its own darks, and from the
//! middle distance on the haze is as it was. Under a deck the haze is the deck's light on the
//! air, some two hundred times as bright as a near trunk's shade, so even the half per cent of
//! it that Bevy's squared-exponential falloff puts at ten metres lifted that shade threefold
//! (luma 6 to 19 in the yard's snowfall, weather round 20), where Horizon Forbidden West's and
//! Red Dead Redemption 2's near shadows stay near black through falling snow. Real air does
//! veil the near metres a little; we depart from it there, as those games do. Measuring the
//! falloff from a start instead lost blind: the middle distance went as dark as the near.
//!
//! Bevy's falloff, in `bevy_pbr/render/fog.wesl` (MIT OR Apache-2.0, by the Bevy
//! contributors), is patched where it is loaded, as an asset: its squared falloff's share is
//! eased in from [`NEAR_M`] to [`FULL_M`]. If Bevy's source no longer has the line, the patch
//! is not made and says so, and the haze lies up to the eye.
//!
//! How much it is eased is the weather's: in full under a deck, not at all under a clear sky,
//! where the forest's green air among the near trunks is what greys their shade toward the
//! reference's (the looks check's lush shade blue/green 0.45 with it, 0.51 without, weather
//! round 21). Bevy's fog uniform has no spare field, so the share rides in the directional
//! in-scatter colour's alpha, which Bevy only multiplies into its colour ([`directional`]):
//! from [`PLAIN_ALPHA`] for none to twice it for all.

use bevy::prelude::*;

/// Where Bevy keeps the fog's falloffs.
const LIBRARY: &str = "embedded://bevy_pbr/render/fog.wesl";

/// Nearer than this the haze is not drawn at all, metres: the yard's nearest trunks in its
/// snowfall stand about here. Eased in to 45 m, their shade took luma 10 at its 5th percentile
/// (21 with the haze up to the eye, 6 with none) and the path's far end kept 156.
pub const NEAR_M: f32 = 10.0;
/// From this far out the haze is all there, as the falloff has it, metres.
pub const FULL_M: f32 = 45.0;

/// The directional in-scatter's alpha with the haze up to the eye; twice it, eased in full.
pub const PLAIN_ALPHA: f32 = 0.5;

/// The fog's directional in-scatter colour, drawn as `colour` at `PLAIN_ALPHA` is, carrying
/// how much of the near haze is eased away (0 none, 1 all) in its alpha.
pub fn directional(colour: LinearRgba, eased: f32) -> Color {
    let alpha = PLAIN_ALPHA * (1.0 + eased.clamp(0.0, 1.0));
    let kept = PLAIN_ALPHA / alpha;
    Color::LinearRgba(
        LinearRgba::rgb(colour.red * kept, colour.green * kept, colour.blue * kept)
            .with_alpha(alpha),
    )
}

/// Bevy's squared falloff.
const SQUARED: &str = "    let distance_times_density = distance * fog_params.be.x;";

pub fn plugin(app: &mut App) {
    super::shader_patch::patch_library(
        app,
        LIBRARY,
        patched,
        "Bevy's squared fog falloff was not found: the haze lies up to the eye and lifts the near shade",
    );
}

/// The library with the squared falloff eased in over the near metres, if it has it.
fn patched(source: &str) -> Option<String> {
    let eased = format!(
        "{SQUARED}\n    fog_color.a *= mix(1.0, smoothstep({NEAR_M:.3}, {FULL_M:.3}, distance), \
         clamp(fog_params.directional_light_color.a / {PLAIN_ALPHA:.3} - 1.0, 0.0, 1.0));"
    );
    super::shader_patch::swap_once(source, SQUARED, &eased)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_squared_falloff_is_eased_in() {
        let library = format!(
            "fn exponential_fog() {{\n    let x = distance * density;\n}}\nfn exponential_squared_fog() {{\n{SQUARED}\n}}\n"
        );
        let out = patched(&library).unwrap();
        assert!(out.contains(&format!(
            "{SQUARED}\n    fog_color.a *= mix(1.0, smoothstep(10.000, 45.000, distance), clamp(fog_params.directional_light_color.a / 0.500 - 1.0, 0.0, 1.0));\n}}"
        )));
        assert!(out.contains("    let x = distance * density;\n}"));
        assert!(patched("fn a() {}").is_none());
        assert!(patched(&format!("{library}{library}")).is_none());
    }

    #[test]
    fn the_ease_rides_in_the_alpha_and_the_in_scatter_is_drawn_the_same() {
        let colour = LinearRgba::rgb(0.4, 0.3, 0.2);
        for eased in [0.0, 0.5, 1.0] {
            let c = directional(colour, eased).to_linear();
            // Bevy draws the directional in-scatter as its colour times its alpha.
            assert!((c.red * c.alpha - colour.red * PLAIN_ALPHA).abs() < 1e-6);
            // The patch reads the share back as the alpha over the plain one, less one.
            assert!(
                (c.alpha / PLAIN_ALPHA - 1.0 - eased).abs() < 1e-6,
                "{eased}"
            );
        }
    }
}
