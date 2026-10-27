//! A breath's lights: its stream's, the flash where it first strikes and a marched fireball's
//! on the face it strikes, split from `light` for its size (D396).

use super::super::{BreathTier, JET_SPEED_MPS};
use super::{BREATH_NIGHT_STOPS, Blazes, Flicker, Lit, NIGHT_STOPS, signal, sway};
use bevy::prelude::*;

/// How far a jet's light reaches, metres (a blaze's, `LIGHT_RANGE_M`, 22).
const JET_RANGE_M: f32 = 30.0;
/// A jet's light just past the mouth, on the breather's head and neck, along its stream and
/// where it strikes: at 1e7 the dragon and the wall it struck showed none of it by day
/// (castle film r7), but at 5e7 and a pale yellow the ground, the wall and the dragon by night
/// stood brighter than the flame, near white (breath r27: the fireball's crop 365% as lit as
/// at noon), where the flame is the brightest thing by it (D415). Its colour an orange-yellow,
/// greener than a blaze's, and half as bright: at #ff9438 and 4e6 the wall and ground it
/// struck went past the tonemapper's highlight limit as salmon pink, as a campfire's stones
/// did at a red light's low green (`HEARTH_LIGHT`), where the references' firelit walls read
/// orange-yellow (breath r33, r34).
const JET_LUMENS: f32 = 2.0e6;
const JET_LIGHT: Color = Color::srgb(1.0, 0.74, 0.32);
/// A marched breath's light (D430), dimmer than the cards': at theirs the wall and ground
/// round its larger fireball went past Neutral's shoulder at dusk (sRGB 237, 185, 119 head-on)
/// and read salmon beside it. Orange, not yellow: at (1, 0.78, 0.3) and 1.4e6 the grass under
/// it read a flat yellow-green flood (vol10's review); the film mask (`BREATH_LIT`) keeps the
/// orange from going salmon past the shoulder now. Dimmer at 0.35e6: at 0.6e6 the ground by
/// the hit stood 1.6 to 5 times white before the tonemapper (vol12's probe). Amber: redder
/// only made up for the film mask's shoulder taken a channel at a time, which ran the ground's
/// green out with its red to a pale lime-yellow; the shoulder keeps the hue now, and at
/// (1, 0.5, 0.16) the wall read coral, at (1, 0.58, 0.22) terracotta (fire/hue's blind A/Bs).
const MARCHED_LUMENS: f32 = 0.35e6;
const MARCHED_LIGHT: Color = Color::srgb(1.0, 0.64, 0.26);

/// A jet's light, its lumens and colour, as its breath is drawn.
pub(in super::super) fn jet_light(tier: BreathTier) -> (f32, Color) {
    match tier {
        BreathTier::Cards => (JET_LUMENS, JET_LIGHT),
        BreathTier::Marched => (MARCHED_LUMENS, MARCHED_LIGHT),
    }
}

/// The flash where a jet's stream first strikes (docs/fire.md, "Composites": 0.1 to 0.3 s),
/// as bright as the blast it lights, then gone: `FLASH_S` long, easing out, its light the
/// white of the blast's hottest.
const FLASH_LUMENS: f32 = 8.0e7;
const FLASH_S: f32 = 0.25;
const FLASH_LIGHT: Color = Color::srgb(1.0, 0.85, 0.62);

/// How bright a flash is `age` seconds after its stream struck, a share of its peak.
pub(in super::super) fn flash(age: f32) -> f32 {
    if !(0.0..FLASH_S).contains(&age) {
        return 0.0;
    }
    let left = 1.0 - age / FLASH_S;
    left * left
}

/// Each jet's flash, while it lasts: `jets` each one's state as `Drawn` keeps it (on, since
/// when, when it last came on).
pub(super) fn flashes(blazes: &Blazes, jets: &[(bool, f32, f32)], t: f32) -> Vec<Lit> {
    blazes
        .jets
        .iter()
        .zip(jets)
        .filter_map(|(jet, &(on, since, came_on))| {
            let struck = came_on + jet.reach / JET_SPEED_MPS;
            // Struck only if it was still on when its front got there.
            let reached = on || struck <= since;
            let share = flash(t - struck);
            (reached && share > 0.0).then(|| {
                let way = jet.way.normalize_or(Vec3::Z);
                let at = jet.from + way * (jet.reach - 1.5) + jet.from.normalize_or(Vec3::Y) * 1.5;
                Lit {
                    at,
                    lumens: FLASH_LUMENS * share,
                    color: FLASH_LIGHT,
                    range: JET_RANGE_M * 1.5,
                    radius: 2.0,
                    glow: 1.0 + share,
                    kept: false,
                    night_stops: NIGHT_STOPS,
                    dusk_gain: 1.0,
                    casts: true,
                    still: None,
                }
            })
        })
        .collect()
}

