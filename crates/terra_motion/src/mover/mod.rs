//! The mover (D53, docs/controller.md): one kinematic character controller every body is moved
//! by, the player, people and creatures alike, with only its kind's numbers different. A tick
//! of a body builds its velocity from its intent (or its move's drive), collides and slides it
//! through what the [`Query`] answers (`slide`, ported from Jolt's `CharacterVirtual`), steps it
//! up a stair, holds it to the floor going down, and reads its ground state; after every body
//! has moved, overlapping bodies are separated by mass (`push`). Bodies are moved in the order
//! of their IDs, whatever order they are given in, so the same world and intents give the same
//! positions everywhere (D138).
//!
//! - `query`: what stops a body, the one query trait and its sources.
//! - `slide`: collide and slide, the step up and stick to the floor (Jolt, MIT).
//! - `shapes`: what stops a body besides the ground: cubes, doors, props, trees and rocks.
//! - `push`: bodies apart by mass.
//! - `grid`: the bodies' broadphase, a grid of them by place.
//! - `lanes`: a tick's steps shared over threads, on whatever pool the caller has.

mod footing;
mod grid;
mod lanes;
mod push;
pub mod query;
pub mod shapes;
mod slide;

#[cfg(any(test, feature = "course"))]
pub mod course;

use grid::Gathered;
pub use grid::Grid;
pub use lanes::{Executor, InTurn, Lanes};
pub use query::{Capsule, Capsules, Contact, Hit, Query, Sdf};
use serde::Deserialize;
pub use shapes::{Block, Obstacle, Obstacles, Part, Shaped};
use slide::{Shape, Support, Touch};

use glam::{DVec3, Vec3};
use std::collections::BTreeMap;

/// A kind's movement numbers (docs/controller.md, "Numbers that are data"), read from
/// `assets/data/movers.ron`, which explains each.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Kind {
    pub radius: f32,
    pub height: f32,
    /// Kilograms; infinite for a body that never gives way.
    pub mass: f32,
    pub jog: f32,
    pub sprint: f32,
    pub swim: f32,
    /// Toward a wished velocity and back to standing on the ground, and in water, m/s².
    pub accel: f32,
    pub decel: f32,
    pub swim_accel: f32,
    /// The fraction of `accel` a body has in the air.
    pub air_control: f32,
    /// How fast its facing turns toward its way, deg/s.
    pub turn_deg_s: f32,
    pub step_height: f32,
    pub slope_limit_deg: f32,
    /// Speed by the floor's grade along the way, five (deg, factor) points, downhill negative.
    pub slope_speed: [(f32, f32); 5],
    /// Launch speed, m/s.
    pub jump_speed: f32,
    pub coyote_s: f32,
    pub buffer_s: f32,
    /// How far down a body is held to the floor, the predictive contact distance and the
    /// padding kept from what it is swept at, m.
    pub stick_m: f32,
    pub predictive_m: f32,
    pub padding_m: f32,
    /// Water deeper than this fraction of its height is swum; afloat, this fraction of it is
    /// under; the water's drag on it, 1/s.
    pub swim_depth: f32,
    pub float_depth: f32,
    pub water_drag: f32,
}

impl Kind {
    /// A kind by name from a movers file.
    pub fn parse(text: &str, name: &str) -> Result<Kind, String> {
        let kinds: BTreeMap<String, Kind> = ron::from_str(text).map_err(|why| why.to_string())?;
        kinds
            .get(name)
            .copied()
            .ok_or_else(|| format!("no kind {name}"))
    }

    /// The speed factor on a floor of a grade along the way, deg (uphill positive).
    pub fn slope_factor(&self, grade_deg: f32) -> f32 {
        let curve = &self.slope_speed;
        if grade_deg <= curve[0].0 {
            return curve[0].1;
        }
        for pair in curve.windows(2) {
            let ((a, fa), (b, fb)) = (pair[0], pair[1]);
            if grade_deg <= b {
                return fa + (fb - fa) * (grade_deg - a) / (b - a).max(1e-6);
            }
        }
        curve[curve.len() - 1].1
    }
}

