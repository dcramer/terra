//! What a blow that reached a body does (D405), by how it was taken (`moves::judge`): a hit
//! costs health, holds both in a hit-stop and makes the struck flinch; a block costs no
//! health, only stamina (`stamina::guarded`), and shoves both apart; a parry staggers the
//! attacker; a heavy blow breaks a guard held too long, as does any blow on a guard with too
//! little stamina left to hold it; a roll's i-frames let it by.
//! And the fighter's own state those change, ticked in its own time.

use crate::moves::{Runner, Taken};
use glam::Vec3;

/// How far either side of straight ahead a guard covers, degrees.
pub const GUARD_DEG: f32 = 75.0;
/// The nearest a blow's drive takes a fighter to the other, feet to feet, metres: the
/// shields and sword arms touch about here, so a blow never carries one into the other.
pub const NEAREST_M: f32 = 1.3;
/// How long a shove from a blow taken on the guard lasts, seconds.
pub const SHOVE_S: f32 = 0.15;
/// How slow a body's own time runs in a hit-stop.
const STOPPED: f32 = 0.05;

/// Whether a guard facing `facing` covers a blow coming from `from` (both along the ground,
/// `from` toward the attacker).
pub fn guards(facing: Vec3, from: Vec3) -> bool {
    facing.dot(from) > libm::cosf(GUARD_DEG.to_radians())
}

/// A body thrown off its stride by a blow, for a while.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reel {
    /// Struck full on.
    Flinch,
    /// Its blow parried.
    Stagger,
    /// Its guard broken.
    Broken,
}

impl Reel {
    /// How long it stops a body acting, seconds of its own time.
    pub fn seconds(self) -> f32 {
        match self {
            Reel::Flinch => 0.45,
            Reel::Stagger => 1.2,
            Reel::Broken => 0.8,
        }
    }
}

/// What a blow does to one of the two.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Effect {
    pub damage: f32,
    pub reel: Option<Reel>,
    /// A shove along the way from the attacker to the struck, metres a second.
    pub shove_mps: f32,
}

/// What a blow does: the hit-stop both are held in, and its effect on the struck (`on`)
/// and on the attacker (`by`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outcome {
    pub stop_s: f32,
    pub on: Effect,
    pub by: Effect,
}

/// What a blow of `damage` and `hit_stop` does, as it was taken.
pub fn outcome(taken: Taken, damage: f32, hit_stop: f32) -> Outcome {
    let none = Effect::default();
    match taken {
        Taken::Hit => Outcome {
            stop_s: hit_stop,
            on: Effect {
                damage,
                reel: Some(Reel::Flinch),
                shove_mps: 0.0,
            },
            by: none,
        },
        Taken::Blocked => Outcome {
            stop_s: hit_stop * 0.6,
            on: Effect {
                shove_mps: 2.6,
                ..none
            },
            by: Effect {
                shove_mps: -1.2,
                ..none
            },
        },
        Taken::Parried => Outcome {
            stop_s: 0.12,
            on: none,
            by: Effect {
                reel: Some(Reel::Stagger),
                ..none
            },
        },
        Taken::GuardBroken => Outcome {
            stop_s: hit_stop,
            on: Effect {
                damage: damage * 0.3,
                reel: Some(Reel::Broken),
                shove_mps: 0.0,
            },
            by: none,
        },
        Taken::Dodged => Outcome::default(),
    }
}

/// A body in a fight: the move it is in, its health, its guard (how long it has been up), its
/// hit-stop, a flinch or stagger that stops it acting, being down, and a shove from a blow.
#[derive(Clone, Debug, PartialEq)]
pub struct Fighting {
    pub runner: Runner,
    pub health: f32,
    pub guard_s: Option<f32>,
    pub stop_s: f32,
    pub stagger_s: f32,
    pub down_s: Option<f32>,
    /// A shove along the ground, metres a second, and how long it has left.
    pub shove: Vec3,
    pub shove_s: f32,
}

impl Fighting {
    pub fn new(health: f32) -> Self {
        Self {
            runner: Runner::default(),
            health,
            guard_s: None,
            stop_s: 0.0,
            stagger_s: 0.0,
            down_s: None,
            shove: Vec3::ZERO,
            shove_s: 0.0,
        }
    }

    /// How fast its own time runs: all but still in a hit-stop.
    pub fn rate(&self) -> f32 {
        if self.stop_s > 0.0 { STOPPED } else { 1.0 }
    }

