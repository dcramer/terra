//! The shared seed build, and seeding a worktree's build cache from it, so a worktree's first
//! build compiles only Terra's own crates instead of all of Bevy (D183).
//!
//! **The seed** is a build of origin/main kept for this alone, in the repository's git folder
//! (`.git/terra-seed/`): origin/main's files, checked out there through an index of its own
//! rather than as a worktree, so no worktree tool lists or clears it away (D259), and a
//! `target/` built by the cargo commands agents' builds start with (`BUILDS`, and each
//! member's tests alone, `MEMBER_TESTS`). It is current
//! while it was built from the manifests origin/main has (every `Cargo.toml`, `Cargo.lock`,
//! `rust-toolchain.toml` and `.cargo/config.toml`), which decide what the dependencies are
//! built as. `cargo xtask seed` fetches and, when they changed, builds it again, holding
//! `.git/terra-seed/lock`, so sessions starting together wait for one build rather than each
//! building their own. Only the dependencies that changed are rebuilt; the first build takes
//! the longest. `--refresh` runs its builds again whatever the manifests say. Each build ends
//! with a sweep of what the builds no longer use (`sweep`), since Cargo deletes nothing.
//! `--renew`, in a worktree that lives long, seeds it again when the seed moved on.
//!
//! **The main checkout** is kept up to date by the same command: it is fast-forwarded to
//! origin/main when it is on `main` with no changes and no commits origin/main lacks, and
//! otherwise left alone with a line saying what is in the way. Nothing there is ever stashed,
//! reset or rebased.
//!
//! **Seeding a worktree** copies only the dependencies' units: their fingerprints are keyed by
//! version and features, so they stay fresh wherever they are. Our own crates' units are left
//! out, since Cargo judges those by file times and could otherwise take the seed's code for
//! this checkout's. It follows Cargo's own locking: this checkout's locks are held for the
//! whole seed, so a `cargo` command started meanwhile waits for it rather than building from
//! nothing. The compiled libraries, most of the size, are hard links to the seed's rather than
//! copies, so a worktree costs a few hundred megabytes instead of ten gigabytes: rustc and the
//! linker replace an `.rlib`, `.rmeta` or `.so` with a new file when they build it again, so a
//! rebuild on either side leaves the other's alone. Everything else is copied, since Cargo and
//! rustc rewrite fingerprints, dep-info and build-script output in place.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime};

use crate::stage::{Said, Step};
use crate::windows::{WINDOWS_TARGET, on_wsl};
use crate::{Task, output, root, sweep};

/// The task this module runs (D410).
pub(crate) const TASKS: &[Task] = &[Task {
    name: "seed",
    takes: "[--refresh]",
    does: "keep the shared seed build current with origin/main and link its compiled dependencies into this worktree, so its first build is quick; fast-forwards the main checkout when it is clean (D183)",
    group: "This machine",
    run: seed,
}];

/// The locks Cargo 1.97 takes in a profile directory: `.cargo-lock` shared, the other two
/// exclusive, for the length of a build. `cargo run` lets them go before the program starts.
pub(super) const LOCKS: [&str; 3] = [".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock"];

/// What a background seed prints to its parent once it holds this checkout's locks.
const LOCKED: &str = "locked";

/// The seed's folder in the repository's git folder.
const SEED: &str = "terra-seed";

/// What the seed builds: what `cargo xtask check` builds, and the sandbox that shots, films
/// and the workbench run. Cargo builds a dependency again for each set of features the
/// packages chosen ask of it, so each is the command agents run, not a cover-all build.
const BUILDS: &[&[&str]] = &[
    &["clippy", "--workspace", "--all-targets"],
    &["test", "--workspace", "--no-run"],
    &["build", "-p", "terra_build_mesh", "--example", "kits"],
    &[
        "build",
        "--release",
        "-p",
        "terra_build_scenes",
        "--example",
        "artifacts",
    ],
    // The sandbox's dev build, as xtask and the remote build it (`tasks::sandbox::dev_features`).
    &[
        "build",
        "-p",
        "terra_sandbox",
        "--features",
        "track_location",
    ],
    // The sandbox optimised, which every render, shot, film and bench runs: Bevy in release
    // is 4 minutes and 2.7 GB a worktree built alone (D406).
    &["build", "--release", "-p", "terra_sandbox"],
];

