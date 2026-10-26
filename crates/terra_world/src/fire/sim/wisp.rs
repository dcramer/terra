//! Shed wisps over a baked loop (D493, docs/fire-module.md's "Shed wisps"): a filmed campfire's
//! tongue now and then lets go of its tip, which rises away faster than the tongue and is gone
//! in a tenth of a second or two (a shed, motion.ts); our Mantaflow bakes do not tear so (fire
//! rounds g23, g34, g35). So a baked fire sheds its own, as shipped games add pieces over a baked
//! flame ("secondary flame licks"): small soft ellipsoids of flame that the same marcher draws
//! (`fire.wesl`'s `wisp_flame`), each leaving one of the tongue tips the baker lists for the
//! frame it is born in, at the gas's velocity there plus the game's wind beyond the bake's,
//! shrinking and thinning through the drawn flame's edge over its life.
//!
//! Each fire has `WISPS` lanes, each lane a wisp at most in each `period`, born at a time and
//! from a tip of its own seed's choosing, so a fire's wisps are known at any time from its seed
//! and its place in the loop alone, nothing kept from frame to frame, and no two fires shed alike.

use super::{Placement, Sim};
use bevy::prelude::*;

/// The wisps a fire draws at once at most: one a lane. Must match `WISPS` in fire.wesl.
pub const WISPS: usize = 4;
/// How many wisps each tongue tip of the loop sheds a second: filmed campfires shed 0.75-1.7 a
/// tongue a second (g26's targets, from motion.ts on the filmed references), and not every wisp
/// is seen to (fire round g37: at 1.5 about one in sixteen was).
const TIP_RATE: f32 = 3.0;
/// The most tips a frame lists (the baker's `TIPS`).
const TIPS: usize = 3;
/// How long a wisp burns, seconds at the bake's size (longer for a larger fire, as its loop
/// plays slower): a filmed tongue's life, 0.04-0.12 s, so it rises at most about half the
/// flame's height over the tip it left (at 0.06-0.16 s it rose past the flame's own height and
/// made the measured flame taller, fire round g37).
const LIFE_S: [f32; 2] = [0.04, 0.12];
/// Its half width at birth, metres at the bake's size: about 8 cm across, a tongue tip's width,
/// so it is as thick as the tip it left and seen as a piece of flame, not a spark (at 5 cm and a
/// flame of 0.6 fading straight, few were seen to come free, fire round g37).
const RADIUS_M: f32 = 0.04;
/// Its flame at birth, in the bake's units (the shader's BAKED_EDGE 0.08 is the drawn edge): a
/// tongue's body's, held as a burning pocket's is and falling away as it burns out.
const FLAME: f32 = 0.9;
/// How far along its way it reaches each side of its middle, besides its half width: the way it
/// goes in half a 30th of a second, as a velocity-aligned sprite is stretched, so a fast wisp
/// reads as one piece from frame to frame (a film's 30th of a second moves it 18 cm at the
/// tips' median 5.4 m/s), not a bead that jumps.
const SMEAR_S: f32 = 0.5 / 30.0;
/// The wind the bake was simulated in (its sidecar: a gusting 1.8 m/s mean down its +x); the
/// game's beyond it carries a wisp on downwind.
const BAKE_WIND_MPS: f32 = 1.8;
/// A tip whose gas is not rising at least so fast, m/s, sheds nothing: a shed rises away.
const LEAST_RISE_MPS: f32 = 0.5;
/// How far over the bake's box a wisp reaches at most, metres at the bake's size; the marcher's
/// box grows up by it. Must match `WISP_REACH` in fire.wesl.
const REACH_M: f32 = 0.6;

/// A lane's time for one wisp at most, seconds: so the lanes together shed `TIP_RATE` a tip a
/// second when a frame lists `TIPS`, and longer than any wisp lives, so a lane has one at a time.
fn period() -> f32 {
    WISPS as f32 / (TIPS as f32 * TIP_RATE)
}

