//! Bevy's volumetric fog scatters a point or spot light with no phase function
//! (`bevy_pbr/src/volumetric_fog/volumetric_fog.wesl`, MIT OR Apache-2.0, by the Bevy
//! contributors; Bevy 0.20 and its main branch): its comment says the light is modulated by
//! the phase, and the directional light's is, by Henyey-Greenstein, but a point light's
//! in-scatter is the fog's density times the light's whole intensity over its distance
//! squared. That is the phase function left at 1 where an even scatter's is 1/(4 pi): every
//! ray within a fire light's reach glowed some twelve times too bright and as much looking
//! away from the fire as at it, and a blaze's light washed the whole frame orange at dusk and
//! white at night (fire sheet, round 5). Unreal's volumetric fog weighs its local lights by
//! the same Henyey-Greenstein phase as its sun (its Volumetric Fog docs and Wronski's
//! SIGGRAPH 2014 talk), so the one line is given the phase the directional light's has: a fire
//! glows in the haze round itself, brightest looking toward it.
//!
//! Its in-scatter also falls off as `1 / max(d², 0.0001)`, a light a centimetre across: the
//! air within a hand of a fire light took thousands of times its light and showed as a hot
//! white dot in the glow round every campfire (fire sheet, stage 4). A fire's light is not a
//! point but its flames, as wide as its `radius`; inside a sphere light the illuminance stops
//! rising (Lagarde and de Rousiers, "Moving Frostbite to PBR", SIGGRAPH 2014, the sphere
//! light's illuminance), so the light's radius is added to the distance, squared, as Unreal's
//! source radius softens its falloff: held at the radius (`max`), the air inside it was lit
//! evenly and showed as a ball with an edge over every campfire (fire sheet review, 3/10);
//! added, the glow is as bright at the light and fades with no edge.

use bevy::prelude::*;

/// Where Bevy keeps the volumetric fog's shader.
const SHADER: &str = "embedded://bevy_pbr/volumetric_fog/volumetric_fog.wesl";

/// Bevy's in-scatter of a point or spot light, with no phase.
const WRONG: &str =
    "let light_color_per_step = (*light).color_inverse_square_range.rgb * light_factors_per_step;";

/// The same weighed by the fog's phase for the light's direction (`L`, toward the light) and
/// the ray's (`V`), as the directional light's is.
fn right() -> String {
    format!(
        "let light_color_per_step = (*light).color_inverse_square_range.rgb * light_factors_per_step * henyey_greenstein(dot(L, V)) * {LOCAL_SCATTER:?};"
    )
}

/// How much of a fire light's in-scatter the air takes, in `right`: the haze is mist-thick by the
/// ground (0.03 to 0.2 a metre, scattering forward at 0.7) for the sun's shafts, and a hearth's
/// light in it was a ball over the logs past the tonemapper's white, sRGB 241,157,125 at its
/// middle with the fire's own fog volumes off and gone with its light out of the fog (fire
/// sheet, 2026-10-19). At 0.15 it is a faint warm glow over the flames, under the knee.
const LOCAL_SCATTER: f32 = 0.15;

/// Bevy's fall-off of a point light's in-scatter, held to a centimetre.
const POINT: &str = "let distance_atten = getDistanceAttenuation(distance_square, (*light).color_inverse_square_range.w);";

/// The same softened by the light's radius, as a sphere light's is.
const SPHERE: &str = "let distance_atten = getDistanceAttenuation(distance_square + (*light).position_radius.w * (*light).position_radius.w, (*light).color_inverse_square_range.w);";

pub fn plugin(app: &mut App) {
    super::shader_patch::patch_library(
        app,
        SHADER,
        patched,
        "Bevy's point-light in-scatter was not found: fire lights in the fog would scatter with no phase, so they are left out of it",
    );
}

/// The fog's shader with its point lights' in-scatter weighed by the phase, if it has Bevy's.
fn patched(source: &str) -> Option<String> {
    let phased = super::shader_patch::swap_once(source, WRONG, &right())?;
    super::shader_patch::swap_once(&phased, POINT, SPHERE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_light_scatters_by_the_phase_as_the_sun_does() {
        let shader = format!("fn f() {{\n            {POINT}\n            {WRONG}\n}}\n");
        let out = patched(&shader).unwrap();
        assert!(out.contains("henyey_greenstein(dot(L, V))") && !out.contains(WRONG));
        assert!(out.contains(&format!(
            "henyey_greenstein(dot(L, V)) * {LOCAL_SCATTER:?};"
        )));
        // And its fall-off is softened by the light's radius.
        assert!(
            out.contains("distance_square + (*light).position_radius.w") && !out.contains(POINT)
        );
        // Not there, or there twice: left alone.
        assert!(patched("fn f() {}").is_none());
        assert!(patched(&format!("{shader}{shader}")).is_none());
    }
}
