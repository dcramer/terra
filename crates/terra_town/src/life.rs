//! A town's people through their days (D324): who lives where and does what, by role; each
//! choosing a free place for what the hour asks, walking there on the path graph by A* and
//! out of everyone's way by ORCA, setting off early enough to be there on time; and anyone
//! far from every viewer not ticked at all, placed from the clock when someone comes near,
//! as plants are of their cell (D47, D313) and creatures by distance (D145).
//!
//! Time is the world's clock in hours since it began (a day is 24), and a step is real
//! seconds: people walk in seconds, and their days run in hours, `seconds_an_hour` apart.

use crate::graph::Graph;
use crate::orca::{self, Body, Horizons, Mover, Walls};
use crate::plan::{Count, Plan};
use crate::town::{self, On, ThingKind, Town};
use glam::Vec2;

mod route;

/// How near the nearest viewer someone is ticked in full, and at all (D145's distances).
pub const FULL_M: f32 = 64.0;
pub const REDUCED_M: f32 = 256.0;
/// How long before it is wanted a town's day is begun, when it is started settled.
pub const SETTLE_H: f64 = 8.0;
/// How often someone ticked at reduced detail thinks, in steps; they still move every step.
const THINK_EVERY: u64 = 8;
/// A body's radius on the ground, an adult's.
const RADIUS_M: f32 = 0.3;
/// How near a place on a route counts as reached, and the last place, the spot itself.
const PASSED_M: f32 = 0.7;
const ARRIVED_M: f32 = 0.2;
/// How near a door or a gate counts as through it.
const THRESHOLD_M: f32 = 0.9;
/// The walk's and jog's paces on the base body (m/s, `terra_motion::gaits`) and a walker's
/// spread off them (D585).
pub const WALK_PACE: f32 = terra_motion::gaits::WALK;
pub const JOG_PACE: f32 = terra_motion::gaits::JOG;
pub const GAIT_SPREAD: f32 = 0.08;

/// How detailed someone is kept, by how near the nearest viewer is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    Full,
    Reduced,
    /// Not ticked: where they are is worked out from the clock when someone comes near.
    Far,
}

/// What someone is doing now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    /// Indoors at home.
    Inside,
    /// Out of the town, past a gate.
    Away,
    /// On the way to a spot, at a run or not.
    Walking { run: bool },
    /// At a spot, doing an activity, until an hour.
    Doing { activity: usize, until: f64 },
}

/// What a body shows: nothing, a walk or a run at a speed, or an activity's clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shows<'a> {
    Hidden,
    Walk(f32),
    Run(f32),
    Clip(&'a str),
}

/// A place by a thing where something is done.
#[derive(Clone, Debug, PartialEq)]
pub struct Spot {
    pub thing: usize,
    pub at: Vec2,
    pub facing: Vec2,
    /// The activities done there, by index.
    pub does: Vec<usize>,
    pub shared: bool,
    /// Who is there or on the way.
    pub taken: Option<usize>,
    /// The graph's node someone arrives by, before the last steps to it.
    pub node: u32,
    /// Whether it can be gone to: a building site's only while its building goes up
    /// ([`Life::set_open`]).
    pub open: bool,
}

/// One of the town's people.
#[derive(Clone, Debug, PartialEq)]
pub struct Person {
    pub role: usize,
    /// Their home's door, a thing.
    pub home: usize,
    /// The thing that is theirs to work, if any: a stall, a forge.
    pub owns: Option<usize>,
    pub seed: u32,
    /// How tall against the base body: a child is smaller and steps shorter.
    pub scale: f32,
    pub radius: f32,
    /// Their walking speed, and running.
    pub walk: f32,
    pub run: f32,
    pub at: Vec2,
    pub velocity: Vec2,
    pub facing: Vec2,
    pub state: State,
    pub detail: Detail,
    /// The spot they are at or going to, and the one they were at before.
    pub spot: Option<usize>,
    last_spot: Option<usize>,
    /// The activity they are going to do there.
    going: Option<usize>,
    /// The window of their day this is for: its start, in hours.
    pub window: f64,
    /// When they set off for the window, if they set off for it.
    late_counted: bool,
    pub route: Vec<Vec2>,
    pub next: usize,
    /// How long they have made no headway, in seconds, and where from.
    held: f32,
    held_at: Vec2,
    /// A count of choices made, so each comes out the same on every machine.
    draws: u32,
    /// Where they will go next and when they will set off.
    planned: Option<Planned>,
    /// When they set off on their walk, and how long its route takes at their speed.
    set_off: f64,
    route_s: f32,
    /// Across the street from its middle, as each walks a way of their own.
    lane: f32,
}

/// The next window of someone's day as planned: its start, the activity and spot, when to
/// set off for it, and from where that was worked out.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Planned {
    window: f64,
    activity: usize,
    spot: usize,
    leave: f64,
    from: Vec2,
}

/// How many hours before a window someone plans for it: more than the longest walk across a
/// town takes, at a child's pace.
const PLAN_AHEAD_H: f64 = 4.0;
/// The least of a window worth walking to: shorter, it is passed over.
const MIN_STAY_H: f64 = 10.0 / 60.0;

/// How far apart two things' spots must be, so no one stands where another does.
const SPOTS_APART_M: f32 = 0.9;
/// How far a way between nodes keeps from where someone stands at a spot.
const SPOT_CLEAR_M: f32 = 0.7;
/// How long someone held up waits before squeezing past.
const SQUEEZE_S: f32 = 2.0;
/// How long before they edge past those standing about, too.
const EDGE_S: f32 = 2.0;
/// How often someone held up is routed again from where they are.
const REROUTE_S: f32 = 3.0;
/// Slower than this, a walker stands: the body plays a walk only above it, at the speed the
/// ground passes (`pace.rs`), so its feet do not slide.
pub const STANDING_MPS: f32 = 0.12;

