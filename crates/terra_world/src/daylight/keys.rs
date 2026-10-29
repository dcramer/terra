//! The world's light keyed by the sun's height (`daylight`): the keys and the sun's colour
//! above the air (D212).

use super::Light;
use crate::scene::EXPOSURE_EV100;
use bevy::prelude::*;

/// The light at a few heights of the sun, in degrees, from deep night to high noon. The
/// light between is blended. At 20 degrees it is the look the art was made in. By night
/// most light is the sky's, which has no direction, so the moon's shadows are faint.
///
/// The sun's light is as it is above the air: the air (`air`) dims and warms it on its
/// way down, the more the lower the sun, and below the horizon it lights only the sky,
/// as twilight. There it hands over to the moon, which fades in as the sky darkens. Deep
/// night's moon and ambient are half what they were before fire round 10 (median sRGB, share
/// under 20, against RDR2's and Sons of the Forest's campfire frames' 2-27 and 59-88%): the
/// yard's 6 m campfire view 34/28% -> 18/54%, the town's street 24/37% -> 11/82%. At a
/// quarter (round 10) the yard was 10/74% but the town's street went black (4/92%) and the
/// lake's shore and hills to silhouettes (fire round 11). At 0.6 of that (fire round 23), with
/// the campfire's light two and a half stops up by night, the yard's field out of its reach is
/// half as bright (sRGB 14 against 26) and the fire sits in a pool of its own light (blind
/// 5.5/10 against 4.5). Its ambient at a third (fire round 25), that field is sRGB 4.4, not 14:
/// the sky lit the grass to the frame's edge (blind 6/10 against 5).
///
/// The sun's colour is the same at every height, as above the air (D212): what reaches the
/// ground, the sky and the clouds is reddened by the air alone, as Bevy's atmosphere gives
/// it on every surface and in its sky, and `air::sun_through` for the rest. Keys warmed
/// under 20 degrees toward a black body's 2,200 K (settlement r21) reddened every one of
/// them twice, and the tundra's sky at a 10-degree sun was brown, blue with the light white
/// (sky job 1). Each low key's sun stays as bright on the ground as those warm keys left it
/// (`held`), so the dusk's exposure holds and only the doubled reddening goes.
pub(super) fn keys() -> [(f32, Light); 7] {
    let v = Vec3::new;
    let moon = v(0.62, 0.72, 1.0);
    // The sun through the air at 0, 6, 12 and 20 degrees: about 2,200, 3,000 and 3,800 K, and
    // the art's near white (settlement r21).
    let (w0, w6, w12, w20) = (
        v(1.0, 0.6, 0.34),
        v(1.0, 0.69, 0.44),
        v(1.0, 0.77, 0.55),
        v(1.0, 0.96, 0.9),
    );
    [
        (
            -18.0,
            Light {
                light: moon,
                warm: moon,
                illuminance: 210.0,
                horizon: v(0.07, 0.09, 0.17),
                zenith: v(0.015, 0.02, 0.06),
                glow: v(0.25, 0.3, 0.45),
                shade: v(0.42, 0.52, 0.9),
                ambient: 30.0,
                sky: 1.6,
                night: 1.0,
                exposure: 9.0,
                colour: 1.0,
            },
        ),
        (
            -6.0,
            Light {
                light: moon,
                warm: moon,
                illuminance: 125.0,
                // The blue hour's sky, nearly as bright on screen as the sunset's: the camera
                // opens only a stop through the civil twilight and the land's light halves,
                // so the land shows about as bright as at sunset, and the sky that lights it
                // must too (D432). At (0.32, 0.27, 0.42) and (0.07, 0.09, 0.22) it fell six
                // times as fast as the land, and the settlement film's sky 3 degrees under
                // was sRGB 4-9 over a meadow of 39: night, not dusk (settlement r31).
                horizon: v(0.7, 0.6, 0.8),
                zenith: v(0.15, 0.23, 0.5),
                glow: v(0.85, 0.45, 0.35),
                shade: v(0.48, 0.52, 0.88),
                ambient: 600.0,
                sky: 1.6,
                night: 0.6,
                exposure: 9.0,
                colour: 0.8,
            },
        ),
        (
            0.0,
            Light {
                light: ABOVE_AIR,
                illuminance: LOW_SUN_LUX * held(w0),
                warm: w0,
                horizon: v(0.95, 0.55, 0.38),
                zenith: v(0.16, 0.24, 0.52),
                glow: v(1.0, 0.45, 0.22),
                shade: v(0.8, 0.66, 0.86),
                ambient: 1_150.0,
                sky: 1.7,
                night: 0.0,
                exposure: 10.0,
                colour: 0.74,
            },
        ),
        (
            6.0,
            Light {
                light: ABOVE_AIR,
                illuminance: LOW_SUN_LUX * held(w6),
                warm: w6,
                horizon: v(1.0, 0.64, 0.42),
                zenith: v(0.2, 0.36, 0.72),
                glow: v(1.0, 0.6, 0.32),
                shade: v(0.76, 0.68, 0.92),
                ambient: 1_550.0,
                sky: 1.6,
                night: 0.0,
                exposure: 10.4,
                colour: 0.74,
            },
        ),
        (
            12.0,
            Light {
                light: ABOVE_AIR,
                illuminance: LOW_SUN_LUX * held(w12),
                warm: w12,
                horizon: v(1.0, 0.72, 0.5),
                zenith: v(0.21, 0.4, 0.78),
                glow: v(1.0, 0.6, 0.32),
                shade: v(0.72, 0.72, 0.96),
                ambient: 1_660.0,
                sky: 1.6,
                night: 0.0,
                exposure: 10.4 + (EXPOSURE_EV100 - 10.4) * 0.43,
                colour: 0.84,
            },
        ),
        (
            20.0,
            Light {
                light: ABOVE_AIR,
                illuminance: LOW_SUN_LUX * held(w20),
                warm: w20,
                horizon: v(0.92, 0.86, 0.78),
                zenith: v(0.22, 0.44, 0.84),
                glow: v(1.0, 0.74, 0.46),
                shade: v(0.66, 0.76, 1.0),
                ambient: 1_800.0,
                sky: 1.6,
                night: 0.0,
                exposure: EXPOSURE_EV100,
                colour: 1.0,
            },
        ),
        (
            60.0,
            Light {
                light: ABOVE_AIR,
                warm: ABOVE_AIR,
                illuminance: HIGH_SUN_LUX,
                horizon: v(0.86, 0.9, 0.96),
                zenith: v(0.2, 0.44, 0.86),
                glow: v(1.0, 0.92, 0.8),
                shade: v(0.68, 0.78, 1.0),
                ambient: 2_000.0,
                sky: 1.6,
                night: 0.0,
                exposure: EXPOSURE_EV100 + 1.0,
                colour: 1.0,
            },
        ),
    ]
}

