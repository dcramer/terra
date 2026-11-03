//! What a run says of its phases and parts (`terra_world::bench_run`, `debug`), held to the
//! budgets (D607): the settle phase's frames, ticks and freezes; each phase's groups of
//! systems (`terra_gen::stat`) and each world's schedules (the recorder's marks, D576) by
//! their median ms a frame; the render world's CPU; and the GPU's costliest passes.
//!
//! As Unreal's PerfReportTool reads a CSV profiler capture against its budgets: a frame budget
//! a phase and a budget a stat group, each miss naming the stat, its budget, what was
//! measured and where to look (D556).

use super::budgets::{FILE, Limits};

/// The settle phase as its line says it.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Settle {
    pub frames: u32,
    pub median_ms: f32,
    pub p99_ms: f32,
    pub hitches: u32,
    pub freezes: u32,
    pub tick_p99_ms: Option<f32>,
}

/// A run's phases and parts, None or empty where it did not say them.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Said {
    pub settle: Option<Settle>,
    /// Each phase's groups and schedules by name, their median ms a frame: settle, then play.
    pub groups: [Vec<(String, f32)>; 2],
    pub schedules: [Vec<(String, f32)>; 2],
    /// The play phase's render world on the CPU, median ms a frame, and its GPU passes'.
    pub render_cpu_ms: Option<f32>,
    pub gpu: Vec<(String, f32)>,
}

impl Said {
    /// Reads a line into what it says: true for the settle, groups and schedules lines, which
    /// the bench shows; false for any other, the frame parts and GPU passes lines among them,
    /// which it reads and leaves to be shown as worth saying.
    pub(super) fn read(&mut self, line: &str) -> bool {
        if let Some(settle) = parse_settle(line) {
            self.settle = Some(settle);
            return true;
        }
        for (what, into) in [
            ("groups", &mut self.groups),
            ("schedules", &mut self.schedules),
        ] {
            if let Some((phase, each)) = parse_named(line, what) {
                into[phase] = each;
                return true;
            }
        }
        if let Some(ms) = parse_render_cpu(line) {
            self.render_cpu_ms = Some(ms);
        } else if let Some(passes) = line.strip_prefix("SANDBOX GPU passes, medians in ms: ") {
            self.gpu = passes.split(", ").filter_map(name_and_ms).collect();
        }
        false
    }

    /// What the run missed of its settle and play phases' budgets.
    pub(super) fn misses(&self, settle: &Limits, play: &Limits) -> Vec<String> {
        let mut missed = self.settle_misses(settle);
        for (phase, at, limits) in [("settle", 0, settle), ("play", 1, play)] {
            missed.extend(named_misses(
                Kind::Group,
                phase,
                &limits.groups,
                &self.groups[at],
            ));
            let schedules = &self.schedules[at];
            missed.extend(named_misses(
                Kind::Schedule,
                phase,
                &limits.schedules,
                schedules,
            ));
        }
        missed.extend(named_misses(Kind::Pass, "play", &play.gpu, &self.gpu));
        if settle.render_cpu_ms.is_some() || !settle.gpu.is_empty() {
            missed.push(format!(
                "the settle phase budgets the render CPU or GPU passes, which a run says only for \
                 play ({FILE}): budget them in the scene's play"
            ));
        }
        match (play.render_cpu_ms, self.render_cpu_ms) {
            (Some(most), Some(ms)) if ms > most => missed.push(format!(
                "render CPU {ms:.2} ms a frame at the median, over the play phase's {most:.2} ms \
                 ({FILE}): the run's `CPU passes` line names the render world's costliest"
            )),
            (Some(_), None) => missed.push(format!(
                "render CPU budgeted ({FILE}) but not said: no `SANDBOX frame parts` line with a \
                 render CPU in ms (terra_world::shell says it at the run's end)"
            )),
            _ => {}
        }
        missed
    }

