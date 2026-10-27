//! The controller's plain-Rust course (docs/controller.md, "Production criteria"; D53, D457):
//! analytic fixtures a mover is driven through by scripted intents, and the numbers each
//! criterion of `tools/controller/criteria.json` is judged by, measured against geometry and
//! the kind's own numbers, never against the mover's idea of them.
//!
//! A mover is driven through [`Mover`], so a change of mover leaves the course unchanged.
//! [`score`] runs every case on every core (D406) and gives a [`Measure`] a criterion;
//! [`print`] writes them as `cargo xtask controller` reads them.
//!
//! - `field`: the fixtures (steps, slopes, stairs, a tunnel, walls with a gap, a pool, a
//!   round world) as signed distance fields.
//! - `runs`: the cases and what each measures.
//! - `data`: the cases measured against the kind's own numbers (speeds, rates, curves), for two
//!   kinds, so two kinds' files give two results (criterion 14).
//! - `sources`: what stops a body besides the ground (a base's cubes and door, a prop, a
//!   rock), each a step and the door a wall (criteria 1 and 5).
//! - `stairs`: flights of the ground's and a base's steps walked up and down at the kind's
//!   speed, and a doorway's sill crossed without a hop (criteria 5 to 7).
//! - `flora`: stumps and logs as flora fits them, taller than a step and never climbed, by a
//!   root or from a slope's high side, or no taller and stepped onto (criterion 5).
//! - `water`: a lake walked into, waded, swum across and walked out of, straight and at an
//!   angle to its shores, and a body floated to rest (criterion 17, D74).
//! - `dive`: diving by looking down, rising on jump, floating back up, breath used under water
//!   and drowning, a diver along a lake's bed and out up its shore (criterion 17, D478).
//! - `ledge`: a swimmer climbing out onto a bank up to its kind's limit over the water, and
//!   never onto a higher one (D523).
//! - `climb`: faces walked and jumped at, climbed, pulled up onto, lunged up and kicked off
//!   from; overhangs to their limit and ceilings never (D505-D511).
//! - `holding`: what climbing's holding, moving, overhangs and lunges cost in stamina and
//!   letting go at none, grip, the fight stance, knocks, the hold's gap and climbing down
//!   (D505-D507, D510, D511).
//! - `fall`: falls that must not hurt (a jump, a step, a stair, a short drop), drops that
//!   must, onto level ground and a slope, and into deep water (D476).
//! - `pushes`: people easing apart, a man walked past one standing, a monster walking through
//!   men and into a wall with one, and a man pushing a monster (D479, criterion 15).
//! - `closing`: two bodies closing at 10-60 m/s, head on, at a glance and one falling onto
//!   another, never passing through each other (criterion 1, D53).
//! - `far`: the same walks on a world 30 km across, where f32 is 2 mm fine (criterion 22, D482).

mod climb;
mod closing;
mod data;
mod dive;
mod fall;
mod far;
pub mod field;
mod flora;
mod holding;
mod ledge;
mod pushes;
mod runs;
mod sources;
mod stairs;
mod wade;
mod water;

pub use field::{Ball, Course, Field, Frame, Solid, Water};
use glam::{DVec3, Vec3};

/// The fixed tick (D138).
pub const TICK: f32 = 1.0 / 64.0;

pub use crate::mover::{Intent, Kind, Push};

impl Kind {
    /// A man, by `assets/data/movers.ron` (docs/controller.md's stand-ins).
    pub fn man() -> Self {
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/data/movers.ron"
        ));
        Kind::parse(text, "Man").expect("the movers file holds a man")
    }

    /// The kind with its climbing off (D136: no climb speed), for the rows that ask a face of
    /// the walk alone (a slope too steep stops it, a thing taller than a step is not stepped
    /// onto): a body pushing into a face for half a second holds it (D507), which climbing's
    /// own rows ask.
    pub fn walking(self) -> Self {
        Kind { climb: 0.0, ..self }
    }
}

/// A body as a mover leaves it after a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// The bottom of the capsule.
    pub feet: Vec3,
    /// The same in f64, where the mover keeps it so (`feet` as f64 where it does not).
    pub position: DVec3,
    /// The body's up: its capsule stands along it.
    pub up: Vec3,
    /// Its whole velocity, level and vertical.
    pub velocity: Vec3,
    pub grounded: bool,
}

