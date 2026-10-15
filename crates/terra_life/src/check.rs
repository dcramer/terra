//! The life check (D326): a town's day watched step by step for what would break the look of
//! people going about it, and counted for how bustling it is.
//!
//! **It fails on** anyone held in one place for more than [`STUCK_S`] while walking; anyone
//! standing in a wall or a thing; two people inside each other for more than a moment;
//! feet sliding, a walker moving slower or faster than the walk or the run can be played at
//! for more than a moment; and anyone more than [`LATE_MIN`] game minutes late for what
//! their day says. **It counts**, each hour: who is out, walking, on the square, and at each
//! activity.

use crate::life::{Arrival, JOG_PACE, Life, State, WALK_PACE};
use glam::Vec2;
use std::collections::BTreeMap;
use terra_build::town::inside;

/// How long a walker may make no headway, in seconds.
pub const STUCK_S: f32 = 5.0;
/// How late anyone may be for a window of their day, in game minutes.
pub const LATE_MIN: f32 = 5.0;
/// How long someone on their way may stand before they read as idling without a purpose.
pub const AIMLESS_S: f32 = 1.5;
/// How long a moment is: two people overlapping, or feet sliding, for less is not counted.
const MOMENT_S: f32 = 0.5;
/// The fastest a moving clip plays against its pace (`pace.rs`); slower, a person's walk
/// is played at the speed they go, down to standing.
const FASTEST: f32 = 1.6;

/// What went wrong, by kind, each with who, when (in hours) and where.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Faults {
    /// Who, when, where, and what they were going to and where it was.
    pub stuck: Vec<(usize, f64, Vec2, String)>,
    pub in_walls: Vec<(usize, f64, Vec2)>,
    pub through: Vec<(usize, usize, f64, Vec2)>,
    pub sliding: Vec<(usize, f64, Vec2)>,
    pub late: Vec<Arrival>,
}

impl Faults {
    pub fn count(&self) -> usize {
        self.stuck.len()
            + self.in_walls.len()
            + self.through.len()
            + self.sliding.len()
            + self.late.len()
    }
}

/// An hour's counts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hour {
    pub hour: i64,
    /// Averaged over the hour's steps.
    pub out: f32,
    pub walking: f32,
    pub on_square: f32,
    /// Out of doors with nowhere they are seen going: on the way somewhere but standing in it
    /// for more than [`AIMLESS_S`], held up by others, which reads as idling without a
    /// purpose. Everyone else is walking to a place or doing something at it, facing it.
    pub aimless: f32,
    /// Who was at each activity, averaged.
    pub doing: BTreeMap<String, f32>,
}

/// Watches a town's life a step at a time.
#[derive(Clone, Debug, Default)]
pub struct Watch {
    pub faults: Faults,
    pub hours: Vec<Hour>,
    /// Per person: seconds held in one place while walking and where from, seconds sliding,
    /// whether already counted.
    held: Vec<(f32, Vec2, bool)>,
    sliding: Vec<(f32, bool)>,
    /// Per pair touching: seconds, whether counted.
    touching: BTreeMap<(usize, usize), (f32, bool)>,
    in_wall: Vec<bool>,
    steps_this_hour: u32,
    arrivals_seen: usize,
}

