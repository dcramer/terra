//! A scripted run held while what it put in the world mid-run is still coming (D135).
//!
//! The loading screen waits for everything at the start, and a script's frames are counted
//! from its end. A script also puts things in the world as it goes, such as a campfire
//! staged on frame 2, and nothing waited for those: on a busy machine (a render worker at a
//! load of 40) the campfire's model had not loaded by the shot on frame 330, five seconds
//! on, so whole hours of the fire sheet came out with no logs or stones. So a scripted run
//! stands still while a file asked to be waited for ([`super::Loading::wait_for`]) or a
//! model put in the world is still coming, and for the loading screen's quiet frames after:
//! its frames are not counted, so no call, press or shot happens, and the game clock is
//! held, so what a frame shows is what it shows on a fast machine. A bench is never held,
//! since a load's hitches are what it measures.

use super::{Loading, QUIET_FRAMES, Screen, asked_coming, placing};
use crate::shell::Options;
use bevy::{
    prelude::*,
    world_serialization::{WorldAssetRoot, WorldInstance, WorldInstanceSpawner},
};
use std::time::{Duration, Instant};

/// A hold that lasts longer than this is said, so a slow load shows in the run's log.
const SAY_AFTER: Duration = Duration::from_secs(1);

/// Whether a scripted run is held, and since when.
#[derive(Resource, Debug)]
pub struct Hold {
    /// Frames in a row with nothing coming.
    quiet: u32,
    /// When the hold began, while held.
    since: Option<Instant>,
    /// Whether the hold paused the game clock, so only it starts it again.
    paused_clock: bool,
    /// Gave up on what was coming: no hold again until nothing is.
    gave_up: bool,
}

impl Default for Hold {
    fn default() -> Self {
        Self {
            quiet: QUIET_FRAMES,
            since: None,
            paused_clock: false,
            gave_up: false,
        }
    }
}

/// How a frame changed the hold.
#[derive(Debug, PartialEq)]
enum Turn {
    Held,
    Released { waited: Duration, gave_up: bool },
}

impl Hold {
    /// Whether the script stands still this frame.
    pub fn holds(&self) -> bool {
        self.since.is_some()
    }

    /// One frame with `coming` things still coming, at `now`; the clock held while held.
    fn frame(&mut self, coming: usize, now: Instant, clock: &mut Time<Virtual>) -> Option<Turn> {
        self.quiet = if coming > 0 {
            0
        } else {
            self.quiet.saturating_add(1)
        };
        if coming == 0 {
            self.gave_up = false;
        }
        let turn = match self.since {
            None if coming > 0 && !self.gave_up => {
                self.since = Some(now);
                Some(Turn::Held)
            }
            Some(since) if self.quiet >= QUIET_FRAMES || now - since > super::GIVE_UP_SCRIPTED => {
                self.since = None;
                self.gave_up = self.quiet < QUIET_FRAMES;
                Some(Turn::Released {
                    waited: now - since,
                    gave_up: self.gave_up,
                })
            }
            _ => None,
        };
        match turn {
            Some(Turn::Held) if !clock.is_paused() => {
                clock.pause();
                self.paused_clock = true;
            }
            Some(Turn::Released { .. }) if self.paused_clock => {
                clock.unpause();
                self.paused_clock = false;
            }
            _ => {}
        }
        turn
    }
}