/// What a body asks of a tick: a level way to go (unit or zero), sprinting, a jump pressed this
/// tick, a share of its kind's speed, or a move's drive (root motion, a roll) in place of
/// walking.
#[derive(Clone, Copy, Debug)]
pub struct Intent {
    pub wish: Vec3,
    pub sprint: bool,
    pub jump: bool,
    /// Its top speed as a share of its kind's jog, sprint or swim: guarding, a move's pace,
    /// deep snow or mud (1 for none).
    pub scale: f32,
    /// A level velocity it is driven at, its own speed and way kept (a roll, a lunge); the
    /// ground and what stops it still answer.
    pub drive: Option<Vec3>,
}

impl Default for Intent {
    fn default() -> Self {
        Intent {
            wish: Vec3::ZERO,
            sprint: false,
            jump: false,
            scale: 1.0,
            drive: None,
        }
    }
}

/// A body's ground state (D53).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    Grounded,
    /// On ground past the slope limit, sliding down it.
    Steep,
    Airborne,
    Swimming,
}

/// A body as the mover keeps it.
#[derive(Clone, Debug)]
pub struct Body {
    pub id: u32,
    pub kind: Kind,
    /// The bottom of the capsule, in the world: `position` rounded to f32, for what shows it.
    pub feet: Vec3,
    /// The bottom of the capsule, in f64: what the mover keeps (Jolt's large worlds keep a
    /// character's position a double). A tick works in offsets from it (`query`'s frames).
    pub position: DVec3,
    /// Away from the world's centre: its capsule stands along it.
    pub up: Vec3,
    pub velocity: Vec3,
    /// The level way it faces, turned toward its way at its kind's turn rate.
    pub facing: Vec3,
    pub ground: Ground,
    /// The floor's normal while grounded.
    pub normal: Vec3,
    /// How fast it was falling when it landed this tick (open question 1, fall damage).
    pub landed: Option<f32>,
    /// How far a step raised it this tick: what is shown runs it down to nothing (presentation,
    /// docs/controller.md), the mover never.
    pub stepped: f32,
    /// Whether it jumped this tick: the mover decides, by its buffer and coyote time, so what
    /// a jump costs is spent on this, not on the press.
    pub jumped: bool,
    support: Support,
    /// The contacts, in the frame of `frame`: the position the tick that found them began at.
    active: Vec<Touch>,
    frame: DVec3,
    /// Time since it last stood, and whether it left by jumping (coyote time).
    air_s: f32,
    left_by_jump: bool,
    /// Time left on a jump pressed in the air (the buffer).
    buffer: f32,
}

impl Body {
    /// What it touched as its last tick left it, in the world: where, the normal, and whether
    /// the move met it (for a dump of why a body stopped, `build.contacts`).
    pub fn contacts(&self) -> impl Iterator<Item = (DVec3, Vec3, bool)> + '_ {
        self.active.iter().map(|t| {
            (
                self.frame + t.contact.at.as_dvec3(),
                t.contact.normal,
                t.met,
            )
        })
    }
}

/// The world a tick moves bodies through: what stops them, the water, gravity toward the
/// centre (D63).
pub struct World<'a> {
    pub ground: &'a dyn Query,
    /// How far from the centre the water's surface over a point is, where water is.
    pub water: &'a dyn Fn(Vec3) -> Option<f32>,
    pub gravity: f32,
    pub centre: Vec3,
}

/// Every body the mover moves, kept in ID order.
#[derive(Default)]
pub struct Bodies {
    bodies: Vec<Body>,
    /// The pushes' partners, kept from tick to tick (Verlet lists).
    pairs: push::Lists,
}

/// Fewer bodies than this ask each other directly: the broadphase's upkeep would cost more
/// than it saves.
const CROWD_FROM: usize = 16;
/// How far beyond a body's own reach the bodies around it are gathered for its tick's
/// asks, m: its move and the sweeps' motions within it (one beyond looks the grid up again).
const GATHER_M: f64 = 1.0;
/// How many neighbouring bodies a lane takes at a time when a tick is shared over threads.
const RUN: usize = 8;
/// How many rounds of pushing a tick takes (docs/controller.md, step 6).
const PUSH_ROUNDS: usize = 4;
/// How much higher than its floor's own slope a walking body's move may lift it before it is
/// put back on the floor, m: the solver's press into the floor is less.
const LIFT_M: f32 = 1.0e-3;

