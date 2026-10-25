//! `cargo xtask check` (D37): the hook before a push, formatting, the size and
//! one-definition lints and clippy of the crates changed since main, and nothing else; and
//! `--all`, the full check the scheduled run on main runs (`.github/workflows/main.yml`),
//! with the gates only it runs: every test, the tools'
//! tests, the kits, foliage, artifacts and fuzz ratchets, the combat and controller scorecards (D452, D457), the
//! Blender scripts, and the shared
//! crates kept free of Bevy (D28) and of the clock and the system's randomness.
//!
//! Each stage runs quietly (D388, `stage`): a line when it passes, only what is needed to fix
//! it when it fails, everything in `target/check/<stage>.log`. `--verbose`, or
//! `TERRA_VERBOSE=1`, streams everything as it is said.

use crate::build::{KITS, artifacts_command, fuzz_args};
use crate::scope::{self, Scope};
use crate::stage::{self, Said, Step};
use crate::{Task, output, root};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// The task this module runs (D410).
pub(crate) const TASKS: &[Task] = &[Task {
    name: "check",
    takes: "[--all | --base <commit>] [--verbose]",
    does: "the hook before a push (D37): formatting, the size and one-definition lints and clippy of the crates changed since main, in seconds; --base <commit> takes what changed since that commit instead of the merge-base with origin/main (the hook's remote check, where origin/main is not known); --all is the full check over everything, as the scheduled run on main runs it: every test, the tools' tests, the kit, foliage, artifact, fuzz, combat, controller and Blender checks and the shared crates' rules",
    group: "Play and check",
    run: check,
}];

/// The only crates allowed to depend on Bevy (D28): the applications, the world on screen
/// that the game stands on (D198), and the crates cut from the game (D407).
const BEVY_CRATES: [&str; 9] = [
    "terra_bases",
    "terra_building",
    "terra_creatures",
    "terra_folk",
    "terra_game",
    "terra_sandbox",
    "terra_survival",
    "terra_walker",
    "terra_world",
];

/// A test the full check names when it runs longer, in seconds (D406): the full check waits on
/// its slowest test.
const SLOW_TEST_SECS: u64 = 120;

/// Where the check's stages keep their logs, relative to the root.
const LOGS: &str = "target/check";

/// How a stage runs: a command, a check made here, or not at all, and why.
enum How {
    Command(Command),
    Here(fn(&mut String) -> bool),
    Skip(&'static str),
}

/// The gate (D37). Before a push (no flag), over everything that differs from the merge-base
/// with origin/main: formatting of the crates changed, the code-health lints and clippy on
/// them, nothing else. `--all` is the full check over everything, as the scheduled run on
/// main runs it. The stages run in order and the first to fail ends it.
pub(crate) fn check(args: &[String]) -> bool {
    let mut all = false;
    let mut base = None;
    let usage = || {
        eprintln!("usage: cargo xtask check [--all | --base <commit>] [--verbose]");
        false
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--all" => all = true,
            "--base" => match args.next() {
                Some(commit) if !commit.starts_with('-') => base = Some(commit.as_str()),
                _ => return usage(),
            },
            "--verbose" => stage::set_verbose(),
            _ => return usage(),
        }
    }
    if all && base.is_some() {
        return usage();
    }
    let started = Instant::now();
    let scope = if all {
        Scope::everything("--all")
    } else {
        scope::push(base)
    };
    let name = if all { "full check" } else { "check" };
    println!("{name} over {}", scope.summary());
    let stages = if all { full() } else { hook(&scope) };
    let count = stages.len();
    let mut skipped = Vec::new();
    for (done, (step, how)) in stages.into_iter().enumerate() {
        let passed = match how {
            How::Command(command) => step.run(command),
            How::Here(check) => step.run_here(check),
            How::Skip(why) => {
                skipped.push(format!("{} ({why})", step.name()));
                true
            }
        };
        if !passed {
            println!(
                "✗ {name} failed at {} ({done} of {count} stages passed, {}); TERRA_VERBOSE=1 shows everything",
                step.name(),
                stage::duration(started.elapsed()),
            );
            return false;
        }
    }
    if !skipped.is_empty() {
        println!("- not run: {}", skipped.join(", "));
    }
    let took = stage::duration(started.elapsed());
    if all {
        println!("✓ full check passed in {took}");
    } else {
        println!(
            "✓ check passed in {took}; the tests of what you changed are yours to run, and the \
             full check is `cargo xtask check --all` (D37)"
        );
    }
    true
}

