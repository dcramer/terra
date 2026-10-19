//! `cargo xtask nightly` (D411): the full run on main, run by hand until it is scheduled.
//! In its own worktree, `.git/terra-nightly/src`, detached at origin/main, it runs what no hook
//! runs whole: clippy and every test of the workspace, the slow ones too, the kit checker, the
//! artifacts ratchet quick and over every scene, fuzz over seeds 0 to 200, the size ratchet and
//! the docs check after rendering their indexes.
//!
//! When every stage passes it is the one place a baseline is lowered (D414): what fell, and an
//! index that was stale, is committed alone and pushed to main from this machine, with the
//! hooks; if main moved meanwhile, only what its measurement still holds for is carried onto
//! the new main, three times at most. When a stage fails, `git bisect` finds the first commit
//! since the last passing run where it fails. Either way `.git/terra-nightly/report.txt` says
//! what happened. `TERRA_NIGHTLY=0`
//! turns it off. `--probe` is bisect's: one stage at the commit checked out.

use crate::baselines::{self, ARTIFACTS, SIZES};
use crate::check::niced;
use crate::stage::{self, Said, Step};
use crate::worktrees::{git, git_output};
use crate::{Task, output, root, scope};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The task this module runs (D410).
pub(crate) const TASKS: &[Task] = &[
    Task {
        name: "nightly",
        takes: "",
        does: "the full run on origin/main (D411), by hand until scheduled: every test, the kits, the artifacts ratchet over every scene, fuzz over seeds 0 to 200, sizes and docs, in its own worktree; lowers the baselines to what it measured and pushes that alone, or bisects a failure to its first bad commit; .git/terra-nightly/report.txt. TERRA_NIGHTLY=0 turns it off",
        group: "Play and check",
        run: nightly,
    },
    Task {
        name: "ci",
        takes: "[--dry-run] [--no-run] | --watch [minutes]",
        does: "the CI agent (docs/ci-agent.md): runs nightly and turns its report into GitHub issues labelled ci-agent, one a failing stage, closed when it passes, and a status issue; `--watch` runs it whenever origin/main moves, as its container does (tools/ci)",
        group: "Play and check",
        run: |args| crate::node("tools/ci/ci.ts", args),
    },
    Task {
        name: "continuous",
        takes: "[--watch [minutes] | --install [minutes]]",
        does: "the heavy checks no hook runs (the fire sheet first; tools/continuous/checks.json), on this GPU machine at origin/main's tip in their own worktree, each when what it reads changed; a regression names its commit range; .git/terra-continuous/status.txt. `--watch` loops, `--install` a systemd user timer (D37, D398)",
        group: "Play and check",
        run: |args| crate::node("tools/continuous/continuous.ts", args),
    },
];

/// Set to 0, the run does nothing (D411).
const OFF: &str = "TERRA_NIGHTLY";

/// The run's folder in the repository's git folder: its lock, worktree, logs, report, the last
/// passing run's commit, and the probe bisect runs.
const STATE: &str = "terra-nightly";

/// How many times a lowering is pushed before it is left to the next run.
const PUSHES: usize = 3;

/// The most lines of a failing stage the report keeps.
const REPORT_LINES: usize = 20;

/// A stage of the run: what it is called, how its output reads, what builds it, and the
/// command the run runs (lowering, where the stage has a baseline) and the one a bisect's
/// probe runs (holding to the baseline). A probe skips a commit where `build` fails; a stage
/// whose check is the build (clippy) or that builds nothing has none.
struct Stage {
    name: &'static str,
    said: Said,
    build: &'static [&'static str],
    run: &'static [&'static str],
    hold: &'static [&'static str],
}

const CLIPPY: &[&str] = &[
    "cargo",
    "clippy",
    "-q",
    "--message-format=short",
    "--workspace",
    "--all-targets",
    "--",
    "-D",
    "warnings",
];
const TESTS: &[&str] = &[
    "cargo",
    "test",
    "-q",
    "--message-format=short",
    "--workspace",
];
const KITS: &[&str] = &[
    "cargo",
    "run",
    "-q",
    "-p",
    "terra_build",
    "--example",
    "kits",
];
const SIZES_HOLD: &[&str] = &["cargo", "xtask", "sizes"];
const RENDER: &[&str] = &["node", "tools/docs/docs.ts", "--render"];
const DOCS: &[&str] = &["node", "tools/docs/docs.ts"];
const ARTIFACTS_BUILD: &[&str] = &[
    "cargo",
    "build",
    "-q",
    "--release",
    "-p",
    "terra_build_scenes",
    "--example",
    "artifacts",
];