impl Bodies {
    /// Places a body of a kind with its feet at a point, facing a level way.
    pub fn add(&mut self, id: u32, kind: Kind, feet: Vec3, facing: Vec3, world: &World) {
        let up = (feet - world.centre).normalize_or(Vec3::Y);
        let mut body = Body {
            id,
            kind,
            feet,
            position: feet.as_dvec3(),
            up,
            velocity: Vec3::ZERO,
            facing: facing
                .reject_from(up)
                .normalize_or(up.any_orthonormal_vector()),
            ground: Ground::Airborne,
            normal: Vec3::ZERO,
            landed: None,
            stepped: 0.0,
            jumped: false,
            support: Support::InAir,
            active: Vec::new(),
            frame: feet.as_dvec3(),
            air_s: f32::INFINITY,
            left_by_jump: false,
            buffer: 0.0,
        };
        settle(&mut body, feet.as_dvec3(), Vec3::ZERO, world);
        let at = self.bodies.partition_point(|b| b.id < id);
        if self.bodies.get(at).is_some_and(|b| b.id == id) {
            self.bodies[at] = body;
        } else {
            self.bodies.insert(at, body);
        }
    }

    pub fn get(&self, id: u32) -> Option<&Body> {
        let at = self.bodies.binary_search_by_key(&id, |b| b.id).ok()?;
        Some(&self.bodies[at])
    }

    /// Takes a body out, as when it is gone from the world: what it was, if it was there.
    pub fn remove(&mut self, id: u32) -> Option<Body> {
        let at = self.bodies.binary_search_by_key(&id, |b| b.id).ok()?;
        Some(self.bodies.remove(at))
    }

    /// Puts a body's feet at a point, moving at a velocity, its ground read there afresh: a
    /// spawn, respawn or teleport, or held still (downed). Its kind and facing are kept and a
    /// jump in its buffer is dropped. False if there is no such body.
    pub fn place(&mut self, id: u32, feet: Vec3, velocity: Vec3, world: &World) -> bool {
        let Ok(at) = self.bodies.binary_search_by_key(&id, |b| b.id) else {
            return false;
        };
        settle(&mut self.bodies[at], feet.as_dvec3(), velocity, world);
        true
    }

    pub fn iter(&self) -> impl Iterator<Item = &Body> {
        self.bodies.iter()
    }

    /// One tick of every body, each with its intent (none for a body not named), then the
    /// pushes. Every body steps against the others as the tick began (Jacobi), so the bodies
    /// may be stepped in any order and on any number of threads to the same bits
    /// ([`Bodies::tick_on`]); the pushes go in ID order after.
    pub fn tick(&mut self, world: &World, intents: &[(u32, Intent)], dt: f32) {
        let before = self.before(intents);
        steps(&mut self.bodies, 0, &before, world, dt);
        push::separate(&mut self.bodies, &mut self.pairs, world.ground, PUSH_ROUNDS);
    }

    /// What a tick's steps read of every body: as it began.
    fn before(&self, intents: &[(u32, Intent)]) -> Before {
        let mut asked: Vec<(u32, Intent)> = intents.to_vec();
        asked.sort_by_key(|(id, _)| *id);
        let capsules: Vec<(u32, Capsule, Vec3)> = self
            .bodies
            .iter()
            .map(|b| (b.id, capsule(b), b.velocity))
            .collect();
        let extent = capsules
            .iter()
            .map(|(_, c, _)| (c.b - c.a).length() * 0.5 + c.radius)
            .fold(0.0, f32::max);
        let grid = (self.bodies.len() >= CROWD_FROM)
            .then(|| Grid::of(capsules.iter().map(|(_, c, _)| middle(c)), extent));
        Before {
            asked,
            capsules,
            extent,
            grid,
        }
    }
}

/// Every body as a tick began, which its steps read and none changes: the intents by ID, the
/// bodies' capsules and velocities by ID, the farthest a body's surface lies from its
/// capsule's middle, and a crowd's grid of them.
struct Before {
    asked: Vec<(u32, Intent)>,
    capsules: Vec<(u32, Capsule, Vec3)>,
    extent: f32,
    grid: Option<Grid>,
}