/// A character controller as the course drives it: bodies added by ID, then ticked with an
/// intent each, in the order given.
pub trait Mover {
    /// Places a body of a kind with its feet at a point, facing a level direction.
    fn add(&mut self, id: u32, kind: &Kind, feet: Vec3, facing: Vec3, field: &dyn Field);
    /// One tick of every body named, each with its intent.
    fn tick(&mut self, field: &dyn Field, intents: &[(u32, Intent)], dt: f32);
    fn body(&self, id: u32) -> Body;
    /// The level way a body faces; `None` for a body not placed.
    fn facing(&self, id: u32) -> Option<Vec3>;
    /// How fast a body was falling when it landed in the last tick, if it landed in it.
    fn landed(&self, id: u32) -> Option<f32>;
    /// The share of its health a body's landing in the last tick took (D476), 0 if none.
    fn fall(&self, id: u32) -> f32;
    /// How much breath a body has left, s (D74).
    fn breath(&self, id: u32) -> f32;
    /// The share of its health drowning took in the last tick, 0 if none.
    fn drown(&self, id: u32) -> f32;
    /// Whether a body is holding a face, climbing (D477); false for a mover that does not climb.
    fn climbing(&self, id: u32) -> bool;
    /// How much stamina a body has left to climb on, of the mover's full pool (D506, D61): a
    /// climber spends it, and lets go at none.
    fn stamina(&self, id: u32) -> f32;
    /// Ticks its bodies on so many threads from now on, if it can share them out; one
    /// unless told.
    fn threads(&mut self, _threads: usize) {}
}

/// A criterion's number, as `tools/controller/criteria.json` names it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub id: &'static str,
    pub value: f32,
}

/// The faces a climbing body's cost is timed on (`cost.climb`, terra_walker's mover bench):
/// the sheer wall, the dug cliff and a base's wall of cubes, with where a body stands to walk
/// into each (a metre short of it, facing it along [`field::forward_at`]).
pub fn climbing_faces(kind: &Kind) -> Vec<(&'static str, Course, Vec3)> {
    let start = field::world(climb::FACE_U - 1.0, 0.0, 0.0);
    [
        ("wall", climb::Face::Wall),
        ("dug cliff", climb::Face::Cliff),
        ("base's wall", climb::Face::Cubes),
    ]
    .into_iter()
    .map(|(name, face)| (name, climb::faces(face, kind), start))
    .collect()
}

/// How many of the crowd's reruns, on so many threads each, end otherwise than its run on one
/// (`determinism.crowd`'s crowd): for a mover whose lanes run on a pool the course cannot name,
/// as the game's (Bevy's compute pool, terra_walker).
pub fn crowd_reruns<M: Mover>(new: impl Fn() -> M + Sync, threads: &[usize]) -> usize {
    runs::crowd_reruns(&new, threads)
}

/// Every case of the course, run on every core, measured: a number a criterion.
pub fn score<M: Mover>(new: impl Fn() -> M + Sync) -> Vec<Measure> {
    let mut measures = runs::score(&new);
    measures.extend(data::score(&new));
    measures.extend(far::score(&new));
    let (fall, deepest_fall) = fall::score(&new);
    measures.extend(fall);
    let (sources, deepest) = sources::score(&new);
    measures.extend(sources);
    let (stairs, deeper) = stairs::score(&new);
    measures.extend(stairs);
    let (flora, deepest_flora) = flora::score(&new);
    measures.extend(flora);
    let (water, deepest_water) = water::score(&new);
    measures.extend(water);
    let (wade, deepest_wade) = wade::score(&new);
    measures.extend(wade);
    let (dive, deepest_dive) = dive::score(&new);
    measures.extend(dive);
    let (ledge, deepest_ledge) = ledge::score(&new);
    measures.extend(ledge);
    let (pushes, deepest_push, overlap) = pushes::score(&new);
    measures.extend(pushes);
    let (closing, deepest_closing) = closing::score(&new);
    measures.extend(closing);
    let (climb, deepest_climb) = climb::score(&new);
    measures.extend(holding::score(&new));
    measures.extend(climb);
    let deepest = deepest
        .max(deepest_push)
        .max(deepest_closing)
        .max(deeper)
        .max(deepest_flora)
        .max(deepest_water)
        .max(deepest_wade)
        .max(deepest_dive)
        .max(deepest_ledge)
        .max(deepest_climb)
        .max(deepest_fall);
    // What a body met besides the ground counts toward how deep it ever went.
    for m in measures.iter_mut() {
        match m.id {
            "penetration.max" => m.value = m.value.max(deepest),
            "push.overlap" => m.value = m.value.max(overlap),
            _ => {}
        }
    }
    measures
}

/// The measures as `cargo xtask controller` reads them: a line each, `scorecard`, the id and
/// the number, tab-separated.
pub fn print(measures: &[Measure]) {
    for m in measures {
        println!("scorecard\t{}\t{}", m.id, m.value);
    }
}
