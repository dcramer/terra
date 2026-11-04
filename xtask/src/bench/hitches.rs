//! The sandbox's hitch warnings (`terra_world::debug::report`, D576): every ten seconds or so
//! it warns how many frames took over the hitch length, and which system the frame recorder
//! found longest in most of them. The bench keeps them, summed over a run, so a scene that
//! misses on hitches names the system to look at (D556), not a log nobody kept.

/// Where to look when no warning named a system: the profiler times every frame's systems.
const PROFILE: &str = "`cargo xtask profile walk --scene <scene>` times each frame's systems";

fn frames(count: u32) -> String {
    format!("{count} frame{}", if count == 1 { "" } else { "s" })
}

/// Which systems were longest in a run's hitch frames, as its warnings said.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Longest {
    /// Frames the warnings counted over the hitch length.
    pub frames: u32,
    /// Hitch frames the recorder explained, and how many of them each named system was the
    /// longest in: a warning names only its top system, so the counts are at least these.
    pub explained: u32,
    pub systems: Vec<(String, u32)>,
}

/// Reads `12 frames took over 27 ms since the last report, the worst 55.0 ms; the longest in
/// 8 of 10: forest::stream.` (whatever the log put before it) as the frames counted and, where
/// it says, the system longest in most, in how many, of how many explained.
pub(super) fn parse(line: &str) -> Option<(u32, Option<(String, u32, u32)>)> {
    let (before, rest) = line.split_once(" took over ")?;
    let before = before
        .strip_suffix(" frames")
        .or_else(|| before.strip_suffix(" frame"))?;
    let frames = before.rsplit(' ').next()?.parse().ok()?;
    rest.split_once(" ms since the last report")?;
    let longest = rest.split_once("; the longest in ").and_then(|(_, named)| {
        let (most, named) = named.split_once(" of ")?;
        let (of, system) = named.split_once(": ")?;
        let system = system.trim_end();
        let system = system.strip_suffix('.').unwrap_or(system);
        Some((system.to_string(), most.parse().ok()?, of.parse().ok()?))
    });
    Some((frames, longest))
}

impl Longest {
    /// Counts a log line if it is a hitch warning; whether it was.
    pub(super) fn read(&mut self, line: &str) -> bool {
        let Some((frames, longest)) = parse(line) else {
            return false;
        };
        self.frames += frames;
        if let Some((system, most, of)) = longest {
            self.explained += of;
            self.count(&system, most);
        }
        true
    }

    fn count(&mut self, system: &str, most: u32) {
        match self.systems.iter_mut().find(|(name, _)| name == system) {
            Some((_, count)) => *count += most,
            None => self.systems.push((system.to_string(), most)),
        }
        // Most first, then by name, so the same warnings say the same.
        self.systems
            .sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    }

