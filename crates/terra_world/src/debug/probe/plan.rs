//! The probe's plan and its gate, plain Rust with no engine so they are tested alone: the
//! variants and what each changes, the order they run in, when a frame is settled enough to
//! count, and the table of what each variant cost.

use std::collections::BTreeMap;
use terra_gen::tune::parse_assignment;

/// One thing a variant changes while it runs.
#[derive(Clone, Debug, PartialEq)]
pub enum Lever {
    /// A tunable (D547) set to a value: `name=value`.
    Tune(String, f32),
    /// Every mesh whose material's type name holds this part, hidden: `hide=WindExtension`.
    Hide(String),
}

/// A variant: its name, and its levers, none for the base.
#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    pub name: String,
    pub levers: Vec<Lever>,
}

/// What `--probe` asks: the variants, run in rounds of every one in turn (A B A B).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Plan {
    pub variants: Vec<Variant>,
}

/// How `--probe` is written, for its errors.
pub const GRAMMAR: &str = "`--probe \"<name>: <lever>, …\"` a variant, repeated for each, a \
     lever `<tunable>=<value>` (the bare tunable name, D547, no `tune.` prefix) or \
     `hide=<part of a material's type name>`, e.g. `--probe \"old: \
     fog.light_span=0\" --probe \"new: fog.light_span=1\"` (D609)";

impl Plan {
    /// Reads variants, `name: lever, lever`, each its own argument; `;` parts one text into
    /// several too, for a shell script, where an agent's command guard refuses it.
    pub fn parse(text: &str) -> Result<Plan, String> {
        let mut plan = Plan::default();
        plan.add(text)?;
        Ok(plan)
    }

    /// Adds the variants of one `--probe`.
    pub fn add(&mut self, text: &str) -> Result<(), String> {
        let variants = &mut self.variants;
        let before = variants.len();
        for part in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            let (name, levers) = part
                .split_once(':')
                .ok_or(format!("--probe's variant `{part}` has no name: {GRAMMAR}"))?;
            let name = name.trim().to_string();
            if name.is_empty() || name.contains(char::is_whitespace) {
                return Err(format!(
                    "--probe's variant names are one word, not `{name}`: {GRAMMAR}"
                ));
            }
            if variants.iter().any(|v| v.name == name) {
                return Err(format!("--probe names the variant {name} twice: {GRAMMAR}"));
            }
            let levers = levers
                .split([',', ' '])
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(|l| match l.strip_prefix("hide=") {
                    Some("") => Err(format!("--probe's `hide=` names no material: {GRAMMAR}")),
                    Some(part) => Ok(Lever::Hide(part.to_string())),
                    // j19 wrote `tune.render.x=0` after the old grammar's wording and lost
                    // its timing to "names no tunable".
                    None if l.starts_with("tune.") => Err(format!(
                        "--probe's lever `{l}` names its tunable with a `tune.` prefix, which \
                         no tunable has: drop the prefix, `{}`; {GRAMMAR}",
                        &l["tune.".len()..]
                    )),
                    None => parse_assignment(l)
                        .map(|(n, v)| Lever::Tune(n, v))
                        .map_err(|e| format!("--probe's lever `{l}`: {e}; {GRAMMAR}")),
                })
                .collect::<Result<Vec<_>, _>>()?;
            variants.push(Variant { name, levers });
        }
        if variants.len() == before {
            return Err(format!("--probe names no variant: {GRAMMAR}"));
        }
        Ok(())
    }

    /// The phases in order, each a variant and its round: every round runs every variant
    /// once, so a drift over the run (heat, the clock, streaming) falls on all of them alike,
    /// and two rounds of one variant are its A/A pair.
    pub fn phases(&self, rounds: usize) -> Vec<(usize, usize)> {
        (0..rounds.max(1))
            .flat_map(|round| (0..self.variants.len()).map(move |v| (v, round)))
            .collect()
    }

    /// Every tunable some variant sets, once each, in the order first named.
    pub fn tunables(&self) -> Vec<&str> {
        let mut names: Vec<&str> = Vec::new();
        for lever in self.variants.iter().flat_map(|v| &v.levers) {
            if let Lever::Tune(name, _) = lever
                && !names.contains(&name.as_str())
            {
                names.push(name);
            }
        }
        names
    }
}

/// When a phase's frames count: how long after a switch to wait at least, how many frames
/// in a row no pipeline may be compiling, how long a phase may wait in all, and the share of
/// the variant's most vertices a frame must draw.
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub wait: u32,
    pub idle: u32,
    pub timeout: u32,
    pub drawn_share: f64,
}