/// Cargo's own quiet modes: no progress, the compiler's messages a line each, the test harness
/// a dot a test. What they leave out is in the stage's log when streamed.
fn cargo(command: &str, rest: &[String]) -> Command {
    let mut cargo = niced("cargo");
    cargo.arg(command);
    if !stage::verbose() {
        cargo.args(["-q", "--message-format=short"]);
    }
    cargo.args(rest);
    cargo
}

fn clippy(selected: &[String]) -> (Step, How) {
    let shown = selected.join(" ");
    let mut args = selected.to_vec();
    args.extend(["--all-targets", "--", "-D", "warnings"].map(String::from));
    let step = Step::new("clippy", Said::Cargo, LOGS)
        .rerun(format!("cargo clippy {shown} --all-targets -- -D warnings"));
    (step, How::Command(cargo("clippy", &args)))
}

/// The code-health rules (D396): no file or function past its size, no shared primitive
/// written out again. Lints in all but name, well under a second together, so the hook runs
/// them beside clippy.
fn code_health() -> [(Step, How); 2] {
    [
        (
            Step::new("sizes", Said::Printed, LOGS).rerun("cargo xtask sizes"),
            How::Here(crate::sizes::sizes_hold),
        ),
        (
            Step::new("one definition", Said::Printed, LOGS),
            How::Here(crate::primitives::one_definition),
        ),
    ]
}

/// The hook's stages: formatting, the code-health rules, and clippy of the crates changed,
/// the workspace when the build itself changed (`Scope::all`).
fn hook(scope: &Scope) -> Vec<(Step, How)> {
    let lint = if scope.all() {
        clippy(&["--workspace".to_string()])
    } else if scope.changed.is_empty() {
        (
            Step::new("clippy", Said::Cargo, LOGS),
            How::Skip("no crate changed"),
        )
    } else {
        clippy(&scope::package_args(&scope.changed))
    };
    let mut stages = vec![fmt(scope)];
    if scope.rust {
        stages.extend(code_health());
    }
    stages.push(lint);
    stages
}

/// The full check's stages, over everything.
fn full() -> Vec<(Step, How)> {
    let command = |program: &str, args: &[&str]| {
        let mut command = niced(program);
        command.args(args);
        command
    };
    let workspace = vec!["--workspace".to_string()];
    // A line a test, to the log alone, so the slow ones are timed (`flag_tests_over`).
    let tests = cargo(
        "test",
        &["--workspace", "--", "--format=pretty"].map(String::from),
    );
    let [sizes, one_definition] = code_health();
    let mut stages = vec![
        fmt(&Scope::everything("--all")),
        sizes,
        one_definition,
        clippy(&workspace),
        (
            Step::new("tests", Said::Cargo, LOGS)
                .rerun("cargo test --workspace")
                .flag_tests_over(SLOW_TEST_SECS),
            How::Command(tests),
        ),
        tools_tests(),
        (
            Step::new("kits", Said::Cargo, LOGS).rerun("cargo xtask kits"),
            How::Command(command("cargo", KITS)),
        ),
        (
            Step::new("foliage", Said::Printed, LOGS).rerun("cargo xtask foliage"),
            How::Command(command("node", &["tools/textures/foliage.ts"])),
        ),
    ];
    for (name, rerun, args) in [
        (
            "artifacts",
            "cargo xtask artifacts --quick --ratchet",
            vec!["--quick".to_string(), "--ratchet".into()],
        ),
        (
            "fuzz",
            "cargo xtask fuzz --ratchet",
            fuzz_args(&["--ratchet".into()]),
        ),
    ] {
        let (program, args) = artifacts_command(&args);
        let mut run = niced(program);
        run.args(args);
        stages.push((
            Step::new(name, Said::Cargo, LOGS).rerun(rerun),
            How::Command(run),
        ));
    }
    stages.push((
        Step::new("combat", Said::Printed, LOGS).rerun("cargo xtask combat"),
        How::Command(command("node", &["tools/combat/scorecard.ts"])),
    ));
    stages.push((
        Step::new("controller", Said::Printed, LOGS).rerun("cargo xtask controller"),
        How::Command(command("node", &["tools/controller/scorecard.ts"])),
    ));
    stages.push((
        Step::new("docs", Said::Printed, LOGS).rerun("node tools/docs/docs.ts"),
        How::Command(command("node", &["tools/docs/docs.ts"])),
    ));
    stages.push(blender_scripts());
    stages.push((
        Step::new("shared crates free of Bevy", Said::Printed, LOGS),
        How::Here(shared_crates_avoid_bevy),
    ));
    stages.push((
        Step::new("shared crates repeatable", Said::Printed, LOGS),
        How::Here(shared_crates_are_repeatable),
    ));
    stages
}

