//! What the climate does to a body (D153, climate.md): how warm it is kept or chilled, and how
//! the ground under it slows it. Shared (D312): every machine works out the same body
//! temperature from the same surroundings.
//!
//! **Warmth** is a heat balance, as thermal comfort models make it (Fanger's, ISO 7730's):
//! the body makes heat by working; the sun and a fire add to it; it loses heat from its
//! skin to the air through its clothing and the still air round it, which the wind thins.
//! Within a band the body holds itself at its normal temperature, shivering or sweating;
//! past what those can make up, its temperature drifts toward where the surroundings push
//! it, the farther the faster. Clothing keeps the heat in, more so dry.
//!
//! **Going**: deep snow and wet mud slow a walker.

/// A body's temperature when nothing troubles it, in degrees Celsius.
pub const NORMAL_C: f32 = 37.0;
/// Within this band nothing happens; beyond it, the body suffers (climate.md's table).
pub const COMFORT_C: (f32, f32) = (36.0, 38.0);
/// Past these it is hypothermia or heatstroke, and health falls.
pub const DANGER_C: (f32, f32) = (35.0, 39.5);

/// The skin's temperature, which the heat is lost from.
const SKIN_C: f32 = 34.0;
/// Heat passed to the surroundings by radiation, and by convection in still air, in watts
/// a square metre of skin for each degree, and how the wind's speed raises the convection
/// (ISO 7730's forced convection, 8.3 v^0.6).
const RADIATIVE: f32 = 4.7;
const STILL_AIR: f32 = 3.1;
const FORCED: f32 = 8.3;
/// A clo of clothing's insulation, in square metres kelvin a watt.
const CLO: f32 = 0.155;
/// Soaked clothing keeps only so much of its warmth.
const WET_KEEPS: f32 = 0.5;
/// What wet skin loses besides, drying, in watts a square metre.
const WET_LOSS: f32 = 30.0;
/// The share of the sunlight on level ground, and of a fire's heat on the side it faces, a
/// standing body takes in: its outline against the sun, its skin's and clothes' darkness.
const SUN_TAKEN: f32 = 0.12;
const FIRE_TAKEN: f32 = 0.35;
/// What a body makes at rest, in watts a square metre of skin: one met.
pub const MET: f32 = 58.0;
/// How much of an imbalance shivering and sweating make up, in watts a square metre.
const SHIVERS: f32 = 100.0;
const SWEATS: f32 = 220.0;
/// How far the body's temperature is pushed for each watt a square metre that is not made
/// up, and how far it can be pushed at all.
const PUSH_C: f32 = 0.03;
const PUSHED_C: (f32, f32) = (20.0, 44.0);
/// How long, in seconds of play, the body takes to go most of the way to where it is pushed:
/// a cold night out takes minutes to tell, not seconds (D134's half-hour day).
const DRIFT_S: f32 = 300.0;

/// What a body stands in: the air, the wind, the sun, what falls on it, a fire's heat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surroundings {
    pub air_c: f32,
    pub wind_m_s: f32,
    /// The sunlight on level ground where it stands, after what shades it, in watts a
    /// square metre.
    pub sun_w_m2: f32,
    /// How wet it is, 0 dry to 1 soaked.
    pub wet: f32,
    /// A fire's heat on the side facing it, in watts a square metre.
    pub fire_w_m2: f32,
}

/// What a body's heat comes to, in watts a square metre of skin, at its normal temperature:
/// what it makes (`work_met`, in mets) and takes in, less what it loses through clothing of
/// `clo`. Above nothing it warms, below it cools.
pub fn balance(around: &Surroundings, clo: f32, work_met: f32) -> f32 {
    let wet = around.wet.clamp(0.0, 1.0);
    let wind = around.wind_m_s.max(0.0);
    let convection = STILL_AIR.max(FORCED * libm::powf(wind, 0.6));
    let air = 1.0 / (RADIATIVE + convection);
    let clothes = CLO * clo.max(0.0) * (1.0 - (1.0 - WET_KEEPS) * wet);
    let lost = (SKIN_C - around.air_c) / (clothes + air) + WET_LOSS * wet;
    MET * work_met + SUN_TAKEN * around.sun_w_m2.max(0.0) + FIRE_TAKEN * around.fire_w_m2.max(0.0)
        - lost
}

/// The temperature a heat balance pushes a body toward: its normal one while shivering or
/// sweating can make the balance up, and away from it with what they cannot.
pub fn pushed_to(balance: f32) -> f32 {
    let unmet = (balance + SHIVERS).min(0.0) + (balance - SWEATS).max(0.0);
    (NORMAL_C + PUSH_C * unmet).clamp(PUSHED_C.0, PUSHED_C.1)
}

/// How hard the body is working to hold its temperature, -1 shivering all it can to 1
/// sweating all it can: what makes it hungry or thirsty (climate.md).
pub fn effort(balance: f32) -> f32 {
    if balance < 0.0 {
        (balance / SHIVERS).max(-1.0)
    } else {
        (balance / SWEATS).min(1.0)
    }
}

/// A body's temperature after `seconds`, drifting toward where it is pushed.
pub fn drift(body_c: f32, pushed_c: f32, seconds: f32) -> f32 {
    let kept = libm::expf(-seconds.max(0.0) / DRIFT_S);
    pushed_c + (body_c - pushed_c) * kept
}

