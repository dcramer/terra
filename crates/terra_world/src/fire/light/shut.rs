//! A kept fire's light in a room shut off from the sky (D488): its top light, hung high,
//! wide and unshadowed so a campfire lights the faces round it at dusk (`HEARTH_TOP`), gives
//! most of its light to the shadowed pit key, so a torch lights a pool that walls stop.
//!
//! Unshadowed and reaching 10 m, the top light carried through a dungeon's walls from the
//! next corridor and room, and every torch's added to one even orange over the loculi and
//! the goal, with no dark between the lights (barrow rounds 10 and 11, all three reviewers).
//! Shipped dungeons light a torch as one shadowed point light of a short reach (The Elder
//! Scrolls' and Diablo's torches: a pool on the wall and floor round it, black past it), the
//! dark between filled by a dim ambient (`indoor`'s fill).

use super::Lit;

/// The share of a kept fire's top light given to its pit key in a room wholly shut off from
/// the sky, less as the room opens to it (`indoor::ShutOff`). The rest stays a soft glow
/// round the flame.
const TOP_TO_KEY: f32 = 0.8;

/// Moves that share of each kept fire's top light (`Lit::casts` false) to its pit key, the
/// fire's light as a whole unchanged: what the top light gives up the key takes. The lights
/// are as `lit` makes them, keyed by the hour; a fire's two are told apart by `Lit::still`.
pub(super) fn give_top_to_key(lights: &mut [Lit], shut_off: f32) {
    let share = TOP_TO_KEY * shut_off.clamp(0.0, 1.0);
    if share <= 0.0 {
        return;
    }
    let fire = |lit: &Lit| lit.still.filter(|_| lit.kept).map(|still| still.fire);
    let mut given: Vec<(usize, f32)> = Vec::new();
    for lit in lights.iter_mut().filter(|lit| !lit.casts) {
        if let Some(fire) = fire(lit) {
            given.push((fire, lit.lumens * share));
            lit.lumens *= 1.0 - share;
        }
    }
    for lit in lights.iter_mut().filter(|lit| lit.casts) {
        if let Some(fire) = fire(lit) {
            lit.lumens += given
                .iter()
                .filter(|(f, _)| *f == fire)
                .map(|(_, l)| l)
                .sum::<f32>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Lit, shadows::Still};
    use super::*;
    use bevy::prelude::*;

    fn light(fire: usize, casts: bool, lumens: f32) -> Lit {
        Lit {
            at: Vec3::ZERO,
            lumens,
            color: Color::WHITE,
            range: 6.0,
            radius: 0.1,
            glow: 1.0,
            kept: true,
            night_stops: 0.0,
            dusk_gain: 1.0,
            dusk_reach: 1.0,
            casts,
            still: Some(Still {
                at: Vec3::ZERO,
                shines: 1.0,
                fire,
            }),
        }
    }

    #[test]
    fn in_a_shut_room_a_torchs_light_is_mostly_its_shadowed_key() {
        let mut lights = vec![
            light(0, true, 3.0),
            light(0, false, 4.0),
            light(1, true, 1.0),
            light(1, false, 2.0),
        ];
        give_top_to_key(&mut lights, 1.0);
        for fire in [(0, 7.0), (1, 3.0)] {
            let (key, top): (Vec<&Lit>, Vec<&Lit>) = lights
                .iter()
                .filter(|l| l.still.unwrap().fire == fire.0)
                .partition(|l| l.casts);
            let (key, top) = (key[0].lumens, top[0].lumens);
            // The fire's light whole, each fire's own.
            assert!((key + top - fire.1).abs() < 1e-4, "{key} + {top}");
            // Most of it shadowed: walls stop it.
            assert!(key > 3.0 * top, "{key} against {top}");
        }
    }

    #[test]
    fn under_the_sky_a_fire_keeps_its_top_light() {
        let mut lights = vec![light(0, true, 3.0), light(0, false, 4.0)];
        let before = lights.clone();
        give_top_to_key(&mut lights, 0.0);
        assert_eq!(lights, before);
    }
}
