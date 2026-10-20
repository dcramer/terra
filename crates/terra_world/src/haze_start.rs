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

use bevy::prelude::*;

/// Where Bevy keeps the fog's falloffs.
const LIBRARY: &str = "embedded://bevy_pbr/render/fog.wesl";

/// Nearer than this the haze is not drawn at all, metres: the yard's nearest trunks in its
/// snowfall stand about here. Eased in to 45 m, their shade took luma 10 at its 5th percentile
/// (21 with the haze up to the eye, 6 with none) and the path's far end kept 156.
pub const NEAR_M: f32 = 10.0;
/// From this far out the haze is all there, as the falloff has it, metres.
pub const FULL_M: f32 = 45.0;

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
    let eased =
        format!("{SQUARED}\n    fog_color.a *= smoothstep({NEAR_M:.3}, {FULL_M:.3}, distance);");
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
            "{SQUARED}\n    fog_color.a *= smoothstep(10.000, 45.000, distance);\n}}"
        )));
        assert!(out.contains("    let x = distance * density;\n}"));
        assert!(patched("fn a() {}").is_none());
        assert!(patched(&format!("{library}{library}")).is_none());
    }
}