/// A command run at a lower priority, as everything the check runs is (D406): its builds and
/// tests take every core, and renders, films and the developer's game started beside it stay
/// responsive. `nice` is on Linux and macOS; elsewhere it runs as it is.
pub(crate) fn niced(program: &str) -> Command {
    if cfg!(unix) {
        let mut command = Command::new("nice");
        command.args(["-n", "10", program]);
        command
    } else {
        Command::new(program)
    }
}

/// Formatting, of the crates changed: it goes file by file, so they are all a change can
/// unsettle.
fn fmt(scope: &Scope) -> (Step, How) {
    let step = Step::new("fmt", Said::Fmt, LOGS).rerun("cargo fmt --all (it fixes them)");
    if !scope.all() && !scope.rust {
        return (step, How::Skip("no Rust changed"));
    }
    let mut fmt = Command::new("cargo");
    fmt.args(["fmt", "--check"]);
    if scope.all() || scope.changed.is_empty() {
        fmt.arg("--all");
    } else {
        fmt.args(scope::package_args(&scope.changed));
    }
    (step, How::Command(fmt))
}

/// The tools' tests (D121), every one.
fn tools_tests() -> (Step, How) {
    let step = Step::new("tools tests", Said::NodeTest, LOGS)
        .rerun("node --test $(git ls-files 'tools/**/*.test.ts' 'tools/**/*.test.mjs')");
    let Some(files) = output(
        "git",
        &["ls-files", "tools/**/*.test.ts", "tools/**/*.test.mjs"],
    ) else {
        return (
            step,
            How::Here(|out| {
                out.push_str("git cannot list the tools' tests\n");
                false
            }),
        );
    };
    let mut node = Command::new("node");
    node.args(["--test", "--test-reporter=spec"])
        .args(files.lines());
    (step, How::Command(node))
}

/// The Blender scripts (D121) compile and find what they import, checked inside the pinned
/// Blender without running them. A machine with no Blender skips this.
fn blender_scripts() -> (Step, How) {
    let step = blender_step();
    if output("tools/blender/run.sh", &["--find"]).is_none() {
        return (step, How::Skip("no Blender here (cargo xtask doctor)"));
    }
    let Some(files) = output("git", &["ls-files", "tools/**/*.py"]) else {
        return (
            step,
            How::Here(|out| {
                out.push_str("git cannot list them\n");
                false
            }),
        );
    };
    let mut run = Command::new("tools/blender/run.sh");
    run.arg("tools/blender/check.py").args(files.lines());
    (step, How::Command(run))
}

fn blender_step() -> Step {
    Step::new("Blender scripts", Said::Printed, LOGS)
        .rerun("tools/blender/run.sh tools/blender/check.py $(git ls-files 'tools/**/*.py')")
}