/// How long a walk from a point along a route is.
fn polyline(from: Vec2, route: &[Vec2]) -> f32 {
    let mut last = from;
    let mut length = 0.0;
    for &point in route {
        length += last.distance(point);
        last = point;
    }
    length
}

/// Someone arriving for a window of their day.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arrival {
    pub person: usize,
    /// The window's start, in hours.
    pub window: f64,
    /// How many game minutes after it they got there; less than nothing if early.
    pub late: f32,
    /// When they set off, in hours; how long the walk took, and how long its route at their
    /// own speed would, in seconds.
    pub set_off: f64,
    pub walked_s: f32,
    pub route_s: f32,
}

/// How a run of a town's life went: what a check reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    /// Each arrival for a window of someone's day: who, the window's start in hours, and how
    /// many game minutes after it they got there (less than nothing if early).
    pub arrivals: Vec<Arrival>,
}

/// A town and its people.
#[derive(Clone, Debug)]
pub struct Life {
    pub town: Town,
    pub plan: Plan,
    /// The activities, by index: their names as the plan's.
    pub activities: Vec<String>,
    /// Each role's day: from each hour, the activities done, by index.
    days: Vec<Vec<(f32, Vec<usize>)>>,
    pub spots: Vec<Spot>,
    pub people: Vec<Person>,
    graph: Graph,
    walls: Walls,
    clear: Clear,
    seconds_an_hour: f32,
    steps: u64,
    /// When the town was started, from the clock: windows planned before it are not judged.
    started: f64,
    pub record: Record,
    /// Bodies from outside the town's life, players and the camera, that its walkers go
    /// round as they go round each other, set before each step.
    pub visitors: Vec<Body>,
}

/// Everything walkers go round, on a grid, so whether a straight walk is clear is asked of
/// the few outlines near it rather than every one in the town.
#[derive(Clone, Debug, Default)]
struct Clear {
    quads: Vec<[Vec2; 4]>,
    grid: std::collections::BTreeMap<(i32, i32), Vec<usize>>,
}

const CLEAR_CELL_M: f32 = 8.0;

impl Clear {
    fn new(town: &Town) -> Clear {
        let quads: Vec<[Vec2; 4]> = town
            .footprints
            .iter()
            .flatten()
            .copied()
            .chain(town.things.iter().filter_map(|t| t.outline()))
            .collect();
        let mut grid: std::collections::BTreeMap<(i32, i32), Vec<usize>> = Default::default();
        for (index, quad) in quads.iter().enumerate() {
            let low = quad.iter().copied().reduce(Vec2::min).unwrap_or(Vec2::ZERO) - town::CLEAR_M;
            let high = quad.iter().copied().reduce(Vec2::max).unwrap_or(Vec2::ZERO) + town::CLEAR_M;
            for x in cell(low).0..=cell(high).0 {
                for y in cell(low).1..=cell(high).1 {
                    grid.entry((x, y)).or_default().push(index);
                }
            }
        }
        Clear { quads, grid }
    }

    /// Whether a body of a radius can stand at a point, clear of everything.
    fn room(&self, at: Vec2, radius: f32) -> bool {
        let here = cell(at);
        (here.0 - 1..=here.0 + 1).all(|x| {
            (here.1 - 1..=here.1 + 1).all(|y| {
                self.grid
                    .get(&(x, y))
                    .into_iter()
                    .flatten()
                    .all(|&index| town::segment_polygon(at, at, &self.quads[index]) >= radius)
            })
        })
    }

    /// Whether a straight walk keeps clear of everything built and every thing, as the
    /// town's own paths do (`Town::walkable`).
    fn walkable(&self, a: Vec2, b: Vec2) -> bool {
        let (low, high) = (cell(a.min(b)), cell(a.max(b)));
        for x in low.0..=high.0 {
            for y in low.1..=high.1 {
                for &index in self.grid.get(&(x, y)).into_iter().flatten() {
                    if town::segment_polygon(a, b, &self.quads[index]) < town::CLEAR_M {
                        return false;
                    }
                }
            }
        }
        true
    }
}

fn cell(at: Vec2) -> (i32, i32) {
    (
        libm::floorf(at.x / CLEAR_CELL_M) as i32,
        libm::floorf(at.y / CLEAR_CELL_M) as i32,
    )
}

/// A repeatable draw from a seed and some numbers, 0 to 1.
fn unit(parts: &[u32]) -> f32 {
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    for &part in parts {
        state ^= part as u64;
        state = terra_gen::hash::splitmix64(state.wrapping_add(0x9e37_79b9_7f4a_7c15));
    }
    (state >> 40) as f32 / (1u64 << 24) as f32
}

impl Life {
    /// A town's people, from its plan and a seed: households in every home, each person a
    /// role, the things owned given out.
    pub fn new(town: Town, plan: Plan, seed: u32, seconds_an_hour: f32) -> Life {
        let activities: Vec<String> = plan.activities.keys().cloned().collect();
        let index = |name: &str| activities.iter().position(|a| a == name);
        let days = plan
            .roles
            .iter()
            .map(|role| {
                role.day
                    .iter()
                    .map(|window| {
                        (
                            window.from,
                            window.does.iter().filter_map(|a| index(a)).collect(),
                        )
                    })
                    .collect()
            })
            .collect();
        let graph = Graph::new(
            town.paths.nodes.iter().map(|(at, _)| *at).collect(),
            &town.paths.edges,
        );
        let mut walls = Walls::default();
        for quad in town.footprints.iter().flatten() {
            walls.add(quad);
        }
        for thing in &town.things {
            if let Some(ground) = thing.outline() {
                walls.add(&ground);
            }
        }
        let mut life = Life {
            spots: Vec::new(),
            people: Vec::new(),
            activities,
            days,
            graph,
            walls,
            clear: Clear::new(&town),
            seconds_an_hour,
            steps: 0,
            started: f64::NEG_INFINITY,
            record: Record::default(),
            visitors: Vec::new(),
            plan,
            town,
        };
        life.spots = life.make_spots();
        // No way runs past where someone stands at a spot, between them and what they are
        // at: walks go round the spots, and the spots are come to again by what is left.
        let edges: Vec<[u32; 2]> = life
            .town
            .paths
            .edges
            .iter()
            .copied()
            .filter(|&[a, b]| {
                let (pa, pb) = (life.graph.nodes[a as usize], life.graph.nodes[b as usize]);
                life.spots.iter().all(|spot| {
                    spot.shared
                        || spot.node == a
                        || spot.node == b
                        || town::point_segment(spot.at, pa, pb) >= SPOT_CLEAR_M
                })
            })
            .collect();
        life.graph = Graph::new(life.graph.nodes.clone(), &edges);
        life.spots = life.make_spots();
        life.people = life.make_people(seed);
        life
    }