/// Each member's tests alone, `cargo test -p <member> --no-run`, after `BUILDS`: what a job runs
/// for the crate it changed and what the combat and controller scorecards run (`cargo test -p
/// terra_game --lib …`). Cargo resolves features for the packages chosen, and dev-dependencies'
/// features only for those, so `-p terra_game` builds Bevy's render stack differently from
/// `--workspace` (terra_world's tests ask wgpu for its `noop` backend): seeded only with the
/// workspace's build, a fresh worktree's first combat run rebuilt 120 dependency units, 8 of
/// its 9 minutes. Stable Cargo has no workspace-wide feature unification yet (`-Z
/// feature-unification`), so the seed holds each member's own.
const MEMBER_TESTS: [&str; 4] = ["test", "-p", "{member}", "--no-run"];

/// Under WSL, the Windows builds `play`, `win` and `bench` make, on the real GPU.
const WINDOWS_BUILDS: &[&[&str]] = &[
    &[
        "build",
        "--target",
        WINDOWS_TARGET,
        "-p",
        "terra_sandbox",
        "--features",
        "track_location",
    ],
    &[
        "build",
        "--target",
        WINDOWS_TARGET,
        "--release",
        "-p",
        "terra_sandbox",
    ],
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// By hand, or from the hook's regular run in the main checkout.
    Plain,
    /// Started by `--background`, which waits only until it holds this checkout's locks.
    Child,
    /// `--refresh`: the seed's builds run again whatever the manifests say.
    Refresh,
    /// `--renew`: a worktree's build seeded from an older seed, or not from one, is deleted
    /// and seeded again, so a worktree that lives long (the nightly's, continuous's) links the
    /// seed's dependencies rather than building each new one itself beside the old.
    Renew,
}

/// The file in a worktree's `target/` naming the seed build it was seeded from.
const SEEDED: &str = "seeded";

pub fn seed(args: &[String]) -> bool {
    match args.first().map(String::as_str) {
        Some("--background") => background(),
        Some("--child") => run(Mode::Child),
        Some("--refresh") => run(Mode::Refresh),
        Some("--renew") => run(Mode::Renew),
        None => run(Mode::Plain),
        Some(_) => {
            eprintln!("usage: cargo xtask seed [--refresh | --renew]");
            false
        }
    }
}

/// Starts the seed in its own process and returns once it holds this checkout's locks, so
/// the hook that called it can end and nothing can build in between.
fn background() -> bool {
    let target = root().join("target");
    let log = std::fs::create_dir_all(&target).and_then(|()| File::create(target.join("seed.log")));
    let Ok(log) = log else {
        eprintln!("seed: cannot write target/seed.log");
        return true;
    };
    let Ok(exe) = std::env::current_exe() else {
        return true;
    };
    let child = {
        let mut command = Command::new(exe);
        command
            .args(["seed", "--child"])
            .current_dir(root())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(log);
        // Its own process group, so it outlives the hook that started it.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        command.spawn()
    };
    let Ok(mut child) = child else {
        eprintln!("seed: could not start");
        return true;
    };
    let mut line = String::new();
    if let Some(out) = child.stdout.take() {
        let _ = BufReader::new(out).read_line(&mut line);
    }
    if line.trim() == LOCKED {
        println!(
            "This worktree's target/ is being seeded in the background from the shared seed \
             build of origin/main (cargo xtask seed, log in target/seed.log). A cargo command \
             started meanwhile waits for it (\"Blocking waiting for file lock\"), then builds \
             only Terra's own crates. That is seconds, unless main's dependencies changed and \
             the seed is built again first, which can take several minutes."
        );
    }
    true
}

fn run(mode: Mode) -> bool {
    run_in(&root(), mode)
}

/// Seeds another worktree, as the prune does after deleting the build of one a session still
/// has open (D259).
pub(super) fn reseed(worktree: &Path) -> bool {
    run_in(worktree, Mode::Plain)
}

