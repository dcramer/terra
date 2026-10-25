//! Collide and slide, ported from Jolt Physics 5.6.0's `CharacterVirtual`
//! (Jolt/Physics/Character/CharacterVirtual.cpp: `DetermineConstraints`, `SolveConstraints`,
//! `MoveShape`, `UpdateSupportingContact`, `WalkStairs`, `StickToFloor`; D53, D196). Our
//! departures: contacts and sweeps are asked of our own [`Query`] (the ground is a field, not a
//! mesh); other bodies' contacts never push a body out (pushing is by mass, after every body has
//! moved); a sweep's hit is kept as a contact for the rest of the tick, since a field answers
//! the nearest face alone and may hide one behind it, and a collision iteration stopped on such
//! a face goes on to solve against it; and a step up is refused when the floor it
//! lands on is higher than the step (Unreal's `StepUp`), so a capsule's round bottom does not
//! add to the step.
//!
//! Jolt's licence:
//!
//! Copyright 2021 Jorrit Rouwe
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy of this
//! software and associated documentation files (the "Software"), to deal in the Software
//! without restriction, including without limitation the rights to use, copy, modify, merge,
//! publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons
//! to whom the Software is furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all copies or
//! substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
//! INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR
//! PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE
//! FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
//! OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//! DEALINGS IN THE SOFTWARE.

use super::query::{Capsule, Contact, Query, TOLERANCE};
use glam::Vec3;

/// Jolt's `mMaxCollisionIterations`.
const COLLISION_ITERATIONS: usize = 5;
/// Jolt's `mMaxConstraintIterations`.
const CONSTRAINT_ITERATIONS: usize = 15;
/// Jolt's `mMinTimeRemaining`, s.
const MIN_TIME: f32 = 1.0e-4;
/// Jolt's `mPenetrationRecoverySpeed`: a penetration is resolved in one update.
const RECOVERY: f32 = 1.0;
/// Jolt's `ExtendedUpdateSettings`: the least step forward and the forward test, m, and the
/// most a contact may face away from the move and still be stepped up (75 deg).
const STEP_FORWARD_MIN: f32 = 0.02;
const STEP_FORWARD_TEST: f32 = 0.15;
const STEP_COS_FORWARD: f32 = 0.258_819;

/// A body's shape and how it meets the world in a tick.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub radius: f32,
    pub height: f32,
    pub up: Vec3,
    /// The cosine of the walkable limit.
    pub cos_limit: f32,
    pub predictive: f32,
    pub padding: f32,
    pub dt: f32,
}

impl Shape {
    pub fn capsule(&self, feet: Vec3) -> Capsule {
        Capsule::standing(feet, self.up, self.radius, self.height)
    }

    pub fn too_steep(&self, normal: Vec3) -> bool {
        normal.dot(self.up) < self.cos_limit
    }
}

/// A contact as a tick knows it: whether the body met it, Jolt's `mHadCollision`.
#[derive(Clone, Copy, Debug)]
pub struct Touch {
    pub contact: Contact,
    pub met: bool,
}

/// What a body stands on, Jolt's `EGroundState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    OnGround,
    OnSteepGround,
    NotSupported,
    InAir,
}

impl Support {
    pub fn supported(self) -> bool {
        matches!(self, Support::OnGround | Support::OnSteepGround)
    }
}

/// A contact plane, Jolt's `Constraint`.
#[derive(Clone, Copy, Debug)]
struct Constraint {
    touch: usize,
    velocity: Vec3,
    normal: Vec3,
    /// The plane's constant: its signed distance at no displacement.
    distance: f32,
    steep: bool,
    toi: f32,
    projected: f32,
}