/// The artifacts checker's command with its own arguments.
macro_rules! artifacts {
    ($($arg:literal),*) => {
        &["cargo", "run", "-q", "--release", "-p", "terra_build_scenes", "--example", "artifacts", "--", $($arg),*]
    };
}

const STAGES: &[Stage] = &[
    Stage {
        name: "clippy",
        said: Said::Cargo,
        build: &[],
        run: CLIPPY,
        hold: CLIPPY,
    },
    Stage {
        name: "tests",
        said: Said::Cargo,
        build: &[
            "cargo",
            "test",
            "-q",
            "--message-format=short",
            "--workspace",
            "--no-run",
        ],
        run: TESTS,
        hold: TESTS,
    },
    Stage {
        name: "kits",
        said: Said::Cargo,
        build: &[
            "cargo",
            "build",
            "-q",
            "-p",
            "terra_build",
            "--example",
            "kits",
        ],
        run: KITS,
        hold: KITS,
    },
    Stage {
        name: "artifacts quick",
        said: Said::Printed,
        build: ARTIFACTS_BUILD,
        run: artifacts!("--quick", "--lower"),
        hold: artifacts!("--quick", "--ratchet"),
    },
    Stage {
        name: "artifacts",
        said: Said::Printed,
        build: ARTIFACTS_BUILD,
        run: artifacts!("--lower"),
        hold: artifacts!("--ratchet"),
    },
    Stage {
        name: "fuzz",
        said: Said::Printed,
        build: ARTIFACTS_BUILD,
        run: artifacts!("--fuzz", "--lower"),
        hold: artifacts!("--fuzz", "--ratchet"),
    },
    Stage {
        name: "sizes",
        said: Said::Printed,
        build: &["cargo", "build", "-q", "-p", "xtask"],
        run: &["cargo", "xtask", "sizes", "--lower"],
        hold: SIZES_HOLD,
    },
    Stage {
        name: "docs rendered",
        said: Said::Printed,
        build: &[],
        run: RENDER,
        hold: RENDER,
    },
    Stage {
        name: "docs",
        said: Said::Printed,
        build: &[],
        run: DOCS,
        hold: DOCS,
    },
];

/// How a stage ended: whether it passed, and the lines of its log that say why it failed.
struct Outcome {
    name: &'static str,
    ok: bool,
    lines: Vec<String>,
}

/// The run (D411); see the module's comment.
pub(crate) fn nightly(args: &[String]) -> bool {
    match args {
        [] => {}
        [flag, name, tip] if flag == "--probe" => std::process::exit(probe(name, tip)),
        _ => {
            eprintln!("usage: cargo xtask nightly");
            return false;
        }
    }
    if std::env::var(OFF).is_ok_and(|v| v == "0") {
        println!("nightly: off ({OFF}=0, D411)");
        return true;
    }
    let Some(state) = output(
        "git",
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .map(|common| PathBuf::from(common).join(STATE)) else {
        eprintln!("nightly: git has no repository here");
        return false;
    };
    let logs = state.join("logs");
    let lock = std::fs::create_dir_all(&logs).and_then(|()| File::create(state.join("lock")));
    let mut lock = match lock {
        Ok(lock) => lock,
        Err(error) => {
            eprintln!("nightly: cannot lock {}: {error}", state.display());
            return false;
        }
    };
    match lock.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            println!("nightly: already running (.git/{STATE}/lock)");
            return true;
        }
        Err(std::fs::TryLockError::Error(error)) => {
            eprintln!("nightly: cannot lock {}: {error}", state.display());
            return false;
        }
    }
    let _ = writeln!(lock, "{}", std::process::id());
    let src = state.join("src");
    let tip = match prepare(&src, &logs) {
        Ok(tip) => tip,
        Err(why) => {
            println!("✗ nightly: {why}");
            return false;
        }
    };
    let report = state.join("report.txt");
    let last = std::fs::read_to_string(&report).unwrap_or_default();
    if last_run(&last).is_some_and(|(_, commit)| commit == tip) {
        println!("nightly: origin/main at {} was run already", short(&tip));
        return true;
    }
    println!("nightly over origin/main at {}", short(&tip));
    let outcomes: Vec<Outcome> = STAGES.iter().map(|s| run_stage(s, &src, &logs)).collect();
    let when = output("date", &["-u", "+%Y-%m-%d %H:%M UTC"]).unwrap_or_default();
    let passed = outcomes.iter().all(|o| o.ok);
    let text = if passed {
        let _ = std::fs::write(state.join("good"), format!("{tip}\n"));
        format!("passed at {tip} ({when}): {}\n", land(&src, &logs, &tip))
    } else {
        let good = std::fs::read_to_string(state.join("good")).ok();
        let first = outcomes.iter().find(|o| !o.ok).map_or("", |o| o.name);
        let found = bisect(&src, &state, first, good.as_deref().map(str::trim), &tip);
        failure(&tip, &when, &found, &outcomes)
    };
    let _ = std::fs::write(&report, &text);
    println!(
        "{} nightly {} (.git/{STATE}/report.txt)",
        if passed { "✓" } else { "✗" },
        text.lines().next().unwrap_or_default()
    );
    passed
}