fn run_in(here: &Path, mode: Mode) -> bool {
    // A child's stdout is its parent's pipe, read once; everything else goes to the log.
    let child = mode == Mode::Child;
    let say = |text: String| {
        if child {
            eprintln!("{text}");
        } else {
            println!("{text}");
        }
    };
    let here = here.to_path_buf();
    let Some(main) = main_checkout() else {
        say("seed: git cannot find the main checkout".into());
        return false;
    };
    let worktree = main != here;
    if worktree && mode == Mode::Renew && !renew(&here.join("target"), &say) {
        return false;
    }

    // This checkout's locks first, so nothing builds here while the seed is made and copied.
    let mut held = Vec::new();
    let mut ours = Vec::new();
    let mut profiles = Vec::new();
    if worktree {
        ours = match workspace_packages(&here) {
            Ok(ours) => ours,
            Err(error) => {
                say(format!(
                    "seed: cannot list the workspace's packages: {error}"
                ));
                return false;
            }
        };
        let to_target = here.join("target");
        profiles = seed_profiles()
            .into_iter()
            .filter(|profile| !built(&to_target.join(profile), &ours))
            .collect();
        for profile in &profiles {
            let dir = to_target.join(profile);
            if let Err(error) = std::fs::create_dir_all(&dir) {
                say(format!("seed: cannot make {}: {error}", dir.display()));
                return false;
            }
            for name in LOCKS {
                match lock(&dir.join(name)) {
                    Ok(file) => held.push(file),
                    Err(error) => {
                        say(format!("seed: cannot lock {}: {error}", dir.display()));
                        return false;
                    }
                }
            }
        }
    }
    if child {
        println!("{LOCKED}");
        let _ = std::io::stdout().flush();
    }

    if output("git", &["fetch", "-q", "origin", "main"]).is_none() {
        say("seed: could not fetch origin; going by the origin/main this repository has".into());
    }
    say(update_main(&main));
    let (from_target, seed_lock) = match ensure(mode == Mode::Refresh, child, &say) {
        Ok(found) => found,
        Err(error) => {
            say(format!("seed: {error}"));
            return false;
        }
    };
    if !worktree {
        say("seed: this is the main checkout; the shared seed is ready for worktrees".into());
        return true;
    }
    if profiles.is_empty() {
        say("seed: this worktree has built already; nothing to link".into());
        return true;
    }

    let to_target = here.join("target");
    let started = Instant::now();
    let mut ok = true;
    for profile in &profiles {
        let (from, to) = (from_target.join(profile), to_target.join(profile));
        if !from.join(".fingerprint").is_dir() {
            continue;
        }
        let (files, size) = copy_profile(&from, &to, &ours);
        match files {
            Ok(count) => say(format!(
                "seed: {}: {count} files, {:.1} GB linked, {:.2} GB copied",
                profile.display(),
                size.linked as f64 / 1e9,
                size.copied as f64 / 1e9
            )),
            Err(error) => {
                say(format!("seed: {}: {error}", profile.display()));
                ok = false;
            }
        }
    }
    // Only a worktree that has built nothing, whose path dependencies' units all came from the
    // seed just now: one built here may hold a unit of a file since put back as the seed's.
    if ok
        && profiles.len() == seed_profiles().len()
        && let Some(seed) = from_target.parent()
    {
        let dirs = paths::foreign(&here, &ours);
        let (dated, differ) = paths::backdate(&seed.join("src"), &here, &dirs);
        say(format!(
            "seed: path dependencies: {dated} files dated as the seed's, {differ} that differ \
             left to rebuild"
        ));
    }
    if ok && let Some(stamp) = from_target.parent().and_then(Stamp::read) {
        let _ = std::fs::write(to_target.join(SEEDED), &stamp.commit);
    }
    say(format!(
        "seed: done in {:.0} s",
        started.elapsed().as_secs_f64()
    ));
    drop(seed_lock);
    drop(held);
    ok
}

/// Deletes a worktree's build when the seed it was seeded from is not the seed there is now;
/// false if a build under way keeps it.
fn renew(target: &Path, say: &dyn Fn(String)) -> bool {
    let seeded = std::fs::read_to_string(target.join(SEEDED)).ok();
    let current = seed_dir()
        .and_then(|dir| Stamp::read(&dir))
        .map(|s| s.commit);
    if !target.is_dir() || (seeded.is_some() && seeded == current) {
        return true;
    }
    if !sweep::free_build(target) {
        say("seed: --renew: a build is under way here, so its build is kept".into());
        return false;
    }
    say("seed: --renew: this build was seeded from an older seed, so it is seeded again".into());
    true
}