/// Jolt's `DetermineConstraints`: a plane a contact, and an upright one more behind a slope
/// too steep to walk.
fn constraints(touches: &[Touch], shape: &Shape) -> Vec<Constraint> {
    let mut out = Vec::with_capacity(touches.len() * 2);
    for (i, t) in touches.iter().enumerate() {
        let c = &t.contact;
        let mut velocity = c.velocity;
        // Bodies are separated by mass after the tick; the ground pushes out at once.
        let distance = if c.body.is_some() {
            c.distance.max(0.0)
        } else {
            if c.distance < 0.0 {
                velocity -= c.normal * c.distance * RECOVERY / shape.dt;
            }
            c.distance
        };
        let mut plane = Constraint {
            touch: i,
            velocity,
            normal: c.normal,
            distance,
            steep: false,
            toi: 0.0,
            projected: 0.0,
        };
        if shape.too_steep(c.normal) {
            let dot = c.normal.dot(shape.up);
            if dot > 1.0e-3 {
                plane.steep = true;
                let normal = (c.normal - shape.up * dot).normalize();
                out.push(plane);
                plane = Constraint {
                    velocity: normal * c.velocity.dot(normal),
                    normal,
                    distance: distance / normal.dot(c.normal),
                    steep: false,
                    ..plane
                };
            }
        }
        out.push(plane);
    }
    out
}

/// Jolt's `SolveConstraints`: the displacement a velocity makes against the planes in the
/// time left, and how much of the time it took.
fn solve(
    velocity: Vec3,
    time: f32,
    planes: &mut [Constraint],
    touches: &mut [Touch],
    up: Vec3,
) -> (Vec3, f32) {
    if planes.is_empty() {
        return (velocity * time, time);
    }
    let mut remaining = time;
    let mut order: Vec<usize> = (0..planes.len()).collect();
    let (mut velocity, mut last_velocity) = (velocity, velocity);
    let (mut displacement, mut simulated) = (Vec3::ZERO, 0.0);
    let mut previous: Vec<usize> = Vec::with_capacity(CONSTRAINT_ITERATIONS);
    for _ in 0..CONSTRAINT_ITERATIONS {
        for c in planes.iter_mut() {
            c.projected = c.normal.dot(c.velocity - velocity);
            c.toi = if c.projected < 1.0e-6 {
                f32::MAX
            } else {
                let distance = c.normal.dot(displacement) + c.distance;
                if distance - c.projected * remaining > -1.0e-4 {
                    f32::MAX
                } else {
                    (distance / c.projected).max(0.0)
                }
            };
        }
        order.sort_by(|&a, &b| {
            let (l, r) = (&planes[a], &planes[b]);
            if l.toi <= 0.0 && r.toi <= 0.0 {
                return r.projected.total_cmp(&l.projected);
            }
            let ground = |c: &Constraint| touches[c.touch].contact.body.is_some();
            l.toi
                .total_cmp(&r.toi)
                .then(ground(l).cmp(&ground(r)))
                .then(a.cmp(&b))
        });
        let first = order[0];
        if planes[first].toi >= remaining {
            return (displacement + velocity * remaining, simulated + remaining);
        }
        touches[planes[first].touch].met = true;
        let toi = planes[first].toi;
        displacement += velocity * toi;
        remaining -= toi;
        simulated += toi;
        if remaining < MIN_TIME {
            return (displacement, simulated);
        }
        if toi > 1.0e-4 {
            previous.clear();
        }
        let normal = planes[first].normal;
        if planes[first].steep {
            // Cancel the velocity up the slope before its upright plane is met.
            let upright = normal - up * normal.dot(up);
            let relative = velocity - planes[first].velocity;
            velocity -= upright * relative.dot(upright).min(0.0) / upright.length_squared();
        }
        let relative = velocity - planes[first].velocity;
        let mut new_velocity = velocity - normal * relative.dot(normal);
        // The earlier plane this new velocity breaks most, to slide along both.
        let (mut deepest, mut other) = (0.0, None);
        for &p in &previous {
            if p == first {
                continue;
            }
            let other_normal = planes[p].normal;
            let into = (planes[p].velocity - new_velocity).dot(other_normal);
            if into > deepest {
                let dot = other_normal.dot(normal);
                if dot < 0.984 && dot > -0.984 {
                    deepest = into;
                    other = Some(p);
                }
            }
            let v = planes[first].velocity;
            planes[first].velocity -= other_normal * v.dot(other_normal).min(0.0);
            let v = planes[p].velocity;
            planes[p].velocity -= normal * v.dot(normal).min(0.0);
        }
        if let Some(p) = other {
            let slide = normal.cross(planes[p].normal).normalize();
            let along = |v: Vec3| v - slide * v.dot(slide);
            new_velocity = slide * new_velocity.dot(slide)
                + along(planes[first].velocity)
                + along(planes[p].velocity);
        }
        velocity = new_velocity;
        previous.push(first);
        if planes[first].projected < 1.0e-8 && velocity.length_squared() < 1.0e-8 {
            break;
        }
        if planes[first].velocity.length_squared() > 1.0e-16 {
            last_velocity = planes[first].velocity;
        } else if velocity.dot(last_velocity) < 0.0 {
            break;
        }
    }
    (displacement, simulated)
}