/// How far out of danger a body is, in degrees: below nothing it is too cold, above too
/// hot; within the band, nothing.
pub fn danger(body_c: f32) -> f32 {
    (body_c - DANGER_C.0).min(0.0) + (body_c - DANGER_C.1).max(0.0)
}

/// How fast a walker can go over ground with so much snow lying and so wet, as a share of
/// its pace on dry, bare ground: a foot of snow halves it, as a walker without snowshoes
/// finds; mud slows it by a sixth.
pub fn going(snow_depth_m: f32, ground_wetness: f32) -> f32 {
    let snow = 1.0 / (1.0 + snow_depth_m.max(0.0) / 0.3);
    let mud = 1.0 - 0.16 * ((ground_wetness - 0.7) / 0.3).clamp(0.0, 1.0);
    (snow * mud).max(0.35)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(air_c: f32, wind_m_s: f32) -> Surroundings {
        Surroundings {
            air_c,
            wind_m_s,
            sun_w_m2: 0.0,
            wet: 0.0,
            fire_w_m2: 0.0,
        }
    }

    /// Where a body ends up after a long time, from its normal temperature.
    fn settles(around: Surroundings, clo: f32, work_met: f32) -> f32 {
        let mut body = NORMAL_C;
        for _ in 0..600 {
            body = drift(body, pushed_to(balance(&around, clo, work_met)), 10.0);
        }
        body
    }

    #[test]
    fn a_mild_day_troubles_no_one() {
        assert_eq!(settles(out(20.0, 2.0), 1.0, 1.0), NORMAL_C);
        // Lightly dressed on a warm day, or with nothing on in a warm room.
        assert_eq!(settles(out(28.0, 0.0), 0.0, 1.0), NORMAL_C);
    }

    #[test]
    fn a_freezing_wind_chills_a_body_with_nothing_on_into_danger() {
        let body = settles(out(-10.0, 5.0), 0.0, 1.0);
        assert!(danger(body) < -5.0, "{body}");
        // The wind is what makes it worse: still air at the same cold loses less.
        assert!(balance(&out(-10.0, 5.0), 0.0, 1.0) < balance(&out(-10.0, 0.0), 0.0, 1.0));
    }

    #[test]
    fn clothing_keeps_the_cold_out_and_less_so_wet() {
        let cold = out(-8.0, 3.0);
        let thin = settles(cold, 0.6, 1.0);
        let warm = settles(cold, 2.5, 1.0);
        assert!(
            danger(thin) < 0.0,
            "a shirt does not do in the winter: {thin}"
        );
        assert_eq!(warm, NORMAL_C, "a coat and furs do");
        let soaked = Surroundings { wet: 1.0, ..cold };
        assert!(balance(&soaked, 2.5, 1.0) < balance(&cold, 2.5, 1.0));
        // And clothing slows the cold: after a minute out, less is lost through more.
        let after = |clo| drift(NORMAL_C, pushed_to(balance(&cold, clo, 1.0)), 60.0);
        assert!(after(1.2) > after(0.6));
    }

    #[test]
    fn a_fire_and_work_warm_and_the_sun_does() {
        let night = out(-5.0, 1.0);
        let cold = balance(&night, 0.6, 1.0);
        let by_fire = Surroundings {
            fire_w_m2: 250.0,
            ..night
        };
        assert!(balance(&by_fire, 0.6, 1.0) > cold + 50.0);
        assert!(balance(&night, 0.6, 3.0) > cold + 100.0);
        let sunny = Surroundings {
            sun_w_m2: 800.0,
            ..night
        };
        assert!(balance(&sunny, 0.6, 1.0) > cold);
        assert_eq!(settles(by_fire, 0.6, 1.0), NORMAL_C);
    }

    #[test]
    fn hard_work_in_the_heat_of_the_sun_overheats() {
        let noon = Surroundings {
            sun_w_m2: 900.0,
            ..out(36.0, 0.5)
        };
        assert!(danger(settles(noon, 0.6, 4.0)) > 0.0);
        assert_eq!(
            settles(noon, 0.6, 1.0),
            NORMAL_C,
            "resting, sweating makes it up"
        );
    }

    #[test]
    fn the_body_drifts_toward_where_it_is_pushed_and_never_past() {
        let mut body = NORMAL_C;
        let pushed = 30.0;
        for _ in 0..100 {
            let next = drift(body, pushed, 5.0);
            assert!(next < body && next > pushed);
            body = next;
        }
        assert!((drift(30.5, 37.0, 1e6) - 37.0).abs() < 1e-4);
        assert_eq!(drift(36.0, 36.0, 10.0), 36.0);
    }

    #[test]
    fn shivering_and_sweating_say_how_hard_the_body_works() {
        assert_eq!(effort(0.0), 0.0);
        assert!(effort(-50.0) < 0.0 && effort(-50.0) > -1.0);
        assert_eq!(effort(-500.0), -1.0);
        assert_eq!(effort(500.0), 1.0);
    }

    #[test]
    fn deep_snow_and_mud_slow_a_walker() {
        assert_eq!(going(0.0, 0.3), 1.0);
        assert!(
            (going(0.3, 0.0) - 0.5).abs() < 1e-6,
            "a foot of it halves it"
        );
        assert!(going(0.05, 0.0) > going(0.2, 0.0));
        assert!(going(0.0, 1.0) < 0.9 && going(0.0, 1.0) > 0.8);
        assert!(going(5.0, 1.0) >= 0.35, "never to a standstill");
    }
}