    /// Every spot round every thing, as the plan says, each with the node it is come to by.
    fn make_spots(&self) -> Vec<Spot> {
        let main: Vec<bool> = (0..self.graph.nodes.len() as u32)
            .map(|n| self.graph.in_main(n))
            .collect();
        // Every spot the plan puts round every thing, but none on top of another's, or
        // without room to stand.
        let mut spots: Vec<Spot> = Vec::new();
        for (index, thing) in self.town.things.iter().enumerate() {
            let Some(plans) = self.plan.things.get(thing.kind.name()) else {
                continue;
            };
            let stand = self.town.paths.nodes[thing.node as usize].0;
            for plan in plans {
                let local = Vec2::new(plan.at.0, plan.at.1);
                let at = if plan.from_stand {
                    stand + town::place(Vec2::ZERO, thing.facing, local)
                } else {
                    thing.place(local)
                };
                let facing = match plan.face {
                    Some((x, y)) => town::place(Vec2::ZERO, thing.facing, Vec2::new(x, y)),
                    None => thing.at - at,
                }
                .normalize_or(thing.front());
                let crowded = spots
                    .iter()
                    .any(|other| other.thing != index && other.at.distance(at) < SPOTS_APART_M);
                if crowded || !self.clear.room(at, RADIUS_M + 0.1) {
                    continue;
                }
                spots.push(Spot {
                    thing: index,
                    at,
                    facing,
                    does: plan
                        .does
                        .iter()
                        .filter_map(|a| self.activities.iter().position(|b| b == a))
                        .collect(),
                    shared: plan.shared,
                    taken: None,
                    node: u32::MAX,
                    open: true,
                });
            }
        }
        // Each arrived at from a node the last steps are clear from and that no one standing
        // at another spot stands on: the thing's own nodes first, then the nearest.
        let held: Vec<Vec2> = spots.iter().filter(|s| !s.shared).map(|s| s.at).collect();
        for spot in &mut spots {
            let at = spot.at;
            let usable =
                |node: usize| main[node] && self.clear.walkable(self.graph.nodes[node], at);
            let free = |node: usize| {
                let point = self.graph.nodes[node];
                held.iter().all(|&other| {
                    other.distance(at) < 1e-3 || other.distance(point) >= SPOTS_APART_M
                })
            };
            let mut own: Vec<usize> = (0..self.graph.nodes.len())
                .filter(|&node| {
                    self.town.paths.nodes[node].1 == On::Thing(spot.thing) && usable(node)
                })
                .collect();
            own.sort_by(|&x, &y| {
                self.graph.nodes[x]
                    .distance(at)
                    .total_cmp(&self.graph.nodes[y].distance(at))
            });
            let near = self
                .graph
                .nearest_of(at, 24, |n| usable(n as usize) && free(n as usize))
                .map(|n| n as usize);
            let node = own
                .iter()
                .copied()
                .find(|&node| free(node))
                .or(near)
                .or(own.first().copied())
                .or_else(|| {
                    self.graph
                        .nearest_of(at, 24, |n| usable(n as usize))
                        .map(|n| n as usize)
                });
            if let Some(node) = node {
                spot.node = node as u32;
            }
        }
        spots.retain(|spot| spot.node != u32::MAX);
        spots
    }

