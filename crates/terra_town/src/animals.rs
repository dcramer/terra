//! A town's animals (docs/demos.md, a living town; D324): hens scratching about their yard,
//! goats penned behind a house, dogs at their owners' heels or lying by the door. Each kind
//! is data (`animals` in `assets/data/life.ron`): where it lives, how many, how it moves.
//! Like the people, they are the same on every machine, and nothing ticks far from every
//! viewer: there they are wherever their seed and the clock put them in their yard.

use crate::life::{Life, State};
use crate::town::{ThingKind, inside};
use glam::Vec2;
use serde::Deserialize;

/// Where a kind of animal lives.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum Lives {
    /// In a hen yard, loose.
    Yard,
    /// In a pen of low walls.
    Pen,
    /// With a household: at its owner's heels when they are out, else by its door.
    Follows,
}

/// A kind of animal, as data.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Kind {
    pub lives: Lives,
    /// How many to a yard or a pen, at least and at most; for dogs, how many in the town.
    pub count: (u32, u32),
    /// How fast it walks and runs, metres a second, and how wide it is.
    pub walk: f32,
    #[serde(default)]
    pub run: f32,
    pub radius: f32,
    /// How long it stands, pecks or grazes at a place before moving on, in seconds.
    pub rests: (f32, f32),
}

/// What an animal is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Doing {
    /// Going somewhere, at a run or not.
    Moving { run: bool },
    /// Standing, pecking or grazing, until a time.
    Resting { until: f32 },
}

/// One of the town's animals.
#[derive(Clone, Debug, PartialEq)]
pub struct Animal {
    /// Its kind, by its name in the data.
    pub kind: String,
    pub at: Vec2,
    pub facing: Vec2,
    pub speed: f32,
    pub doing: Doing,
    /// Where it lives: the yard or pen's outline, or for a dog the person it follows.
    home: Home,
    seed: u32,
    draws: u32,
    target: Vec2,
    /// For a dog, where its owner has been, the newest last, to follow round corners.
    trail: Vec<Vec2>,
}

#[derive(Clone, Debug, PartialEq)]
enum Home {
    Area(Vec<Vec2>),
    Owner { person: usize, door: Vec2 },
}

/// How much of a role's day is spent about the town's streets and market: its windows of
/// browsing, selling or chatting.
fn about_town(plan: &crate::plan::Plan, role: usize) -> usize {
    plan.roles.get(role).map_or(0, |role| {
        role.day
            .iter()
            .filter(|window| {
                window
                    .does
                    .iter()
                    .any(|does| matches!(does.as_str(), "Browse" | "Sell" | "Chat"))
            })
            .count()
    })
}