/// The profile folders the seed builds, relative to a target folder.
fn seed_profiles() -> Vec<PathBuf> {
    let mut profiles = vec![PathBuf::from("debug"), PathBuf::from("release")];
    if windows() {
        profiles.extend(["debug", "release"].map(|p| Path::new(WINDOWS_TARGET).join(p)));
    }
    profiles
}

/// Whether the seed builds for Windows too: under WSL, with the target installed.
fn windows() -> bool {
    on_wsl()
        && output("rustup", &["target", "list", "--installed"])
            .is_some_and(|targets| targets.contains(WINDOWS_TARGET))
}

/// The shared seed's folder, in the repository's git folder.
fn seed_dir() -> Option<PathBuf> {
    output(
        "git",
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .map(|common| PathBuf::from(common).join(SEED))
}

/// What the seed was last built from: whether every build passed, the commit, and the
/// manifests.
struct Stamp {
    ok: bool,
    commit: String,
    manifests: String,
}

impl Stamp {
    fn read(dir: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(dir.join("stamp")).ok()?;
        let mut lines = text.splitn(3, '\n');
        let ok = lines.next()? == "ok";
        let commit = lines.next()?.to_string();
        let manifests = lines.next()?.to_string();
        Some(Self {
            ok,
            commit,
            manifests,
        })
    }

    fn write(&self, dir: &Path) -> std::io::Result<()> {
        let state = if self.ok { "ok" } else { "failed" };
        std::fs::write(
            dir.join("stamp"),
            format!("{state}\n{}\n{}", self.commit, self.manifests),
        )
    }
}

/// Makes the seed current with origin/main, building it when its manifests changed, and
/// gives its target folder with the seed's lock held, so it does not change while it is
/// linked from.
fn ensure(refresh: bool, child: bool, say: &dyn Fn(String)) -> Result<(PathBuf, File), String> {
    let dir = seed_dir().ok_or("git cannot find the repository")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let held = File::options()
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("lock"))
        .map_err(|e| format!("cannot open the seed's lock: {e}"))?;
    if held.try_lock().is_err() {
        say("seed: waiting while another session builds the shared seed".into());
        held.lock()
            .map_err(|e| format!("cannot lock the seed: {e}"))?;
    }
    let commit = output("git", &["rev-parse", "origin/main"]).ok_or("there is no origin/main")?;
    let wanted = manifests(&commit).ok_or("git cannot list origin/main's files")?;
    let target = dir.join("target");
    let stamp = Stamp::read(&dir);
    if !refresh
        && target.is_dir()
        && stamp
            .as_ref()
            .is_some_and(|stamp| stamp.manifests == wanted)
    {
        return Ok((target, held));
    }

    say(format!(
        "seed: building the shared seed at origin/main ({}), since {}. Only what changed is \
         built; from nothing it takes 15 to 25 minutes, and cargo commands in a worktree being \
         seeded wait for it.",
        &commit[..commit.len().min(10)],
        match &stamp {
            _ if refresh => "--refresh asked for it",
            None => "there is none yet",
            Some(_) => "main's manifests changed",
        }
    ));
    let _ = std::fs::remove_file(dir.join("stamp"));
    checkout(&dir, &commit)?;
    let started = Instant::now();
    let mut failed = Vec::new();
    let builds = seed_builds(&workspace_packages(&dir.join("src"))?);
    for args in &builds {
        let shown = format!("cargo {}", args.join(" "));
        let mut cargo = Command::new("cargo");
        cargo
            .args(args)
            .current_dir(dir.join("src"))
            .env("CARGO_TARGET_DIR", &target)
            // The worktree's compiler, from the `cargo run` this runs under, would override the
            // seed's own rust-toolchain.toml.
            .env_remove("RUSTUP_TOOLCHAIN");
        // Each build says only how it went; what cargo said is in the seed's `logs/` (D388).
        let mut step = Step::new(&format!("seed: {shown}"), Said::Cargo, dir.join("logs"))
            .rerun(format!("cd {} && {shown}", dir.join("src").display()));
        if child {
            // A child's stdout is its parent's pipe, read once.
            step = step.on_stderr();
        }
        let passed = step.run(cargo);
        if !passed {
            failed.push(args.join(" "));
        }
    }
    say(sweep(&dir, &builds, failed.is_empty()));
    Stamp {
        ok: failed.is_empty(),
        commit: commit.clone(),
        manifests: wanted,
    }
    .write(&dir)
    .map_err(|e| format!("cannot write the seed's stamp: {e}"))?;
    say(format!(
        "seed: the shared seed built in {:.0} s{}",
        started.elapsed().as_secs_f64(),
        if failed.is_empty() {
            String::new()
        } else {
            format!(
                "; these failed at origin/main, so what they built is used as far as it got: {}",
                failed.join("; ")
            )
        }
    ));
    Ok((target, held))
}