/// Holds a scripted run while what it put in the world is coming. Before anything counts
/// the frame (`shell::scripted_presses`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn watch(
    options: Res<Options>,
    screen: Res<State<Screen>>,
    mut hold: ResMut<Hold>,
    mut loading: ResMut<Loading>,
    assets: Res<AssetServer>,
    spawner: Res<WorldInstanceSpawner>,
    models: Query<(&WorldAssetRoot, Option<&WorldInstance>)>,
    (mut clock, frame): (ResMut<Time<Virtual>>, Res<crate::shell::ScriptFrame>),
) {
    if !options.scripted() || options.bench || *screen.get() != Screen::Playing {
        return;
    }
    let files = asked_coming(&mut loading, &assets);
    let models = placing(&assets, &spawner, &models);
    match hold.frame(files + models, Instant::now(), &mut clock) {
        Some(Turn::Released { waited, gave_up }) if gave_up => warn!(
            "Script frame {} held {} s for what never came, going on without it: {files} files, {models} models.",
            frame.0,
            waited.as_secs()
        ),
        Some(Turn::Released { waited, .. }) if waited > SAY_AFTER => info!(
            "Script frame {} held {:.1} s while what it put in the world loaded.",
            frame.0,
            waited.as_secs_f32()
        ),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::counting;

    /// A scripted run's frames over `coming` (what is coming on each real frame): which
    /// script frame each real frame was, and the game clock's seconds then.
    fn run(coming: &[usize]) -> Vec<(u32, f32)> {
        let options =
            Options::from_args(["--shots", "out/x@5"].map(String::from).into_iter()).unwrap();
        let screen = State::new(Screen::Playing);
        let mut hold = Hold::default();
        let mut clock = Time::<Virtual>::default();
        let (mut real, mut generic) = (Time::<Real>::default(), Time::default());
        real.update_with_duration(Duration::ZERO);
        let start = Instant::now();
        let mut script = 0;
        coming
            .iter()
            .enumerate()
            .map(|(i, &coming)| {
                // As the game's frame goes: the clock steps, then the hold, then the count.
                real.update_with_duration(Duration::from_millis(100));
                bevy::time::update_virtual_time(&mut generic, &mut clock, &real);
                hold.frame(coming, start + Duration::from_millis(i as u64), &mut clock);
                if counting(&options, &screen, &hold) {
                    script += 1;
                }
                (script, clock.elapsed_secs())
            })
            .collect()
    }

    #[test]
    fn a_script_stands_still_while_a_model_it_put_in_loads_and_for_quiet_frames_after() {
        // A model asked for on the script's third frame takes 20 frames to come.
        let mut coming = vec![0; 3];
        coming.extend([1; 20]);
        coming.extend([0; 15]);
        let frames = run(&coming);
        // Counted up to the third frame, then held at 3 while it loads and settles.
        assert_eq!(frames[2].0, 3);
        for (i, &(script, _)) in frames
            .iter()
            .enumerate()
            .skip(3)
            .take(20 + QUIET_FRAMES as usize - 1)
        {
            assert_eq!(script, 3, "frame {i} counted while held");
        }
        // Then on, a frame a frame.
        let last = frames.last().unwrap().0;
        assert_eq!(last, 3 + (15 - QUIET_FRAMES + 1));
    }

    #[test]
    fn a_held_scripts_clock_stands_still_so_a_frame_sees_the_same_game_time_however_slow_the_load()
    {
        let fast = run(&[0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let mut slow_load = vec![0, 0];
        slow_load.extend([1; 40]);
        slow_load.extend([0; 15]);
        let slow = run(&slow_load);
        let at = |frames: &[(u32, f32)], script: u32| {
            frames.iter().find(|f| f.0 == script).map(|f| f.1).unwrap()
        };
        for script in [2, 4, 6] {
            let (a, b) = (at(&fast, script), at(&slow, script));
            assert!(
                (a - b).abs() < 0.11,
                "script frame {script}: {a} s against {b} s"
            );
        }
    }

    #[test]
    fn a_hold_starts_no_clock_a_script_had_stopped() {
        let mut hold = Hold::default();
        let mut clock = Time::<Virtual>::default();
        clock.pause();
        let now = Instant::now();
        assert_eq!(hold.frame(1, now, &mut clock), Some(Turn::Held));
        for _ in 0..QUIET_FRAMES {
            hold.frame(0, now, &mut clock);
        }
        assert!(!hold.holds());
        assert!(clock.is_paused());
    }

    #[test]
    fn what_never_comes_holds_a_script_only_so_long() {
        let mut hold = Hold::default();
        let mut clock = Time::<Virtual>::default();
        let now = Instant::now();
        hold.frame(1, now, &mut clock);
        let later = now + super::super::GIVE_UP_SCRIPTED + Duration::from_secs(1);
        assert!(matches!(
            hold.frame(1, later, &mut clock),
            Some(Turn::Released { gave_up: true, .. })
        ));
        // Still stuck: not held again for it.
        assert_eq!(hold.frame(1, later, &mut clock), None);
        assert!(!hold.holds() && !clock.is_paused());
    }
}
