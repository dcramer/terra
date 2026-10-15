//! Climate (docs/climate.md, D312 to D314): the quantities a season and the weather are
//! made of, as fields of a place and a time, and what the weather of the days before leaves.
//!
//! - **The clock** is days, [`WorldTime`]; the server sends it and nothing else (D52).
//! - **A world's climate** is numbers, [`Climate`]: its year, its base temperature and how
//!   far each quantity swings, and its fronts. No season is ever named; "winter" is a label
//!   for the cold part of a temperate year, read off the quantities.
//! - **The fields**, [`Fields`]: temperature, daylight, precipitation (rain or snow by the
//!   temperature), wind and the air's wetness at a [`Place`] and a moment, from the
//!   climate and the world's seed. The weather is fronts of noise drifting with the clock.
//! - **The past**, [`past`] and [`PastCache`]: snow, wet ground, ice, and the warmth and
//!   chill plants count, folded per [`Cell`] over the days before.
//! - **Plants' years**, [`LeafYear`]: when a kind's leaves come out, turn and fall, by the
//!   counts, as data; a plant's [`Leaves`] are worked out, never ticked.
//! - **Bodies**, [`body`]: how warm the surroundings keep a body (D153), and how snow and
//!   mud slow a walker.
//!
//! Pure and repeatable: no engine, no clock read, no system randomness, `libm` for its
//! sines and exponentials (D28, D30), so every machine works out the same climate.

pub mod body;
mod fields;
mod past;
mod plants;
mod sun;
mod theme;
mod time;

pub use fields::{Conditions, Day, Daylight, Fields, Place, Wind, snow_share};
pub use past::{
    CELL_M, CHILL_BASE_C, Cell, GROUND_HOLDS_MM, Past, PastCache, WARMTH_BASE_C, fold_start, past,
    step,
};
pub use plants::{LeafYear, Leaves, Ramp, SPREAD};
pub use sun::{day_length_hours, declination, sun_height, sunlight};
pub use theme::{Climate, Fronts, Humidity, Precipitation, Temperatures, Winds};
pub use time::{HOURS, WorldTime, Year};