fn unit(parts: &[u32]) -> f32 {
    let mut x: u32 = 0x811c_9dc5;
    for &part in parts {
        x ^= part;
        x = terra_gen::hash::prospector32_round(x.wrapping_mul(0x0100_0193));
    }
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// A town's animals.
#[derive(Clone, Debug, Default)]
pub struct Animals {
    pub kinds: std::collections::BTreeMap<String, Kind>,
    pub animals: Vec<Animal>,
    seconds: f32,
}

impl Animals {
    /// The animals of a town and its people, from the kinds and a seed: hens in each hen yard,
    /// goats in each pen, dogs given to households.
    pub fn new(life: &Life, kinds: std::collections::BTreeMap<String, Kind>, seed: u32) -> Animals {
        let mut animals = Vec::new();
        for (name, kind) in &kinds {
            let count = |salt: u32| {
                kind.count.0
                    + (unit(&[seed, salt]) * (kind.count.1 - kind.count.0 + 1) as f32) as u32
            };
            match kind.lives {
                Lives::Yard | Lives::Pen => {
                    let wanted = if kind.lives == Lives::Yard {
                        ThingKind::Yard
                    } else {
                        ThingKind::Pen
                    };
                    for (index, thing) in life.town.things.iter().enumerate() {
                        if thing.kind != wanted {
                            continue;
                        }
                        let Some(quad) = thing
                            .building
                            .and_then(|b| life.town.yards.iter().find(|(home, _)| *home == b))
                            .map(|(_, quad)| *quad)
                        else {
                            continue;
                        };
                        // Inside its walls or fence, a little in from them.
                        let middle = quad.iter().copied().sum::<Vec2>() / 4.0;
                        let area: Vec<Vec2> = quad
                            .iter()
                            .map(|&corner| corner + (middle - corner).normalize_or_zero() * 1.0)
                            .collect();
                        for n in 0..count(index as u32) {
                            let s = (unit(&[seed, index as u32, n]) * u32::MAX as f32) as u32;
                            let at = point_in(&area, s, 0);
                            animals.push(Animal {
                                kind: name.clone(),
                                at,
                                facing: Vec2::X,
                                speed: 0.0,
                                doing: Doing::Resting { until: 0.0 },
                                home: Home::Area(area.clone()),
                                seed: s,
                                draws: 1,
                                target: at,
                                trail: Vec::new(),
                            });
                        }
                    }
                }
                Lives::Follows => {
                    // Given to households in turn, an adult who goes out a lot their owner.
                    let homes: Vec<usize> = life
                        .town
                        .things
                        .iter()
                        .enumerate()
                        .filter(|(_, t)| t.kind == ThingKind::Door)
                        .map(|(i, _)| i)
                        .collect();
                    for n in 0..count(7) as usize {
                        if homes.is_empty() {
                            break;
                        }
                        let door = homes[(unit(&[seed, 8, n as u32]) * homes.len() as f32)
                            as usize
                            % homes.len()];
                        // Of the household's grown-ups, the one most about the streets and the
                        // market: a dog goes where its owner goes.
                        let Some(person) = life
                            .people
                            .iter()
                            .enumerate()
                            .filter(|(_, p)| p.home == door && p.scale > 0.85)
                            .max_by_key(|(i, p)| (about_town(&life.plan, p.role), usize::MAX - i))
                            .map(|(i, _)| i)
                        else {
                            continue;
                        };
                        let at = life.town.things[door].place(Vec2::new(1.5, 1.2));
                        animals.push(Animal {
                            kind: name.clone(),
                            at,
                            facing: Vec2::X,
                            speed: 0.0,
                            doing: Doing::Resting { until: 0.0 },
                            home: Home::Owner { person, door: at },
                            seed: (unit(&[seed, 9, n as u32]) * u32::MAX as f32) as u32,
                            draws: 1,
                            target: at,
                            trail: Vec::new(),
                        });
                    }
                }
            }
        }
        Animals {
            kinds,
            animals,
            seconds: 0.0,
        }
    }

    /// Puts each dog at its owner's heels if its owner is out, as if the day had been going
    /// on since the night (`Life::start_settled`), rather than at its door to run after them.
    pub fn settle(&mut self, life: &Life) {
        for animal in &mut self.animals {
            let Home::Owner { person, .. } = animal.home else {
                continue;
            };
            let owner = &life.people[person];
            if !matches!(owner.state, State::Inside | State::Away) {
                animal.at = owner.at - owner.facing * 1.6;
                animal.target = animal.at;
                animal.facing = owner.facing;
                animal.trail.clear();
            }
        }
    }

    /// Every animal a step on, near a viewer; far from all of them, nothing moves.
    pub fn step(&mut self, life: &Life, step: f32, viewers: &[Vec2]) {
        self.seconds += step;
        let now = self.seconds;
        for animal in &mut self.animals {
            let near = viewers
                .iter()
                .any(|v| v.distance(animal.at) < crate::life::REDUCED_M);
            if !near {
                continue;
            }
            let Some(kind) = self.kinds.get(&animal.kind) else {
                continue;
            };
            match &animal.home {
                Home::Area(area) => {
                    let area = area.clone();
                    match animal.doing {
                        Doing::Resting { until } if now >= until => {
                            animal.draws += 1;
                            // Somewhere near in its yard, mostly; now and then across it.
                            let far = unit(&[animal.seed, animal.draws, 1]) < 0.2;
                            let reach = if far { 6.0 } else { 1.6 };
                            let mut target = animal.at;
                            for attempt in 0..6 {
                                let angle = unit(&[animal.seed, animal.draws, 2, attempt])
                                    * std::f32::consts::TAU;
                                let distance =
                                    (0.4 + unit(&[animal.seed, animal.draws, 3, attempt])) * reach;
                                let there = animal.at + Vec2::from_angle(angle) * distance;
                                if inside(there, &area) {
                                    target = there;
                                    break;
                                }
                            }
                            animal.target = target;
                            animal.doing = Doing::Moving { run: false };
                        }
                        Doing::Resting { .. } => {
                            animal.speed = 0.0;
                        }
                        Doing::Moving { .. } => {
                            if walk_to(animal, animal.target, kind.walk, step, 0.08) {
                                animal.draws += 1;
                                let rest = kind.rests.0
                                    + unit(&[animal.seed, animal.draws, 4])
                                        * (kind.rests.1 - kind.rests.0);
                                animal.doing = Doing::Resting { until: now + rest };
                            }
                        }
                    }
                }
                Home::Owner { person, door } => {
                    let (person, door) = (*person, *door);
                    let owner = &life.people[person];
                    let out = !matches!(owner.state, State::Inside | State::Away);
                    if out {
                        // At heel: along the trail the owner walked, a couple of metres behind.
                        if animal
                            .trail
                            .last()
                            .is_none_or(|last| last.distance(owner.at) > 0.8)
                        {
                            animal.trail.push(owner.at);
                            if animal.trail.len() > 64 {
                                animal.trail.remove(0);
                            }
                        }
                        let behind = owner.at - owner.facing * 1.6;
                        let to_owner = animal.at.distance(owner.at);
                        // The oldest place on the trail it has not reached yet.
                        while animal.trail.len() > 1 && animal.trail[0].distance(animal.at) < 1.0 {
                            animal.trail.remove(0);
                        }
                        let target = if to_owner < 4.0 {
                            behind
                        } else {
                            animal.trail.first().copied().unwrap_or(behind)
                        };
                        let run = to_owner > 7.0;
                        let speed = if run {
                            kind.run.max(kind.walk)
                        } else {
                            kind.walk.max(owner.velocity.length())
                        };
                        // Sat, it stays sat until its owner is well off or walking away, and
                        // sits only once at heel with its owner still: one test for both
                        // flicked it between sitting and standing frame to frame while its
                        // owner shuffled at their work (the settlement film's pen, D381).
                        let sat = matches!(animal.doing, Doing::Resting { .. });
                        let (near, still) = if sat { (3.5, 0.6) } else { (2.2, 0.2) };
                        if to_owner < near && owner.velocity.length() < still {
                            animal.speed = 0.0;
                            animal.doing = Doing::Resting { until: now + 1.0 };
                        } else {
                            animal.doing = Doing::Moving { run };
                            // Stood where it was going, it waits there until the place it
                            // wants is a stride off, rather than stepping after every
                            // shuffle of its owner's and standing again a frame later.
                            let stood = animal.speed == 0.0 && animal.at.distance(target) < 0.9;
                            if !stood {
                                walk_to(animal, target, speed, step, 0.3);
                            }
                        }
                    } else {
                        // Its owner indoors: back to the door, and lie there.
                        animal.trail.clear();
                        if walk_to(animal, door, kind.walk, step, 0.3) {
                            animal.doing = Doing::Resting { until: now + 1.0 };
                        } else {
                            animal.doing = Doing::Moving { run: false };
                        }
                    }
                }
            }
        }
    }
}

/// A point inside an area from a seed, by trying a few.
fn point_in(area: &[Vec2], seed: u32, salt: u32) -> Vec2 {
    let low = area.iter().copied().reduce(Vec2::min).unwrap_or_default();
    let high = area.iter().copied().reduce(Vec2::max).unwrap_or_default();
    for attempt in 0..16 {
        let p = low
            + (high - low)
                * Vec2::new(
                    unit(&[seed, salt, attempt, 1]),
                    unit(&[seed, salt, attempt, 2]),
                );
        if inside(p, area) {
            return p;
        }
    }
    area.iter().copied().sum::<Vec2>() / area.len().max(1) as f32
}

/// An animal a step toward a place at a speed; whether it is there.
fn walk_to(animal: &mut Animal, target: Vec2, speed: f32, step: f32, near: f32) -> bool {
    let toward = target - animal.at;
    let distance = toward.length();
    if distance <= near {
        animal.speed = 0.0;
        return true;
    }
    let way = toward / distance;
    let moved = (speed * step).min(distance);
    animal.at += way * moved;
    animal.speed = moved / step.max(1e-6);
    let turn = 1.0 - libm::expf(-step * 6.0);
    animal.facing = animal.facing.lerp(way, turn).normalize_or(way);
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_drawn_inside_an_area_is_inside_it() {
        let area = [
            Vec2::new(0.0, 0.0),
            Vec2::new(4.0, 0.0),
            Vec2::new(4.0, 3.0),
            Vec2::new(0.0, 3.0),
        ];
        for seed in 0..50 {
            assert!(inside(point_in(&area, seed, 0), &area));
        }
    }
}
