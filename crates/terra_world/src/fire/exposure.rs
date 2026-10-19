//! The exposure weight (docs/fire.md, "The exposure rule"; D415): how far a fire's brightness
//! follows the eye's exposure. A fire is drawn at `L × (E / E_ref)^w`, its luminance `L` pulled
//! toward the eye's adaptation `E` by the weight `w`, one number a realm: 0 is physical
//! (Frostbite), a fire weak by day; 1 is the screen's scale, the same at noon and night, and
//! pasted on. Lights stay physical.
//!
//! In the shader the flames, walls, tongues and breaths are drawn on the screen's scale as
//! tuned at dusk, so the weight is a factor on that: `(k / k_dusk)^(1 - w)`, `k` the frame's
//! exposure multiplier (the camera's times the eye's adaptation, `adaptation`), `k_dusk` the
//! camera's at dusk. Unread by a realm, the weight is 1, today's look.

use bevy::prelude::*;

/// The weight a realm is expected to want (Leaning, to be measured by the fire check).
pub const LEANING_WEIGHT: f32 = 0.4;

/// A realm's fire exposure. Absent, fires are drawn on the screen's scale (`weight` 1).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct FireExposure {
    /// The exposure weight `w`, 0 physical to 1 the screen's scale.
    pub weight: f32,
    /// The camera's exposure at dusk, EV100, where a fire is drawn as tuned (`E_ref`):
    /// the day's exposure with the sun six degrees under (`daylight`).
    pub dusk_ev100: f32,
}

impl Default for FireExposure {
    fn default() -> Self {
        Self {
            weight: 1.0,
            dusk_ev100: 9.0,
        }
    }
}

impl FireExposure {
    /// For the fire's uniform: x the weight, y the log2 of the exposure multiplier at dusk,
    /// as Bevy's `Exposure::exposure` makes it from EV100 (`2^-ev100 / 1.2`).
    pub fn packed(&self) -> Vec4 {
        Vec4::new(self.weight, -self.dusk_ev100 - libm::log2f(1.2), 0.0, 0.0)
    }

    /// The factor a fire on the screen's scale is drawn at, for a frame whose exposure
    /// multiplier is `2^stops`; as the shader's `weighted`.
    pub fn factor(&self, stops: f32) -> f32 {
        let dusk = self.packed().y;
        libm::exp2f((1.0 - self.weight) * (stops - dusk))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frame's stops for a camera at `ev100` with the eye adapted `adapted` stops.
    fn stops(ev100: f32, adapted: f32) -> f32 {
        libm::log2f(libm::exp2f(-ev100) / 1.2) + adapted
    }

    #[test]
    fn the_weight_runs_from_physical_to_the_screen() {
        let screen = FireExposure::default();
        let physical = FireExposure {
            weight: 0.0,
            ..screen
        };
        let leaning = FireExposure {
            weight: LEANING_WEIGHT,
            ..screen
        };
        let (noon, dusk) = (stops(11.8, 0.0), stops(9.0, 0.0));
        // At dusk every weight draws a fire as tuned.
        for fire in [screen, physical, leaning] {
            assert!((fire.factor(dusk) - 1.0).abs() < 1.0e-5);
        }
        // On the screen's scale, the same by day and night, whatever the eye does.
        assert!((screen.factor(noon) - 1.0).abs() < 1.0e-5);
        assert!((screen.factor(stops(9.0, 1.25)) - 1.0).abs() < 1.0e-5);
        // Physical: by day dimmer by all the stops the exposure closed (2.8: 7x).
        let ratio = physical.factor(dusk) / physical.factor(noon);
        assert!((ratio - libm::exp2f(2.8)).abs() < 0.01, "{ratio}");
        // Leaning: the drawn fire's dusk-to-noon ratio near the doc's 3 (The Witcher 3's).
        let ratio = leaning.factor(dusk) / leaning.factor(noon);
        assert!((2.5..4.0).contains(&ratio), "{ratio}");
    }
}