/// A marched breath's fireball's light on the face it strikes (D430): its jet's lights hang
/// over the stream, the nearest most of a metre from the face, so the wall showed a small
/// patch of weak orange and the ground none (vol5's review), where the references' walls and
/// ground glow orange metres round the hit. It stands out from the face, so it spreads wide
/// and even rather than burning one patch past the tonemapper's shoulder, orange rather than
/// yellow, and grows in with the fireball over the 0.6 s after the stream strikes. Brighter
/// and nearer the face than at first (2e6, 3 m out), it is the pool the ground's light falls
/// off from, the stream's lights taking no night stop (vol10's review: a flat flood). Under
/// half that (at 3.2e6 the ground by the hit stood 5 times white, vol12's probe), and orange:
/// the film mask's shoulder keeps the hue, so the redder (1, 0.42, 0.1) that made up for its
/// running green out with red read coral on the wall, and (1, 0.5, 0.16) terracotta (fire/hue's
/// blind A/Bs).
const STRIKE_LUMENS: f32 = 1.5e6;
const STRIKE_LIGHT: Color = Color::srgb(1.0, 0.6, 0.2);
const STRIKE_OUT_M: f32 = 1.8;
const STRIKE_GROW_S: f32 = 0.6;

/// Each marched jet's fireball light while it burns: `jets` each one's state as `Drawn` keeps
/// it (on, since when, when it last came on).
pub(super) fn strikes(
    blazes: &Blazes,
    jets: &[(bool, f32, f32)],
    t: f32,
    tier: BreathTier,
) -> Vec<Lit> {
    if tier != BreathTier::Marched {
        return Vec::new();
    }
    blazes
        .jets
        .iter()
        .zip(jets)
        .filter_map(|(jet, &(on, since, _))| {
            let struck = since + jet.face / JET_SPEED_MPS;
            let grown = ((t - struck) / STRIKE_GROW_S).clamp(0.0, 1.0);
            (on && jet.face > 0.0 && grown > 0.0).then(|| {
                let way = jet.way.normalize_or(Vec3::Z);
                let hit = jet.from + way * jet.face;
                let normal = jet.normal.normalize_or(-way);
                let flicker = Flicker::of(0.3);
                let glow = signal(7.1, t, flicker);
                Lit {
                    at: hit + normal * STRIKE_OUT_M + hit.normalize_or(Vec3::Y) * 1.0,
                    lumens: STRIKE_LUMENS * grown * glow,
                    color: STRIKE_LIGHT,
                    range: JET_RANGE_M,
                    radius: 1.5,
                    glow,
                    kept: false,
                    night_stops: BREATH_NIGHT_STOPS,
                    dusk_gain: 1.0,
                    casts: true,
                    still: None,
                }
            })
        })
        .collect()
}

/// A breath's stream as three lights along it, at its start, middle and reach.
pub(super) fn jet_lights(blazes: &Blazes, t: f32, tier: BreathTier) -> Vec<Lit> {
    let (jet_lumens, jet_light) = jet_light(tier);
    // Marched, its stream's lights take no night stop, only its strike's: hung metres over the
    // ground, they lit it as one flat flood past the shoulder (vol10's review), where the
    // references' ground glows round the hit and falls off away from it (`strikes`).
    let jet_stops = match tier {
        BreathTier::Cards => NIGHT_STOPS,
        BreathTier::Marched => 0.0,
    };
    let up = |at: Vec3| at.normalize_or(Vec3::Y);
    blazes
        .jets
        .iter()
        .filter(|jet| jet.on)
        .enumerate()
        .flat_map(|(j, jet)| {
            let way = jet.way.normalize_or(Vec3::Z);
            let flicker = Flicker::of(0.3);
            [0.06, 0.4, 0.85].map(|along| {
                let at = jet.from + way * jet.reach * along;
                let seed = j as f32 * 0.37 + along;
                let glow = signal(seed, t, flicker);
                Lit {
                    at: at + up(at) * 1.5 + sway(seed, t, flicker, 0.3),
                    lumens: jet_lumens * glow,
                    color: jet_light,
                    night_stops: jet_stops,
                    dusk_gain: 1.0,
                    casts: true,
                    still: None,
                    range: JET_RANGE_M,
                    radius: 0.5,
                    glow,
                    kept: false,
                }
            })
        })
        .collect()
}
