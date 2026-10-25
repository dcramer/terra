//! The controller's plain-Rust course (docs/controller.md, "Production criteria"; D53, D457):
//! analytic fixtures a mover is driven through by scripted intents, and the numbers each
//! criterion of `tools/controller/criteria.json` is judged by, measured against geometry and
//! the kind's own numbers, never against the mover's idea of them.
//!
//! A mover is driven through [`Mover`], so stage 2's controller replaces today's walker
//! (driven through its adapter in `terra_walker`'s tests) without the course changing.
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

mod data;
pub mod field;
mod runs;
mod sources;

pub use field::{Ball, Course, Field, Solid, Water};
use glam::Vec3;

/// The fixed tick (D138).
pub const TICK: f32 = 1.0 / 64.0;

pub use crate::mover::{Intent, Kind};

impl Kind {
    /// A man, by `assets/data/movers.ron` (docs/controller.md's stand-ins).
    pub fn man() -> Self {
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/data/movers.ron"
        ));
        Kind::parse(text, "Man").expect("the movers file holds a man")
    }
}

/// A body as a mover leaves it after a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// The bottom of the capsule.
    pub feet: Vec3,
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
    /// The level way a body faces; `None` from a mover that keeps no facing.
    fn facing(&self, _id: u32) -> Option<Vec3> {
        None
    }
    /// How fast a body was falling when it landed in the last tick, if it landed in it;
    /// `None` too from a mover that reports no landings.
    fn landed(&self, _id: u32) -> Option<f32> {
        None
    }
}

/// A criterion's number, as `tools/controller/criteria.json` names it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub id: &'static str,
    pub value: f32,
}

/// Every case of the course, run on every core, measured: a number a criterion.
pub fn score<M: Mover>(new: impl Fn() -> M + Sync) -> Vec<Measure> {
    let mut measures = runs::score(&new);
    measures.extend(data::score(&new));
    let (sources, deepest) = sources::score(&new);
    measures.extend(sources);
    // What a body met besides the ground counts toward how deep it ever went.
    for m in measures.iter_mut().filter(|m| m.id == "penetration.max") {
        m.value = m.value.max(deepest);
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