/// The run's worktree at origin/main, made the first time and seeded (D183), cleared of
/// anything a run before left; the commit it is at.
fn prepare(src: &Path, logs: &Path) -> Result<String, String> {
    if !src.join(".git").exists() {
        let _ = git(&root(), &["worktree", "prune"]);
        let path = src.to_string_lossy();
        if !git(&root(), &["fetch", "-q", "origin"])
            || !git(
                &root(),
                &["worktree", "add", "-q", "--detach", &path, "origin/main"],
            )
        {
            return Err(format!("cannot make its worktree at {path}"));
        }
    }
    let _ = git(src, &["bisect", "reset"]);
    for args in [
        &["fetch", "-q", "origin"][..],
        &["reset", "-q", "--hard"],
        &["clean", "-fdq", "--", ARTIFACTS],
        &["checkout", "-q", "--detach", "origin/main"],
    ] {
        if !git(src, args) {
            return Err(format!(
                "git {} failed in {}",
                args.join(" "),
                src.display()
            ));
        }
    }
    if !src.join("target").exists() {
        let mut seed = Command::new("cargo");
        seed.args(["xtask", "seed"]).current_dir(src);
        Step::new("seed", Said::Printed, logs).run(seed);
    }
    git_output(src, &["rev-parse", "HEAD"])
        .map(|tip| tip.trim().to_string())
        .ok_or_else(|| "git cannot say where origin/main is".to_string())
}

/// A command of a stage, at a lower priority, in `dir`, as the full run on main (D414).
fn command(words: &[&str], dir: &Path) -> Command {
    let mut command = niced(words[0]);
    command.args(&words[1..]).current_dir(dir);
    command
}

fn run_stage(stage: &Stage, src: &Path, logs: &Path) -> Outcome {
    let step = Step::new(stage.name, stage.said, logs).rerun(format!(
        "{} (in {})",
        stage.hold.join(" "),
        src.display()
    ));
    let ok = step.run(command(stage.run, src));
    let lines = if ok {
        Vec::new()
    } else {
        let text = std::fs::read_to_string(step.log()).unwrap_or_default();
        let body = text.split_once("\n\n").map_or("", |(_, body)| body);
        let lines = stage::excerpt(body, stage.said)
            .into_iter()
            .filter(|line| !line.trim().is_empty())
            .collect();
        stage::cap(lines, REPORT_LINES, REPORT_LINES / 2)
    };
    Outcome {
        name: stage.name,
        ok,
        lines,
    }
}