    fn settle_misses(&self, limits: &Limits) -> Vec<String> {
        let wanted = limits.seconds.is_some_and(|s| s > 0.0);
        let Some(settle) = self.settle.as_ref().filter(|_| wanted) else {
            return match wanted {
                true => vec![format!(
                    "the settle phase not said: no `SANDBOX settle phase` line, where {FILE} \
                     gives it {:.0} s (terra_world::bench_run says it, given --bench-settle)",
                    limits.seconds.unwrap_or(0.0)
                )],
                false => Vec::new(),
            };
        };
        let mut missed = Vec::new();
        let look = "the log's hitch and `freeze:` lines name what held its frames (D576), \
                    and the settle's groups line which group spent most";
        if let Some(most) = limits.frame_p99_ms
            && settle.p99_ms > most
        {
            missed.push(format!(
                "the settle phase's 99th percentile {:.1} ms over its {most:.1} ms ({FILE}): {look}",
                settle.p99_ms
            ));
        }
        for (what, count, most) in [
            ("hitches", settle.hitches, limits.hitches),
            ("freezes", settle.freezes, limits.freezes),
        ] {
            if let Some(most) = most
                && count > most
            {
                missed.push(format!(
                    "{count} {what} in the settle phase, past the {most} it allows ({FILE}): {look}"
                ));
            }
        }
        if let (Some(most), Some(p99)) = (limits.tick_p99_ms, settle.tick_p99_ms)
            && p99 > most
        {
            missed.push(format!(
                "the settle phase's ticks' 99th percentile {p99:.1} ms over the {most:.1} ms of a \
                 tick (D138, {FILE}): the fixed loop's systems, by `cargo xtask profile`"
            ));
        }
        missed
    }

    /// Several runs as one: each value the middle of the runs'.
    pub(super) fn middle<'a>(runs: impl Iterator<Item = &'a Said>) -> Said {
        let runs: Vec<&Said> = runs.collect();
        let settles: Vec<&Settle> = runs.iter().filter_map(|s| s.settle.as_ref()).collect();
        let settle = (!settles.is_empty()).then(|| {
            let mut by_p99 = settles.clone();
            by_p99.sort_by(|a, b| a.p99_ms.total_cmp(&b.p99_ms));
            Settle {
                hitches: settles.iter().map(|s| s.hitches).max().unwrap_or(0),
                freezes: settles.iter().map(|s| s.freezes).max().unwrap_or(0),
                ..by_p99[by_p99.len() / 2].clone()
            }
        });
        let named =
            |pick: fn(&Said) -> &Vec<(String, f32)>| middle_named(runs.iter().map(|s| pick(s)));
        Said {
            settle,
            groups: [named(|s| &s.groups[0]), named(|s| &s.groups[1])],
            schedules: [named(|s| &s.schedules[0]), named(|s| &s.schedules[1])],
            render_cpu_ms: middle(runs.iter().filter_map(|s| s.render_cpu_ms).collect()),
            gpu: named(|s| &s.gpu),
        }
    }
}

/// What a budget by name is of, for its messages.
#[derive(Clone, Copy)]
enum Kind {
    Group,
    Schedule,
    Pass,
}

/// Each named budget missed: over, or not measured at all.
fn named_misses(
    kind: Kind,
    phase: &str,
    budgets: &[(String, f32)],
    measured: &[(String, f32)],
) -> Vec<String> {
    let mut missed = Vec::new();
    for (name, most) in budgets {
        let ms = measured.iter().find(|(n, _)| n == name).map(|(_, ms)| *ms);
        let (what, look, untimed) = match kind {
            Kind::Group => (
                "group",
                format!(
                    "its systems are those that open `terra_gen::stat_scope!(\"{name}\")` or are \
                     added `terra_world::timed!(\"{name}\", …)` (grep them); `cargo xtask \
                     profile` times each"
                ),
                format!(
                    "no system opens `terra_gen::stat_scope!(\"{name}\")` or ran in the phase; a \
                     renamed group wants its name changed in {FILE}"
                ),
            ),
            Kind::Schedule => (
                "schedule",
                "a `cargo xtask profile` report (a hitch-log build) times its systems".to_string(),
                "the recorder's marks time each schedule in a build without the `hitch-log` \
                 feature (D576); a renamed schedule wants its name changed in the file"
                    .to_string(),
            ),
            Kind::Pass => (
                "GPU pass",
                "the run's `GPU passes in the last tenth` and `triangles a frame` lines say \
                 what it drew; `cargo xtask profile --gpu` times its draws"
                    .to_string(),
                "it is not among the run's costliest 20 passes (`SANDBOX GPU passes`): renamed, \
                 gone, or so cheap its budget is not needed"
                    .to_string(),
            ),
        };
        match ms {
            Some(ms) if ms > *most => missed.push(format!(
                "{what} `{name}` {ms:.2} ms a frame at the median in the {phase} phase, over its \
                 {most:.2} ms ({FILE}): {look}"
            )),
            Some(_) => {}
            None => missed.push(format!(
                "{what} `{name}` budgeted in the {phase} phase ({FILE}) but not timed: {untimed}"
            )),
        }
    }
    missed
}