/// The distance from a capsule to a plane through a contact, for a hit kept from a sweep.
fn plane_distance(capsule: &Capsule, contact: &Contact) -> f32 {
    let along = |p: Vec3| (p - contact.at).dot(contact.normal);
    along(capsule.a).min(along(capsule.b)) - capsule.radius
}

/// Jolt's `MoveShape`: a velocity's move for a time against what the query answers, each
/// collision iteration solved on the contacts near where it starts and checked by a sweep.
/// Leaves the contacts of the last iteration, marked where the body met them, in `active`.
pub fn move_shape(
    feet: Vec3,
    velocity: Vec3,
    time: f32,
    query: &dyn Query,
    shape: &Shape,
    active: &mut Vec<Touch>,
) -> Vec3 {
    let mut feet = feet;
    let mut remaining = time;
    let mut kept: Vec<Contact> = Vec::new();
    let mut contacts = Vec::new();
    for _ in 0..COLLISION_ITERATIONS {
        if remaining < MIN_TIME {
            break;
        }
        let capsule = shape.capsule(feet);
        contacts.clear();
        query.contacts(&capsule, shape.predictive, &mut contacts);
        for hit in &kept {
            let distance = plane_distance(&capsule, hit);
            let known = contacts
                .iter()
                .any(|c: &Contact| c.normal.dot(hit.normal) > 0.999);
            if !known && distance < shape.predictive {
                contacts.push(Contact { distance, ..*hit });
            }
        }
        active.clear();
        active.extend(contacts.iter().map(|&contact| Touch {
            contact,
            met: false,
        }));
        let mut planes = constraints(active, shape);
        let (mut displacement, mut simulated) =
            solve(velocity, remaining, &mut planes, active, shape.up);
        let length = displacement.length();
        let mut unseen = false;
        if length > 1.0e-4
            && let Some(hit) = query.sweep(&capsule, displacement)
        {
            let fraction = (hit.fraction - shape.padding / length).max(0.0);
            unseen = !contacts
                .iter()
                .any(|c| c.normal.dot(hit.contact.normal) > 0.999);
            displacement *= fraction;
            simulated *= fraction;
            kept.push(hit.contact);
            let contact = Contact {
                distance: plane_distance(&capsule.moved(displacement), &hit.contact),
                ..hit.contact
            };
            active.push(Touch { contact, met: true });
        }
        feet += displacement;
        remaining -= simulated;
        // Stopped by a face the solve did not know of (a field's contact blends a shallow
        // crease's two faces into one): the next iteration solves against it too. Jolt stops
        // here, its contacts already holding every face.
        if displacement.length_squared() < 1.0e-8 && !unseen {
            break;
        }
    }
    feet
}

/// Contacts at a place, none met yet: for a body put somewhere, or moved by a step or a stick.
pub fn contacts_at(feet: Vec3, query: &dyn Query, shape: &Shape, active: &mut Vec<Touch>) {
    let mut contacts = Vec::new();
    query.contacts(&shape.capsule(feet), shape.predictive, &mut contacts);
    active.clear();
    active.extend(contacts.into_iter().map(|contact| Touch {
        contact,
        met: false,
    }));
}

