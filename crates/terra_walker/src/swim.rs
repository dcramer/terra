//! A swimmer's figure (D478, docs/controller.md "What is presentation"): which swim clip shows
//! it and how far it is pitched. Treading water at rest, a stroke at the surface, a dive's stroke
//! under it. The strokes are made lying in the water (Uthana's text-to-motion, D532), level and
//! afloat at the man's float line (art/animation/made.json `afloat`), so the clip holds the lie
//! (D584): the game only pitches a stroke along its way up or down, as Unreal's swimmers and
//! Subnautica's divers are pitched by their velocity, eased in and out as a turn and a dive
//! begin; level at the surface, where the mover holds the body afloat. The tread is made
//! upright and is shown as made. The feet are let go while swimming (`feet`: a swimmer is off
//! the ground). What the server never sees: the way is read from the body's velocity, which
//! every client has.

use crate::character::Body;
use crate::stepping::Stepping;
use crate::{Motion, Player};
use bevy::prelude::*;
use std::f32::consts::PI;

/// A swimmer strokes going faster than this, m/s, and treads water again below `STROKE_UNTIL`.
const STROKE_FROM: f32 = 0.6;
const STROKE_UNTIL: f32 = 0.4;
/// How fast the shown pitch follows the stroke's way, 1/s: 95% of the way in half a second.
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

/// How far a stroke's head is pitched up from level, rad (down below it): along the way it goes,
/// head first straight up or down at most; none treading or out of the water, where the clip is
/// shown as made.
pub fn pitch(motion: &Motion, swim: Option<Swim>) -> f32 {
    match swim {
        None | Some(Swim::Tread) => 0.0,
        Some(Swim::Stroke | Swim::Dive) => libm::atan2f(motion.rising, motion.velocity.length()),
    }
}

/// On a player: how far its figure is pitched up (rad), and the pitch its swim asks; the figure
/// eases from one to the other about `pivot`, metres over the feet, where a floating body's
/// surface is (the kind's float depth), which the strokes lie about.
#[derive(Component, Debug, Clone, Copy)]
pub struct Pitch {
    pub shown: f32,
    pub asked: f32,
    pub pivot: f32,
}

impl Default for Pitch {
    fn default() -> Self {
        Self {
            shown: 0.0,
            asked: 0.0,
            pivot: 1.35,
        }
    }
}

impl Pitch {
    /// A frame of `dt` seconds: the pitch shown eased toward the one asked.
    pub fn ease(&mut self, dt: f32) {
        let gap = self.asked - self.shown;
        self.shown = if gap.abs() < 1e-4 {
            self.asked
        } else {
            self.shown + gap * (1.0 - libm::expf(-dt * EASE_HZ))
        };
    }

    /// The figure's place under its player, `below` it after a step up (`stepping`): turned
    /// about the player's right by the pitch shown, about the pivot, and faced as the model
    /// faces (+Z; things here face -Z).
    pub fn place(&self, below: f32) -> (Vec3, Quat) {
        let turn = Quat::from_rotation_x(self.shown);
        let pivot = Vec3::Y * self.pivot;
        let at = pivot - turn * pivot - Vec3::Y * below;
        (at, turn * Quat::from_rotation_y(PI))
    }
}

/// Each frame, after the step's smoothing: each player's figure pitched as its swim asks.
pub(crate) fn lay(
    time: Res<Time>,
    movers: Option<Res<crate::movement::Movers>>,
    mut players: Query<(&mut Pitch, Option<&Stepping>, &Children), With<Player>>,
    mut figures: Query<&mut Transform, (With<Body>, Without<Player>)>,
) {
    let pivot = movers.map(|movers| movers.kind().float_depth * movers.kind().height);
    for (mut pitch, stepping, children) in &mut players {
        if let Some(pivot) = pivot {
            pitch.pivot = pivot;
        }
        let was = pitch.shown;
        pitch.ease(time.delta_secs());
        if was == 0.0 && pitch.shown == 0.0 {
            continue;
        }
        let (at, turned) = pitch.place(stepping.map_or(0.0, |s| s.below));
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

    /// Where a level stroke's head points once its figure is placed: the clips lie head first
    /// along the model's front (+Z), as made.
    fn head(motion: &Motion, swim: Swim) -> Vec3 {
        let mut figure = Pitch {
            asked: pitch(motion, Some(swim)),
            ..Pitch::default()
        };
        figure.shown = figure.asked;
        figure.place(0.0).1 * Vec3::Z
    }

    #[test]
    fn a_level_stroke_points_its_head_along_the_way_it_goes() {
        // At the surface: level, as the clip was made, ahead (-Z).
        let level = swimmer(2.0, 0.0, false);
        assert!(head(&level, Swim::Stroke).distance(Vec3::NEG_Z) < 1e-5);
        // Diving 45 deg down, rising 30 deg up, and straight up or down: the head along the way.
        for (level, rising) in [(1.0, -1.0), (1.5, 0.866), (0.0, 1.5), (0.0, -1.5)] {
            let motion = swimmer(level, rising, true);
            let way = Vec3::new(0.0, rising, -level).normalize();
            let got = head(&motion, Swim::Dive);
            assert!(got.distance(way) < 1e-4, "{got} against {way}");
        }
        // A tread, and a body out of the water, are shown as made.
        assert_eq!(pitch(&swimmer(0.1, -1.0, true), Some(Swim::Tread)), 0.0);
        assert_eq!(pitch(&level, None), 0.0);
    }

    #[test]
    fn a_pitched_figure_turns_about_the_surface_and_eases_there() {
        let mut figure = Pitch {
            shown: 0.0,
            asked: -PI / 4.0,
            pivot: 1.35,
        };
        for _ in 0..30 {
            figure.ease(1.0 / 60.0);
        }
        // Half a second: 95% of the way.
        assert!(figure.shown < 0.95 * -PI / 4.0 * 0.99 && figure.shown > -PI / 4.0);
        // Unpitched, the figure stands where the player is, faced as it faces.
        let (at, turned) = Pitch::default().place(0.0);
        assert!(at.length() < 1e-6 && (turned * Vec3::Z).distance(Vec3::NEG_Z) < 1e-6);
        // The pivot stays where it was: the surface over the feet.
        figure.shown = -PI / 4.0;
        let (at, turned) = figure.place(0.0);
        let pivot = Vec3::Y * 1.35;
        assert!((at + turned * Quat::from_rotation_y(PI).inverse() * pivot).distance(pivot) < 1e-5);
    }
}
