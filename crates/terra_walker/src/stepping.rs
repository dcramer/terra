//! Step smoothing (docs/controller.md "What is presentation"): a step up moves the mover's
//! capsule at once, as Jolt does; the figure is shown that much lower and rises to it over
//! about 0.12 s, so a step reads as a stride and not a pop (Unreal's mesh smoothing, the
//! Source engine's view smoothing). The mover never sees it; the camera follows the body.
//!
//! The body is shown between ticks (`terra_world::tick`), so a step a tick took appears over
//! the next frame or two: the figure is held down by what of the step has appeared, frame by
//! frame, not by the whole step at the tick, or it would dip by the part not yet shown and
//! then jump (the filmed course's `step.smooth` saw a 0.3 m step dip 0.14 m, then rise 0.22 m
//! in one frame, tools/controller/yard.ts).

use crate::Player;
use crate::character::Body;
use bevy::prelude::*;

/// How long the figure takes to rise to its body after a step up, s.
const RISE_S: f32 = 0.12;
/// A step not yet shown is let go after so long: the body was put somewhere, not stepped.
const SHOWN_WITHIN_S: f32 = 0.1;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, show);
}

/// How far below its body a player's figure is shown, after a step up, and how fast that
/// runs down to nothing; and what of the steps taken has not yet been shown.
#[derive(Component, Default, Debug)]
pub struct Stepping {
    pub below: f32,
    rate: f32,
    /// Of the steps the ticks took, how much the shown body has not yet risen, and for how
    /// long.
    pending: f32,
    pending_s: f32,
    /// The shown body's height (its distance from the planet's centre) last frame.
    height: Option<f32>,
}

impl Stepping {
    /// A step up of so many metres this tick: the figure stays where it was shown as the
    /// body rises to it, and rises to the body over `RISE_S` from here.
    pub fn step(&mut self, by: f32) {
        if by > 0.0 {
            self.pending += by;
            self.pending_s = 0.0;
            self.rate = (self.below + self.pending) / RISE_S;
        }
    }

    /// A frame of so many seconds gone by, the body shown at `height` from the planet's
    /// centre: the figure rises toward it, held down by what of a step the body rose.
    pub fn show(&mut self, height: f32, dt: f32) {
        self.below = (self.below - self.rate * dt).max(0.0);
        if let Some(was) = self.height {
            let risen = (height - was).clamp(0.0, self.pending);
            self.below += risen;
            self.pending -= risen;
        }
        self.pending_s += dt;
        if self.pending_s > SHOWN_WITHIN_S {
            self.pending = 0.0;
        }
        self.height = Some(height);
    }
}

/// Each frame: each player's figure shown its step's height below its body, run down.
fn show(
    time: Res<Time>,
    mut players: Query<(&mut Stepping, &Transform, &Children), With<Player>>,
    mut figures: Query<&mut Transform, (With<Body>, Without<Player>)>,
) {
    for (mut stepping, body, children) in &mut players {
        stepping.show(body.translation.length(), time.delta_secs());
        for child in children {
            if let Ok(mut figure) = figures.get_mut(*child) {
                let y = -stepping.below;
                if figure.translation.y != y {
                    figure.translation.y = y;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Criterion 6 (docs/controller.md): the shown root rises no more than a quarter of the
    /// step in any frame at 60 a second, never dips, and has settled within 0.15 s, with the
    /// body shown rising to the step over the frames after the tick that took it, as the
    /// tick's interpolation shows it.
    #[test]
    fn a_step_is_shown_risen_to_over_a_few_frames_and_settles_soon() {
        let frame = 1.0 / 60.0;
        for appears in [1, 2, 3] {
            let mut stepping = Stepping::default();
            let ground = 2000.0;
            stepping.show(ground, frame);
            stepping.step(0.5);
            let mut shown = vec![ground];
            for i in 1..=9 {
                let body = ground + 0.5 * (i.min(appears) as f32 / appears as f32);
                stepping.show(body, frame);
                shown.push(body - stepping.below);
            }
            for pair in shown.windows(2) {
                let rose = pair[1] - pair[0];
                assert!(
                    (-1e-4..=0.5 / 4.0 + 1e-4).contains(&rose),
                    "{appears}: {shown:?}"
                );
            }
            assert!(
                (shown.last().unwrap() - (ground + 0.5)).abs() < 1e-4,
                "settled by 0.15 s: {shown:?}"
            );
        }
        // No step, nothing shown, and a slope's rise is never held back.
        let mut stepping = Stepping::default();
        stepping.step(0.0);
        stepping.show(10.0, frame);
        stepping.show(10.05, frame);
        assert_eq!(stepping.below, 0.0);
    }
}