/// Sweeps the seed's build of what its builds no longer use (D259), so it holds one build of
/// origin/main rather than every build since it was made: every incremental cache, since
/// worktrees are never seeded with one, and, when every build passed, each dependency's unit
/// none of them names. Asked again with Cargo's JSON messages, the builds are all fresh and take
/// seconds. Our own crates' units are left, since they are never seeded and Cargo names a
/// binary by its copy without the hash. A worktree linked to a swept unit keeps its own link.
/// Says what it freed.
fn sweep(dir: &Path, builds: &[Vec<String>], passed: bool) -> String {
    let target = dir.join("target");
    let mut live = HashSet::new();
    let ours = workspace_packages(&dir.join("src")).unwrap_or_default();
    let keep = |name: &str| is_ours(name, &ours);
    let mut listed = passed && !ours.is_empty();
    for args in builds {
        if !listed {
            break;
        }
        let messages = Command::new("cargo")
            .args(args)
            .arg("--message-format=json")
            .current_dir(dir.join("src"))
            .env("CARGO_TARGET_DIR", &target)
            .env_remove("RUSTUP_TOOLCHAIN")
            .stderr(Stdio::null())
            .output();
        match messages {
            Ok(out) if out.status.success() => {
                live.extend(sweep::live_hashes(&String::from_utf8_lossy(&out.stdout)));
            }
            _ => listed = false,
        }
    }
    let (mut incremental, mut units) = (0, 0);
    for profile in profile_dirs(&target) {
        let profile = target.join(profile);
        incremental += sweep::incremental(&profile, Duration::ZERO, SystemTime::now());
        if listed {
            units += sweep::stale_units(&profile, &live, &keep);
        }
    }
    format!(
        "seed: swept {:.1} GB of incremental caches and {}",
        incremental as f64 / 1e9,
        if listed {
            format!("{:.1} GB of units no build uses", units as f64 / 1e9)
        } else {
            "no units, since a build failed".into()
        }
    )
}

/// Checks a commit's files out into the seed's `src/`, through an index of its own, so files
/// that did not change keep their times and Cargo does not rebuild what they make.
fn checkout(dir: &Path, commit: &str) -> Result<(), String> {
    let common = dir.parent().ok_or("the seed has no parent folder")?;
    let src = dir.join("src");
    std::fs::create_dir_all(&src).map_err(|e| format!("cannot make {}: {e}", src.display()))?;
    let done = Command::new("git")
        .arg("--git-dir")
        .arg(common)
        .arg("--work-tree")
        .arg(&src)
        .args(["read-tree", "--reset", "-u", commit])
        .env("GIT_INDEX_FILE", dir.join("index"))
        .current_dir(&src)
        .status()
        .is_ok_and(|status| status.success());
    if done {
        Ok(())
    } else {
        Err(format!(
            "git could not check {commit} out into {}",
            src.display()
        ))
    }
}

/// The manifests a commit's dependencies are built from, a line each, as git names their
/// contents: the workspace's `Cargo.toml`s, `Cargo.lock`, `rust-toolchain.toml` and
/// `.cargo/config.toml`; and the builds the seed is made of, so a build added to `BUILDS` is
/// built into the seed by the next session rather than by each worktree (D406).
fn manifests(commit: &str) -> Option<String> {
    // The members' builds are named by the pattern: the members are in the manifest lines.
    let builds: Vec<String> = seed_builds(&["{member}".to_string()])
        .iter()
        .map(|args| args.join(" "))
        .collect();
    output("git", &["ls-tree", "-r", commit])
        .map(|listing| format!("{}\nbuilds {}", manifest_lines(&listing), builds.join("; ")))
}

