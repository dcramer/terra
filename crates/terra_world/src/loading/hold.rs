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
use crate::shell::{Options, ScriptFrame};
use bevy::{
    diagnostic::FrameCount,
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

/// Where a scripted run's frame count stands on its script's first frame: past any load's
/// frames, so the count never goes back.
const SCRIPT_COUNT_FROM: u32 = 1 << 20;

/// A scripted run's frame count (`FrameCount`, the shaders' `globals.frame_count`) is its
/// script's frame from the script's first frame on, and stands still while the script is
/// held. Bevy counts every frame drawn, the loading screen's and a hold's too, and those are
/// as many as the load took on the wall clock; what the count picks — TAA's jitter of eight
/// (`prepare_taa_jitter`), the history textures' turn, the shadows', SSAO's, the fog's and
/// the dither's noise — then differed run to run on one frame of one script, and a film of
/// one build was not the same film twice (D472). Offline renderers seed their sampling by
/// the frame being rendered for the same reason. After Bevy's count, in `Last`.
pub(crate) fn count_frames(
    options: Res<Options>,
    script: Res<ScriptFrame>,
    mut count: ResMut<FrameCount>,
) {
    if options.scripted() && !options.bench && script.0 > 0 {
        count.0 = SCRIPT_COUNT_FROM + script.0;
    }
}

/// A scripted run's script opens on a camera cut: on its first frame every camera's temporal
/// history is dropped, as a renderer's camera cut drops it (Bevy's `TemporalAntiAliasing::
/// reset`, Unreal's `bCameraCut`). The loading screen's frames are drawn too, as many as the
/// load took on the wall clock, each blended into TAA's history at the jitter its frame count
/// gave it, so the first script frames carried a history that differed run to run: an A/A of
/// one build differed on every pixel of its second frame with TAA on and matched with it off
/// (D472). The auto exposure needs no reset: the loading screen and a hold stop the game
/// clock, and the eye adapts by the clock's step, so it stands still through them. After
/// whatever set the reset this frame (the shot's own cuts, `cine::film`), in `Last`.
pub(crate) fn cut_history(
    options: Res<Options>,
    script: Res<ScriptFrame>,
    mut cameras: Query<&mut bevy::anti_alias::taa::TemporalAntiAliasing>,
) {
    if !options.scripted() || options.bench || script.0 != 1 || !script.is_changed() {
        return;
    }
    for mut taa in &mut cameras {
        taa.reset = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::counting;

    /// The frame count on each script frame of a scripted run whose load took `loading`
    /// frames and which is held `held` frames after its third.
    fn counts(loading: usize, held: usize) -> Vec<(u32, u32)> {
        let mut app = App::new();
        app.add_plugins(bevy::diagnostic::FrameCountPlugin)
            .insert_resource(
                Options::from_args(["--shots", "out/x@5"].map(String::from).into_iter()).unwrap(),
            )
            .init_resource::<ScriptFrame>()
            .add_systems(
                Last,
                count_frames.after(bevy::diagnostic::update_frame_count),
            );
        let mut steps = vec![false; loading];
        steps.extend([true; 3]);
        steps.extend(vec![false; held]);
        steps.extend([true; 8]);
        let mut seen = Vec::new();
        for counted in steps {
            if counted {
                app.world_mut().resource_mut::<ScriptFrame>().0 += 1;
            }
            app.update();
            let script = app.world().resource::<ScriptFrame>().0;
            if counted {
                seen.push((script, app.world().resource::<FrameCount>().0));
            }
        }
        seen
    }

    #[test]
    fn a_scripted_runs_frames_are_counted_alike_however_long_its_load_and_holds() {
        let fast = counts(3, 0);
        assert_eq!(fast.len(), 11);
        assert_eq!(
            fast,
            counts(240, 17),
            "a slow load and a hold moved the count"
        );
        assert_eq!(fast, counts(1000, 4));
        // Still a frame a frame, as the jitter and the history textures take turns by it.
        assert!(fast.windows(2).all(|w| w[1].1 == w[0].1 + 1));
    }

    /// The real frames on which a scripted run's camera dropped its temporal history, counted
    /// from the script's first frame, for a load of `loading` frames and a hold of `held`
    /// after the third script frame, with the camera's own cut on real frame `cut`.
    fn cuts(loading: usize, held: usize, cut: Option<usize>) -> Vec<i64> {
        use bevy::anti_alias::taa::TemporalAntiAliasing;
        let mut app = App::new();
        app.insert_resource(
            Options::from_args(["--shots", "out/x@5"].map(String::from).into_iter()).unwrap(),
        )
        .init_resource::<ScriptFrame>()
        .add_systems(Last, cut_history);
        let camera = app
            .world_mut()
            .spawn(TemporalAntiAliasing { reset: false })
            .id();
        let mut steps = vec![false; loading];
        steps.extend([true; 3]);
        steps.extend(vec![false; held]);
        steps.extend([true; 8]);
        let mut seen = Vec::new();
        for (i, counted) in steps.into_iter().enumerate() {
            if counted {
                app.world_mut().resource_mut::<ScriptFrame>().0 += 1;
            }
            if cut == Some(i) {
                app.world_mut()
                    .get_mut::<TemporalAntiAliasing>(camera)
                    .unwrap()
                    .reset = true;
            }
            app.update();
            let mut taa = app
                .world_mut()
                .get_mut::<TemporalAntiAliasing>(camera)
                .unwrap();
            if taa.reset {
                seen.push(i as i64 - loading as i64);
            }
            // As the render world's extraction does once it has read it.
            taa.reset = false;
        }
        seen
    }

    #[test]
    fn a_scripted_runs_first_frame_drops_the_history_however_long_its_load_and_holds() {
        // The script's first frame alone, however many loading frames were drawn before it.
        assert_eq!(cuts(3, 0, None), vec![0]);
        assert_eq!(cuts(240, 17, None), vec![0]);
        // Not again on a hold, nor on the frames after it.
        assert_eq!(cuts(1000, 4, None), vec![0]);
        // A cut of the run's own is kept.
        assert_eq!(cuts(5, 2, Some(5 + 6)), vec![0, 6]);
    }

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