/// The report of a failed run. Its first block, to the first blank line, is what the session
/// hook prints: the verdict, the first bad commit and the first failing stage's lines.
fn failure(tip: &str, when: &str, found: &str, outcomes: &[Outcome]) -> String {
    let failed: Vec<&Outcome> = outcomes.iter().filter(|o| !o.ok).collect();
    let names: Vec<&str> = failed.iter().map(|o| o.name).collect();
    let mut text = format!("failed at {tip} ({when}): {}\n", names.join(", "));
    text.push_str(&format!("  {} {found}\n", names[0]));
    for line in &failed[0].lines {
        text.push_str(&format!("    {line}\n"));
    }
    text.push('\n');
    for outcome in &failed[1..] {
        text.push_str(&format!("{}:\n", outcome.name));
        for line in &outcome.lines {
            text.push_str(&format!("    {line}\n"));
        }
    }
    let stages: Vec<String> = outcomes
        .iter()
        .map(|o| format!("{} {}", if o.ok { "✓" } else { "✗" }, o.name))
        .collect();
    text.push_str(&format!(
        "stages: {}\nlogs: .git/{STATE}/logs/\n",
        stages.join(", ")
    ));
    text
}

/// The first commit since the last passing run where a stage fails, as the report says it.
fn bisect(src: &Path, state: &Path, stage: &str, good: Option<&str>, tip: &str) -> String {
    let Some(good) = good else {
        return "failed; not bisected: no run passed before".into();
    };
    if good == tip || !git(src, &["merge-base", "--is-ancestor", good, tip]) {
        return format!(
            "failed; not bisected: the last passing run's {} is not before it",
            short(good)
        );
    }
    // The probe is this program as it is now, so it is the same at every commit.
    let probe = state.join("probe");
    let log = state.join("logs").join("bisect.log");
    let Ok(file) = std::env::current_exe()
        .and_then(|exe| std::fs::copy(exe, &probe))
        .and_then(|_| File::create(&log))
    else {
        return "failed; not bisected: cannot copy xtask to probe with".into();
    };
    println!("… bisecting {stage} from {} to {}", short(good), short(tip));
    let ran = git(src, &["bisect", "start", tip, good])
        && file.try_clone().is_ok_and(|out| {
            Command::new("git")
                .args(["bisect", "run"])
                .arg(&probe)
                .args(["nightly", "--probe", stage, tip])
                .current_dir(src)
                .stdout(out)
                .stderr(Stdio::from(file))
                .status()
                .is_ok()
        });
    let said = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = git(src, &["bisect", "reset"]);
    let _ = git(src, &["reset", "-q", "--hard"]);
    let _ = git(src, &["checkout", "-q", "--detach", tip]);
    let subject = |sha: &str| {
        git_output(src, &["log", "-1", "--format=%s", sha])
            .map(|s| s.trim().chars().take(200).collect::<String>())
            .unwrap_or_default()
    };
    match (ran, first_bad(&said)) {
        (true, Some(Found::Commit(sha))) => {
            format!("first fails at {} {}", short(&sha), subject(&sha))
        }
        (true, Some(Found::OneOf(shas))) => format!(
            "first fails at one of {} (the commits between did not build)",
            shas.iter().map(|s| short(s)).collect::<Vec<_>>().join(", ")
        ),
        _ => format!("failed; git bisect found nothing (.git/{STATE}/logs/bisect.log)"),
    }
}

/// What `git bisect run` found.
#[derive(Debug, PartialEq)]
enum Found {
    Commit(String),
    /// Skipped commits left it one of these.
    OneOf(Vec<String>),
}

/// The first bad commit in what `git bisect run` said.
fn first_bad(said: &str) -> Option<Found> {
    let hex = |word: &str| word.len() >= 7 && word.chars().all(|c| c.is_ascii_hexdigit());
    if let Some(line) = said
        .lines()
        .find(|l| l.ends_with(" is the first bad commit"))
    {
        let sha = line.split(' ').next()?;
        return hex(sha).then(|| Found::Commit(sha.to_string()));
    }
    let (_, after) = said.split_once("The first bad commit could be any of:\n")?;
    let shas: Vec<String> = after
        .lines()
        .map_while(|l| l.split_whitespace().next().filter(|w| hex(w)))
        .map(String::from)
        .collect();
    (!shas.is_empty()).then_some(Found::OneOf(shas))
}

/// What a probe ends with, for `git bisect run`: 0 good, 1 bad, 125 skip (it did not
/// build), 128 to stop bisecting (the baselines could not be put over the commit's or taken
/// off again).
fn probe_exit(overlaid: bool, built: bool, passed: bool) -> i32 {
    match (overlaid, built, passed) {
        (false, _, _) => 128,
        (true, false, _) => 125,
        (true, true, true) => 0,
        (true, true, false) => 1,
    }
}