fn middle(mut values: Vec<f32>) -> Option<f32> {
    values.sort_by(f32::total_cmp);
    values.get(values.len() / 2).copied()
}

/// Each name's middle value over the runs that said it.
fn middle_named<'a>(lists: impl Iterator<Item = &'a Vec<(String, f32)>>) -> Vec<(String, f32)> {
    let lists: Vec<_> = lists.collect();
    let mut names: Vec<&String> = lists
        .iter()
        .flat_map(|l| l.iter().map(|(n, _)| n))
        .collect();
    names.sort();
    names.dedup();
    let value = |name: &String| {
        let values = lists
            .iter()
            .filter_map(|l| l.iter().find(|(n, _)| n == name));
        middle(values.map(|(_, ms)| *ms).collect())
    };
    (names.into_iter())
        .filter_map(|name| Some((name.clone(), value(name)?)))
        .collect()
}

/// `name 1.25` as the name and its ms.
fn name_and_ms(part: &str) -> Option<(String, f32)> {
    let (name, ms) = part.trim().rsplit_once(' ')?;
    Some((name.to_string(), ms.parse().ok()?))
}

/// Reads `SANDBOX settle phase, the first 30 s after the loading screen lifted: 1800 frames,
/// median 8.00 ms, 99th percentile 20.00 ms, worst 90.00 ms; 3 hitches over 26.7 ms, 0
/// freezes of 100 ms or more; ticks' 99th percentile 4.00 ms` (or `-` for no ticks).
fn parse_settle(line: &str) -> Option<Settle> {
    let rest = line.strip_prefix("SANDBOX settle phase, ")?;
    let (_, rest) = rest.split_once(": ")?;
    let (frames, rest) = rest.split_once(" frames, median ")?;
    let (median, rest) = rest.split_once(" ms, 99th percentile ")?;
    let (p99, rest) = rest.split_once(" ms, worst ")?;
    let (_, rest) = rest.split_once("; ")?;
    let (hitches, rest) = rest.split_once(" hitches over ")?;
    let (_, rest) = rest.split_once(", ")?;
    let (freezes, rest) = rest.split_once(" freezes of ")?;
    let (_, ticks) = rest.split_once("ticks' 99th percentile ")?;
    Some(Settle {
        frames: frames.parse().ok()?,
        median_ms: median.parse().ok()?,
        p99_ms: p99.parse().ok()?,
        hitches: hitches.parse().ok()?,
        freezes: freezes.parse().ok()?,
        tick_p99_ms: ticks.strip_suffix(" ms").and_then(|ms| ms.parse().ok()),
    })
}

/// Reads `SANDBOX <groups|schedules> in the <settle|play> phase, ms a frame, median (99th):
/// creatures 1.00 (2.00), flora 0.50 (0.90); 40 scopes a frame` as the phase (0 settle, 1
/// play) and each name's median.
fn parse_named(line: &str, what: &str) -> Option<(usize, Vec<(String, f32)>)> {
    let rest = line.strip_prefix("SANDBOX ")?.strip_prefix(what)?;
    let rest = rest.strip_prefix(" in the ")?;
    let (phase, rest) = rest.split_once(" phase, ms a frame, median (99th): ")?;
    let phase = ["settle", "play"].iter().position(|p| *p == phase)?;
    let each = rest.split_once("; ").map_or(rest, |(each, _)| each);
    let each = each
        .split(", ")
        .filter_map(|part| {
            let (rest, _p99) = part.trim().rsplit_once(' ')?;
            name_and_ms(rest)
        })
        .collect();
    Some((phase, each))
}

