//! A plant kind's year (climate.md, D313): when its leaves come out, turn and fall, said in
//! the counts the past keeps (warmth and chill, `past`) and the day's length, as data. Real
//! plants keep these clocks: leaves unfold after so much warmth since midwinter (growing
//! degree-day models of budburst), and turn and drop after so much chill once the days
//! shorten (Delpierre et al. 2009's model of leaf colouring). An oak turns and drops, a pine
//! barely changes, grass browns in the cold and the drought and greens again.
//!
//! Plants do not tick (D314): a plant's leaves are worked out from its cell's past, its
//! kind's year and its own seed, when it is drawn or gathered. A plant's own seed moves it a
//! little along its kind's year, so a stand turns over days rather than all at once.

use crate::Past;
use serde::{Deserialize, Serialize};

/// A stretch of a count over which something happens: it starts once the count passes
/// `from` and is done `over` later.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ramp {
    pub from: f32,
    pub over: f32,
}

impl Ramp {
    /// How far along it is at a count, 0 to 1.
    pub fn at(&self, count: f32) -> f32 {
        ((count - self.from) / self.over.max(1e-3)).clamp(0.0, 1.0)
    }
}

/// When a kind's leaves come and go, as data per kind (`assets/data/plant_years.ron`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeafYear {
    /// Its name, as the kinds that grow know it.
    pub kind: String,
    /// Warmth, degree-days above 5 °C since midwinter, over which its leaves come out (or,
    /// for grass, green again).
    pub flush: Ramp,
    /// Chill, degree-days below 10 °C since midsummer, over which they turn their autumn
    /// colour, counted only once the day is shorter than `turns_under_hours`.
    pub turn: Ramp,
    /// Chill over which they fall.
    pub fall: Ramp,
    /// The day's length under which the chill counts toward turning and falling, all of it
    /// once the day is an hour shorter still.
    pub turns_under_hours: f32,
    /// How much of the leaves turn, and how much of them fall: all for an oak, little for a
    /// pine, which keeps its needles some years; grass browns but stays.
    pub turns: f32,
    pub drops: f32,
    /// For what browns in a drought: dryness of the ground (1 less its wetness) over which
    /// it does.
    #[serde(default)]
    pub wilts: Option<Ramp>,
    /// The colour its leaves turn, as sRGB: an oak's tawny brown, a birch's yellow, grass's
    /// straw. Only for drawing it.
    pub autumn: (f32, f32, f32),
    /// The leaf cards its leaves are drawn with (`assets/data/foliage.ron`), so a shader can
    /// tell its leaves from another kind's. Only for drawing it.
    #[serde(default)]
    pub cards: Vec<String>,
}

/// A plant's leaves at a moment.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Leaves {
    /// How much of its leaves it has, 0 bare to 1 in full leaf.
    pub on: f32,
    /// How far those it has have turned: 0 green, 1 in their autumn colour (or brown, for
    /// grass).
    pub turned: f32,
}

/// How much faster or slower than its kind a plant's own seed has it count, at most: a stand
/// turns over some days, the earliest counting a quarter faster and the latest a quarter
/// slower, so none is ever ahead of its kind before the count begins.
pub const SPREAD: f32 = 0.25;