/// A run of bodies stepped, the first the `from`th by ID, each against the others as the tick
/// began.
fn steps(bodies: &mut [Body], from: usize, before: &Before, world: &World, dt: f32) {
    let mut gathered = Gathered::default();
    for (k, body) in bodies.iter_mut().enumerate() {
        let id = body.id;
        let intent = before
            .asked
            .binary_search_by_key(&id, |(id, _)| *id)
            .map_or(Intent::default(), |at| before.asked[at].1);
        if let Some(grid) = before.grid.as_ref() {
            let me = &before.capsules[from + k].1;
            let (a, b) = (me.world(me.a), me.world(me.b));
            let reach = me.radius + (me.b - me.a).length() + before.extent;
            let r = DVec3::splat(f64::from(reach) + GATHER_M);
            grid.gather(a.min(b) - r, a.max(b) + r, &mut gathered);
        }
        let others = Capsules {
            bodies: &before.capsules,
            skip: id,
            near: before.grid.as_ref(),
            gathered: before.grid.as_ref().map(|_| &gathered),
        };
        let query = (world.ground, &others);
        step(body, &intent, &query, world, dt);
    }
}

/// A body put at a point at a velocity, its up, contacts and ground state read there.
fn settle(body: &mut Body, position: DVec3, velocity: Vec3, world: &World) {
    let up = up_at(position, world).unwrap_or(body.up);
    (body.position, body.feet) = (position, position.as_vec3());
    (body.up, body.velocity, body.frame) = (up, velocity, position);
    body.facing = body
        .facing
        .reject_from(up)
        .normalize_or(up.any_orthonormal_vector());
    (body.landed, body.stepped, body.jumped, body.buffer) = (None, 0.0, false, 0.0);
    let shape = shape(body, 1.0 / 64.0);
    slide::contacts_at(Vec3::ZERO, world.ground, &shape, &mut body.active);
    let (support, normal) = slide::support(&mut body.active, Vec3::ZERO, &shape, Vec3::ZERO);
    body.support = support;
    let rising = velocity.dot(up) > 1.0e-6;
    (body.ground, body.normal, body.air_s) = if support == Support::OnGround && !rising {
        (Ground::Grounded, normal, 0.0)
    } else {
        (Ground::Airborne, Vec3::ZERO, f32::INFINITY)
    };
    body.left_by_jump = false;
}

/// Away from the world's centre at a place, if it is not the centre.
fn up_at(position: DVec3, world: &World) -> Option<Vec3> {
    (position - world.centre.as_dvec3())
        .try_normalize()
        .map(|up| up.as_vec3())
}

/// The middle of a capsule's axis in the world: where the broadphase keeps its body.
fn middle(capsule: &Capsule) -> DVec3 {
    capsule.world((capsule.a + capsule.b) * 0.5)
}

/// A body's capsule in its own frame: its feet at its position.
fn capsule(body: &Body) -> Capsule {
    Capsule::standing(Vec3::ZERO, body.up, body.kind.radius, body.kind.height).at(body.position)
}

fn shape(body: &Body, dt: f32) -> Shape {
    let k = &body.kind;
    Shape {
        radius: k.radius,
        height: k.height,
        up: body.up,
        cos_limit: libm::cosf(k.slope_limit_deg.to_radians()),
        predictive: k.predictive_m,
        padding: k.padding_m,
        dt,
        origin: body.frame,
    }
}

/// A level velocity along the floor at the same speed (Godot's constant floor speed).
fn along_floor(level: Vec3, normal: Vec3, up: Vec3) -> Vec3 {
    let n_up = normal.dot(up);
    if n_up < 1e-3 || level.length_squared() < 1e-12 {
        return level;
    }
    let along = level - up * (level.dot(normal) / n_up);
    along.normalize_or_zero() * level.length()
}

/// A vector moved toward another by at most a length.
fn approach(from: Vec3, to: Vec3, by: f32) -> Vec3 {
    let gap = to - from;
    let length = gap.length();
    if length <= by || length < 1e-9 {
        to
    } else {
        from + gap * (by / length)
    }
}