    /// Several runs' warnings as one.
    pub(super) fn sum<'a>(runs: impl Iterator<Item = &'a Longest>) -> Longest {
        let mut all = Longest::default();
        for run in runs {
            all.frames += run.frames;
            all.explained += run.explained;
            for (system, most) in &run.systems {
                all.count(system, *most);
            }
        }
        all
    }

    /// `forest::stream in 8 of the 10 hitch frames the recorder explained, Update in 2`: the
    /// three longest in most; none where the warnings named no system.
    pub(super) fn named(&self) -> Option<String> {
        let (first, most) = self.systems.first()?;
        let rest: String = self.systems[1..]
            .iter()
            .take(2)
            .map(|(system, most)| format!(", {system} in {most}"))
            .collect();
        Some(format!(
            "{first} in {most} of the {} hitch frames the recorder explained{rest}",
            self.explained
        ))
    }

    /// The run's line: how many frames the warnings counted and which systems were longest
    /// in them; none where it warned of none.
    pub(super) fn said(&self) -> Option<String> {
        (self.frames > 0).then(|| {
            let named = self.named().map_or("named no system".to_string(), |named| {
                format!("longest in most: {named}")
            });
            format!(
                "hitch warnings: {} over the hitch length; {named}",
                frames(self.frames)
            )
        })
    }

    /// Where a scene that hitched too often should look (D556).
    pub(super) fn look(&self) -> String {
        match self.named() {
            Some(named) => format!("the longest system was {named}"),
            None if self.frames > 0 => format!(
                "the log's hitch warnings counted {} but named no system; {PROFILE}",
                frames(self.frames)
            ),
            None => format!("the run's log warned of none, so named no system; {PROFILE}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The warning as Bevy's log prints it, its level and target before it, coloured or not;
    /// one frame, and one with no system named.
    #[test]
    fn reads_the_sandboxs_hitch_warnings() {
        let line = "2026-11-04T07:01:02.345Z  WARN terra_world::debug: 12 frames took over 27 ms since the last report, the worst 55.0 ms; the longest in 8 of 10: terra_forest::stream::lay.";
        assert_eq!(
            parse(line),
            Some((12, Some(("terra_forest::stream::lay".to_string(), 8, 10))))
        );
        let coloured = "\u{1b}[33m WARN\u{1b}[0m \u{1b}[2mterra_world::debug\u{1b}[0m\u{1b}[2m:\u{1b}[0m 1 frame took over 27 ms since the last report, the worst 31.2 ms.";
        assert_eq!(parse(coloured), Some((1, None)));
        assert_eq!(parse("Loaded in 12.0 s"), None);
        assert_eq!(parse("the walk took over 3 s"), None);
    }

    /// Over a run, each system's count is the sum of the warnings naming it, the most first;
    /// two runs sum as one; and the line says the three longest.
    #[test]
    fn sums_which_system_was_longest_over_a_run() {
        let warn = |frames: u32, longest: Option<(&str, u32, u32)>| {
            let named = longest.map_or(String::new(), |(system, most, of)| {
                format!("; the longest in {most} of {of}: {system}")
            });
            format!(
                "WARN terra_world::debug: {frames} frames took over 27 ms since the last report, the worst 40.0 ms{named}."
            )
        };
        let mut run = Longest::default();
        for line in [
            warn(9, Some(("stream", 6, 9))),
            warn(4, Some(("Update", 3, 4))),
            warn(10, Some(("stream", 7, 10))),
            warn(2, None),
            "Loaded in 3.0 s".to_string(),
        ] {
            run.read(&line);
        }
        assert_eq!((run.frames, run.explained), (25, 23));
        assert_eq!(
            run.systems,
            vec![("stream".to_string(), 13), ("Update".to_string(), 3)]
        );
        assert_eq!(
            run.said().unwrap(),
            "hitch warnings: 25 frames over the hitch length; longest in most: stream in 13 of the 23 hitch frames the recorder explained, Update in 3"
        );
        let mut other = Longest::default();
        other.read(&warn(5, Some(("Update", 5, 5))));
        other.read(&warn(5, Some(("grass", 4, 5))));
        let both = Longest::sum([&run, &other].into_iter());
        assert_eq!((both.frames, both.explained), (35, 33));
        assert_eq!(
            both.named().unwrap(),
            "stream in 13 of the 33 hitch frames the recorder explained, Update in 8, grass in 4"
        );
    }

    /// A scene that hitched says the system to look at, or, where nothing named one, the
    /// command that will; never a log the bench did not keep.
    #[test]
    fn a_hitching_scene_says_where_to_look() {
        let mut run = Longest::default();
        assert!(run.said().is_none());
        assert!(
            run.look().contains("warned of none") && run.look().contains("cargo xtask profile")
        );
        run.read("1 frame took over 27 ms since the last report, the worst 31.2 ms.");
        assert!(run.said().unwrap().contains("named no system"));
        assert!(run.look().contains("counted 1 frame but named no system"));
        run.read("3 frames took over 27 ms since the last report, the worst 31.2 ms; the longest in 2 of 3: stream.");
        assert_eq!(
            run.look(),
            "the longest system was stream in 2 of the 3 hitch frames the recorder explained"
        );
    }
}