/// The seed's builds, in order: `BUILDS`, under WSL `WINDOWS_BUILDS`, then `MEMBER_TESTS` for
/// each member, so each reuses what the workspace's build made of the dependencies it shares.
fn seed_builds(members: &[String]) -> Vec<Vec<String>> {
    let windows_builds: &[&[&str]] = if windows() { WINDOWS_BUILDS } else { &[] };
    let fixed = BUILDS
        .iter()
        .chain(windows_builds)
        .map(|args| args.iter().map(|arg| arg.to_string()).collect());
    let mut sorted = members.to_vec();
    sorted.sort();
    let tests = sorted.into_iter().map(|member| {
        MEMBER_TESTS
            .iter()
            .map(|arg| arg.replace("{member}", &member))
            .collect()
    });
    fixed.chain(tests).collect()
}

/// The manifest lines of `git ls-tree -r`'s listing, as `<object> <path>`.
fn manifest_lines(listing: &str) -> String {
    listing
        .lines()
        .filter_map(|line| {
            let (meta, path) = line.split_once('\t')?;
            let object = meta.split_whitespace().nth(2)?;
            let top = matches!(
                path,
                "Cargo.toml" | "Cargo.lock" | "rust-toolchain.toml" | ".cargo/config.toml"
            );
            let member = (path.starts_with("crates/") || path.starts_with("xtask/"))
                && path.ends_with("/Cargo.toml");
            (top || member).then(|| format!("{object} {path}"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The seed's state, as `cargo xtask doctor` reports it, without fetching: whether it is
/// current with origin/main as this repository last fetched it, and what it was built from.
pub(super) fn status() -> (bool, String) {
    let Some(dir) = seed_dir() else {
        return (false, "git cannot find the repository".into());
    };
    let Some(stamp) = Stamp::read(&dir).filter(|_| dir.join("target").is_dir()) else {
        return (false, format!("none yet, in {}", dir.display()));
    };
    let wanted = output("git", &["rev-parse", "origin/main"]).and_then(|c| manifests(&c));
    let current = wanted.is_some_and(|wanted| wanted == stamp.manifests);
    let short = &stamp.commit[..stamp.commit.len().min(10)];
    let state = match (current, stamp.ok) {
        (true, true) => "current with origin/main",
        (true, false) => "current with origin/main, though some of its builds failed there",
        (false, _) => "behind origin/main's manifests",
    };
    (
        current,
        format!("{state}, built at {short}, in {}", dir.display()),
    )
}

/// Fast-forwards the main checkout to origin/main when nobody's work is in the way, and says
/// what happened.
fn update_main(main: &Path) -> String {
    if let Some(why) = main_blocker(main) {
        return format!("seed: the main checkout is left as it is: {why}");
    }
    let behind = git_in(main, &["rev-list", "--count", "HEAD..origin/main"]).unwrap_or_default();
    if behind == "0" {
        return "seed: the main checkout is up to date with origin/main".into();
    }
    match git_in(main, &["merge", "--ff-only", "-q", "origin/main"]) {
        Some(_) => {
            format!("seed: the main checkout fast-forwarded {behind} commits to origin/main")
        }
        None => "seed: the main checkout could not be fast-forwarded (git refused, perhaps an \
                 untracked file in the way or another git command under way there)"
            .into(),
    }
}

/// What stops the main checkout being fast-forwarded: on another branch, changed files, or
/// commits origin/main does not have.
pub(super) fn main_blocker(main: &Path) -> Option<String> {
    let branch = git_in(main, &["symbolic-ref", "--short", "-q", "HEAD"]);
    if branch.as_deref() != Some("main") {
        return Some(format!(
            "it is on {}, not main",
            branch.unwrap_or_else(|| "a detached HEAD".into())
        ));
    }
    let changed = git_in(main, &["status", "--porcelain", "--untracked-files=no"])?
        .lines()
        .count();
    if changed > 0 {
        return Some(format!("it has {changed} changed files"));
    }
    let ahead = git_in(main, &["rev-list", "--count", "origin/main..HEAD"])?;
    if ahead != "0" {
        return Some(format!("it has {ahead} commits origin/main does not"));
    }
    None
}

/// The main checkout's state for `cargo xtask doctor`: whether it is up to date with
/// origin/main as last fetched, and if not, why.
pub(super) fn main_status() -> (bool, String) {
    let Some(main) = main_checkout() else {
        return (false, "git cannot find the main checkout".into());
    };
    if let Some(why) = main_blocker(&main) {
        return (false, format!("not kept up to date: {why}"));
    }
    match git_in(&main, &["rev-list", "--count", "HEAD..origin/main"]).as_deref() {
        Some("0") => (true, "up to date with origin/main".into()),
        Some(n) => (false, format!("{n} commits behind origin/main")),
        None => (false, "git cannot compare it with origin/main".into()),
    }
}

/// A git command's output, run in a given folder.
fn git_in(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The main checkout: the folder that holds the repository every worktree shares.
fn main_checkout() -> Option<PathBuf> {
    let common = output(
        "git",
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    Path::new(&common).parent().map(Path::to_path_buf)
}

/// The names of the workspace's own packages, from the root manifest's `members`, as written
/// there: plain paths and `folder/*`. Anything else is refused, since a package missed here
/// would have its units copied.
fn workspace_packages(root: &Path) -> Result<Vec<String>, String> {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).map_err(|e| e.to_string())?;
    let members = manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("members"))
        .and_then(|rest| {
            rest.split_once('[')?
                .1
                .split_once(']')
                .map(|(list, _)| list)
        })
        .ok_or("no one-line `members` list in Cargo.toml")?;
    let mut dirs = Vec::new();
    for member in members.split(',').map(|m| m.trim().trim_matches('"')) {
        if member.is_empty() {
            continue;
        }
        if let Some(folder) = member.strip_suffix("/*") {
            let entries = std::fs::read_dir(root.join(folder)).map_err(|e| e.to_string())?;
            dirs.extend(entries.flatten().map(|entry| entry.path()));
        } else if member.contains(['*', '?', '[']) {
            return Err(format!("cannot expand the member pattern {member}"));
        } else {
            dirs.push(root.join(member));
        }
    }
    let mut names = Vec::new();
    for dir in dirs {
        let Ok(text) = std::fs::read_to_string(dir.join("Cargo.toml")) else {
            continue;
        };
        let name = text
            .lines()
            .find_map(|line| line.trim().strip_prefix("name"))
            .and_then(|rest| rest.split('"').nth(1))
            .ok_or_else(|| format!("no package name in {}", dir.display()))?;
        names.push(name.to_string());
    }
    if names.is_empty() {
        return Err("no members found".into());
    }
    Ok(names)
}

/// Profile folders in a target folder, such as `debug` or `x86_64-pc-windows-gnu/debug`:
/// those with fingerprints, relative to it.
pub(super) fn profile_dirs(target: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let children = |dir: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .map(|e| e.path())
                    .collect()
            })
            .unwrap_or_default()
    };
    for dir in children(target) {
        let inner = if dir.join(".fingerprint").is_dir() {
            vec![dir]
        } else {
            children(&dir)
                .into_iter()
                .filter(|d| d.join(".fingerprint").is_dir())
                .collect()
        };
        found.extend(
            inner
                .into_iter()
                .filter_map(|d| d.strip_prefix(target).ok().map(Path::to_path_buf)),
        );
    }
    found
}

/// Whether a profile folder already holds a build of anything beyond our own packages, such
/// as a dependency, in which case it is left alone.
fn built(profile: &Path, ours: &[String]) -> bool {
    std::fs::read_dir(profile.join(".fingerprint")).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|e| !is_ours(&e.file_name().to_string_lossy(), ours))
    })
}