/// The level velocity a body wants this tick, and its facing turned toward its way.
fn level_velocity(body: &mut Body, intent: &Intent, swimming: bool, dt: f32) -> Vec3 {
    let (k, up) = (body.kind, body.up);
    let level = |v: Vec3| v - up * v.dot(up);
    let wish = level(intent.wish).normalize_or_zero();
    if wish != Vec3::ZERO {
        let turn = k.turn_deg_s.to_radians() * dt;
        let angle = libm::acosf(body.facing.dot(wish).clamp(-1.0, 1.0));
        body.facing = if angle <= turn {
            wish
        } else {
            let side = up.cross(body.facing);
            let sign = if side.dot(wish) >= 0.0 { 1.0 } else { -1.0 };
            let (s, c) = (libm::sinf(turn * sign), libm::cosf(turn * sign));
            (body.facing * c + side * s).normalize()
        };
    }
    if let Some(drive) = intent.drive {
        return level(drive);
    }
    let grounded = body.ground == Ground::Grounded;
    let mut top = if swimming {
        k.swim
    } else if intent.sprint {
        k.sprint
    } else {
        k.jog
    } * intent.scale.max(0.0);
    if grounded && wish != Vec3::ZERO {
        let grade = along_floor(wish, body.normal, up).dot(up).clamp(-1.0, 1.0);
        top *= k.slope_factor(libm::asinf(grade).to_degrees());
    }
    let rate = if swimming {
        k.swim_accel
    } else if !grounded {
        k.accel * k.air_control
    } else if wish == Vec3::ZERO {
        k.decel
    } else {
        k.accel
    };
    // On the ground the speed carried is the speed along the floor (Godot's constant floor
    // speed), not its level part: that is cos(grade) of it, and the shortfall made up at
    // `rate` would cap the speed on a steep floor at rate·dt / (1 − cos(grade)).
    let current = if grounded {
        level(body.velocity).normalize_or_zero() * body.velocity.length()
    } else {
        level(body.velocity)
    };
    approach(current, wish * top, rate * dt)
}

/// A body's velocity with nothing into what it met. On the ground, what it can stand on and
/// what is under its bottom sphere's centre (Jolt's supporting volume) is floor or the edge of
/// a step it stands on, not a wall: the velocity is already along the floor, and taking a
/// step's edge out of it would slow the body at every step and lift it there.
fn unblocked(mut velocity: Vec3, on_ground: bool, body: &Body, shape: &Shape) -> Vec3 {
    let bottom = (body.position - body.frame).as_vec3() + shape.up * shape.radius;
    for t in &body.active {
        let c = &t.contact;
        let under = (c.at - bottom).dot(shape.up) < -shape.padding;
        if on_ground && (under || !shape.too_steep(c.normal)) {
            continue;
        }
        if t.met || c.distance < shape.padding {
            let into = (velocity - c.velocity).dot(c.normal);
            if into < 0.0 {
                velocity -= c.normal * into;
            }
        }
    }
    velocity
}

/// The tick's frame: its places are offsets from where the body is, and the contacts kept
/// from the last tick are moved into it.
fn reframe(body: &mut Body) {
    let shift = (body.frame - body.position).as_vec3();
    for t in &mut body.active {
        t.contact.at += shift;
    }
    body.frame = body.position;
}

