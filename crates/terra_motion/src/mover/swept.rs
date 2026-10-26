//! Bodies closing fast never pass through each other (D53, `tunnel.closing`). A step meets
//! the others as the tick began (Jacobi), each by its core, so two bodies closing faster than
//! a core's width from both sides in one tick could each stop short of where the other was
//! and cross. After the steps, a fast body's core is cast along its step relative to each
//! other body's (its step's motion less theirs, from where both began), and where the cores
//! touch before the step's end both are put back along their steps to that moment: Jolt's
//! continuous collision (`PhysicsSystem::JobFindCCDContacts`, MIT: only a body moving farther
//! than `mLinearCastThreshold`, 0.75 of its shape's inner radius, is cast, along its motion
//! less the other's), taken back to the time of impact as Jolt's `JobResolveCCDContacts`
//! takes its body back. We depart in putting both bodies of a pair back, not the first alone,
//! since either going on would cross the other; and what each is put back by is swept against
//! the ground ([`push::shove`]), so it never goes into a wall.
//!
//! A pair neither of whose steps is fast cannot cross: a step keeps a body's surface off the
//! other's core where it began, so the cores stay apart unless the other moved more than the
//! first's radius less its core. Only fast bodies are cast; a crowd walking has none, and pays
//! nothing but a length a body.

use super::query::{self, Capsule, Query};
use super::{Before, Body, core_at, push};
use glam::{DVec3, Vec3};

/// The share of the least of the bodies' radius less their core a step must be longer than
/// for its body to be cast (Jolt's `mLinearCastThreshold`).
const FAST_SHARE: f32 = 0.75;
/// How far short of touching two cores are put back, m.
const CLEAR_M: f32 = 2.0e-3;
/// How much farther than a body's reach its cast looks the others up, m: the f32 offsets'
/// rounding.
const SLACK_M: f64 = 0.05;

/// Puts back to their first time of impact every pair of bodies whose cores the tick's steps
/// would carry into each other, each by its step along its own way, swept against the ground,
/// its velocity into the other taken out. The bodies are in ID order, each having stepped
/// from `Body::frame`; the same bits in any order and on any number of threads.
pub(super) fn stop(bodies: &mut [Body], before: &Before, ground: &dyn Query) {
    let steps: Vec<Vec3> = bodies
        .iter()
        .map(|b| (b.position - b.frame).as_vec3())
        .collect();
    let least = bodies
        .iter()
        .map(|b| b.kind.radius - core_at(b, b.frame).radius)
        .fold(f32::INFINITY, f32::min);
    let from = FAST_SHARE * least.max(0.0);
    let fast: Vec<bool> = steps.iter().map(|s| s.length() > from).collect();
    if !fast.contains(&true) {
        return;
    }
    let longest = steps.iter().map(|s| s.length()).fold(0.0, f32::max);
    let cores: Vec<Capsule> = bodies.iter().map(|b| core_at(b, b.frame)).collect();
    // Each body's first impact: the share of its step it keeps, and the way from the other.
    let mut first: Vec<Option<(f32, Vec3)>> = vec![None; bodies.len()];
    let mut near = Vec::new();
    for i in (0..bodies.len()).filter(|&i| fast[i]) {
        let me = &cores[i];
        let (a, b) = (me.world(me.a), me.world(me.b));
        let by = steps[i].as_dvec3();
        let (lo, hi) = (
            a.min(b).min(a + by).min(b + by),
            a.max(b).max(a + by).max(b + by),
        );
        match before.grid.as_ref() {
            // The grid keeps the others where they began: looked up as far as the farthest a
            // core reaches from its middle and the longest step, beyond this one's own.
            Some(grid) => {
                let r = f64::from(me.radius + before.extent + longest) + SLACK_M;
                grid.near(lo - DVec3::splat(r), hi + DVec3::splat(r), &mut near);
            }
            None => {
                near.clear();
                near.extend(0..bodies.len() as u32);
            }
        }
        for &j in &near {
            let j = j as usize;
            // Each pair once: a fast pair by its first body.
            if j == i || (fast[j] && j < i) {
                continue;
            }
            let motion = steps[i] - steps[j];
            let Some(hit) = query::impact(me, motion, &cores[j]) else {
                continue;
            };
            let length = motion.length().max(1e-6);
            let kept = (hit.fraction - CLEAR_M / length).max(0.0);
            let normal = hit.contact.normal;
            for (k, way) in [(i, normal), (j, -normal)] {
                if first[k].is_none_or(|(t, _)| kept < t) {
                    first[k] = Some((kept, way));
                }
            }
        }
    }
    for ((body, step), first) in bodies.iter_mut().zip(&steps).zip(&first) {
        let Some((kept, way)) = *first else {
            continue;
        };
        push::shove(body, -*step * (1.0 - kept), ground);
        let into = body.velocity.dot(way);
        if into < 0.0 {
            body.velocity -= way * into;
        }
    }
}
