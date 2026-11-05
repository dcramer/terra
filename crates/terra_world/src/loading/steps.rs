//! How far the loading bar has come: each step of the load weighted by its measured share
//! of it, as shipped loading screens weight theirs (Unreal's `FScopedSlowTask` gives each
//! piece of work its amount, Godot's `EditorProgress` its steps), not each thing counted
//! alike. Counted alike, the 972 files and 9268 drawables loaded while the land was made
//! filled the bar by the 400th frame of a 30 km world's 91 s load, the land still being
//! worked out (j7's stills, `--loading --shots`).
//!
//! A step is what one part of [`Progress`] says, kept from frame to frame over one load: one
//! reported is as far along as it says, one reported before and not now is done, one only
//! expected waits at none until it is reported, and one expected that a later step has
//! passed (a land with no town to find) is skipped.

use super::Progress;

/// The share of a step nobody has measured, in seconds.
const UNMEASURED_S: f32 = 1.0;

/// The steps of the load under way, in the order first said.
#[derive(Default, Debug)]
pub(super) struct Steps(Vec<Step>, f32);

#[derive(Debug)]
struct Step {
    what: &'static str,
    share_s: f32,
    /// How far along, 0 to 1.
    done: f32,
    /// Whether it has been reported this load, not only expected.
    seen: bool,
    /// Whether it was said this frame.
    said: bool,
}

impl Steps {
    /// Takes in what was said this frame.
    pub(super) fn update(&mut self, progress: &Progress) {
        for step in &mut self.0 {
            step.said = false;
        }
        for part in &progress.0 {
            let at = match self.0.iter().position(|step| step.what == part.what) {
                Some(at) => at,
                None => {
                    self.0.push(Step {
                        what: part.what,
                        share_s: 0.0,
                        done: 0.0,
                        seen: false,
                        said: false,
                    });
                    self.0.len() - 1
                }
            };
            let step = &mut self.0[at];
            step.said = true;
            step.share_s = step.share_s.max(part.share_s);
            if part.reported {
                step.seen = true;
                let all = part.coming + part.done;
                step.done = if all == 0 {
                    1.0
                } else {
                    part.done as f32 / all as f32
                };
            }
        }
        // Done once no longer said.
        for step in &mut self.0 {
            if step.seen && !step.said {
                step.done = 1.0;
            }
        }
        // Passed by: expected, never reported, while a step after it has been.
        let last_seen = self.0.iter().rposition(|step| step.seen);
        let mut at = 0;
        self.0.retain(|step| {
            at += 1;
            step.seen || last_seen.is_none_or(|last| at - 1 > last)
        });
        let done_s = self.0.iter().map(|step| share(step) * step.done).sum();
        self.1 = self.1.max(done_s);
    }

    /// How far along the load is, 0 to 1: the most seconds of its steps ever done, which
    /// never go back, of all its steps' seconds; all done when nothing was said. A step
    /// first expected later lowers it: the land's are said from its first frame.
    pub(super) fn fraction(&self) -> f32 {
        let all: f32 = self.0.iter().map(share).sum();
        if all <= 0.0 {
            return 1.0;
        }
        (self.1 / all).min(1.0)
    }
}

/// A step's share of the load, in seconds.
fn share(step: &Step) -> f32 {
    if step.share_s > 0.0 {
        step.share_s
    } else {
        UNMEASURED_S
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Many files counted while one long step is made fill the bar only by that step's
    /// share; the step done and the next one only expected, the bar waits short of full;
    /// a step passed by is skipped, and the last one done fills it.
    #[test]
    fn the_bar_follows_each_step_by_its_measured_share() {
        let mut steps = Steps::default();
        let mut progress = Progress::default();
        progress.report("Loading", 0, 900);
        progress.expect("Making the land", 30.0);
        progress.report("Making the land", 1, 0);
        progress.expect("Finding the town", 10.0);
        progress.expect("Shaping the ground", 20.0);
        steps.update(&progress);
        // The files' 1 s of 61.
        assert!((steps.fraction() - 1.0 / 61.0).abs() < 1e-5, "{steps:?}");

        // The land made; the town never looked for; the ground not yet begun.
        let mut progress = Progress::default();
        progress.report("Loading", 0, 900);
        steps.update(&progress);
        assert!((steps.fraction() - 31.0 / 61.0).abs() < 1e-5, "{steps:?}");

        // The ground half shaped, which passes the town by.
        let mut progress = Progress::default();
        progress.report("Shaping the ground", 50, 50);
        steps.update(&progress);
        assert!((steps.fraction() - 41.0 / 51.0).abs() < 1e-5, "{steps:?}");

        let mut progress = Progress::default();
        progress.report("Shaping the ground", 0, 100);
        steps.update(&progress);
        assert_eq!(steps.fraction(), 1.0);
        assert_eq!(Steps::default().fraction(), 1.0);
    }
}
