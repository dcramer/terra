//! Steps run quietly (D388): what a step says goes to its log, and the terminal gets one line
//! when it passes, `✓ clippy (41.2s)`, or, when it fails, `✗ clippy` with only what is needed
//! to fix it (the failing tests' panics, the compiler's errors, a checker's faults), at most
//! [`MOST_LINES`] lines, then the log's path and how to run the step alone. The gates' output
//! lands in an agent's context on every commit and push, and a long one is cut short there,
//! sometimes before its failure.
//!
//! `TERRA_VERBOSE=1` streams everything as it is said instead, for every task that runs its
//! steps here (`cargo xtask check --verbose` does the same).

use crate::root;
use std::fmt::Write as _;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The most lines a failing step prints of what it said; the rest is in its log.
pub(crate) const MOST_LINES: usize = 50;
/// The widest a line printed from a log is kept, in characters.
const WIDEST: usize = 300;

static VERBOSE: AtomicBool = AtomicBool::new(false);

/// Streams every step's output as it is said, as `TERRA_VERBOSE=1` does.
pub(crate) fn set_verbose() {
    VERBOSE.store(true, Ordering::Relaxed);
}

/// Whether steps stream everything rather than keep it to their logs.
pub(crate) fn verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
        || std::env::var("TERRA_VERBOSE").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// What a step says, so the part that matters can be picked out of it when it fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Said {
    /// Cargo: the compiler's errors (its short format, or the long one's first line and
    /// place) and the test harness's failures with their panics.
    Cargo,
    /// rustfmt's check: where it would change the code.
    Fmt,
    /// Node's test runner, spec reporter: the failing tests.
    NodeTest,
    /// A checker's own report, as it printed it.
    Printed,
}

/// A step: its name, how its output reads, where its log goes and how to run it alone.
pub(crate) struct Step {
    name: String,
    said: Said,
    logs: PathBuf,
    rerun: Option<String>,
    on_stderr: bool,
}

impl Step {
    /// A step whose log is `<logs>/<name>.log`, `logs` relative to the root.
    pub(crate) fn new(name: &str, said: Said, logs: impl AsRef<Path>) -> Self {
        Self {
            name: name.to_string(),
            said,
            logs: root().join(logs),
            rerun: None,
            on_stderr: false,
        }
    }

    /// The command that runs this step alone, printed when it fails.
    pub(crate) fn rerun(mut self, command: impl Into<String>) -> Self {
        self.rerun = Some(command.into());
        self
    }

