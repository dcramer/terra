//! The sandbox's freeze warnings (`terra_world::recorder`, D576): a frame of
//! `play::FREEZE_MS` or more is warned of as `freeze: frame 41230 took 1840 ms, held by the
//! game world: <its longest systems>; 60 ms outside anything timed (...)`. The bench keeps
//! them, the worst of a run's and how many, so a scene that misses on freezes names what held
//! its worst (D556), not a log nobody kept.

/// Where to look when the log warned of no freeze: the frame recorder's session report.
const LOOK: &str = "the run's log warned of none (a CPU adapter times no frames); `cargo xtask profile walk --scene <scene>` times each frame's systems";

/// A run's freeze warnings: how many, and the worst's length and what it said.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Freezes {
    pub count: u32,
    pub worst: Option<(f32, String)>,
}

/// Reads a freeze warning, whatever the log put before it, as its length in ms and what it
/// said from `frame` to its system list's end (the outside-timed share kept, the gloss on it
/// dropped).
pub(super) fn parse(line: &str) -> Option<(f32, String)> {
    let (_, said) = line.split_once("freeze: frame ")?;
    let (_, rest) = said.split_once(" took ")?;
    let (ms, _) = rest.split_once(" ms, held by ")?;
    let said = match said.split_once(" outside anything timed") {
        Some((said, _)) => format!("{said} outside anything timed"),
        None => said.trim_end().trim_end_matches('.').to_string(),
    };
    Some((ms.parse().ok()?, format!("frame {said}")))
}

impl Freezes {
    /// Counts a log line if it is a freeze warning; whether it was.
    pub(super) fn read(&mut self, line: &str) -> bool {
        let Some((ms, said)) = parse(line) else {
            return false;
        };
        self.count(1, ms, said);
        true
    }

    fn count(&mut self, count: u32, ms: f32, said: String) {
        self.count += count;
        if self.worst.as_ref().is_none_or(|(worst, _)| ms > *worst) {
            self.worst = Some((ms, said));
        }
    }

    /// Several runs' warnings as one: their count summed, the worst of any.
    pub(super) fn sum<'a>(runs: impl Iterator<Item = &'a Freezes>) -> Freezes {
        let mut all = Freezes::default();
        for run in runs {
            if let Some((ms, said)) = &run.worst {
                all.count(run.count, *ms, said.clone());
            }
        }
        all
    }

    /// The run's line: how many frames it warned froze, through its load too, and the worst;
    /// none where it warned of none.
    pub(super) fn said(&self) -> Option<String> {
        let (_, worst) = self.worst.as_ref()?;
        Some(format!(
            "freeze warnings: {} through the run, its load too; the worst: {worst}",
            self.count
        ))
    }

    /// Where a scene that froze should look (D556).
    pub(super) fn look(&self) -> String {
        match &self.worst {
            Some((_, worst)) => format!(
                "the worst of the run's {} freeze warning{} was {worst}",
                self.count,
                if self.count == 1 { "" } else { "s" }
            ),
            None => LOOK.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WARNED: &str = "2026-11-04T07:01:02.345Z  WARN terra_world::recorder: freeze: frame 41230 took 1840 ms, held by the game world: terra_folk::settlement::found 1710 ms, terra_world::flora::plant 4.1 ms; 60 ms outside anything timed (a wait on the GPU, a lock or the other world, or work on the task pools).";

    /// The warning as Bevy's log prints it, its level and target before it, coloured or not;
    /// a line that only mentions freezes is not one.
    #[test]
    fn reads_the_sandboxs_freeze_warnings() {
        assert_eq!(
            parse(WARNED),
            Some((
                1840.0,
                "frame 41230 took 1840 ms, held by the game world: terra_folk::settlement::found 1710 ms, terra_world::flora::plant 4.1 ms; 60 ms outside anything timed".to_string()
            ))
        );
        let coloured = "\u{1b}[33m WARN\u{1b}[0m \u{1b}[2mterra_world::recorder\u{1b}[0m\u{1b}[2m:\u{1b}[0m freeze: frame 7 took 104 ms, held by the render world: nothing timed; 104 ms outside anything timed (a wait on the GPU).";
        assert_eq!(parse(coloured).map(|(ms, _)| ms), Some(104.0));
        assert_eq!(
            parse("SANDBOX against the 13.3 ms budget: 1 freezes of 100 ms or more"),
            None
        );
    }

    /// A run keeps the worst of its warnings and how many; two runs sum as one; and a scene
    /// that froze says what held its worst, or, where nothing warned, the command that will.
    #[test]
    fn a_freezing_scene_says_what_held_its_worst() {
        let mut run = Freezes::default();
        assert!(run.said().is_none());
        assert!(
            run.look().contains("warned of none") && run.look().contains("cargo xtask profile")
        );
        assert!(!run.read("Loaded in 3.0 s"));
        assert!(run.read(
            "WARN terra_world::recorder: freeze: frame 9 took 120 ms, held by the render world: grass 90 ms; 30 ms outside anything timed (a wait)."
        ));
        assert!(run.read(WARNED));
        assert_eq!(run.count, 2);
        assert_eq!(
            run.said().unwrap(),
            "freeze warnings: 2 through the run, its load too; the worst: frame 41230 took 1840 ms, held by the game world: terra_folk::settlement::found 1710 ms, terra_world::flora::plant 4.1 ms; 60 ms outside anything timed"
        );
        let mut other = Freezes::default();
        other.read("freeze: frame 3 took 2100 ms, held by the game world: stream 2000 ms; 100 ms outside anything timed.");
        let both = Freezes::sum([&run, &other, &Freezes::default()].into_iter());
        assert_eq!(both.count, 3);
        assert_eq!(
            both.look(),
            "the worst of the run's 3 freeze warnings was frame 3 took 2100 ms, held by the game world: stream 2000 ms; 100 ms outside anything timed"
        );
    }
}