/// Whether a unit's file or folder name, `<name>-<hash>` with an optional `lib` in front and
/// extension after, belongs to one of our packages.
fn is_ours(file_name: &str, ours: &[String]) -> bool {
    let stem = file_name.split('.').next().unwrap_or(file_name);
    let Some((name, _hash)) = stem.rsplit_once('-') else {
        return false;
    };
    let bare = name.strip_prefix("lib").unwrap_or(name);
    ours.iter().any(|package| {
        let crate_name = package.replace('-', "_");
        [package.as_str(), crate_name.as_str()]
            .iter()
            .any(|n| *n == name || *n == bare)
    })
}

/// Copies the dependencies' units of one profile: outputs and build-script results first, and
/// only once they are all in place, the fingerprints that tell Cargo they are fresh. A copy cut
/// short leaves units with no fingerprint, which Cargo builds again. Returns the files copied
/// and their size.
fn copy_profile(from: &Path, to: &Path, ours: &[String]) -> (Result<usize, String>, Size) {
    let mut total = Size::default();
    let mut count = 0;
    for part in [&["deps", "build"][..], &[".fingerprint"][..]] {
        let mut pairs = Vec::new();
        for folder in part {
            let Ok(entries) = std::fs::read_dir(from.join(folder)) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name();
                if is_ours(&name.to_string_lossy(), ours) {
                    continue;
                }
                list(&entry.path(), &to.join(folder).join(&name), &mut pairs);
            }
        }
        match copy_all(&pairs) {
            Ok(size) => {
                total.linked += size.linked;
                total.copied += size.copied;
                count += pairs.len();
            }
            Err(error) => return (Err(error), total),
        }
    }
    (Ok(count), total)
}

