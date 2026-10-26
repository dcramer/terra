//! A swimmer's figure (D478, docs/controller.md "What is presentation"): which swim clip shows
//! it and how its body lies. Treading water at rest, a stroke at the surface, a dive's stroke
//! under it; the body laid along its stroke, level at the surface and pitched along its way
//! under water, as Subnautica's divers and Unreal's swimmers are tilted by their pitch, eased
//! in and out as a wader starts to swim and comes out (BotW, Valheim, Enshrouded blend the
//! surface swim the same way). The strokes are made standing (Kimodo holds no swimming: a crawl is
//! a windmill of the arms, a dive's pull an overhead sweep, art/animation/made.json), so
//! the tread and the strokes blend in the clip's frame and only this tilt lays them down. The
//! feet are let go while swimming (`feet`: a swimmer is off the ground). What the server never
//! sees: the way is read from the body's velocity, which every client has.

use crate::character::Body;
use crate::stepping::Stepping;
use crate::{Motion, Player};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

/// A swimmer strokes going faster than this, m/s, and treads water again below `STROKE_UNTIL`.
const STROKE_FROM: f32 = 0.6;
const STROKE_UNTIL: f32 = 0.4;
/// How far over level a stroke at the surface lies, rad: the head and shoulders up out of the
/// water, as a breaststroke at the surface is swum (10 deg).
const SURFACE_LIFT: f32 = 10.0 * PI / 180.0;
/// The most a dive lies past level, rad: nearly head down (170 deg from upright).
const MOST_TILT: f32 = 170.0 * PI / 180.0;
/// How fast the shown lie follows the stroke's, 1/s: 95% of the way in half a second.
const EASE_HZ: f32 = 6.0;

/// What a swimmer's figure shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Swim {
    /// At rest in deep water: upright, treading water.
    Tread,
    /// Stroking with the mouth out: at the surface.
    Stroke,
    /// Stroking with the mouth under: a dive.
    Dive,
}

impl Swim {
    pub fn clip(self) -> &'static str {
        match self {
            Swim::Tread => "Swim_Tread_Loop",
            Swim::Stroke => "Swim_Fwd_Loop",
            Swim::Dive => "Swim_Dive_Loop",
        }
    }
}

/// How fast a body goes, every way: a swimmer's stroke is along its whole velocity.
fn speed(motion: &Motion) -> f32 {
    (motion.velocity.length_squared() + motion.rising * motion.rising).sqrt()
}

/// What a body's figure shows swimming, if it swims; `stroking` whether it stroked last frame,
/// so a speed at the threshold does not flicker between treading and stroking.
pub fn swim(motion: &Motion, stroking: bool) -> Option<Swim> {
    if !motion.swimming {
        return None;
    }
    let from = if stroking { STROKE_UNTIL } else { STROKE_FROM };
    Some(if speed(motion) <= from {
        Swim::Tread
    } else if motion.under {
        Swim::Dive
    } else {
        Swim::Stroke
    })
}

/// How far from upright the body lies, rad, its head toward where it faces: none treading or
/// out of the water, along its way stroking (a quarter turn level, more diving down), and at
/// the surface level but for its head and shoulders out; a stroke heading down from the surface
/// (a dive begun, the mouth not yet under) lets its head go down with it, the lift gone by
/// `SURFACE_LIFT` past level, so it lies along its way as the dive's stroke takes over.
pub fn lie(motion: &Motion, swim: Option<Swim>) -> f32 {
    let along = || {
        let level = motion.velocity.length();
        FRAC_PI_2 - libm::atan2f(motion.rising, level)
    };
    match swim {
        None | Some(Swim::Tread) => 0.0,
        Some(Swim::Stroke) => {
            let along = along();
            if along <= FRAC_PI_2 {
                along.min(FRAC_PI_2 - SURFACE_LIFT)
            } else {
                along - SURFACE_LIFT * (1.0 - ((along - FRAC_PI_2) / SURFACE_LIFT).min(1.0))
            }
        }
        Some(Swim::Dive) => along(),
    }
    .clamp(0.0, MOST_TILT)
}

/// On a player: how its figure lies, from upright toward where it faces (rad), and the lie its
/// swim asks; the figure eases from one to the other about `pivot`, metres over the feet,
/// where a floating body's surface is (the kind's float depth), so a laid swimmer's back is
/// awash rather than sunk to its feet.
#[derive(Component, Debug, Clone, Copy)]
pub struct Lie {
    pub shown: f32,
    pub asked: f32,
    pub pivot: f32,
}

impl Default for Lie {
    fn default() -> Self {
        Self {
            shown: 0.0,
            asked: 0.0,
            pivot: 1.35,
        }
    }
}

impl Lie {
    /// A frame of `dt` seconds: the lie shown eased toward the one asked.
    pub fn ease(&mut self, dt: f32) {
        let gap = self.asked - self.shown;
        self.shown = if gap.abs() < 1e-4 {
            self.asked
        } else {
            self.shown + gap * (1.0 - libm::expf(-dt * EASE_HZ))
        };
    }