/// Bisect's probe: one stage held to the tip's baselines at the commit checked out here.
fn probe(name: &str, tip: &str) -> i32 {
    let (Some(stage), Ok(here)) = (
        STAGES.iter().find(|s| s.name == name),
        std::env::current_dir(),
    ) else {
        eprintln!("nightly --probe: no stage {name}");
        return 128;
    };
    let succeeds = |words: &[&str]| {
        command(words, &here)
            .status()
            .is_ok_and(|status| status.success())
    };
    // The tip's baselines over the commit's: the commit fails where it is over what the tip
    // holds, whatever its own baselines said.
    let _ = std::fs::remove_dir_all(here.join(ARTIFACTS));
    let overlaid = git(&here, &["checkout", tip, "--", ARTIFACTS, SIZES]);
    let built = overlaid && (stage.build.is_empty() || succeeds(stage.build));
    let passed = built && succeeds(stage.hold);
    let restored =
        git(&here, &["reset", "-q", "--hard"]) && git(&here, &["clean", "-fdq", "--", ARTIFACTS]);
    let code = probe_exit(overlaid && restored, built, passed);
    println!("nightly --probe {name}: {code}");
    code
}

/// What a run lowers, a file at a time, when main moved before its push.
#[derive(Debug, PartialEq)]
enum Holds {
    /// Carried over as it was measured.
    Keep,
    /// Left to the next run.
    Drop,
    /// The size baseline, carried entry by entry (`sizes::carry_lowered`).
    Merge,
    /// A rendered index, rendered again on the new main.
    Render,
}

/// Whether a file a run lowered at one commit still holds on a later main: an artifacts or
/// fuzz baseline only if nothing building's check reads changed in between (`building`, as
/// the check's own scoping decides) and the file itself did not.
fn holds(path: &str, since: &BTreeSet<String>, building: bool) -> Holds {
    if path == SIZES {
        Holds::Merge
    } else if !baselines::is_baseline(path) {
        Holds::Render
    } else if building || since.contains(path) {
        Holds::Drop
    } else {
        Holds::Keep
    }
}

/// Whether a change touches what building's checks read, as `cargo xtask check` decides
/// whether its kit, artifacts and fuzz stages run.
fn building_touched(since: &BTreeSet<String>) -> bool {
    let files: Vec<String> = since.iter().cloned().collect();
    let scope = scope::classify(&files, &scope::packages());
    if scope.all() || scope.changed.contains("xtask") {
        return true;
    }
    let asked: BTreeSet<String> = scope.changed.union(&scope.readers).cloned().collect();
    scope::with_dependents(&asked).is_none_or(|found| found.contains("terra_build"))
}

/// The files the run changed that it commits: the baselines and the rendered indexes. Any
/// other file a stage changed is put back.
fn lowered(src: &Path) -> Vec<String> {
    let status = git_output(src, &["status", "--porcelain=v1", "-uall"]).unwrap_or_default();
    let mut kept = Vec::new();
    for line in status.lines().filter(|l| l.len() > 3) {
        let path = line[3..].to_string();
        if baselines::is_baseline(&path) || path.ends_with(".md") {
            kept.push(path);
        } else {
            let _ = git(src, &["checkout", "HEAD", "--", &path]);
        }
    }
    kept
}

/// Commits what the run lowered and pushes it to main, carrying it onto a main that moved;
/// what the report says came of it.
fn land(src: &Path, logs: &Path, tip: &str) -> String {
    let mut changed = lowered(src);
    if changed.is_empty() {
        return "nothing to lower".into();
    }
    let mut base = tip.to_string();
    for _ in 0..PUSHES {
        let message = format!(
            "Baselines lowered to main's at {}, as the full run on main (`cargo xtask nightly`) measured them: {} \
             (D414, D411)\n\n`cargo xtask nightly` ran every stage on origin/main at {tip} and \
             wrote only what fell; hand lowering, which raced other changes onto main, is \
             what this replaces.\n",
            short(tip),
            kinds(&changed)
        );
        let mut add = vec!["add", "-A", "--"];
        add.extend(changed.iter().map(String::as_str));
        if !git(src, &add) || !logged(src, logs, "commit", &["commit", "-q", "-m", &message]) {
            return format!(
                "lowered but not committed: its check failed (.git/{STATE}/logs/commit.log)"
            );
        }
        let ours = git_output(src, &["rev-parse", "HEAD"]).unwrap_or_default();
        let ours = ours.trim().to_string();
        if logged(src, logs, "push", &["push", "-q", "origin", "HEAD:main"]) {
            return format!(
                "lowered and pushed as {} ({})",
                short(&ours),
                kinds(&changed)
            );
        }
        let _ = git(src, &["fetch", "-q", "origin"]);
        let new = git_output(src, &["rev-parse", "origin/main"]).unwrap_or_default();
        let new = new.trim().to_string();
        if new == base {
            return format!(
                "lowered but not pushed: its check failed (.git/{STATE}/logs/push.log)"
            );
        }
        carry(src, tip, &ours, &new);
        base = new;
        changed = lowered(src);
        if changed.is_empty() {
            return format!(
                "lowering left to the next run: main moved over what it measured at {}",
                short(tip)
            );
        }
    }
    format!("lowering left to the next run: main moved under it {PUSHES} times")
}