/// Jolt's `UpdateSupportingContact`: the ground state and the ground's normal, from the
/// contacts met (and those within the tolerance the body is not leaving).
pub fn support(active: &mut [Touch], velocity: Vec3, shape: &Shape, feet: Vec3) -> (Support, Vec3) {
    for t in active.iter_mut() {
        let c = &t.contact;
        if !t.met && c.distance < TOLERANCE && c.normal.dot(velocity - c.velocity) <= 1.0e-4 {
            t.met = true;
        }
    }
    let bottom = feet + shape.up * shape.radius;
    let (mut supported, mut sliding, mut normals) = (0, 0, Vec3::ZERO);
    let (mut best, mut best_cos, mut deepest) = (None, f32::MIN, None::<Contact>);
    for t in active.iter().filter(|t| t.met) {
        let c = t.contact;
        let cos = c.normal.dot(shape.up);
        if deepest.is_none_or(|d| c.distance < d.distance) {
            deepest = Some(c);
        }
        // Only what is under the bottom sphere's centre holds a body up.
        if (c.at - bottom).dot(shape.up) > 0.0 {
            continue;
        }
        if cos > best_cos {
            (best, best_cos) = (Some(c), cos);
        }
        if cos >= shape.cos_limit {
            supported += 1;
        } else {
            sliding += 1;
        }
        if cos >= 0.08 {
            normals += c.normal;
        }
    }
    let normal = if normals.length_squared() > 0.0 {
        normals.normalize()
    } else {
        best.or(deepest).map_or(Vec3::ZERO, |c| c.normal)
    };
    let state = if supported > 0 {
        Support::OnGround
    } else if sliding > 0 {
        let deepest = deepest.expect("a sliding contact");
        if (velocity - deepest.velocity).dot(shape.up) > 1.0e-4 {
            Support::OnSteepGround
        } else {
            // Standing on several slopes that hold it is standing.
            let mut touches: Vec<Touch> = active.to_vec();
            let mut planes = constraints(&touches, shape);
            let (displacement, time) = solve(-shape.up, 1.0, &mut planes, &mut touches, shape.up);
            let least = 0.6 * shape.dt;
            if time < 0.001 || displacement.length_squared() < least * least {
                Support::OnGround
            } else {
                Support::OnSteepGround
            }
        }
    } else if best.or(deepest).is_some() {
        Support::NotSupported
    } else {
        Support::InAir
    };
    (state, normal)
}

/// Jolt's `CancelVelocityTowardsSteepSlopes`: on steep ground, no velocity into the slopes.
pub fn cancel_steep(velocity: Vec3, state: Support, active: &[Touch], shape: &Shape) -> Vec3 {
    if matches!(state, Support::OnGround | Support::InAir) {
        return velocity;
    }
    let mut velocity = velocity;
    for t in active
        .iter()
        .filter(|t| t.met && shape.too_steep(t.contact.normal))
    {
        let n = t.contact.normal - shape.up * t.contact.normal.dot(shape.up);
        let dot = n.dot(velocity);
        if dot < 0.0 && n.length_squared() > 1e-12 {
            velocity -= n * dot / n.length_squared();
        }
    }
    velocity
}

/// The steep contacts a level velocity pushes into, Jolt's `CanWalkStairs` test.
fn steep_into(active: &[Touch], velocity: Vec3, shape: &Shape) -> Vec<Vec3> {
    active
        .iter()
        .filter(|t| t.met)
        .map(|t| t.contact)
        .filter(|c| c.normal.dot(velocity - c.velocity) < 0.0 && shape.too_steep(c.normal))
        .map(|c| c.normal)
        .collect()
}

/// The ground under a sweep down from a place: the hit, and where the feet end.
fn floor_under(
    feet: Vec3,
    down: Vec3,
    query: &dyn Query,
    shape: &Shape,
) -> Option<(Vec3, Contact)> {
    let hit = query.sweep(&shape.capsule(feet), down)?;
    Some((feet + down * hit.fraction, hit.contact))
}

