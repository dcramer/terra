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

pub mod field;
mod runs;

pub use field::{Ball, Course, Field, Solid, Water};
use glam::Vec3;

/// The fixed tick (D138).
pub const TICK: f32 = 1.0 / 64.0;

/// A kind's numbers the course judges a mover by (docs/controller.md, "Numbers that are
/// data"). Until kinds' files hold them (stage 2), [`Kind::man`] gives the doc's stand-ins.
#[derive(Clone, Copy, Debug)]
pub struct Kind {
    pub radius: f32,
    pub height: f32,
    /// Kilograms; infinite for a body that never gives way.
    pub mass: f32,
    pub jog: f32,
    pub sprint: f32,
    /// Launch speed, m/s.
    pub jump_speed: f32,
    pub step_height: f32,
    pub slope_limit_deg: f32,
    pub coyote_s: f32,
    pub buffer_s: f32,
}

impl Kind {
    /// A man, by docs/controller.md's stand-ins: today's walker's capsule and speeds, Jolt's
    /// slope limit, a step one dug voxel high with padding.
    pub fn man() -> Self {
        Kind {
            radius: 0.3,
            height: 1.8,
            mass: 75.0,
            jog: 4.3,
            sprint: 7.4,
            jump_speed: 6.0,
            step_height: 0.55,
            slope_limit_deg: 50.0,
            coyote_s: 0.1,
            buffer_s: 0.15,
        }
    }
}

/// What a body asks of a tick: a level way to go (unit or zero), sprinting, a jump pressed
/// this tick, or a move's drive (root motion) in place of walking.
#[derive(Clone, Copy, Debug, Default)]
pub struct Intent {
    pub wish: Vec3,
    pub sprint: bool,
    pub jump: bool,
    pub drive: Option<Vec3>,
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
}

/// A criterion's number, as `tools/controller/criteria.json` names it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub id: &'static str,
    pub value: f32,
}

/// Every case of the course, run on every core, measured: a number a criterion.
pub fn score<M: Mover>(new: impl Fn() -> M + Sync) -> Vec<Measure> {
    runs::score(&new)
}

/// The measures as `cargo xtask controller` reads them: a line each, `scorecard`, the id and
/// the number, tab-separated.
pub fn print(measures: &[Measure]) {
    for m in measures {
        println!("scorecard\t{}\t{}", m.id, m.value);
    }
}