/// A git command whose output goes to a log of its own, as the run (D414).
fn logged(src: &Path, logs: &Path, name: &str, args: &[&str]) -> bool {
    let Ok(out) = File::create(logs.join(format!("{name}.log"))) else {
        return false;
    };
    let Ok(err) = out.try_clone() else {
        return false;
    };
    Command::new("git")
        .args(args)
        .current_dir(src)
        .stdout(out)
        .stderr(err)
        .status()
        .is_ok_and(|status| status.success())
}

/// The worktree put at a new main with what was lowered at `tip` (committed as `ours`) and
/// still holds there.
fn carry(src: &Path, tip: &str, ours: &str, new: &str) {
    let list = |from: &str, to: &str| -> BTreeSet<String> {
        git_output(src, &["diff", "--name-only", "--no-renames", from, to])
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    };
    let since = list(tip, new);
    let building = building_touched(&since);
    let files = list(tip, ours);
    let show = |at: &str, path: &str| git_output(src, &["show", &format!("{at}:{path}")]);
    let _ = git(src, &["reset", "-q", "--hard"]);
    let _ = git(src, &["checkout", "-q", "--detach", new]);
    let mut render = false;
    for path in &files {
        let file = src.join(path);
        match holds(path, &since, building) {
            Holds::Keep => {
                let _ = match show(ours, path) {
                    Some(text) => std::fs::write(&file, text),
                    None => std::fs::remove_file(&file),
                };
            }
            Holds::Merge => {
                let (at, mine) = (show(tip, path), show(ours, path));
                let now = std::fs::read_to_string(&file).unwrap_or_default();
                let carried = crate::sizes::carry_lowered(
                    &at.unwrap_or_default(),
                    &mine.unwrap_or_default(),
                    &now,
                    &since,
                );
                let _ = std::fs::write(&file, carried);
            }
            Holds::Render => render = true,
            Holds::Drop => {}
        }
    }
    if render {
        let _ = command(RENDER, src).stdout(Stdio::null()).status();
    }
}

/// The last run's verdict and commit, from its report's first line, `passed at <commit> …`
/// or `failed at <commit> …`.
fn last_run(report: &str) -> Option<(bool, &str)> {
    let (verdict, rest) = report.lines().next()?.split_once(" at ")?;
    let passed = match verdict {
        "passed" => true,
        "failed" => false,
        _ => return None,
    };
    let commit = rest.split_whitespace().next()?;
    (commit.len() >= 7 && commit.chars().all(|c| c.is_ascii_hexdigit())).then_some((passed, commit))
}

