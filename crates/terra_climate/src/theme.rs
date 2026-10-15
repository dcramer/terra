//! A world's climate as numbers (D149, D78): its base climate, how far each quantity swings
//! over the year, and its weather. A temperate world swings in temperature, a monsoon world
//! in rain, another barely at all; no code differs between them.
//!
//! Shares of the year are counted from midwinter, as [`Year`] counts them.

use crate::Year;
use serde::{Deserialize, Serialize};

/// A world's climate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Climate {
    pub year: Year,
    pub temperature: Temperatures,
    pub precipitation: Precipitation,
    pub wind: Winds,
    pub humidity: Humidity,
    pub fronts: Fronts,
}

/// The air's temperature, in degrees Celsius.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Temperatures {
    /// The year's mean at sea level on the equator and at the poles. Between, it falls with
    /// the square of the sine of the latitude, as Held and Suarez's idealised Earth does.
    pub equator_c: f32,
    pub pole_c: f32,
    /// How much colder the air is a kilometre higher: about 6.5 on Earth.
    pub lapse_c_per_km: f32,
    /// Half the difference between the warmest and the coldest time of year, at 45° of
    /// latitude; it grows with the sine of the latitude, so the equator has none.
    pub annual_swing_c: f32,
    /// How many days the warmest and coldest times come after midsummer and midwinter:
    /// about a month on Earth, as the ground and the sea take time to warm and cool.
    pub annual_lag_days: f32,
    /// Half the difference between a clear day's warmest, mid-afternoon, and its coldest,
    /// before dawn. Cloud halves it.
    pub daily_swing_c: f32,
    /// How far a warm or a cold spell moves the temperature, typically (a standard deviation).
    pub weather_swing_c: f32,
}

/// Rain and snow. Which of them falls is the temperature's doing, not the theme's.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Precipitation {
    /// The share of the time that something falls, on average over the year.
    pub share: f32,
    /// How far that share swings over the year, as a fraction of itself: 0 falls evenly,
    /// near 1 makes wet and dry seasons.
    pub annual_swing: f32,
    /// When it falls most, as a share of the year after midwinter.
    pub wettest: f32,
    /// How hard it falls at its heaviest, in millimetres of water an hour.
    pub heaviest_mm_per_hour: f32,
}

/// The wind.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Winds {
    /// Its usual speed, in metres a second.
    pub mean_m_s: f32,
    /// How far its speed swings with the weather, as a fraction of the usual speed
    /// (a standard deviation); it blows harder where something falls.
    pub gustiness: f32,
    /// Where it usually blows from, in degrees clockwise from north.
    pub prevailing_from_deg: f32,
    /// How far it turns from there with the weather, at most, in degrees.
    pub turning_deg: f32,
}

/// The wetness of the air, as relative humidity from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Humidity {
    /// Its mean over the year.
    pub mean: f32,
    /// How far it swings over the year.
    pub annual_swing: f32,
    /// When the air is most humid, as a share of the year after midwinter.
    pub most_humid: f32,
}

/// The weather's fronts: noise drifting across the world with the clock.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fronts {
    /// How far across a front is, in metres.
    pub size_m: f32,
    /// How many days a spell of weather lasts at a place, about: the time a front takes
    /// to pass.
    pub spell_days: f32,
}
