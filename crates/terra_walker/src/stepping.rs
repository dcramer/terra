//! Step smoothing (docs/controller.md "What is presentation"): a step up moves the mover's
//! capsule at once, as Jolt does; the figure is shown that much lower and rises to it over
//! about 0.12 s, so a step reads as a stride and not a pop (Unreal's mesh smoothing, the
//! Source engine's view smoothing). The mover never sees it; the camera follows the body.

use crate::Player;
use crate::character::Body;
use bevy::prelude::*;

/// How long the figure takes to rise to its body after a step up, s.
const RISE_S: f32 = 0.12;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, show);
}

/// How far below its body a player's figure is shown, after a step up, and how fast that
/// runs down to nothing.
#[derive(Component, Default, Debug)]
pub struct Stepping {
    pub below: f32,
    rate: f32,
}

impl Stepping {
    /// A step up of so many metres this tick: the figure stays where it was shown, and rises
    /// to the body over `RISE_S` from here.
    pub fn step(&mut self, by: f32) {
        if by > 0.0 {
            self.below += by;
            self.rate = self.below / RISE_S;
        }
    }

    /// A frame of so many seconds gone by.
    pub fn run_down(&mut self, dt: f32) {
        self.below = (self.below - self.rate * dt).max(0.0);
    }
}

/// Each frame: each player's figure shown its step's height below its body, run down.
fn show(
    time: Res<Time>,
    mut players: Query<(&mut Stepping, &Children), With<Player>>,
    mut figures: Query<&mut Transform, With<Body>>,
) {
    for (mut stepping, children) in &mut players {
        stepping.run_down(time.delta_secs());
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
    /// step in any frame at 60 a second, and has settled within 0.15 s.
    #[test]
    fn a_step_is_shown_risen_to_over_a_few_frames_and_settles_soon() {
        let mut stepping = Stepping::default();
        stepping.step(0.5);
        let frame = 1.0 / 60.0;
        let mut shown = Vec::new();
        for _ in 0..9 {
            stepping.run_down(frame);
            shown.push(stepping.below);
        }
        let mut before = 0.5;
        for &below in &shown {
            assert!(
                before - below <= 0.5 / 4.0 + 1e-6,
                "rose {}",
                before - below
            );
            before = below;
        }
        assert_eq!(*shown.last().unwrap(), 0.0, "settled by 0.15 s: {shown:?}");
        // No step, nothing shown.
        stepping.step(0.0);
        assert_eq!(stepping.below, 0.0);
    }
}