/// What a lowering holds, by kind: `3 artifacts files, the size baseline, 1 index`.
fn kinds(changed: &[String]) -> String {
    let sizes = changed.iter().any(|p| p == SIZES);
    let artifacts = changed
        .iter()
        .filter(|p| p.as_str() != SIZES && baselines::is_baseline(p))
        .count();
    let indexes = changed.len() - artifacts - usize::from(sizes);
    let mut parts = Vec::new();
    if artifacts > 0 {
        parts.push(format!("{artifacts} artifacts and fuzz files"));
    }
    if sizes {
        parts.push("the size baseline".to_string());
    }
    if indexes > 0 {
        parts.push(format!("{indexes} rendered indexes"));
    }
    parts.join(", ")
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(10)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_probe_skips_what_does_not_build_and_stops_when_it_cannot_set_up() {
        assert_eq!(probe_exit(true, true, true), 0);
        assert_eq!(probe_exit(true, true, false), 1);
        assert_eq!(probe_exit(true, false, false), 125);
        assert_eq!(probe_exit(false, false, false), 128);
        assert_eq!(probe_exit(false, true, true), 128);
    }

    #[test]
    fn every_stage_holds_in_its_probe_and_lowers_only_in_the_run() {
        for stage in STAGES {
            assert!(!stage.hold.contains(&"--lower"), "{}", stage.name);
            assert!(STAGES.iter().filter(|s| s.name == stage.name).count() == 1);
        }
        let lowering = STAGES.iter().filter(|s| s.run.contains(&"--lower")).count();
        assert_eq!(
            lowering, 4,
            "artifacts quick and full, fuzz and sizes lower"
        );
    }

    #[test]
    fn the_last_run_is_read_from_its_reports_first_line() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let passed = format!("passed at {sha} (2026-10-18 04:00 UTC): nothing to lower\n");
        assert_eq!(last_run(&passed), Some((true, sha)));
        let failed = format!("failed at {sha} (when): fuzz\n  fuzz first fails at …\n");
        assert_eq!(last_run(&failed), Some((false, sha)));
        assert_eq!(last_run(""), None);
        assert_eq!(last_run("nightly at noon"), None);
        assert_eq!(last_run("passed at main"), None);
    }

    #[test]
    fn the_first_bad_commit_is_read_from_what_bisect_said() {
        let said = "running probe\nnightly --probe fuzz: 1\n\
                    abcdef1234567890 is the first bad commit\ncommit abcdef1234567890\n";
        assert_eq!(
            first_bad(said),
            Some(Found::Commit("abcdef1234567890".into()))
        );
        let skipped = "There are only 'skip'ped commits left to test.\n\
                       The first bad commit could be any of:\n\
                       1111111aaaa\n2222222bbbb\nWe cannot bisect more!\n";
        assert_eq!(
            first_bad(skipped),
            Some(Found::OneOf(vec![
                "1111111aaaa".into(),
                "2222222bbbb".into()
            ]))
        );
        assert_eq!(first_bad("bisect run failed: exit code 128"), None);
    }

    #[test]
    fn a_lowering_holds_on_a_moved_main_only_where_nothing_it_measured_changed() {
        let fuzz = format!("{ARTIFACTS}/fuzz/seed-2.tsv");
        let sheet = format!("{ARTIFACTS}/all/sheet.tsv");
        let since: BTreeSet<String> = [sheet.clone()].into_iter().collect();
        // Building unchanged: a file kept, unless the move changed that file itself.
        assert_eq!(holds(&fuzz, &since, false), Holds::Keep);
        assert_eq!(holds(&sheet, &since, false), Holds::Drop);
        // Building changed: every artifacts and fuzz count is measured again next run.
        assert_eq!(holds(&fuzz, &BTreeSet::new(), true), Holds::Drop);
        // Sizes entry by entry whatever moved; the indexes rendered again.
        assert_eq!(holds(SIZES, &since, true), Holds::Merge);
        assert_eq!(holds("docs/decisions.md", &since, true), Holds::Render);
    }

    #[test]
    fn a_failure_reports_its_first_stage_first_and_the_hook_reads_one_block() {
        let outcomes = [
            Outcome {
                name: "clippy",
                ok: true,
                lines: vec![],
            },
            Outcome {
                name: "fuzz",
                ok: false,
                lines: vec!["seed 7: z-fighting 3, was 0".into()],
            },
            Outcome {
                name: "docs",
                ok: false,
                lines: vec!["docs: 1 faults".into()],
            },
        ];
        let sha = "0123456789abcdef";
        let text = failure(sha, "now", "first fails at 0123456789 x", &outcomes);
        let block = text.split("\n\n").next().unwrap();
        assert!(block.starts_with(&format!("failed at {sha} (now): fuzz, docs\n")));
        assert!(block.contains("  fuzz first fails at 0123456789 x\n    seed 7"));
        assert!(!block.contains("docs: 1 faults"));
        assert_eq!(last_run(&text), Some((false, sha)));
    }
}