impl LeafYear {
    /// A plant's leaves, from its cell's past, the day's length, and where its seed puts it,
    /// -1 late to 1 early in its kind's year.
    pub fn leaves(&self, past: &Past, day_hours: f32, seed: f32) -> Leaves {
        let pace = 1.0 + seed.clamp(-1.0, 1.0) * SPREAD;
        let warming = past.warming();
        // The chill counts as the days shorten past the kind's length, fully an hour on, so
        // the day it gets there does not jump: at the middle latitudes an hour of daylight
        // goes in some three weeks round the equinox. After midwinter what it took stays
        // taken as the days lengthen, until the warmth brings it back.
        let shortened = if warming {
            1.0
        } else {
            (self.turns_under_hours - day_hours).clamp(0.0, 1.0)
        };
        let chill = past.chill * shortened * pace;
        let fallen = self.drops * self.fall.at(chill);
        let mut turned = self.turns * self.turn.at(chill);
        let mut on = 1.0 - fallen;
        if warming {
            let flush = self.flush.at(past.warmth * pace);
            on = on.max(flush);
            turned = turned.min(1.0 - flush);
        }
        if let Some(wilts) = self.wilts {
            turned = turned.max(wilts.at((1.0 - past.ground_wetness()) * pace));
        }
        Leaves {
            on: on.clamp(0.0, 1.0),
            turned: turned.clamp(0.0, 1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::tests::temperate;
    use crate::{Fields, Place, past};
    use glam::Vec3;
    use terra_gen::Seed;

    /// A broadleaf that turns and drops all its leaves, and an evergreen that barely does.
    fn oak() -> LeafYear {
        LeafYear {
            kind: "oak".into(),
            flush: Ramp {
                from: 150.0,
                over: 120.0,
            },
            turn: Ramp {
                from: 20.0,
                over: 80.0,
            },
            fall: Ramp {
                from: 90.0,
                over: 120.0,
            },
            turns_under_hours: 12.0,
            turns: 1.0,
            drops: 1.0,
            wilts: None,
            autumn: (0.45, 0.28, 0.1),
            cards: Vec::new(),
        }
    }

    fn pine() -> LeafYear {
        LeafYear {
            turns: 0.1,
            drops: 0.1,
            kind: "pine".into(),
            ..oak()
        }
    }

    fn here() -> Place {
        Place {
            at: Vec3::new(300.0, 1900.0, 200.0),
            height_m: 40.0,
            latitude: 40.0_f32.to_radians(),
            sunward: 0.0,
            catch: 1.0,
        }
    }

    /// A kind's leaves through a year of the temperate world, a day at a time.
    fn through_a_year(kind: &LeafYear, seed: f32) -> Vec<Leaves> {
        let mut climate = temperate();
        climate.year.start = 0.0;
        let fields = Fields::new(climate, Seed(3));
        (0..365)
            .map(|day| {
                let hours = fields.daylight(here().latitude, day).day_hours;
                kind.leaves(&past(&fields, here(), day), hours, seed)
            })
            .collect()
    }

    #[test]
    fn an_oak_is_in_leaf_in_summer_bare_in_winter_and_turns_between() {
        let year = through_a_year(&oak(), 0.0);
        // Counted from midwinter: high summer is green and full, deep winter bare.
        let summer = year[200];
        assert_eq!(
            summer,
            Leaves {
                on: 1.0,
                turned: 0.0
            }
        );
        assert_eq!(year[20].on, 0.0, "bare in the coldest weeks");
        // Its leaves turn before they fall: on some day in autumn they are turned but on.
        let turned_on = (182..365).any(|d| year[d].turned > 0.5 && year[d].on > 0.9);
        assert!(turned_on, "it turns, then drops");
        // They come back in spring, green.
        let spring = (60..182).find(|d| year[*d].on > 0.99).expect("leafs out");
        assert!(year[spring].turned < 0.01);
        // Nothing jumps from one day to the next.
        for d in 1..365 {
            assert!((year[d].on - year[d - 1].on).abs() < 0.2, "day {d}");
            assert!((year[d].turned - year[d - 1].turned).abs() < 0.2, "day {d}");
        }
    }

    #[test]
    fn a_pine_barely_changes() {
        for leaves in through_a_year(&pine(), 0.0) {
            assert!(leaves.on >= 0.9 && leaves.turned <= 0.1, "{leaves:?}");
        }
    }

    #[test]
    fn an_early_plant_is_further_along_than_a_late_one_and_both_are_full_in_summer() {
        let (early, late) = (through_a_year(&oak(), 1.0), through_a_year(&oak(), -1.0));
        assert_eq!(early[200], late[200]);
        let autumn_early: f32 = (182..365).map(|d| early[d].on).sum();
        let autumn_late: f32 = (182..365).map(|d| late[d].on).sum();
        assert!(autumn_early < autumn_late, "the early one drops first");
        let spring_early: f32 = (0..182).map(|d| early[d].on).sum();
        let spring_late: f32 = (0..182).map(|d| late[d].on).sum();
        assert!(spring_early > spring_late, "and leafs out first");
    }

    #[test]
    fn nothing_turns_while_the_days_are_long_however_cold() {
        let chilled = Past {
            chill: 500.0,
            warmth: 2000.0,
            warmth_days: 200.0,
            chill_days: 30.0,
            ..Past::default()
        };
        assert_eq!(
            oak().leaves(&chilled, 14.0, 0.0),
            Leaves {
                on: 1.0,
                turned: 0.0
            }
        );
        assert_eq!(
            oak().leaves(&chilled, 10.0, 0.0),
            Leaves {
                on: 0.0,
                turned: 1.0
            }
        );
    }

    #[test]
    fn grass_browns_in_a_drought() {
        let grass = LeafYear {
            drops: 0.0,
            wilts: Some(Ramp {
                from: 0.7,
                over: 0.3,
            }),
            ..oak()
        };
        let summer = |ground_mm| Past {
            ground_mm,
            warmth: 1500.0,
            warmth_days: 200.0,
            chill_days: 20.0,
            ..Past::default()
        };
        let wet = grass.leaves(&summer(crate::GROUND_HOLDS_MM), 14.0, 0.0);
        let dry = grass.leaves(&summer(0.0), 14.0, 0.0);
        assert_eq!(wet.turned, 0.0);
        assert_eq!(dry.turned, 1.0);
        assert_eq!(dry.on, 1.0, "brown, not gone");
    }
}
