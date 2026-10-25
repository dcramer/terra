//! The air's haze over what is drawn starts some metres out from the eye (D475), as Unreal's sky
//! atmosphere's aerial perspective does with its Start Depth (its SkyAtmosphere component's
//! "Aerial Perspective Start Depth"): what stands nearer keeps its own colour, and from there
//! the haze builds as it did, over the distance past the start. Our air is seen through a lens
//! that makes every distance through it [`crate::air::AirScale`] times longer (D212, D299), so
//! the haze over hills reads at the small planet's scale; over the near metres that lens laid
//! the haze of a few hundred metres of sunlit air on everything, where real air over tens of
//! metres shows nothing. On the forest's dark crowns, the exposure opened for the shade under
//! them, that was most of their colour: leaves drawn pure red took back 40 to 60% of their green
//! and 70 to 80% of their blue from it, and their median hue went from 64 degrees, Sons of the
//! Forest's olive, to 93 lit and 119 in shade, a grey teal (forest models round 2).
//!
//! Bevy's sky pass, in `bevy_pbr/src/atmosphere/render_sky.wesl` (MIT OR Apache-2.0, by the
//! Bevy contributors), is patched where it is loaded, as an asset: the distance it reads the
//! aerial-view table and the transmittance at is shortened by [`START_M`] world metres, in the
//! air's own metres at the lens's scale. If Bevy's source no longer has the lines, the patch is
//! not made and says so, and the haze lies up to the eye.

use bevy::prelude::*;

/// Where Bevy keeps the sky pass.
const SHADER: &str = "embedded://bevy_pbr/atmosphere/render_sky.wesl";

/// Nearer than this, metres in the world, the air lays no haze on what is drawn; past it the
/// haze is that of the distance beyond.
pub const START_M: f32 = 50.0;

/// The sky pass's bindings, to which the atmosphere's own is added for its transform.
const BINDINGS: &str = "    bindings::{view, settings},";
/// Where the sky pass reads the distance to what is drawn, in the air's metres.
const DISTANCE: &str = "        let t = ndc_to_camera_dist(vec3(uv_to_ndc(in.uv), depth));";

pub fn plugin(app: &mut App) {
    super::shader_patch::patch_library(
        app,
        SHADER,
        patched,
        "Bevy's sky pass was not found as expected: the air's haze lies up to the eye and veils the near crowns",
    );
}

/// The sky pass with the haze over what is drawn starting `START_M` out, if it has the lines.
fn patched(source: &str) -> Option<String> {
    let bound = super::shader_patch::swap_once(
        source,
        BINDINGS,
        "    bindings::{view, settings, atmosphere},",
    )?;
    // The air's metres per world metre: the length of the world-to-air transform's first
    // column, its scale being even.
    let started = format!(
        "{}\n        let t = max(t_seen - {START_M:.3} * length(atmosphere.world_to_atmosphere[0].xyz), 0.0);",
        DISTANCE.replace("let t =", "let t_seen =")
    );
    super::shader_patch::swap_once(&bound, DISTANCE, &started)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_distance_read_is_shortened_by_the_start_in_the_airs_metres() {
        let pass = format!(
            "import package::atmosphere::{{\n{BINDINGS}\n}};\nfn main() {{\n    if depth == 0.0 {{\n    }} else {{\n{DISTANCE}\n        inscattering = sample_aerial_view_lut(in.uv, t);\n    }}\n}}\n"
        );
        let out = patched(&pass).unwrap();
        assert!(out.contains("bindings::{view, settings, atmosphere},"));
        assert!(out.contains(&format!(
            "        let t_seen = ndc_to_camera_dist(vec3(uv_to_ndc(in.uv), depth));\n        let t = max(t_seen - {START_M:.3} * length(atmosphere.world_to_atmosphere[0].xyz), 0.0);\n        inscattering = sample_aerial_view_lut(in.uv, t);"
        )));
        assert!(patched("fn a() {}").is_none());
        assert!(patched(&format!("{pass}{pass}")).is_none());
    }
}