/// Bytes seeded, as hard links to the seed's files and as copies.
#[derive(Default)]
struct Size {
    linked: u64,
    copied: u64,
}

/// Every file under a path, with where it goes, skipping any already there.
fn list(from: &Path, to: &Path, pairs: &mut Vec<(PathBuf, PathBuf)>) {
    if from.is_dir() {
        let Ok(entries) = std::fs::read_dir(from) else {
            return;
        };
        for entry in entries.flatten() {
            list(&entry.path(), &to.join(entry.file_name()), pairs);
        }
    } else if !to.exists() {
        pairs.push((from.to_path_buf(), to.to_path_buf()));
    }
}

/// Links or copies files on every core, keeping their modification times, which Cargo
/// compares. `std::fs::copy` clones the file where the file system can (APFS, Btrfs, XFS).
fn copy_all(pairs: &[(PathBuf, PathBuf)]) -> Result<Size, String> {
    let next = AtomicUsize::new(0);
    let linked = AtomicUsize::new(0);
    let copied = AtomicUsize::new(0);
    let failed = std::sync::Mutex::new(None);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(16));
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some((from, to)) = pairs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    match copy_one(from, to) {
                        Ok((size, true)) => {
                            linked.fetch_add(size as usize, Ordering::Relaxed);
                        }
                        Ok((size, false)) => {
                            copied.fetch_add(size as usize, Ordering::Relaxed);
                        }
                        Err(error) => {
                            *failed.lock().unwrap() = Some(format!("{}: {error}", from.display()));
                            next.store(pairs.len(), Ordering::Relaxed);
                        }
                    }
                }
            });
        }
    });
    match failed.into_inner().unwrap() {
        Some(error) => Err(error),
        None => Ok(Size {
            linked: linked.into_inner() as u64,
            copied: copied.into_inner() as u64,
        }),
    }
}

/// Whether a file is one rustc or the linker only ever replaces, never rewrites in place, so a
/// hard link to it is safe. Checked on Linux with Rust 1.97 and LLD: an `.rlib`, `.rmeta` and
/// proc-macro `.so` rebuilt under the same name came out as new files, while the `.d` beside
/// them was rewritten in place.
fn linkable(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e == "rlib" || e == "rmeta" || e == "so")
}

/// Links or copies one file; says whether it was linked. A link that fails, as it does across
/// file systems, falls back to a copy.
fn copy_one(from: &Path, to: &Path) -> std::io::Result<(u64, bool)> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if linkable(from) && std::fs::hard_link(from, to).is_ok() {
        return Ok((std::fs::metadata(from)?.len(), true));
    }
    let size = std::fs::copy(from, to)?;
    let modified = std::fs::metadata(from)?.modified()?;
    File::options()
        .write(true)
        .open(to)?
        .set_modified(modified)?;
    Ok((size, false))
}

fn lock(path: &Path) -> std::io::Result<File> {
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.lock()?;
    Ok(file)
}

mod paths;

#[cfg(test)]
mod tests;