impl Watch {
    /// Looks at the town after a step of `step` seconds, at `hours` on the clock.
    pub fn see(&mut self, life: &Life, hours: f64, step: f32) {
        let count = life.people.len();
        if self.held.len() != count {
            self.held = life.people.iter().map(|p| (0.0, p.at, false)).collect();
            self.sliding = vec![(0.0, false); count];
            self.in_wall = vec![false; count];
        }
        let out: Vec<bool> = life
            .people
            .iter()
            .map(|p| !matches!(p.state, State::Inside | State::Away))
            .collect();
        // Everything built and every thing, each shrunk a little, so brushing one is not
        // standing in it.
        let shrunk = |quad: &[Vec2; 4]| {
            let middle = (quad[0] + quad[1] + quad[2] + quad[3]) * 0.25;
            quad.map(|corner| corner + (middle - corner).normalize_or_zero() * 0.1)
        };
        let solids: Vec<[Vec2; 4]> = life
            .town
            .footprints
            .iter()
            .flatten()
            .copied()
            .chain(life.town.things.iter().filter_map(|t| t.outline()))
            .map(|quad| shrunk(&quad))
            .collect();
        for (index, person) in life.people.iter().enumerate() {
            // Held in place while walking.
            let walking = matches!(person.state, State::Walking { .. });
            let (held, from, counted) = &mut self.held[index];
            if !walking || person.at.distance(*from) > 0.5 {
                *held = 0.0;
                *from = person.at;
                *counted = false;
            } else {
                *held += step;
                if *held > STUCK_S && !*counted {
                    *counted = true;
                    let going = match life.going_to(index) {
                        Some((what, to)) => format!("going to {what} at {:.1}, {:.1}", to.x, to.y),
                        None => "going nowhere".to_string(),
                    };
                    self.faults.stuck.push((index, hours, person.at, going));
                }
            }
            // Feet: the clip played slower or faster than it can be.
            let speed = person.velocity.length();
            let (slide, counted) = &mut self.sliding[index];
            let pace = match person.state {
                State::Walking { run: true } => JOG_PACE * person.scale,
                _ => WALK_PACE * person.scale,
            };
            // Moving while at a spot, or faster than the walk or run is played.
            let wrong = (!walking && speed > 0.05) || (walking && speed > pace * FASTEST);
            if wrong {
                *slide += step;
                if *slide > MOMENT_S * 2.0 && !*counted {
                    *counted = true;
                    self.faults.sliding.push((index, hours, person.at));
                }
            } else {
                *slide = 0.0;
                *counted = false;
            }
            // In a wall or a thing.
            if out[index] {
                let walled = solids.iter().any(|quad| inside(person.at, quad));
                if walled && !self.in_wall[index] {
                    self.faults.in_walls.push((index, hours, person.at));
                }
                self.in_wall[index] = walled;
            }
        }
        // Inside each other.
        let mut now: BTreeMap<(usize, usize), (f32, bool)> = BTreeMap::new();
        for a in 0..count {
            if !out[a] {
                continue;
            }
            for (b, &seen) in out.iter().enumerate().skip(a + 1) {
                if !seen {
                    continue;
                }
                let (pa, pb) = (&life.people[a], &life.people[b]);
                if pa.at.distance(pb.at) < (pa.radius + pb.radius) * 0.6 {
                    let (mut seconds, mut counted) =
                        self.touching.get(&(a, b)).copied().unwrap_or((0.0, false));
                    seconds += step;
                    if seconds > MOMENT_S && !counted {
                        counted = true;
                        self.faults.through.push((a, b, hours, pa.at));
                    }
                    now.insert((a, b), (seconds, counted));
                }
            }
        }
        self.touching = now;
        // Late.
        for arrival in &life.record.arrivals[self.arrivals_seen..] {
            if arrival.late > LATE_MIN {
                self.faults.late.push(*arrival);
            }
        }
        self.arrivals_seen = life.record.arrivals.len();
        // This hour's counts.
        let hour = libm::floor(hours) as i64;
        if self.hours.last().is_none_or(|h| h.hour != hour) {
            self.finish_hour();
            self.hours.push(Hour {
                hour,
                ..Hour::default()
            });
        }
        self.steps_this_hour += 1;
        let square = &life.town.square.outline;
        let Some(current) = self.hours.last_mut() else {
            return;
        };
        for (index, person) in life.people.iter().enumerate() {
            if !out[index] {
                continue;
            }
            current.out += 1.0;
            if matches!(person.state, State::Walking { .. }) {
                current.walking += 1.0;
            }
            if inside(person.at, square) {
                current.on_square += 1.0;
            }
            if matches!(person.state, State::Walking { .. }) && self.held[index].0 > AIMLESS_S {
                current.aimless += 1.0;
            }
            if let State::Doing { .. } = person.state
                && let Some(activity) = life.activity(index)
            {
                *current.doing.entry(activity.to_string()).or_default() += 1.0;
            }
        }
    }

    /// Turns the last hour's sums into averages.
    fn finish_hour(&mut self) {
        let steps = self.steps_this_hour.max(1) as f32;
        if let Some(hour) = self.hours.last_mut() {
            hour.out /= steps;
            hour.walking /= steps;
            hour.on_square /= steps;
            hour.aimless /= steps;
            for value in hour.doing.values_mut() {
                *value /= steps;
            }
        }
        self.steps_this_hour = 0;
    }

    /// Done watching: the last hour averaged.
    pub fn finish(&mut self) {
        self.finish_hour();
    }
}