/// One body's tick: velocity, collide and slide, stick, step, ground state (Jolt's
/// `ExtendedUpdate`, with our velocity, jump and water rules).
fn step(body: &mut Body, intent: &Intent, query: &dyn Query, world: &World, dt: f32) {
    let up = up_at(body.position, world).unwrap_or(body.up);
    body.up = up;
    let query = &footing::NoStep::new(query, up, body.kind.step_height);
    reframe(body);
    let shape = shape(body, dt);
    let k = body.kind;
    let g = world.gravity;
    let from_centre = (body.position - world.centre.as_dvec3()).length() as f32;
    let depth = (world.water)(body.feet).map_or(0.0, |surface| surface - from_centre);
    let swimming = depth > k.swim_depth * k.height;

    if intent.jump {
        body.buffer = k.buffer_s + dt;
    }
    let standing = body.ground == Ground::Grounded;
    let coyote = !body.left_by_jump && body.air_s <= k.coyote_s + 1e-6;
    let jump = body.buffer > 0.0 && !swimming && (standing || coyote);

    let level = level_velocity(body, intent, swimming, dt);
    let vertical = body.velocity.dot(up);
    let (rise, rise_end) = if jump {
        (k.jump_speed, k.jump_speed - g * dt)
    } else if swimming {
        let under = (depth / k.height).clamp(0.0, 1.0);
        let lift = g * under / k.float_depth.max(1e-3) - g;
        let end = (vertical + lift * dt) * (1.0 - k.water_drag * dt).max(0.0);
        (vertical, end)
    } else if standing {
        (0.0, -g * dt)
    } else {
        (vertical, vertical - g * dt)
    };
    // The tick moves by the mean of its start and end speeds: exact under constant gravity.
    // Gravity's half tick presses a standing body into its floor: no wish to go along it, so
    // not what a step is judged by (Jolt's desired step is the level velocity, gravity on up).
    let press = body.normal * (g * dt * 0.5) * f32::from(u8::from(standing && !jump));
    let mut wanted = if standing && !jump {
        along_floor(level, body.normal, up) - press
    } else {
        level + up * (rise + rise_end) * 0.5
    };
    wanted = slide::cancel_steep(wanted, body.support, &body.active, &shape);

    let start = Vec3::ZERO;
    let was_supported = body.support.supported();
    let was_airborne = matches!(body.ground, Ground::Airborne);
    let mut feet = slide::move_shape(start, wanted, dt, query, &shape, &mut body.active);
    let (mut support, mut normal) = slide::support(&mut body.active, wanted, &shape, feet);
    let rising = (feet - start).dot(up) / dt > 1.0e-6;
    // Jolt sticks a body that left its floor and is not moving up. One walking on the ground
    // whose move rose is put back on its floor too (Unreal's floor kept under a walking body):
    // slid up the round of an edge, it stands over the edge, not on it.
    let fell_off = was_supported && !support.supported() && !rising;
    let floor_rise = along_floor(level, body.normal, up).dot(up) * dt;
    let rose = standing && (feet - start).dot(up) > floor_rise.max(0.0) + LIFT_M;
    if (fell_off || rose)
        && !jump
        && !swimming
        && let Some(stuck) = slide::stick_to_floor(feet, k.stick_m, query, &shape)
    {
        feet = stuck;
        slide::contacts_at(feet, query, &shape, &mut body.active);
        (support, normal) = slide::support(&mut body.active, Vec3::ZERO, &shape, feet);
    }
    body.stepped = 0.0;
    if support.supported() && !swimming {
        let stepped = slide::walk_stairs(
            start,
            feet,
            wanted + press,
            &body.active,
            query,
            &shape,
            k.step_height,
        );
        if let Some(stepped) = stepped {
            body.stepped = (stepped.feet - feet).dot(up);
            feet = stepped.feet;
            slide::contacts_at(feet, query, &shape, &mut body.active);
            (_, normal) = slide::support(&mut body.active, Vec3::ZERO, &shape, feet);
            support = Support::OnGround;
        }
    }
    // Perched on an edge too steep to stand on, over walkable floor within the stick distance
    // (Unreal's perch): standing.
    if matches!(support, Support::OnSteepGround) && !rising && !swimming {
        if let Some(floor) = slide::floor_below(feet, k.stick_m, query, &shape)
            && !shape.too_steep(floor.normal)
        {
            (support, normal) = (Support::OnGround, floor.normal);
        }
    } else if support == Support::OnGround {
        // The floor it walks along and is carried along is the one under its centre, not the
        // round of an edge its foot rests on (Unreal's floor; Jolt's surface normal): taken for
        // a ramp, that round lifts the body off the step's top and launches it. Its foot on an
        // edge over a lower floor, it walks along the floor it last stood on, level if none.
        normal = match slide::floor_stood_on(feet, k.stick_m, query, &shape) {
            Some(floor) => floor.normal,
            None if standing => body.normal,
            None => up,
        };
    }
    body.support = support;
    body.position = body.frame + feet.as_dvec3();
    body.feet = body.position.as_vec3();

    let velocity = if support == Support::OnGround && !jump {
        along_floor(level, normal, up)
    } else {
        level + up * rise_end
    };
    body.velocity = unblocked(
        velocity,
        support == Support::OnGround && !jump,
        body,
        &shape,
    );
    body.ground = if swimming {
        Ground::Swimming
    } else {
        match support {
            Support::OnGround => Ground::Grounded,
            Support::OnSteepGround => Ground::Steep,
            _ => Ground::Airborne,
        }
    };
    body.normal = if body.ground == Ground::Grounded {
        normal
    } else {
        Vec3::ZERO
    };
    // The speed it met the ground at, not the tick's end: what it fell at the tick's start and
    // what it gained over the height it dropped in it (exact under constant gravity).
    let dropped = (start - feet).dot(up).max(0.0);
    let falling = (-vertical).max(0.0);
    body.landed = (was_airborne && body.ground == Ground::Grounded)
        .then(|| libm::sqrtf(falling * falling + 2.0 * g * dropped));
    body.jumped = jump;
    if jump {
        (body.left_by_jump, body.buffer) = (true, 0.0);
    }
    if body.ground == Ground::Grounded {
        (body.air_s, body.left_by_jump) = (0.0, false);
    } else {
        body.air_s += dt;
    }
    body.buffer = (body.buffer - dt).max(0.0);
}