    /// The things an activity done at one's own can be done at: one's own, and one's home's.
    fn own_things(&self, person: &Person) -> Vec<usize> {
        let home_building = self.town.things[person.home].building;
        self.town
            .things
            .iter()
            .enumerate()
            .filter(|(index, thing)| {
                Some(*index) == person.owns
                    || (thing.building.is_some()
                        && thing.building == home_building
                        && thing.kind != ThingKind::Door)
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// The town's people: households of the plan's size in every home, given roles in the
    /// plan's order, those owning a thing first; someone whose role works at their own yard
    /// living at a home with one.
    fn make_people(&self, seed: u32) -> Vec<Person> {
        let doors: Vec<usize> = self
            .town
            .things
            .iter()
            .enumerate()
            .filter(|(_, t)| t.kind == ThingKind::Door)
            .map(|(i, _)| i)
            .collect();
        if doors.is_empty() {
            return Vec::new();
        }
        let (fewest, most) = self.plan.household;
        let sizes: Vec<u32> = doors
            .iter()
            .enumerate()
            .map(|(i, _)| fewest + (unit(&[seed, 1, i as u32]) * (most - fewest + 1) as f32) as u32)
            .map(|size| size.min(most))
            .collect();
        let total: u32 = sizes.iter().sum();
        let mut living = vec![0u32; doors.len()];
        let mut people = Vec::new();
        let mut left = total as i64;
        for (role_index, role) in self.plan.roles.iter().enumerate() {
            let owned: Vec<usize> = match &role.count {
                Count::Per(kind) => self
                    .town
                    .things
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.kind.name() == kind)
                    .map(|(i, _)| i)
                    .collect(),
                _ => Vec::new(),
            };
            let count = match &role.count {
                Count::Per(_) => owned.len() as i64,
                Count::Fixed(n) => *n as i64,
                Count::Share(share) => (total as f32 * share).round() as i64,
                Count::Rest => left,
            }
            .min(left)
            .max(0) as usize;
            // Kinds their own work needs at home.
            let needs: Vec<usize> = self.days[role_index]
                .iter()
                .flat_map(|(_, does)| does.iter().copied())
                .filter(|&a| self.plan.activities[&self.activities[a]].own)
                .collect();
            for n in 0..count {
                let owns = owned.get(n).copied();
                let draft = Person {
                    role: role_index,
                    home: doors[0],
                    owns,
                    seed: unit(&[seed, 2, people.len() as u32]).to_bits(),
                    scale: 1.0,
                    radius: RADIUS_M,
                    walk: 1.0,
                    run: 2.0,
                    at: Vec2::ZERO,
                    velocity: Vec2::ZERO,
                    facing: Vec2::Y,
                    state: State::Inside,
                    detail: Detail::Far,
                    spot: None,
                    last_spot: None,
                    going: None,
                    window: f64::NEG_INFINITY,
                    late_counted: true,
                    route: Vec::new(),
                    next: 0,
                    held: 0.0,
                    held_at: Vec2::ZERO,
                    draws: 0,
                    planned: None,
                    set_off: 0.0,
                    route_s: 0.0,
                    lane: 0.0,
                };
                // A home with room, the emptiest first, one with what their work needs where
                // it has to be theirs.
                let serves = |home: usize| {
                    let person = Person {
                        home: doors[home],
                        ..draft.clone()
                    };
                    let own = self.own_things(&person);
                    needs.iter().all(|&activity| {
                        self.spots
                            .iter()
                            .any(|spot| own.contains(&spot.thing) && spot.does.contains(&activity))
                    })
                };
                let pick = (0..doors.len())
                    .filter(|&h| living[h] < sizes[h])
                    .min_by_key(|&h| (!serves(h), living[h], h))
                    .or_else(|| (0..doors.len()).min_by_key(|&h| (living[h], h)));
                let Some(home) = pick else {
                    continue;
                };
                living[home] += 1;
                let id = people.len() as u32;
                let child = role.child;
                let scale = if child {
                    0.62 + unit(&[seed, 3, id]) * 0.12
                } else {
                    0.94 + unit(&[seed, 3, id]) * 0.12
                };
                let spread = |n: u32| 1.0 + (unit(&[seed, n, id]) * 2.0 - 1.0) * GAIT_SPREAD;
                people.push(Person {
                    home: doors[home],
                    scale,
                    radius: RADIUS_M * if child { 0.8 } else { 1.0 },
                    walk: WALK_PACE * scale * spread(4),
                    run: JOG_PACE * scale * spread(5),
                    lane: (unit(&[seed, 6, id]) - 0.5) * 1.8,
                    ..draft
                });
                left -= 1;
            }
        }
        people
    }

    /// The window of a role's day an hour of the clock is in: its start and end in hours,
    /// and its activities.
    fn window_at(&self, role: usize, hours: f64) -> (f64, f64, &[usize]) {
        let day = &self.days[role];
        let midnight = libm::floor(hours / 24.0) * 24.0;
        let hour = (hours - midnight) as f32;
        let index = day
            .iter()
            .rposition(|(from, _)| *from <= hour)
            .unwrap_or(day.len() - 1);
        let start = if day[index].0 <= hour {
            midnight + day[index].0 as f64
        } else {
            midnight - 24.0 + day[index].0 as f64
        };
        let (next_from, next_day) = match day.get(index + 1) {
            Some((from, _)) => (*from as f64, 0.0),
            None => (day[0].0 as f64, 24.0),
        };
        let end = libm::floor(start / 24.0) * 24.0 + next_from + next_day;
        (start, end, &day[index].1)
    }

    /// The window someone is in: the clock's, or the next if they have set off for it early.
    fn window_of(&self, person: usize, hours: f64) -> (f64, f64, Vec<usize>) {
        let p = &self.people[person];
        let (start, end, does) = self.window_at(p.role, hours);
        if p.window > start + 1e-9 {
            let (start, end, does) = self.window_at(p.role, p.window + 1e-6);
            return (start, end, does.to_vec());
        }
        (start, end, does.to_vec())
    }

    /// How many hours a walk of so many metres takes someone, with a margin for the crowd.
    fn hours_to_walk(&self, person: &Person, metres: f32) -> f64 {
        let seconds = metres / person.walk * 1.5 + 30.0;
        seconds as f64 / self.seconds_an_hour as f64
    }

    /// A choice for someone, 0 to 1, the same on every machine.
    fn draw(&mut self, person: usize) -> f32 {
        let p = &mut self.people[person];
        p.draws += 1;
        unit(&[p.seed, p.draws])
    }

    /// Picks a spot for an activity: a free one (or a shared one) that does it, theirs if it
    /// must be, the nearer likelier; one they can walk to in the hours `within` gives if there
    /// is any.
    fn choose_spot(
        &mut self,
        person: usize,
        activity: usize,
        within: Option<f64>,
        strict: bool,
    ) -> Option<usize> {
        let p = &self.people[person];
        let rules = &self.plan.activities[&self.activities[activity]];
        let own = rules.own.then(|| self.own_things(p));
        let home_has_spot = self
            .spots
            .iter()
            .any(|spot| spot.thing == p.home && spot.does.contains(&activity));
        let candidates: Vec<(usize, f32, f32)> = self
            .spots
            .iter()
            .enumerate()
            .filter(|(_, spot)| spot.open && spot.does.contains(&activity))
            .filter(|(i, spot)| {
                spot.shared
                    || spot.taken.is_none()
                    || spot.taken == Some(person)
                    || p.spot == Some(*i)
            })
            .filter(|(_, spot)| own.as_ref().is_none_or(|own| own.contains(&spot.thing)))
            .filter(|(_, spot)| !rules.inside || spot.thing == p.home || !home_has_spot)
            .map(|(i, spot)| {
                let distance = spot.at.distance(p.at);
                // Work that comes and goes is taken nearest first, as a builder goes on to
                // the next site along rather than across the town.
                let falloff = if rules.first { 6.0 } else { 30.0 };
                (
                    i,
                    1.0 / (1.0 + distance / falloff).powi(4 - 2 * usize::from(!rules.first) as i32),
                    distance,
                )
            })
            .collect();
        // Those near enough to get to in time, if any are.
        let reach = |distance: f32| {
            within.is_none_or(|hours| self.hours_to_walk(p, distance * 1.3) <= hours)
        };
        let candidates: Vec<(usize, f32)> = if candidates.iter().any(|c| reach(c.2)) {
            candidates
                .iter()
                .filter(|c| reach(c.2))
                .map(|c| (c.0, c.1))
                .collect()
        } else if strict {
            return None;
        } else {
            candidates.iter().map(|c| (c.0, c.1)).collect()
        };
        let total: f32 = candidates.iter().map(|c| c.1).sum();
        if total <= 0.0 {
            return None;
        }
        let mut roll = self.draw(person) * total;
        for (index, weight) in &candidates {
            roll -= weight;
            if roll <= 0.0 {
                return Some(*index);
            }
        }
        candidates.last().map(|c| c.0)
    }

    /// The first of a window's activities, in an order drawn for the person, that has a spot
    /// for them; their home if none does.
    fn choose(
        &mut self,
        person: usize,
        does: &[usize],
        within: Option<f64>,
    ) -> Option<(usize, usize)> {
        let mut order: Vec<usize> = does.to_vec();
        let first = (self.draw(person) * order.len() as f32) as usize % order.len().max(1);
        order.rotate_left(first);
        // Work that comes and goes, before the rest, whenever there is some.
        order.sort_by_key(|&a| !self.plan.activities[&self.activities[a]].first);
        // One that can be got to in time first, then any.
        for strict in [true, false] {
            for &activity in &order {
                if let Some(spot) = self.choose_spot(person, activity, within, strict) {
                    return Some((activity, spot));
                }
            }
        }
        let home = self
            .activities
            .iter()
            .position(|a| self.plan.activities[a].inside)?;
        self.choose_spot(person, home, within, false)
            .map(|spot| (home, spot))
    }

    /// Sends someone to a spot for an activity, from wherever they are.
    fn send(&mut self, person: usize, activity: usize, spot: usize, window: f64, hours: f64) {
        self.release(person);
        let rules = &self.plan.activities[&self.activities[activity]];
        let run = rules.run;
        if !self.spots[spot].shared {
            self.spots[spot].taken = Some(person);
        }
        // Out of a door or a gate first, if they were indoors or away.
        let p = &self.people[person];
        let out_from = match p.state {
            State::Inside => Some(self.spot_of(p.home)),
            State::Away => Some(self.nearest_gate(p.at)),
            _ => None,
        };
        if let Some(Some(out)) = out_from {
            let at = self.spots[out].at;
            self.people[person].at = at;
        }
        let route = self.route_to(&self.people[person], spot);
        let p = &mut self.people[person];
        if p.window != window {
            p.late_counted = false;
        }
        p.set_off = hours;
        p.route_s = polyline(p.at, &route) / p.walk;
        p.window = window;
        p.spot = Some(spot);
        p.going = Some(activity);
        p.route = route;
        p.next = 0;
        p.held = 0.0;
        p.held_at = p.at;
        p.state = State::Walking { run };
    }

    /// The spot at a thing that is entered: a door's, a gate's.
    fn spot_of(&self, thing: usize) -> Option<usize> {
        self.spots.iter().position(|s| s.thing == thing && s.shared)
    }

    fn nearest_gate(&self, at: Vec2) -> Option<usize> {
        self.spots
            .iter()
            .enumerate()
            .filter(|(_, s)| self.town.things[s.thing].kind == ThingKind::Gate)
            .min_by(|a, b| a.1.at.distance(at).total_cmp(&b.1.at.distance(at)))
            .map(|(i, _)| i)
    }

    /// Gives up whatever spot someone held.
    fn release(&mut self, person: usize) {
        self.people[person].last_spot = self.people[person].spot;
        if let Some(spot) = self.people[person].spot.take()
            && self.spots[spot].taken == Some(person)
        {
            self.spots[spot].taken = None;
        }
    }

    /// Opens or closes a thing's spots, as a building site is open while its building goes up
    /// (D381). Closed, whoever is there or on the way stops and thinks again, and no plan
    /// keeps it; opened, whoever's hour would have them at it but is busy at something else
    /// thinks again now rather than when that is done, as builders go to a new site.
    pub fn set_open(&mut self, thing: usize, open: bool, hours: f64) {
        let spots: Vec<usize> = (0..self.spots.len())
            .filter(|&i| self.spots[i].thing == thing && self.spots[i].open != open)
            .collect();
        for &i in &spots {
            self.spots[i].open = open;
        }
        for person in 0..self.people.len() {
            let p = &self.people[person];
            let rethink = if open {
                let does = self.window_of(person, hours).2;
                match p.state {
                    State::Doing { activity, .. } => spots.iter().any(|&i| {
                        !self.spots[i].does.contains(&activity)
                            && self.spots[i].does.iter().any(|a| does.contains(a))
                    }),
                    _ => false,
                }
            } else {
                p.spot.is_some_and(|i| spots.contains(&i))
            };
            if !open && p.planned.is_some_and(|plan| spots.contains(&plan.spot)) {
                self.people[person].planned = None;
            }
            if !rethink {
                continue;
            }
            let p = &mut self.people[person];
            let activity = match p.state {
                State::Doing { activity, .. } => Some(activity),
                _ => p.going,
            };
            if let Some(activity) = activity {
                p.state = State::Doing {
                    activity,
                    until: hours,
                };
            }
            if !open {
                self.release(person);
            }
        }
    }

    /// Everyone placed where the clock says they are, as for someone come into view.
    /// The town at `hours` as if its day had been going on: begun [`SETTLE_H`] before and run
    /// on to then, a tenth of a second a step, a viewer at `viewer` (the square's middle sees
    /// the whole town within [`REDUCED_M`]). Started cold, everyone stands at a spot for the
    /// hour's whole stay and the same choice holds them there, so for hours hardly anyone
    /// walks: at 08:30, 5 walking of 146 started cold, 12 in a day run on from midnight, as
    /// many when settled from 00:30 (it costs about a second, once).
    pub fn start_settled(&mut self, hours: f64, viewer: Vec2) {
        let from = hours - SETTLE_H;
        self.start(from);
        let step = 0.1f32;
        let steps = (SETTLE_H * self.seconds_an_hour as f64 / step as f64) as u64;
        for n in 0..steps {
            let at = from + n as f64 * step as f64 / self.seconds_an_hour as f64;
            self.step(at, step, &[viewer]);
        }
    }

    pub fn start(&mut self, hours: f64) {
        self.started = hours;
        for person in 0..self.people.len() {
            self.place(person, hours);
        }
    }

    /// Where someone is by the clock alone, with nothing ticked: at a spot for what the hour
    /// asks of their role, indoors, or away (D324), as someone far off is worked out when a
    /// viewer comes near.
    pub fn place(&mut self, person: usize, hours: f64) {
        self.release(person);
        let role = self.people[person].role;
        let (start, end, does) = self.window_at(role, hours);
        let does = does.to_vec();
        // The same choice for the same window, however often it is asked.
        let seed = self.people[person].seed;
        self.people[person].draws = unit(&[seed, start.to_bits() as u32]).to_bits() & 0xffff;
        let Some((activity, spot)) = self.choose(person, &does, None) else {
            return;
        };
        let rules = self.plan.activities[&self.activities[activity]].clone();
        let p = &mut self.people[person];
        p.window = start;
        p.late_counted = true;
        p.route.clear();
        p.velocity = Vec2::ZERO;
        p.going = None;
        p.at = self.spots[spot].at;
        p.facing = self.spots[spot].facing;
        if rules.inside {
            p.state = State::Inside;
        } else if rules.away {
            p.state = State::Away;
        } else {
            let stay = rules.stay.0
                + unit(&[seed, 7, start.to_bits() as u32]) * (rules.stay.1 - rules.stay.0);
            p.state = State::Doing {
                activity,
                until: (hours + stay as f64 / 60.0).min(end),
            };
            if !self.spots[spot].shared {
                self.spots[spot].taken = Some(person);
            }
            self.people[person].spot = Some(spot);
        }
    }

    /// The town a step on: everyone's detail from the viewers, those near thinking about
    /// what the hour asks, and everyone walking moved out of each other's way.
    pub fn step(&mut self, hours: f64, step: f32, viewers: &[Vec2]) {
        self.steps += 1;
        for person in 0..self.people.len() {
            let at = self.people[person].at;
            let nearest = viewers
                .iter()
                .map(|v| v.distance(at))
                .fold(f32::MAX, f32::min);
            let detail = if nearest <= FULL_M {
                Detail::Full
            } else if nearest <= REDUCED_M {
                Detail::Reduced
            } else {
                Detail::Far
            };
            let was = self.people[person].detail;
            self.people[person].detail = detail;
            if detail == Detail::Far {
                continue;
            }
            if was == Detail::Far {
                self.place(person, hours);
            }
            let thinks =
                detail == Detail::Full || (self.steps + person as u64).is_multiple_of(THINK_EVERY);
            if thinks {
                self.think(person, hours);
            }
        }
        self.walk(hours, step);
    }

    /// Someone thinks: sets off for the next window if it is time to, else, done with a
    /// spot, picks another for what the window asks.
    fn think(&mut self, person: usize, hours: f64) {
        let role = self.people[person].role;
        let (start, end, does) = self.window_of(person, hours);
        let (next_start, next_end, next_does) = self.window_at(role, end + 1e-6);
        let next_does = next_does.to_vec();
        // What comes next, planned some hours ahead: where, and so when to set off to be
        // there on time; planned again if they have since gone far from where they were. A
        // window too short for the walks either side of it to leave any of it is passed
        // over for the one after, as someone with no time for an errand goes on.
        if self.people[person].window < next_start - 1e-9 && hours >= next_start - PLAN_AHEAD_H {
            let p = &self.people[person];
            let stale = match p.planned {
                Some(plan) => plan.window < next_start - 1e-9,
                None => true,
            };
            // Gone far from where the plan was made: the same place, the walk from here.
            if let Some(plan) = p.planned
                && !stale
                && plan.from.distance(p.at) > 3.0
            {
                let walk = self.hours_to_walk(p, polyline(p.at, &self.route_to(p, plan.spot)));
                let from = p.at;
                self.people[person].planned = Some(Planned {
                    leave: plan.window - walk,
                    from,
                    ..plan
                });
            }
            let mut target = (next_start, next_end, next_does);
            for _ in 0..3 {
                if !stale {
                    break;
                }
                let Some((activity, spot)) = self.choose(person, &target.2, Some(target.0 - hours))
                else {
                    break;
                };
                let p = &self.people[person];
                let walk = self.hours_to_walk(p, polyline(p.at, &self.route_to(p, spot)));
                let (after_start, after_end, after_does) = self.window_at(role, target.1 + 1e-6);
                let from_there = self
                    .spots
                    .iter()
                    .filter(|s| after_does.iter().any(|a| s.does.contains(a)))
                    .map(|s| s.at.distance(self.spots[spot].at))
                    .fold(f32::MAX, f32::min);
                let onward = if from_there.is_finite() {
                    self.hours_to_walk(p, from_there * 1.3)
                } else {
                    0.0
                };
                let arrive = (hours + walk).max(target.0);
                if arrive + MIN_STAY_H + onward > after_start {
                    target = (after_start, after_end, after_does.to_vec());
                    continue;
                }
                let from = p.at;
                // The place is kept for them from now, so no one else plans on it.
                if let Some(old) = self.people[person].planned
                    && self.spots[old.spot].taken == Some(person)
                    && self.people[person].spot != Some(old.spot)
                {
                    self.spots[old.spot].taken = None;
                }
                if !self.spots[spot].shared {
                    self.spots[spot].taken = Some(person);
                }
                self.people[person].planned = Some(Planned {
                    window: target.0,
                    activity,
                    spot,
                    leave: target.0 - walk,
                    from,
                });
                break;
            }
            if let Some(plan) = self.people[person].planned
                && plan.window >= next_start - 1e-9
                && hours >= plan.leave
            {
                self.people[person].planned = None;
                let free = self.spots[plan.spot].open
                    && (self.spots[plan.spot].shared
                        || self.spots[plan.spot].taken.is_none_or(|who| who == person));
                // Taken meanwhile: another place for something the window asks.
                let chosen = if free {
                    Some((plan.activity, plan.spot))
                } else {
                    let (_, _, does) = self.window_at(role, plan.window + 1e-6);
                    let does = does.to_vec();
                    self.choose(person, &does, Some(plan.window - hours))
                };
                if let Some((activity, spot)) = chosen {
                    self.send(person, activity, spot, plan.window, hours);
                    return;
                }
            }
        }
        let p = &self.people[person];
        if matches!(p.state, State::Walking { .. }) {
            return;
        }
        // Passing this window over for a later one: carry on as they are until it is time
        // to set off.
        if p.planned.is_some_and(|plan| plan.window > start + 1e-9) {
            if let State::Doing { activity, until } = p.state
                && hours >= until
            {
                self.people[person].state = State::Doing {
                    activity,
                    until: hours + 5.0 / 60.0,
                };
            }
            return;
        }
        let in_window = (p.window - start).abs() < 1e-9;
        let done = match p.state {
            State::Doing { until, .. } => hours >= until || !in_window,
            State::Inside | State::Away => !in_window,
            State::Walking { .. } => false,
        };
        if !done {
            return;
        }
        if let Some((activity, spot)) = self.choose(person, &does, None) {
            let same = self.people[person].spot == Some(spot);
            let rules = self.plan.activities[&self.activities[activity]].clone();
            if same && !rules.inside && !rules.away {
                // Stay on where they are, for another while.
                let stay = rules.stay.0 + self.draw(person) * (rules.stay.1 - rules.stay.0);
                let p = &mut self.people[person];
                p.window = start;
                p.state = State::Doing {
                    activity,
                    until: (hours + stay as f64 / 60.0).min(end),
                };
            } else if same {
                self.people[person].window = start;
            } else {
                self.send(person, activity, spot, start, hours);
            }
        }
    }

    /// Everyone walking a step on: each wanting toward the next place on their route,
    /// every one kept out of the others' way and off the walls; arrivals taking up their
    /// spot.
    fn walk(&mut self, hours: f64, step: f32) {
        let bodies: Vec<Option<Body>> = self
            .people
            .iter()
            .map(|p| {
                let out =
                    !matches!(p.state, State::Inside | State::Away) && p.detail != Detail::Far;
                out.then_some(Body {
                    at: p.at,
                    velocity: p.velocity,
                    radius: p.radius,
                })
            })
            .collect();
        // A coarse grid of who is where, for neighbours.
        let mut grid: std::collections::BTreeMap<(i32, i32), Vec<usize>> = Default::default();
        let key = |at: Vec2| {
            (
                libm::floorf(at.x / 4.0) as i32,
                libm::floorf(at.y / 4.0) as i32,
            )
        };
        for (index, body) in bodies.iter().enumerate() {
            if let Some(body) = body {
                grid.entry(key(body.at)).or_default().push(index);
            }
        }
        let mut chosen: Vec<Option<Vec2>> = vec![None; self.people.len()];
        for (index, person) in self.people.iter().enumerate() {
            let State::Walking { run } = person.state else {
                continue;
            };
            if person.detail == Detail::Far {
                continue;
            }
            let Some(&target) = person.route.get(person.next) else {
                continue;
            };
            let last = person.next + 1 == person.route.len();
            let fastest = if run { person.run } else { person.walk };
            let toward = target - person.at;
            let distance = toward.length();
            let speed = if last {
                fastest * (distance / 1.2).clamp(0.6, 1.0)
            } else {
                fastest
            };
            let wanted = toward.normalize_or_zero() * speed;
            let (cx, cy) = key(person.at);
            let mut neighbours: Vec<(f32, Body)> = Vec::new();
            for x in cx - 1..=cx + 1 {
                for y in cy - 1..=cy + 1 {
                    for &other in grid.get(&(x, y)).into_iter().flatten() {
                        if other == index {
                            continue;
                        }
                        if let Some(mut body) = bodies[other] {
                            let d = body.at.distance(person.at);
                            // Held up long, someone edges between those standing about,
                            // as people in a crowd turn a shoulder.
                            if person.held > EDGE_S
                                && matches!(self.people[other].state, State::Doing { .. })
                            {
                                body.radius *= 0.6;
                            }
                            if d < 5.0 {
                                neighbours.push((d, body));
                            }
                        }
                    }
                }
            }
            // Visitors, who do not give way: seen a little wider, so a walker leaves them
            // room though it takes only its share of the avoiding.
            for visitor in &self.visitors {
                let d = visitor.at.distance(person.at);
                if d < 5.0 {
                    neighbours.push((
                        d,
                        Body {
                            radius: visitor.radius + 0.15,
                            ..*visitor
                        },
                    ));
                }
            }
            neighbours.sort_by(|a, b| a.0.total_cmp(&b.0));
            neighbours.truncate(10);
            let others: Vec<Body> = neighbours.into_iter().map(|(_, b)| b).collect();
            // Held up a while, they squeeze past, shoulders brushing, as people do.
            let squeeze = if person.held > SQUEEZE_S { 0.5 } else { 1.0 };
            let mover = Mover {
                at: person.at,
                velocity: person.velocity,
                wanted,
                radius: person.radius * squeeze,
                fastest,
            };
            let velocity = orca::velocity(&mover, &others, &self.walls, Horizons::default(), step);
            // Barely moving is standing: the body plays a walk only at a walking speed.
            let velocity = if velocity.length() < STANDING_MPS {
                Vec2::ZERO
            } else {
                velocity
            };
            chosen[index] = Some(velocity);
        }
        for (index, velocity) in chosen.into_iter().enumerate() {
            let Some(velocity) = velocity else {
                continue;
            };
            let p = &mut self.people[index];
            p.velocity = velocity;
            p.at += velocity * step;
            if velocity.length() > 0.2 {
                let turn = 1.0 - libm::expf(-step * 6.0);
                p.facing = p
                    .facing
                    .lerp(velocity.normalize(), turn)
                    .normalize_or(p.facing);
            }
            // On along the route as each place is passed; a door or a gate, which many go
            // through, is reached at its threshold.
            let through = p.spot.is_some_and(|spot| self.spots[spot].shared);
            while let Some(&target) = p.route.get(p.next) {
                let last = p.next + 1 == p.route.len();
                let reach = match (last, through) {
                    (false, _) => PASSED_M,
                    (true, true) => THRESHOLD_M,
                    (true, false) => ARRIVED_M,
                };
                if p.at.distance(target) <= reach {
                    p.next += 1;
                } else {
                    break;
                }
            }
            // Headway: anyone held for a while is routed again from where they are, once
            // each REROUTE_S, still held: their squeezing and edging past go on meanwhile.
            let was = p.held;
            if p.at.distance(p.held_at) > 0.5 {
                p.held = 0.0;
                p.held_at = p.at;
            } else {
                p.held += step;
            }
            let held = p.held;
            if p.next >= p.route.len() {
                self.arrive(index, hours);
            } else if (held / REROUTE_S).floor() > (was / REROUTE_S).floor() {
                let Some(spot) = self.people[index].spot else {
                    continue;
                };
                let route = self.route_to(&self.people[index], spot);
                let p = &mut self.people[index];
                p.route = route;
                p.next = 0;
            }
        }
    }

    /// Someone at the end of their route takes up their spot: goes in, goes out of the
    /// town, or starts what they came to do, and their arrival is kept for the check.
    fn arrive(&mut self, person: usize, hours: f64) {
        let (Some(spot), Some(activity)) = (self.people[person].spot, self.people[person].going)
        else {
            return;
        };
        let rules = self.plan.activities[&self.activities[activity]].clone();
        let stay = rules.stay.0 + self.draw(person) * (rules.stay.1 - rules.stay.0);
        let (_, end, _) = self.window_of(person, hours);
        let p = &mut self.people[person];
        p.at = self.spots[spot].at;
        p.velocity = Vec2::ZERO;
        p.facing = self.spots[spot].facing;
        p.route.clear();
        p.going = None;
        if !p.late_counted {
            p.late_counted = true;
            if p.window - PLAN_AHEAD_H >= self.started {
                let late = ((hours - p.window) * 60.0) as f32;
                self.record.arrivals.push(Arrival {
                    person,
                    window: p.window,
                    late,
                    set_off: p.set_off,
                    walked_s: ((hours - p.set_off) * self.seconds_an_hour as f64) as f32,
                    route_s: p.route_s,
                });
            }
        }
        let p = &mut self.people[person];
        if rules.inside {
            p.state = State::Inside;
            p.spot = None;
        } else if rules.away {
            p.state = State::Away;
            p.spot = None;
        } else {
            let until = (hours + stay as f64 / 60.0).min(end.max(hours));
            p.state = State::Doing { activity, until };
        }
    }

    /// What someone's body shows.
    pub fn shows(&self, person: usize) -> Shows<'_> {
        let p = &self.people[person];
        match p.state {
            State::Inside | State::Away => Shows::Hidden,
            State::Walking { run } => {
                let speed = p.velocity.length();
                if run {
                    Shows::Run(speed)
                } else {
                    Shows::Walk(speed)
                }
            }
            State::Doing { activity, .. } => {
                let rules = &self.plan.activities[&self.activities[activity]];
                Shows::Clip(rules.clip.as_deref().unwrap_or("Idle_Loop"))
            }
        }
    }

    /// The activity someone is doing or going to, by name, if any.
    pub fn activity(&self, person: usize) -> Option<&str> {
        let p = &self.people[person];
        match p.state {
            State::Doing { activity, .. } => Some(&self.activities[activity]),
            _ => p.going.map(|a| self.activities[a].as_str()),
        }
    }

    /// Where someone is going, as a person would say it: the activity and the thing it is
    /// done at, and where that is.
    pub fn going_to(&self, person: usize) -> Option<(String, Vec2)> {
        let p = &self.people[person];
        let spot = &self.spots[p.spot?];
        let thing = self.town.things[spot.thing].kind.name().to_lowercase();
        let what = self.activity(person).unwrap_or("nothing").to_lowercase();
        Some((format!("{what} at the {thing}"), spot.at))
    }

    /// The role's name someone has.
    pub fn role(&self, person: usize) -> &str {
        &self.plan.roles[self.people[person].role].name
    }
}