/// The crates that must not depend on Bevy, by name and folder, or `None` when the crates
/// folder cannot be read. A check that cannot list them fails rather than passing.
fn shared_crates() -> Option<Vec<(String, PathBuf)>> {
    let entries = std::fs::read_dir(root().join("crates")).ok()?;
    let mut found: Vec<(String, PathBuf)> = entries
        .flatten()
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().to_string(),
                entry.path(),
            )
        })
        .filter(|(name, path)| {
            !BEVY_CRATES.contains(&name.as_str()) && path.join("Cargo.toml").exists()
        })
        .collect();
    found.sort();
    (!found.is_empty()).then_some(found)
}

/// What `cargo tree` says of a crate's dependencies, or `None` when it gave no answer:
/// "ok" only when it answered and Bevy is not among them.
fn bevy_verdict(tree: Option<&str>) -> &'static str {
    match tree {
        None => "CARGO TREE FAILED",
        Some(tree) if tree.lines().any(|line| line.starts_with("bevy")) => "DEPENDS ON BEVY",
        Some(_) => "ok",
    }
}

/// Anything every player must agree on lives in crates that know nothing about Bevy.
fn shared_crates_avoid_bevy(out: &mut String) -> bool {
    let _ = writeln!(out, "shared-state crates must not depend on Bevy (D28)");
    let Some(crates) = shared_crates() else {
        let _ = writeln!(out, "  cannot list the crates");
        return false;
    };
    let mut ok = true;
    for (name, _) in crates {
        let tree = output(
            "cargo",
            &["tree", "-p", &name, "-e", "normal", "--prefix", "none"],
        );
        let verdict = bevy_verdict(tree.as_deref());
        let _ = writeln!(out, "  {name}: {verdict}");
        ok &= verdict == "ok";
    }
    ok
}

/// What makes a run of shared code come out differently from the same start: the clock and
/// the operating system's randomness. Shared code takes time and seeds as arguments.
const UNREPEATABLE: [&str; 6] = [
    "Instant::now",
    "SystemTime::now",
    "thread_rng",
    "rand::random",
    "RandomState::new",
    "getrandom",
];

/// The lines of a source that use something in [`UNREPEATABLE`], numbered from 1.
fn unrepeatable(text: &str) -> Vec<(usize, &'static str)> {
    text.lines()
        .enumerate()
        .filter_map(|(number, line)| {
            let what = UNREPEATABLE.iter().find(|what| line.contains(*what))?;
            Some((number + 1, *what))
        })
        .collect()
}

fn shared_crates_are_repeatable(out: &mut String) -> bool {
    let _ = writeln!(
        out,
        "shared-state crates must not read the clock or the system's randomness"
    );
    let Some(crates) = shared_crates() else {
        let _ = writeln!(out, "  cannot list the crates");
        return false;
    };
    let mut ok = true;
    for (name, path) in crates {
        let mut found = Vec::new();
        if !sources(&path.join("src"), &mut found) || found.is_empty() {
            let _ = writeln!(out, "  {name}: cannot read its sources");
            ok = false;
            continue;
        }
        let mut clean = true;
        for file in found {
            // A benchmark's own program may time itself.
            if file.ends_with("main.rs") || file.components().any(|part| part.as_os_str() == "bin")
            {
                continue;
            }
            let at = file.strip_prefix(root()).unwrap_or(&file).display();
            let Ok(text) = std::fs::read_to_string(&file) else {
                let _ = writeln!(out, "  {at}: cannot read it");
                clean = false;
                continue;
            };
            for (number, what) in unrepeatable(&text) {
                let _ = writeln!(out, "  {at}:{number}: {what}");
                clean = false;
            }
        }
        if clean {
            let _ = writeln!(out, "  {name}: ok");
        }
        ok &= clean;
    }
    ok
}

/// Every Rust file under a folder. False if any folder in it could not be read.
pub(crate) fn sources(folder: &Path, found: &mut Vec<PathBuf>) -> bool {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return false;
    };
    let mut ok = true;
    for entry in entries {
        let Ok(entry) = entry else {
            ok = false;
            continue;
        };
        let path = entry.path();
        if path.is_dir() {
            ok &= sources(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    ok
}

/// The gate's stages: what the hook and the full check run.
#[cfg(test)]
#[path = "check/tests.rs"]
mod gate_tests;