/// What a frame says of whether the scene is all there.
#[derive(Clone, Copy, Debug, Default)]
pub struct Seen {
    /// Pipelines still being compiled: until one is built, what it draws is not drawn at all.
    pub compiling: usize,
    /// Vertices the GPU's passes shaded (pipeline statistics), where the adapter gives them.
    pub vertices: Option<f64>,
}

/// Why a frame did not count.
#[derive(Clone, Debug, PartialEq)]
pub enum Why {
    /// Within the wait after the switch.
    Switched,
    /// Pipelines compiling, or not idle for long enough yet.
    Compiling(usize),
    /// Fewer vertices drawn than the variant's most.
    Undrawn { vertices: f64, most: f64 },
}

impl std::fmt::Display for Why {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Why::Switched => write!(f, "the wait after the switch"),
            Why::Compiling(n) => write!(f, "{n} pipelines compiling"),
            Why::Undrawn { vertices, most } => write!(
                f,
                "{:.2}M vertices drawn of the variant's {:.2}M",
                vertices / 1e6,
                most / 1e6
            ),
        }
    }
}

/// What the gate says of a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    Count,
    Wait(Why),
    /// The phase waited past the timeout: it never settled, and why not at the last.
    TimedOut(Why),
}

/// The settle gate (D609): a phase's frame counts only once the switch is `wait` frames
/// past, no pipeline has compiled for `idle` frames in a row, and it draws as many vertices
/// as the variant has drawn at its most. After a shader changed, its pipeline is built
/// seconds later and the ground meanwhile is not drawn at all (play-lake-fps j10: 500
/// frames and more), which a probe counting from the switch times as a saving. Unreal's
/// automated performance tests wait for shaders and PSOs to finish compiling before they
/// capture (`FShaderPipelineCache`, Gauntlet's warm-up), and Unity's Performance Testing
/// package counts only after `WarmupCount` frames.
#[derive(Debug, Default)]
pub struct Gate {
    since: u32,
    idle_run: u32,
}

impl Gate {
    /// Judges the next frame of the phase; `most` is the variant's most vertices on a frame
    /// that passed the wait and the pipelines, over every round so far.
    pub fn frame(&mut self, rules: &Rules, seen: Seen, most: f64) -> Verdict {
        self.since += 1;
        self.idle_run = if seen.compiling == 0 {
            self.idle_run + 1
        } else {
            0
        };
        let why = if self.since <= rules.wait {
            Why::Switched
        } else if self.idle_run < rules.idle {
            Why::Compiling(seen.compiling)
        } else {
            match seen.vertices {
                Some(vertices) if vertices < most * rules.drawn_share => {
                    Why::Undrawn { vertices, most }
                }
                _ => return Verdict::Count,
            }
        };
        if self.since > rules.timeout {
            Verdict::TimedOut(why)
        } else {
            Verdict::Wait(why)
        }
    }

    /// Whether the wait and the pipelines are past, so the frame's vertices may raise the
    /// variant's most.
    pub fn past_pipelines(&self, rules: &Rules) -> bool {
        self.since > rules.wait && self.idle_run >= rules.idle
    }
}

/// One counted frame: its passes' GPU times in ms and the vertices it drew.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub passes: BTreeMap<String, f64>,
    pub vertices: Option<f64>,
}

/// What the whole frame's GPU work is kept under among the passes.
pub const TOTAL: &str = "TOTAL";

/// The median, NaN for none.
pub fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

/// The most vertices any of these frames drew, 0 where none says.
pub fn most_vertices<'a>(frames: impl IntoIterator<Item = &'a Frame>) -> f64 {
    frames
        .into_iter()
        .filter_map(|f| f.vertices)
        .fold(0.0, f64::max)
}

/// A pass's medians: over the variant's frames of all rounds, and of each round alone.
fn pass_medians(rounds: &[Vec<&Frame>], pass: &str) -> (f64, Vec<f64>) {
    let of = |frames: &[&Frame]| -> Vec<f64> {
        frames
            .iter()
            .map(|f| f.passes.get(pass).copied().unwrap_or(0.0))
            .collect()
    };
    let mut all: Vec<f64> = rounds.iter().flat_map(|r| of(r)).collect();
    let each = rounds.iter().map(|r| median(&mut of(r))).collect();
    (median(&mut all), each)
}

/// One row of the table: a pass, each variant's median, and the spread of each variant's
/// round medians, the A/A noise floor.
#[derive(Clone, Debug)]
pub struct Row {
    pub pass: String,
    pub medians: Vec<f64>,
    pub noise: Vec<f64>,
}