/// What a step up gives: where the feet end.
pub struct Stepped {
    pub feet: Vec3,
}

/// Jolt's `ExtendedUpdate` stair step and `WalkStairs`: blocked short of a level move by
/// steep contacts, try up the step, forward, and down onto walkable ground no higher than the
/// step, judging an edge's slope by a floor a little further on (the forward test).
#[allow(clippy::too_many_arguments)]
pub fn walk_stairs(
    start: Vec3,
    feet: Vec3,
    wished: Vec3,
    active: &[Touch],
    query: &dyn Query,
    shape: &Shape,
    step_height: f32,
) -> Option<Stepped> {
    let up_dir = shape.up;
    let level = |v: Vec3| v - up_dir * v.dot(up_dir);
    let wish_step = level(wished) * shape.dt;
    let wish_len = wish_step.length();
    if wish_len <= 0.0 || step_height <= 0.0 {
        return None;
    }
    let forward = wish_step / wish_len;
    let achieved = level(feet - start).dot(forward).max(0.0);
    let steep = steep_into(active, level(wished), shape);
    if achieved + 1.0e-4 >= wish_len || steep.is_empty() {
        return None;
    }
    // Step toward the contact that most opposes the move, within 75 deg of it.
    let mut direction = forward;
    let mut most = STEP_COS_FORWARD;
    for n in &steep {
        let test = (up_dir * n.dot(up_dir) - *n).normalize_or_zero();
        if test.dot(forward) > most {
            (direction, most) = (test, test.dot(forward));
        }
    }
    let step_forward = direction * STEP_FORWARD_MIN.max(wish_len - achieved);
    // Up.
    let mut up = up_dir * step_height;
    if let Some(hit) = query.sweep(&shape.capsule(feet), up) {
        if hit.fraction < 1.0e-6 {
            return None;
        }
        up *= hit.fraction;
    }
    let raised = feet + up;
    // Forward.
    let mut scratch = Vec::new();
    let moved = move_shape(
        raised,
        step_forward / shape.dt,
        shape.dt,
        query,
        shape,
        &mut scratch,
    );
    let across = level(moved - raised);
    if across.length_squared() < 1.0e-8 {
        return None;
    }
    let progress = steep.iter().any(|n| {
        let d = (up_dir * n.dot(up_dir) - *n).normalize_or_zero();
        d.dot(step_forward.normalize()) > std::f32::consts::FRAC_1_SQRT_2
            && d.dot(across) > 0.05 * step_forward.length()
    });
    if !progress {
        return None;
    }
    // Down, as far as it went up.
    let (landed, contact) = floor_under(moved, -up, query, shape)?;
    let mut normal = contact.normal;
    if shape.too_steep(normal) {
        let test = move_shape(
            raised,
            direction * STEP_FORWARD_TEST / shape.dt,
            shape.dt,
            query,
            shape,
            &mut scratch,
        );
        if level(test - raised).length_squared() <= across.length_squared() + 1.0e-8 {
            return None;
        }
        let (_, further) = floor_under(test, -up, query, shape)?;
        normal = further.normal;
        if shape.too_steep(normal) {
            return None;
        }
    }
    // Unreal's StepUp: no floor higher than the step, measured from the floor it left.
    if (contact.at - start).dot(up_dir) > step_height + TOLERANCE {
        return None;
    }
    if (landed - feet).dot(normal) < 1.0e-4 {
        return None;
    }
    Some(Stepped { feet: landed })
}

/// Jolt's `StickToFloor`: a body that was on the ground and is not rising is swept down onto
/// what is under it, up to the stick distance.
pub fn stick_to_floor(feet: Vec3, distance: f32, query: &dyn Query, shape: &Shape) -> Option<Vec3> {
    floor_under(feet, -shape.up * distance, query, shape).map(|(feet, _)| feet)
}