    /// The figure's place under its player, `below` it after a step up (`stepping`): turned
    /// about the player's right by the lie shown, about the pivot, and faced as the model
    /// faces (+Z; things here face -Z).
    pub fn place(&self, below: f32) -> (Vec3, Quat) {
        let turn = Quat::from_rotation_x(-self.shown);
        let pivot = Vec3::Y * self.pivot;
        let at = pivot - turn * pivot - Vec3::Y * below;
        (at, turn * Quat::from_rotation_y(PI))
    }
}

/// Each frame, after the step's smoothing: each player's figure laid as its swim asks.
pub(crate) fn lay(
    time: Res<Time>,
    movers: Option<Res<crate::movement::Movers>>,
    mut players: Query<(&mut Lie, Option<&Stepping>, &Children), With<Player>>,
    mut figures: Query<&mut Transform, (With<Body>, Without<Player>)>,
) {
    let pivot = movers.map(|movers| movers.kind().float_depth * movers.kind().height);
    for (mut lie, stepping, children) in &mut players {
        if let Some(pivot) = pivot {
            lie.pivot = pivot;
        }
        let was = lie.shown;
        lie.ease(time.delta_secs());
        if was == 0.0 && lie.shown == 0.0 {
            continue;
        }
        let (at, turned) = lie.place(stepping.map_or(0.0, |s| s.below));
        for child in children {
            if let Ok(mut figure) = figures.get_mut(*child) {
                figure.translation = at;
                figure.rotation = turned;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn swimmer(level: f32, rising: f32, under: bool) -> Motion {
        Motion {
            velocity: Vec3::new(0.0, 0.0, -level),
            rising,
            swimming: true,
            under,
            ..Motion::facing(Vec3::NEG_Z)
        }
    }

    #[test]
    fn a_swimmer_treads_at_rest_strokes_at_the_surface_and_dives_under() {
        let walker = Motion::facing(Vec3::NEG_Z);
        assert_eq!(swim(&walker, false), None);
        assert_eq!(swim(&swimmer(0.1, 0.0, false), false), Some(Swim::Tread));
        assert_eq!(swim(&swimmer(2.0, 0.0, false), false), Some(Swim::Stroke));
        assert_eq!(swim(&swimmer(1.0, -1.0, true), false), Some(Swim::Dive));
        // Between the thresholds it keeps what it did.
        assert_eq!(swim(&swimmer(0.5, 0.0, false), false), Some(Swim::Tread));
        assert_eq!(swim(&swimmer(0.5, 0.0, false), true), Some(Swim::Stroke));
    }

    #[test]
    fn a_stroke_lies_along_its_way_and_a_tread_stands() {
        let deg = |r: f32| r.to_degrees();
        let level = swimmer(2.0, 0.0, false);
        assert!((deg(lie(&level, Some(Swim::Stroke))) - 80.0).abs() < 0.1);
        // Diving 45 deg down lies 135 deg from upright: along the way it goes.
        let down = swimmer(1.0, -1.0, true);
        assert!((deg(lie(&down, Some(Swim::Dive))) - 135.0).abs() < 0.1);
        let up = swimmer(0.0, 1.5, true);
        assert!(deg(lie(&up, Some(Swim::Dive))).abs() < 0.1);
        // Heading 34 deg down from the surface, the mouth not yet under: along its way, and
        // through level the lie goes on without a jump.
        let going_down = swimmer(1.47, -1.0, false);
        let along = 90.0 + deg(libm::atan2f(1.0, 1.47));
        assert!((deg(lie(&going_down, Some(Swim::Stroke))) - along).abs() < 0.1);
        let lies = |rising: f32| deg(lie(&swimmer(1.0, rising, false), Some(Swim::Stroke)));
        assert!((lies(-0.0001) - lies(0.0)).abs() < 0.1);
        assert!(lies(-0.1) > lies(0.0));
        assert_eq!(lie(&level, Some(Swim::Tread)), 0.0);
        assert_eq!(lie(&level, None), 0.0);
    }

    #[test]
    fn a_laid_figure_turns_head_first_about_the_surface_and_eases_there() {
        let mut lie = Lie {
            shown: 0.0,
            asked: FRAC_PI_2,
            pivot: 1.35,
        };
        for _ in 0..30 {
            lie.ease(1.0 / 60.0);
        }
        // Half a second: 95% of the way.
        assert!(lie.shown > 0.95 * FRAC_PI_2 * 0.99 && lie.shown < FRAC_PI_2);
        lie.shown = FRAC_PI_2;
        let (at, turned) = lie.place(0.0);
        // The model's up (its head) points where the player faces (-Z), its front down.
        let head = turned * Vec3::Y;
        assert!(head.distance(Vec3::NEG_Z) < 1e-5, "{head}");
        assert!((turned * Vec3::Z).distance(Vec3::NEG_Y) < 1e-5);
        // The pivot stays where it was: the surface over the feet.
        let pivot = Vec3::Y * 1.35;
        assert!((at + turned * Quat::from_rotation_y(PI).inverse() * pivot).distance(pivot) < 1e-5);
    }
}