impl Row {
    /// The largest A/A spread of any variant: a difference within it is noise.
    pub fn floor(&self) -> f64 {
        self.noise
            .iter()
            .copied()
            .filter(|n| n.is_finite())
            .fold(0.0, f64::max)
    }

    /// Whether variant `v` differs from the first by more than the noise floor, with a floor
    /// measured at all.
    pub fn past(&self, v: usize) -> bool {
        (self.medians[v] - self.medians[0]).abs() > self.floor()
            && self.noise.iter().any(|n| n.is_finite())
    }

    /// Whether any variant differs from the first past the noise floor.
    pub fn changed(&self) -> bool {
        (1..self.medians.len()).any(|v| self.past(v))
    }
}

/// The table's rows, TOTAL first, then the passes costliest in the first variant, `top` of
/// them, then every other pass whose variants differ past the noise floor, costliest first:
/// a significant change is reported wherever it falls, as Chromium's Pinpoint and Mozilla's
/// Perfherder list every metric whose difference is past it (atmosphere_luts' 0.03 ms under
/// D614 fell outside the 14 costliest and went unseen). `by_variant[v][r]` is variant v's
/// counted frames of round r. A frame that drew less
/// than its variant did at its most over every round is dropped: one counted while something
/// was still missing, which a later round's frames show.
pub fn rows(by_variant: &[Vec<Vec<Frame>>], share: f64, top: usize) -> Vec<Row> {
    let kept: Vec<Vec<Vec<&Frame>>> = by_variant
        .iter()
        .map(|rounds| {
            let most = most_vertices(rounds.iter().flatten());
            rounds
                .iter()
                .map(|r| {
                    r.iter()
                        .filter(|f| f.vertices.is_none_or(|v| v >= most * share))
                        .collect()
                })
                .collect()
        })
        .collect();
    let mut names: Vec<String> = by_variant
        .iter()
        .flatten()
        .flatten()
        .flat_map(|f| f.passes.keys().cloned())
        .collect();
    names.sort();
    names.dedup();
    let mut rows: Vec<Row> = names
        .into_iter()
        .map(|pass| {
            let (medians, noise) = kept
                .iter()
                .map(|rounds| {
                    let (all, each) = pass_medians(rounds, &pass);
                    let finite: Vec<f64> = each.into_iter().filter(|m| m.is_finite()).collect();
                    let spread = if finite.len() < 2 {
                        f64::NAN
                    } else {
                        finite.iter().copied().fold(f64::MIN, f64::max)
                            - finite.iter().copied().fold(f64::MAX, f64::min)
                    };
                    (all, spread)
                })
                .unzip();
            Row {
                pass,
                medians,
                noise,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        (b.pass == TOTAL)
            .cmp(&(a.pass == TOTAL))
            .then(b.medians[0].total_cmp(&a.medians[0]))
    });
    let rest = rows.split_off((top + 1).min(rows.len()));
    rows.extend(rest.into_iter().filter(Row::changed));
    rows
}

/// The table, as Unreal's PerfReportTool prints a CSV capture's summary: a row a pass, a
/// column a variant, then each variant against the first and the A/A noise floor, and
/// whether the difference is past it. Chromium's Pinpoint and Mozilla's Perfherder judge an
/// A/B the same way, against the spread of repeats of each side.
pub fn table(variants: &[&str], rows: &[Row]) -> Vec<String> {
    let width = rows.iter().map(|r| r.pass.len()).max().unwrap_or(5).max(5);
    let mut head = format!("{:width$}", "pass");
    for v in variants {
        head += &format!(" {v:>9}");
    }
    for v in variants.iter().skip(1) {
        head += &format!(" {:>9}", format!("{v}-{}", variants[0]));
    }
    head += &format!(" {:>7}  verdict", "A/A");
    let mut lines = vec![head];
    for row in rows {
        let mut line = format!("{:width$}", row.pass);
        for m in &row.medians {
            line += &format!(" {m:>9.3}");
        }
        let floor = row.floor();
        let mut verdicts = Vec::new();
        for (v, m) in row.medians.iter().enumerate().skip(1) {
            let delta = m - row.medians[0];
            line += &format!(" {delta:>+9.3}");
            verdicts.push(match (row.past(v), delta < 0.0) {
                (false, _) => format!("{} within noise", variants[v]),
                (true, true) => format!("{} saves {:.3}", variants[v], -delta),
                (true, false) => format!("{} costs {:.3}", variants[v], delta),
            });
        }
        line += &format!(" {floor:>7.3}  {}", verdicts.join(", "));
        lines.push(line);
    }
    lines
}