    /// Whether it can start something: not reeling, not down.
    pub fn free(&self) -> bool {
        self.stagger_s <= 0.0 && self.down_s.is_none()
    }

    /// Moves its timers on by a tick of the world's time.
    pub fn age(&mut self, dt: f32) {
        self.stop_s = (self.stop_s - dt).max(0.0);
        let own = dt * self.rate();
        self.stagger_s = (self.stagger_s - own).max(0.0);
        self.shove_s = (self.shove_s - own).max(0.0);
        if let Some(down) = &mut self.down_s {
            *down += dt;
        }
    }

    /// Holds it in a hit-stop at least this long.
    pub fn stop(&mut self, seconds: f32) {
        self.stop_s = self.stop_s.max(seconds);
    }

    /// Takes a blow's effect, shoved along `away` (from the attacker to the struck). Says
    /// what it now shows: a reel (its move cut short, its guard down), or being down when
    /// its health is gone; `None` when it was only shoved or untouched.
    pub fn take(&mut self, effect: Effect, away: Vec3) -> Option<Taking> {
        if effect.shove_mps != 0.0 {
            self.shove = away * effect.shove_mps;
            self.shove_s = SHOVE_S;
        }
        let reel = effect.reel?;
        self.health = (self.health - effect.damage).max(0.0);
        self.runner.clear();
        self.guard_s = None;
        if self.health <= 0.0 {
            self.down_s = Some(0.0);
            return Some(Taking::Down);
        }
        self.stagger_s = reel.seconds();
        Some(Taking::Reel(reel))
    }
}

/// What a body shows after a blow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Taking {
    Reel(Reel),
    Down,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_guard_covers_blows_from_ahead_and_to_either_side_but_not_from_behind() {
        let ahead = Vec3::Z;
        assert!(guards(ahead, Vec3::Z));
        assert!(guards(ahead, Vec3::new(1.0, 0.0, 0.5).normalize()));
        assert!(!guards(ahead, Vec3::X));
        assert!(!guards(ahead, -Vec3::Z));
    }

    #[test]
    fn a_hit_hurts_and_flinches_the_struck_and_a_parry_staggers_the_attacker() {
        let hit = outcome(Taken::Hit, 12.0, 0.1);
        assert_eq!(hit.on.damage, 12.0);
        assert_eq!(hit.on.reel, Some(Reel::Flinch));
        assert_eq!(hit.by, Effect::default());
        let parried = outcome(Taken::Parried, 12.0, 0.1);
        assert_eq!(parried.on.damage + parried.by.damage, 0.0);
        assert_eq!(parried.by.reel, Some(Reel::Stagger));
        // A block hurts no one and pushes the two apart: the struck away, the attacker back.
        let blocked = outcome(Taken::Blocked, 12.0, 0.1);
        assert_eq!(blocked.on.reel.or(blocked.by.reel), None);
        assert!(blocked.on.shove_mps > 0.0 && blocked.by.shove_mps < 0.0);
        // A broken guard lets some of the blow through; a dodge, none.
        let broken = outcome(Taken::GuardBroken, 30.0, 0.2);
        assert!(broken.on.damage > 0.0 && broken.on.damage < 30.0);
        assert_eq!(outcome(Taken::Dodged, 30.0, 0.2), Outcome::default());
    }

    #[test]
    fn a_reeling_body_cannot_act_until_its_reel_has_run_and_a_spent_one_goes_down() {
        let mut body = Fighting::new(20.0);
        body.runner.start("slash");
        body.guard_s = Some(0.4);
        let flinch = outcome(Taken::Hit, 12.0, 0.1);
        assert_eq!(
            body.take(flinch.on, Vec3::X),
            Some(Taking::Reel(Reel::Flinch))
        );
        assert!(!body.free() && body.runner.name().is_none() && body.guard_s.is_none());
        body.age(Reel::Flinch.seconds() + 0.01);
        assert!(body.free());
        assert_eq!(body.take(flinch.on, Vec3::X), Some(Taking::Down));
        assert!(!body.free() && body.health == 0.0);
    }

    #[test]
    fn a_hit_stop_all_but_freezes_a_bodys_own_time() {
        let mut body = Fighting::new(100.0);
        body.stagger_s = 1.0;
        body.stop(0.2);
        body.age(0.1);
        assert!((body.stagger_s - (1.0 - 0.1 * STOPPED)).abs() < 1e-6);
        body.age(0.2);
        assert_eq!(body.rate(), 1.0);
    }
}