/// The sun's colour above the air, sRGB: a little warm of white, as the high sun's key was.
const ABOVE_AIR: Vec3 = Vec3::new(1.0, 0.98, 0.95);

/// The share of the above-air colour's brightness (Rec. 709 luminance, linear) a warm key's
/// colour had: its sun that much dimmer leaves the ground as bright as the warm key did.
fn held(warm: Vec3) -> f32 {
    let luminance = |c: Vec3| {
        let c = Color::srgb(c.x, c.y, c.z).to_linear();
        0.2126 * c.red + 0.7152 * c.green + 0.0722 * c.blue
    };
    luminance(warm) / luminance(ABOVE_AIR)
}

/// The low sun's illuminance above the air, lux, before `held` dims it for the ground.
const LOW_SUN_LUX: f32 = 30_000.0;
/// The high sun's, from the 60-degree key up, which is not held.
const HIGH_SUN_LUX: f32 = 32_000.0;
/// Between these heights of the sun, degrees, the air's sun goes from the held one to the
/// one above the air (`air_sun_gain`).
const AIR_SUN_FROM: f32 = 4.0;
const AIR_SUN_TO: f32 = 12.0;

/// How much brighter than the world's sun, `lux` with the sun `height` degrees up, is the sun
/// the air scatters into its sky, its haze and the environment map drawn from them (D212).
/// `held` dims the low sun on the ground, which also takes the painted sky light on top;
/// Bevy's atmosphere draws its sky in proportion to the same dimmed sun, so at a 12-degree
/// sun the tundra's shaded snow stood brighter than the open sky over it (sky sRGB 0.38
/// against the photographs' 0.78, tundra round 11). The air takes the sun above the air
/// instead from AIR_SUN_TO degrees up, eased from the held one at AIR_SUN_FROM, so the
/// sunset's sky the dusks were made under (town, fire, settlement) keeps its brightness.
pub(super) fn air_sun_gain(height: f32, lux: f32) -> f32 {
    let above =
        LOW_SUN_LUX + (HIGH_SUN_LUX - LOW_SUN_LUX) * ((height - 20.0) / 40.0).clamp(0.0, 1.0);
    let unheld = (above / lux.max(1.0)).max(1.0);
    1.0 + (unheld - 1.0) * terra_gen::smoothstep(AIR_SUN_FROM, AIR_SUN_TO, height)
}