/// One wisp: when it was born and how long it lives (seconds on the cards' clock), and where it
/// left from and how fast it goes (its fire's frame: east, up, north; metres, m/s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wisp {
    pub born: f32,
    pub life: f32,
    pub from: Vec3,
    pub velocity: Vec3,
}

impl Wisp {
    /// What `fire.wesl`'s `wisp_flame` reads of it at `time`: its middle and its flame, its axis
    /// (along its way, as long as it reaches) and its half width; none once it is gone or past
    /// the marcher's box.
    pub fn drawn(&self, time: f32, scale: f32, sim: &Sim) -> Option<[Vec4; 2]> {
        let age = time - self.born;
        if age < 0.0 || age >= self.life {
            return None;
        }
        let t = age / self.life;
        let at = self.from + self.velocity * age;
        let radius = RADIUS_M * scale * (1.0 - 0.5 * t);
        let speed = self.velocity.length();
        let way = if speed > 1.0e-3 {
            self.velocity / speed
        } else {
            Vec3::Y
        };
        let reach = radius + speed * SMEAR_S;
        let top = (sim.box_min[2] + sim.box_size[2] + REACH_M) * scale;
        if at.y + reach * way.y.abs() > top {
            return None;
        }
        Some([
            at.extend(FLAME * (1.0 - t * t)),
            (way * reach).extend(radius),
        ])
    }
}

/// A fire's wisps alive at `time`, a lane each: `downwind` the wind across its frame (east,
/// north), as its loop is turned by.
pub fn alive(
    sim: &Sim,
    placed: Placement,
    seed: f32,
    downwind: Vec2,
    time: f32,
) -> [Option<Wisp>; WISPS] {
    let p = period();
    let slot = libm::floorf(time / p) as i64;
    std::array::from_fn(|lane| {
        // A wisp born late in the slot before may live on into this one.
        [slot, slot - 1].into_iter().find_map(|k| {
            let w = born(sim, placed, seed, downwind, lane as u32, k)?;
            (time >= w.born && time < w.born + w.life).then_some(w)
        })
    })
}

/// The wisp a lane sheds in its slot `k`, if the tip it picks is listed in the frame it is born
/// in and rising.
fn born(
    sim: &Sim,
    placed: Placement,
    seed: f32,
    downwind: Vec2,
    lane: u32,
    k: i64,
) -> Option<Wisp> {
    if k < 0 {
        return None;
    }
    let luck = |salt: u32| {
        terra_gen::hash::unit(terra_gen::hash::of(&[
            seed.to_bits(),
            0x5749_5350,
            lane,
            k as u32,
            salt,
        ]))
    };
    let born = (k as f32 + luck(0)) * period();
    let (a, b, blend) = sim.frames_at(born, placed);
    let frame = if blend < 0.5 { a } else { b };
    let pick = ((luck(1) * TIPS as f32) as usize).min(TIPS - 1);
    let tip = sim.tips.get(frame as usize)?.get(pick)?;
    if tip[5] < LEAST_RISE_MPS {
        return None;
    }
    let s = placed.scale;
    // Its loop plays at `rate`, so the bake's metres a second are `s * rate` of the fire's.
    let speed = s * placed.rate();
    let blown = downwind.length();
    let beyond = if blown > BAKE_WIND_MPS {
        downwind * (blown - BAKE_WIND_MPS) / blown
    } else {
        Vec2::ZERO
    };
    Some(Wisp {
        born,
        life: (LIFE_S[0] + (LIFE_S[1] - LIFE_S[0]) * luck(2)) / placed.rate(),
        from: placed.to_frame(Vec3::new(tip[0], tip[1], tip[2])) * s,
        velocity: placed.to_frame(Vec3::new(tip[3], tip[4], tip[5])) * speed
            + Vec3::new(beyond.x, 0.0, beyond.y),
    })
}

#[cfg(test)]
mod tests;