    /// Says its lines on stderr, for a process whose stdout is a pipe nobody reads.
    pub(crate) fn on_stderr(mut self) -> Self {
        self.on_stderr = true;
        self
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Its log: `<logs>/<name>.log`, the name made safe for a file.
    pub(crate) fn log(&self) -> PathBuf {
        self.logs.join(format!("{}.log", slug(&self.name)))
    }

    fn say(&self, text: &str) {
        if self.on_stderr {
            eprintln!("{text}");
        } else {
            println!("{text}");
        }
    }

    fn live(&self) -> bool {
        if self.on_stderr {
            std::io::stderr().is_terminal()
        } else {
            std::io::stdout().is_terminal()
        }
    }

    /// Runs a command as this step, from the root unless it says otherwise.
    pub(crate) fn run(&self, mut command: Command) -> bool {
        let shown = shown(&command);
        if command.get_current_dir().is_none() {
            command.current_dir(root());
        }
        let started = Instant::now();
        if verbose() {
            self.say(&format!("\n> {shown}"));
            let ok = command.status().is_ok_and(|status| status.success());
            self.verdict(ok, started.elapsed(), "", None);
            return ok;
        }
        let log = self.log();
        let opened = std::fs::create_dir_all(&self.logs)
            .and_then(|()| std::fs::File::create(&log))
            .and_then(|mut file| {
                writeln!(file, "> {shown}\n")?;
                Ok((file.try_clone()?, file))
            });
        let (out, err) = match opened {
            Ok(files) => files,
            Err(error) => {
                // Without a log, say everything rather than nothing.
                self.say(&format!("cannot write {}: {error}", log.display()));
                self.say(&format!("\n> {shown}"));
                let ok = command.status().is_ok_and(|status| status.success());
                self.verdict(ok, started.elapsed(), "", None);
                return ok;
            }
        };
        let status = command
            .stdout(out)
            .stderr(err)
            .spawn()
            .and_then(|mut child| self.wait(&mut child, started));
        let ok = status.as_ref().is_ok_and(ExitStatus::success);
        let mut text = std::fs::read_to_string(&log).unwrap_or_default();
        if let Err(error) = &status {
            let _ = writeln!(text, "could not start `{shown}`: {error}");
            let _ = std::fs::write(&log, &text);
        }
        let body = text.split_once("\n\n").map_or("", |(_, body)| body);
        self.verdict(ok, started.elapsed(), body, Some(&log));
        ok
    }

    /// Runs a check made here in xtask as this step: it writes what it found into the text it
    /// is given, which is its log.
    pub(crate) fn run_here(&self, check: impl FnOnce(&mut String) -> bool) -> bool {
        let started = Instant::now();
        let mut text = String::new();
        let ok = check(&mut text);
        if verbose() {
            self.say(text.trim_end());
            self.verdict(ok, started.elapsed(), "", None);
            return ok;
        }
        let log = self.log();
        let written = std::fs::create_dir_all(&self.logs)
            .and_then(|()| std::fs::write(&log, &text))
            .is_ok();
        self.verdict(
            ok,
            started.elapsed(),
            &text,
            written.then_some(log.as_path()),
        );
        ok
    }

    /// Prints a line saying a step did not run, and why.
    pub(crate) fn skipped(&self, why: &str) {
        self.say(&format!("- {} skipped: {why}", self.name));
    }

    /// Waits for a step, showing at a terminal that it is still going and for how long.
    fn wait(&self, child: &mut Child, started: Instant) -> std::io::Result<ExitStatus> {
        if !self.live() {
            return child.wait();
        }
        let show = |text: &str| {
            if self.on_stderr {
                eprint!("{text}");
                let _ = std::io::stderr().flush();
            } else {
                print!("{text}");
                let _ = std::io::stdout().flush();
            }
        };
        loop {
            if let Some(status) = child.try_wait()? {
                show("\r\x1b[2K");
                return Ok(status);
            }
            show(&format!(
                "\r\x1b[2K… {} {}s",
                self.name,
                started.elapsed().as_secs()
            ));
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    /// The step's line, and when it failed, the part of what it said that matters.
    fn verdict(&self, ok: bool, took: Duration, said: &str, log: Option<&Path>) {
        if ok {
            self.say(&format!("✓ {} ({})", self.name, duration(took)));
            return;
        }
        self.say(&format!("✗ {} ({})", self.name, duration(took)));
        if log.is_none() && said.is_empty() {
            // It streamed: what it said is above.
            return;
        }
        for line in excerpt(said, self.said) {
            self.say(&format!("  {line}"));
        }
        if let Some(log) = log {
            let at = log.strip_prefix(root()).unwrap_or(log);
            self.say(&format!("  full log: {}", at.display()));
        }
        let rerun = match self.said {
            Said::Cargo => test_rerun(said).or_else(|| self.rerun.clone()),
            _ => self.rerun.clone(),
        };
        if let Some(rerun) = rerun {
            self.say(&format!("  run it alone: {rerun}"));
        }
    }
}

/// A cargo build a task runs before what it is for (the sandbox for `bench`, the Windows
/// program for `play`), as a step logged in `target/xtask/`.
pub(crate) fn build(args: &[&str]) -> bool {
    let shown = format!("cargo {}", args.join(" "));
    let mut cargo = Command::new("cargo");
    cargo.args(args);
    Step::new(&shown, Said::Cargo, "target/xtask")
        .rerun(&shown)
        .run(cargo)
}

/// A command as it would be typed, for its log and `TERRA_VERBOSE`.
fn shown(command: &Command) -> String {
    let mut text = command.get_program().to_string_lossy().to_string();
    for arg in command.get_args() {
        text.push(' ');
        text.push_str(&arg.to_string_lossy());
    }
    text
}

/// A name made safe for a file: lower case, words joined by dashes.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// How long something took, as a person reads it: `41.2s`, `3m 05s`.
pub(crate) fn duration(took: Duration) -> String {
    let seconds = took.as_secs_f64();
    if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else {
        let whole = took.as_secs();
        format!("{}m {:02}s", whole / 60, whole % 60)
    }
}

/// The part of what a failing step said that is needed to fix it, at most [`MOST_LINES`]
/// lines. What a step said that matches nothing known is given as it printed it, so a failure
/// never goes without words.
pub(crate) fn excerpt(said: &str, how: Said) -> Vec<String> {
    let picked = match how {
        // A program `cargo run` started that failed by itself: its own report is what matters.
        Said::Cargo => Some(cargo_excerpt(said))
            .filter(|lines| {
                !lines
                    .iter()
                    .all(|line| line.starts_with("error: process didn't exit successfully"))
            })
            .unwrap_or_default(),
        Said::Fmt => said
            .lines()
            .filter(|line| line.starts_with("Diff in "))
            .map(|line| {
                let root = format!("{}/", root().display());
                line.replace(&root, "")
            })
            .collect(),
        Said::NodeTest => node_test_excerpt(said),
        Said::Printed => Vec::new(),
    };
    if picked.is_empty() {
        return cap(printed(said), MOST_LINES, 5);
    }
    cap(picked, MOST_LINES, MOST_LINES - 1)
}

/// Keeps a list of lines to `most`: the first `head` and the last that fit, with a line
/// between them saying how many were left out. Every line is kept to [`WIDEST`] characters.
pub(crate) fn cap(lines: Vec<String>, most: usize, head: usize) -> Vec<String> {
    let narrow = |line: String| {
        if line.chars().count() > WIDEST {
            let cut: String = line.chars().take(WIDEST).collect();
            format!("{cut}…")
        } else {
            line
        }
    };
    if lines.len() <= most {
        return lines.into_iter().map(narrow).collect();
    }
    let head = head.min(most - 1);
    let tail = most - 1 - head;
    let left_out = lines.len() - head - tail;
    let mut out: Vec<String> = lines[..head].to_vec();
    out.push(format!("… {left_out} more lines in the log …"));
    out.extend_from_slice(&lines[lines.len() - tail..]);
    out.into_iter().map(narrow).collect()
}

/// Cargo's progress, and the test harness's passes: never what a failure needs.
fn cargo_noise(line: &str) -> bool {
    const PROGRESS: [&str; 14] = [
        "Compiling ",
        "Checking ",
        "Finished ",
        "Running ",
        "Blocking waiting",
        "Downloaded ",
        "Downloading ",
        "Fresh ",
        "Doc-tests ",
        "Locking ",
        "Updating ",
        "Adding ",
        "Building ",
        "Packaging ",
    ];
    let trimmed = line.trim_start();
    PROGRESS.iter().any(|word| trimmed.starts_with(word))
        || (line.starts_with("test ") && line.ends_with(" ... ok"))
        || line.starts_with("test result: ok.")
        || (line.starts_with("running ") && line.ends_with(" tests"))
        || line.starts_with("running 1 test")
}

/// What a step printed, without cargo's progress and with runs of blank lines made one.
fn printed(said: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in said.lines().filter(|line| !cargo_noise(line)) {
        let blank = line.trim().is_empty();
        if blank && out.last().is_none_or(|last| last.trim().is_empty()) {
            continue;
        }
        out.push(line.trim_end().to_string());
    }
    while out.last().is_some_and(|last| last.trim().is_empty()) {
        out.pop();
    }
    out
}

/// A compiler message in cargo's short format: `path:line:column: error...`.
fn short_error(line: &str) -> bool {
    let Some((at, rest)) = line.split_once(": ") else {
        return false;
    };
    if !rest.starts_with("error") {
        return false;
    }
    let mut parts = at.rsplitn(3, ':');
    let (Some(column), Some(row), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !path.is_empty() && column.parse::<u32>().is_ok() && row.parse::<u32>().is_ok()
}

/// The name of a failing test from the harness's `---- name stdout ----`.
fn failing_test(line: &str) -> Option<&str> {
    line.strip_prefix("---- ")?.strip_suffix(" stdout ----")
}

/// The compiler's errors and the failing tests with what they printed and their panics, each
/// line once, in the order said.
fn cargo_excerpt(said: &str) -> Vec<String> {
    let lines: Vec<&str> = said.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let keep = |line: &str, out: &mut Vec<String>| {
        if !out.iter().any(|kept| kept == line) {
            out.push(line.to_string());
        }
    };
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if failing_test(line).is_some() {
            out.push(line.to_string());
            i += 1;
            while i < lines.len() && !lines[i].starts_with("---- ") && lines[i] != "failures:" {
                let line = lines[i].trim_end();
                i += 1;
                if line == "stack backtrace:" {
                    // With `RUST_BACKTRACE` set, the frames, indented under it, are in the log.
                    while i < lines.len() && lines[i].starts_with(' ') {
                        i += 1;
                    }
                    continue;
                }
                if !line.is_empty()
                    && !line.starts_with("note: run with `RUST_BACKTRACE")
                    && !line.starts_with("note: Some details are omitted")
                {
                    out.push(line.to_string());
                }
            }
            continue;
        }
        if short_error(line) || line.starts_with("error") || line.starts_with("test result: FAILED")
        {
            keep(line, &mut out);
            i += 1;
            // The long format's place under its error.
            if lines
                .get(i)
                .is_some_and(|l| l.trim_start().starts_with("--> "))
            {
                keep(lines[i], &mut out);
                i += 1;
            }
            // The reason cargo gives under its own error, indented below `Caused by:`.
            let blank = usize::from(lines.get(i) == Some(&""));
            if lines.get(i + blank) == Some(&"Caused by:") {
                out.push("Caused by:".to_string());
                i += blank + 1;
                while i < lines.len() && lines[i].starts_with("  ") {
                    out.push(lines[i].trim_end().to_string());
                    i += 1;
                }
            }
            continue;
        }
        i += 1;
    }
    out
}

/// How to run a failing test binary's failing tests alone, from cargo's own advice.
fn test_rerun(said: &str) -> Option<String> {
    let pass = said.lines().find_map(|line| {
        let rest = line.strip_prefix("error: test failed, to rerun pass `")?;
        rest.strip_suffix('`')
    })?;
    let names: Vec<&str> = said.lines().filter_map(failing_test).collect();
    Some(if names.is_empty() {
        format!("cargo test {pass}")
    } else {
        format!("cargo test {pass} -- --exact {}", names.join(" "))
    })
}

/// Node's failing tests, from the spec reporter's list at the end, without the stack frames
/// inside Node itself.
fn node_test_excerpt(said: &str) -> Vec<String> {
    let Some((_, failing)) = said.split_once("✖ failing tests:") else {
        return Vec::new();
    };
    printed(failing)
        .into_iter()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("at ")
                && (trimmed.contains("(node:") || trimmed.contains(" node:")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `cargo clippy -q --message-format=short -- -D warnings` says of one warning.
    const CLIPPY: &str = "\
crates/terra_core/src/lib.rs:12:9: error: unused variable: `planted`
crates/terra_core/src/lib.rs:12:9: error: unused variable: `planted`
error: could not compile `terra_core` (lib) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `terra_core` (lib test) due to 1 previous error
";

    /// What `cargo test -q` says of one failing test among passing ones.
    const TEST: &str = "\
   Compiling terra_core v0.1.0 (/x/crates/terra_core)
    Finished `test` profile [optimized + debuginfo] target(s) in 3.10s
     Running unittests src/lib.rs (target/debug/deps/terra_core-1)

running 3 tests
.F.
failures:

---- tests::two_and_two_make_four stdout ----

thread 'tests::two_and_two_make_four' panicked at crates/terra_core/src/lib.rs:40:9:
assertion `left == right` failed
  left: 5
 right: 4
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    tests::two_and_two_make_four

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p terra_core --lib`
";

    #[test]
    fn a_clippy_failure_is_its_errors_each_once() {
        assert_eq!(
            excerpt(CLIPPY, Said::Cargo),
            vec![
                "crates/terra_core/src/lib.rs:12:9: error: unused variable: `planted`",
                "error: could not compile `terra_core` (lib) due to 1 previous error",
                "error: could not compile `terra_core` (lib test) due to 1 previous error",
            ]
        );
    }

    #[test]
    fn a_failing_test_is_its_name_and_its_panic_without_the_passes() {
        let lines = excerpt(TEST, Said::Cargo);
        assert_eq!(lines[0], "---- tests::two_and_two_make_four stdout ----");
        assert!(
            lines
                .iter()
                .any(|l| l.contains("panicked at crates/terra_core/src/lib.rs:40:9"))
        );
        assert!(lines.iter().any(|l| l == "  left: 5"));
        assert!(lines.iter().any(|l| l.starts_with("test result: FAILED")));
        assert!(
            !lines
                .iter()
                .any(|l| l.contains("Compiling") || l.contains("RUST_BACKTRACE"))
        );
        assert_eq!(
            test_rerun(TEST).as_deref(),
            Some("cargo test -p terra_core --lib -- --exact tests::two_and_two_make_four")
        );
    }

    #[test]
    fn a_failing_tests_backtrace_is_left_to_the_log() {
        let said = "---- tests::a stdout ----\nthread 'tests::a' panicked at src/lib.rs:4:5:\nassertion failed: PLANTED\nstack backtrace:\n   0: __rustc::rust_begin_unwind\n             at /rustc/x/library/std/src/panicking.rs:689:5\n   1: terra_x::tests::a\n             at ./src/lib.rs:4:5\nnote: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.\n\nfailures:\n    tests::a\n";
        assert_eq!(
            excerpt(said, Said::Cargo),
            vec![
                "---- tests::a stdout ----",
                "thread 'tests::a' panicked at src/lib.rs:4:5:",
                "assertion failed: PLANTED",
            ]
        );
    }

    #[test]
    fn a_crashed_test_binary_keeps_cargos_reason() {
        let said = "error: test failed, to rerun pass `-p terra_x --lib`\n\nCaused by:\n  process didn't exit successfully: `deps/terra_x` (signal: 11, SIGSEGV)\nnext thing\n";
        let lines = excerpt(said, Said::Cargo);
        assert!(lines.iter().any(|l| l.contains("SIGSEGV")), "{lines:?}");
        assert!(!lines.iter().any(|l| l == "next thing"), "{lines:?}");
    }

    #[test]
    fn the_long_formats_error_keeps_its_place() {
        let said = "error[E0425]: cannot find value `x` in this scope\n --> src/main.rs:3:5\n  |\n3 |     x\n";
        assert_eq!(
            excerpt(said, Said::Cargo),
            vec![
                "error[E0425]: cannot find value `x` in this scope",
                " --> src/main.rs:3:5"
            ]
        );
    }

    #[test]
    fn what_matches_nothing_known_is_given_as_printed_without_cargos_progress() {
        let said = "   Compiling terra_build v0.1.0\n    Finished `dev` profile\n     Running `target/debug/examples/kits`\nkit stone: 3 problems\n\n\n  wall too thin\n";
        assert_eq!(
            excerpt(said, Said::Cargo),
            vec!["kit stone: 3 problems", "", "  wall too thin"]
        );
    }

    #[test]
    fn a_program_cargo_ran_that_failed_is_its_own_report() {
        let said = "kit stone: wall too thin\nerror: process didn't exit successfully: `target/debug/examples/kits` (exit status: 1)\n";
        let lines = excerpt(said, Said::Cargo);
        assert_eq!(lines[0], "kit stone: wall too thin");
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn a_long_report_keeps_its_head_and_its_end_within_the_cap() {
        let said: String = (1..=200).map(|n| format!("line {n}\n")).collect();
        let lines = excerpt(&said, Said::Printed);
        assert_eq!(lines.len(), MOST_LINES);
        assert_eq!(lines[0], "line 1");
        assert_eq!(lines[4], "line 5");
        assert_eq!(
            lines[5],
            format!("… {} more lines in the log …", 200 - MOST_LINES + 1)
        );
        assert_eq!(lines.last().unwrap(), "line 200");
    }

    #[test]
    fn many_failures_are_cut_at_the_cap_with_a_count() {
        let said: String = (1..=80)
            .map(|n| format!("src/a.rs:{n}:1: error: bad {n}\n"))
            .collect();
        let lines = excerpt(&said, Said::Cargo);
        assert_eq!(lines.len(), MOST_LINES);
        assert_eq!(lines[0], "src/a.rs:1:1: error: bad 1");
        assert_eq!(
            lines.last().unwrap(),
            &format!("… {} more lines in the log …", 80 - MOST_LINES + 1)
        );
    }

    #[test]
    fn a_wide_line_is_cut() {
        let lines = cap(vec!["x".repeat(1000)], 10, 5);
        assert_eq!(lines[0].chars().count(), WIDEST + 1);
    }

    #[test]
    fn node_tests_are_the_failing_ones_without_nodes_own_frames() {
        let said = "✔ passes (0.7ms)\n✖ fails here (4.1ms)\nℹ tests 2\n\n✖ failing tests:\n\ntest at tools/a.test.ts:3:1\n✖ fails here (4.1ms)\n  AssertionError [ERR_ASSERTION]: one is not two\n      at TestContext.<anonymous> (file:///x/tools/a.test.ts:3:35)\n      at Test.run (node:internal/test_runner/test:1047:25)\n      at async startSubtestAfterBootstrap (node:internal/test_runner/harness:296:3) {\n";
        let lines = excerpt(said, Said::NodeTest);
        assert_eq!(lines[0], "test at tools/a.test.ts:3:1");
        assert!(lines.iter().any(|l| l.contains("one is not two")));
        assert!(lines.iter().any(|l| l.contains("a.test.ts:3:35")));
        assert!(
            !lines.iter().any(|l| l.contains("node:internal")),
            "{lines:?}"
        );
    }

    #[test]
    fn rustfmt_says_where() {
        let said = format!(
            "Diff in {}/crates/a/src/lib.rs:3:\n-fn a(){{}}\n+fn a() {{}}\n",
            root().display()
        );
        assert_eq!(
            excerpt(&said, Said::Fmt),
            vec!["Diff in crates/a/src/lib.rs:3:"]
        );
    }

    #[test]
    fn short_errors_are_told_from_other_lines() {
        assert!(short_error("crates/a/src/lib.rs:12:9: error: unused"));
        assert!(short_error("C:\\a\\lib.rs:12:9: error[E0425]: no"));
        assert!(!short_error("crates/a/src/lib.rs:12:9: warning: unused"));
        assert!(!short_error("error: could not compile"));
        assert!(!short_error("note: a:b: error"));
    }

    #[test]
    fn times_and_names_read_as_a_person_writes_them() {
        assert_eq!(duration(Duration::from_millis(41_240)), "41.2s");
        assert_eq!(duration(Duration::from_secs(185)), "3m 05s");
        assert_eq!(
            slug("shared crates free of Bevy"),
            "shared-crates-free-of-bevy"
        );
        assert_eq!(
            slug("cargo build -p terra_sandbox"),
            "cargo-build-p-terra-sandbox"
        );
    }
}