/// Reads the render CPU's median from `SANDBOX frame parts, median: game 4.00 ms, render CPU
/// 3.00 ms, GPU 6.00 ms`.
fn parse_render_cpu(line: &str) -> Option<f32> {
    let rest = line.strip_prefix("SANDBOX frame parts, median: ")?;
    let (_, rest) = rest.split_once("render CPU ")?;
    let (ms, _) = rest.split_once(" ms")?;
    ms.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETTLE: &str = "SANDBOX settle phase, the first 30 s after the loading screen lifted: 1800 frames, median 8.00 ms, 99th percentile 20.00 ms, worst 90.00 ms; 3 hitches over 26.7 ms, 0 freezes of 100 ms or more; ticks' 99th percentile 4.00 ms";

    fn said() -> Said {
        let mut said = Said::default();
        for line in [
            SETTLE,
            "SANDBOX groups in the settle phase, ms a frame, median (99th): creatures 1.00 (2.00), flora 3.00 (5.00); 40 scopes a frame",
            "SANDBOX groups in the play phase, ms a frame, median (99th): creatures 0.50 (1.00), flora 2.00 (3.00); 40 scopes a frame",
            "SANDBOX schedules in the play phase, ms a frame, median (99th): PostUpdate 5.10 (6.00), render.Render 3.00 (4.00)",
        ] {
            assert!(said.read(line), "{line}");
        }
        for line in [
            "SANDBOX frame parts, median: game 8.65 ms, render CPU 3.20 ms, GPU 6.00 ms",
            "SANDBOX GPU passes, medians in ms: main_opaque_pass_3d 2.10, shadows 1.20",
            "SANDBOX GPU passes in the last tenth, medians in ms: main_opaque_pass_3d 9.00",
        ] {
            assert!(!said.read(line), "left to be shown: {line}");
        }
        said
    }

    #[test]
    fn a_run_says_its_settle_groups_schedules_render_cpu_and_passes() {
        let said = said();
        let settle = said.settle.as_ref().unwrap();
        assert_eq!(
            (settle.frames, settle.p99_ms, settle.hitches),
            (1800, 20.0, 3)
        );
        assert_eq!(settle.tick_p99_ms, Some(4.0));
        assert_eq!(said.groups[0][1], ("flora".to_string(), 3.0));
        assert_eq!(said.groups[1][0], ("creatures".to_string(), 0.5));
        assert_eq!(said.schedules[1][0], ("PostUpdate".to_string(), 5.1));
        assert!(said.schedules[0].is_empty());
        assert_eq!(said.render_cpu_ms, Some(3.2));
        assert_eq!(said.gpu[1], ("shadows".to_string(), 1.2));
    }

    fn limits(build: impl FnOnce(&mut Limits)) -> Limits {
        let mut limits = Limits::default();
        build(&mut limits);
        limits
    }

    #[test]
    fn a_group_schedule_or_pass_over_its_budget_fails_naming_it() {
        let said = said();
        let settle = limits(|l| {
            l.seconds = Some(30.0);
            l.frame_p99_ms = Some(26.7);
            l.groups = vec![("flora".into(), 2.5)];
        });
        let play = limits(|l| {
            l.schedules = vec![("PostUpdate".into(), 5.0)];
            l.gpu = vec![("shadows".into(), 1.5)];
            l.render_cpu_ms = Some(3.0);
        });
        let missed = said.misses(&settle, &play);
        assert_eq!(missed.len(), 3, "{missed:#?}");
        assert!(
            missed[0].starts_with(
                "group `flora` 3.00 ms a frame at the median in the settle phase, over its 2.50 ms"
            ),
            "{}",
            missed[0]
        );
        assert!(
            missed[1].starts_with("schedule `PostUpdate` 5.10 ms"),
            "{}",
            missed[1]
        );
        assert!(missed[2].starts_with("render CPU 3.20 ms"), "{}", missed[2]);
        let within = limits(|l| l.groups = vec![("flora".into(), 3.5)]);
        assert!(said.misses(&Limits::default(), &within).is_empty());
    }

    #[test]
    fn a_budget_nothing_timed_fails_as_untimed() {
        let said = said();
        let play = limits(|l| l.groups = vec![("weather".into(), 1.0)]);
        let missed = said.misses(&Limits::default(), &play);
        assert_eq!(missed.len(), 1, "{missed:#?}");
        assert!(
            missed[0].contains("`weather` budgeted in the play phase"),
            "{}",
            missed[0]
        );
        assert!(
            missed[0].contains("stat_scope!(\\\"weather\\\")")
                || missed[0].contains("stat_scope!(\"weather\")"),
            "{}",
            missed[0]
        );
        let unsaid = Said::default();
        let settle = limits(|l| l.seconds = Some(30.0));
        let missed = unsaid.misses(&settle, &Limits::default());
        assert!(
            missed[0].starts_with("the settle phase not said"),
            "{missed:?}"
        );
    }

    #[test]
    fn several_runs_take_each_value_from_the_middle_run() {
        let mut runs = [said(), said(), said()];
        runs[0].groups[1][1].1 = 9.0;
        runs[2].groups[1][1].1 = 1.0;
        let middle = Said::middle(runs.iter());
        assert_eq!(middle.groups[1][1], ("flora".to_string(), 2.0));
        assert_eq!(middle.render_cpu_ms, Some(3.2));
    }
}
