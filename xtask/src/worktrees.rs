//! Lists this repository's worktrees and clears away the ones agents are done with (D259).
//! Claude Code removes a subagent's worktree when the agent made no changes, and otherwise only
//! after `cleanupPeriodDays`, a month by default, while each built worktree holds gigabytes.
//! `cargo xtask worktrees` shows what each holds and what `--prune` would do; a Claude Code hook
//! runs `--prune` in the background at most once every 15 minutes. An agent whose work is pushed runs
//! `--done` to delete its own worktree's build at once, rather than leave it for the prune.
//!
//! Only worktrees under `.claude/worktrees/` are touched, never the one this runs in, never one
//! locked, used by a running process, or used recently:
//! - a subagent's (`agent-…`) whose commits are all on `main` and that has no changes, untracked
//!   files included, is removed with its branch once it has been idle two days;
//! - any worktree's build folder is deleted after one idle hour, since it can be built again
//!   (`cargo xtask seed` makes that quick). Its source files are kept.
//!
//! Idle is measured from the last thing that touched it: an edit to its files, git (a commit or
//! a checkout, not the index, which `git status` rewrites) or a build. A subagent sent another
//! message edits long before it commits, so its edits count. Every rule fails toward keeping: a
//! worktree it cannot read, or whose name or build locks it does not recognise, is left alone.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use super::{output, root, seed};

const HOUR: Duration = Duration::from_secs(60 * 60);
/// How long an idle subagent's worktree is kept, in case its agent is sent another message: a
/// lead session may resume its agents many hours later.
const AGENT_IDLE: Duration = Duration::from_secs(2 * 24 * 60 * 60);
/// Build files can be made again; their retention is separate from source retention.
const BUILD_IDLE: Duration = HOUR;

pub fn worktrees(args: &[String]) -> bool {
    let prune = match args {
        [] => false,
        [flag] if flag == "--prune" => true,
        [flag] if flag == "--done" => return done(),
        _ => {
            eprintln!("usage: cargo xtask worktrees [--prune | --done]");
            return false;
        }
    };
    let Some(common) = output(
        "git",
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    ) else {
        eprintln!("worktrees: git cannot find the repository");
        return false;
    };
    let Some(main_checkout) = Path::new(&common).parent().map(Path::to_path_buf) else {
        return false;
    };
    // Two prunes at once, from two sessions starting together, would race; the second leaves.
    let _held = if prune {
        let target = main_checkout.join("target");
        let lock = std::fs::create_dir_all(&target).and_then(|()| {
            File::options()
                .write(true)
                .create(true)
                .truncate(false)
                .open(target.join("worktrees.lock"))
        });
        match lock.map(|file| file.try_lock().map(|()| file)) {
            Ok(Ok(file)) => Some(file),
            _ => {
                println!("worktrees: another prune is under way");
                return true;
            }
        }
    } else {
        None
    };
    if prune {
        git(&main_checkout, &["worktree", "prune"]);
    }

    let managed = main_checkout.join(".claude").join("worktrees");
    let active_paths = process_paths();
    let now = SystemTime::now();
    let (mut freed, mut ok) = (0u64, true);
    let entries = list(&main_checkout);
    if entries.is_empty() {
        println!("No worktrees besides the main checkout.");
    }
    for tree in entries {
        let name = tree
            .path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into());
        let target = tree.path.join("target");
        let merged = tree
            .head
            .as_deref()
            .is_some_and(|head| on_main(&main_checkout, head));
        let changes = changes(&tree.path);
        let checkout = children(&tree.path)
            .iter()
            .filter(|p| **p != target && !p.ends_with(".git"))
            .map(|p| usage(p))
            .fold(Usage::default(), Usage::add);
        let build = usage(&target);
        let idle = now
            .duration_since(last_used(&tree.path).max(checkout.newest).max(build.newest))
            .unwrap_or_default();
        let agent = name.starts_with("agent-");
        let ours = tree.path.starts_with(&managed) && tree.path != root() && !tree.locked;
        let in_use = tree.path.canonicalize().ok().and_then(|tree_path| {
            active_paths
                .as_ref()
                .map(|paths| paths.iter().any(|path| path.starts_with(&tree_path)))
        });
        let action = choose_action(ours, agent, merged, changes, idle, target.is_dir(), in_use);

        let state = [
            if merged { "on main" } else { "not on main" }.to_string(),
            match changes {
                Some(0) => "clean".into(),
                Some(n) => format!("{n} changed"),
                None => "unreadable".into(),
            },
        ]
        .into_iter()
        .chain(tree.locked.then(|| "locked".into()))
        .chain(match in_use {
            Some(true) => Some("active process".into()),
            None => Some("activity unknown".into()),
            Some(false) => None,
        })
        .collect::<Vec<_>>()
        .join(", ");
        let build = build.bytes;
        let checkout = checkout.bytes;
        let verb = match (action, prune) {
            (Action::Keep, _) => "",
            (Action::Remove, false) => "would remove",
            (Action::FreeBuild, false) => "would delete its build",
            (Action::Remove, true) => "removed",
            (Action::FreeBuild, true) => "build deleted",
        };
        let done = match (action, prune) {
            (_, false) | (Action::Keep, _) => true,
            (Action::Remove, true) => remove(&main_checkout, &tree),
            (Action::FreeBuild, true) => free_build(&target),
        };
        if prune && done {
            freed += match action {
                Action::Remove => build + checkout,
                Action::FreeBuild => build,
                Action::Keep => 0,
            };
        }
        ok &= done;
        println!(
            "{name:<40} {state:<28} idle {:>5}  checkout {:>6}  build {:>6}  {}",
            age(idle),
            gb(checkout),
            gb(build),
            if done { verb } else { "FAILED" },
        );
    }
    if prune {
        println!("worktrees: freed {}", gb(freed));
    } else {
        println!(
            "\n`cargo xtask worktrees --prune` removes a subagent's worktree once it is on main, \
             clean and idle two days. Build folders are deleted after one idle hour. \
             Active worktrees and unreadable states are kept. `--done` deletes this worktree's build now."
        );
    }
    ok
}

/// Process working directories and executable files protect sessions between builds too.
/// If process inspection is unavailable, automatic cleanup keeps the worktrees.
fn process_paths() -> Option<Vec<PathBuf>> {
    let output = Command::new("lsof")
        .args(["-nP", "-F", "n", "-d", "cwd,txt"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let paths: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        // lsof reports protected system processes as /proc/... (readlink: ...).
        // These are not file paths and cannot identify a managed worktree.
        .filter(|path| !path.contains(" (readlink:"))
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|path| path.canonicalize().unwrap_or(path))
        .collect();
    (!paths.is_empty()).then_some(paths)
}

fn choose_action(
    ours: bool,
    agent: bool,
    merged: bool,
    changes: Option<usize>,
    idle: Duration,
    has_build: bool,
    in_use: Option<bool>,
) -> Action {
    if !ours || in_use != Some(false) || changes.is_none() {
        Action::Keep
    } else if agent && merged && changes == Some(0) && idle >= AGENT_IDLE {
        Action::Remove
    } else if has_build && idle >= BUILD_IDLE {
        Action::FreeBuild
    } else {
        Action::Keep
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Keep,
    Remove,
    FreeBuild,
}

/// A worktree as `git worktree list` gives it.
struct Tree {
    path: PathBuf,
    head: Option<String>,
    branch: Option<String>,
    locked: bool,
}

/// Every worktree but the main checkout.
fn list(main: &Path) -> Vec<Tree> {
    parse(&git_output(main, &["worktree", "list", "--porcelain"]).unwrap_or_default())
}

/// Worktrees from `git worktree list --porcelain`, leaving out the first, the main checkout,
/// and any whose folder is gone.
fn parse(text: &str) -> Vec<Tree> {
    let mut trees: Vec<Tree> = text
        .split("\n\n")
        .filter_map(|block| {
            let mut tree = Tree {
                path: PathBuf::new(),
                head: None,
                branch: None,
                locked: false,
            };
            for line in block.lines() {
                let (key, value) = line.split_once(' ').unwrap_or((line, ""));
                match key {
                    "worktree" => tree.path = PathBuf::from(value),
                    "HEAD" => tree.head = Some(value.to_string()),
                    "branch" => tree.branch = Some(value.trim_start_matches("refs/heads/").into()),
                    "locked" => tree.locked = true,
                    "prunable" => return None,
                    _ => {}
                }
            }
            (!tree.path.as_os_str().is_empty()).then_some(tree)
        })
        .collect();
    if !trees.is_empty() {
        trees.remove(0);
    }
    trees
}

/// Whether a commit is on `main`, here or on the remote.
fn on_main(main: &Path, head: &str) -> bool {
    ["origin/main", "main"]
        .iter()
        .any(|base| git(main, &["merge-base", "--is-ancestor", head, base]))
}

/// How many files a worktree has changed or added, untracked ones included.
fn changes(path: &Path) -> Option<usize> {
    git_output(path, &["status", "--porcelain"]).map(|text| text.lines().count())
}

/// When anything last touched a worktree: git's record of its commits and checkouts, or a build.
fn last_used(path: &Path) -> SystemTime {
    let mut paths = vec![path.to_path_buf()];
    if let Some(git_dir) = std::fs::read_to_string(path.join(".git"))
        .ok()
        .and_then(|text| {
            text.trim()
                .strip_prefix("gitdir:")
                .map(|d| PathBuf::from(d.trim()))
        })
    {
        paths.extend(["HEAD", "logs/HEAD"].map(|f| git_dir.join(f)));
    }
    let target = path.join("target");
    for profile in seed::profile_dirs(&target) {
        paths
            .extend(["deps", "incremental", ".fingerprint"].map(|d| target.join(&profile).join(d)));
    }
    paths
        .iter()
        .filter_map(|p| p.metadata().and_then(|m| m.modified()).ok())
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn children(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

/// Removes a worktree and its branch. Git itself refuses one with changes.
fn remove(main: &Path, tree: &Tree) -> bool {
    let path = tree.path.to_string_lossy();
    if !git(main, &["worktree", "remove", &path]) {
        return false;
    }
    match &tree.branch {
        Some(branch) => git(main, &["branch", "-D", branch]),
        None => true,
    }
}

/// Deletes a build folder unless a build is under way in it: every profile's Cargo locks are
/// taken first, and a profile with none of the locks Cargo is known to take is left alone.
fn free_build(target: &Path) -> bool {
    let profiles = seed::profile_dirs(target);
    if profiles.is_empty() {
        return false;
    }
    let mut held = Vec::new();
    for profile in profiles {
        let locks: Vec<File> = seed::LOCKS
            .iter()
            .filter_map(|name| File::open(target.join(&profile).join(name)).ok())
            .collect();
        if locks.is_empty() || locks.iter().any(|file| file.try_lock().is_err()) {
            return false;
        }
        held.extend(locks);
    }
    let removed = std::fs::remove_dir_all(target).is_ok();
    drop(held);
    removed
}

/// Deletes this worktree's build, for an agent whose work is pushed. If it builds here again,
/// `cargo xtask seed` first makes that quick.
fn done() -> bool {
    let here = root();
    let common = output(
        "git",
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    if common.is_none_or(|c| Path::new(&c).parent() == Some(here.as_path())) {
        eprintln!("worktrees: --done is for a worktree, and this is the main checkout");
        return false;
    }
    let target = here.join("target");
    if !target.is_dir() {
        println!("worktrees: this worktree has no build to delete");
        return true;
    }
    let bytes = usage(&target).bytes;
    if !free_build(&target) {
        eprintln!("worktrees: a build is under way here, or its folder could not be deleted");
        return false;
    }
    println!(
        "worktrees: deleted this worktree's build, {}. To build here again, run `cargo xtask \
         seed` first, or the build starts from nothing.",
        gb(bytes)
    );
    true
}

/// What deleting something would free, the files no other folder links to, and when anything
/// in it last changed.
#[derive(Clone, Copy)]
struct Usage {
    bytes: u64,
    newest: SystemTime,
}

impl Default for Usage {
    fn default() -> Self {
        Self {
            bytes: 0,
            newest: SystemTime::UNIX_EPOCH,
        }
    }
}

impl Usage {
    fn add(self, other: Self) -> Self {
        Self {
            bytes: self.bytes + other.bytes,
            newest: self.newest.max(other.newest),
        }
    }
}

fn usage(path: &Path) -> Usage {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return Usage::default();
    };
    let own = Usage {
        bytes: if meta.is_dir() || shared(&meta) {
            0
        } else {
            meta.len()
        },
        newest: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
    };
    if meta.is_dir() {
        children(path)
            .iter()
            .map(|p| usage(p))
            .fold(own, Usage::add)
    } else {
        own
    }
}

#[cfg(unix)]
fn shared(meta: &std::fs::Metadata) -> bool {
    std::os::unix::fs::MetadataExt::nlink(meta) > 1
}

#[cfg(not(unix))]
fn shared(_: &std::fs::Metadata) -> bool {
    false
}

fn age(idle: Duration) -> String {
    match idle.as_secs() / HOUR.as_secs() {
        0 => format!("{}m", idle.as_secs() / 60),
        hours @ 1..48 => format!("{hours}h"),
        hours => format!("{}d", hours / 24),
    }
}

fn gb(bytes: u64) -> String {
    match bytes {
        0 => "-".into(),
        b if b < 100_000_000 => format!("{} MB", b / 1_000_000),
        b => format!("{:.1} GB", b as f64 / 1e9),
    }
}

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn git_output(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktrees_are_read_from_gits_listing() {
        let text = "worktree /r\nHEAD aaa\nbranch refs/heads/main\n\n\
            worktree /r/.claude/worktrees/agent-x\nHEAD bbb\nbranch refs/heads/worktree-agent-x\nlocked claude agent\n\n\
            worktree /r/.claude/worktrees/gone\nHEAD ccc\ndetached\nprunable gitdir file points to non-existent location\n\n\
            worktree /r/.claude/worktrees/plain\nHEAD ddd\ndetached\n";
        let trees = parse(text);
        let paths: Vec<_> = trees
            .iter()
            .map(|t| t.path.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            paths,
            ["/r/.claude/worktrees/agent-x", "/r/.claude/worktrees/plain"]
        );
        assert!(trees[0].locked && !trees[1].locked);
        assert_eq!(trees[0].branch.as_deref(), Some("worktree-agent-x"));
        assert_eq!(trees[1].branch, None);
        assert_eq!(trees[1].head.as_deref(), Some("ddd"));
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_frees_only_what_nothing_else_links_to_and_shows_its_last_edit() {
        let dir = std::env::temp_dir().join(format!("xtask-usage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("inner")).unwrap();
        let before = SystemTime::now() - Duration::from_secs(2);
        std::fs::write(dir.join("own"), [0u8; 100]).unwrap();
        std::fs::write(dir.join("inner/linked"), [0u8; 50]).unwrap();
        std::fs::hard_link(dir.join("inner/linked"), dir.join("elsewhere")).unwrap();
        let found = usage(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(found.bytes, 100);
        assert!(found.newest >= before);
    }

    #[test]
    fn idle_reads_in_the_largest_unit_that_fits() {
        assert_eq!(age(Duration::from_secs(5 * 60)), "5m");
        assert_eq!(age(Duration::from_secs(30 * 60 * 60)), "30h");
        assert_eq!(age(Duration::from_secs(4 * 24 * 60 * 60)), "4d");
    }

    #[test]
    fn build_cleanup_keeps_source_and_waits_for_an_idle_hour() {
        let hour = Duration::from_secs(3600);
        for (agent, merged, changes) in [
            (true, true, Some(0)),
            (true, false, Some(2)),
            (false, false, Some(0)),
        ] {
            assert_eq!(
                choose_action(
                    true,
                    agent,
                    merged,
                    changes,
                    hour - Duration::from_secs(1),
                    true,
                    Some(false)
                ),
                Action::Keep
            );
            assert_eq!(
                choose_action(true, agent, merged, changes, hour, true, Some(false)),
                Action::FreeBuild
            );
        }
        assert_eq!(
            choose_action(
                true,
                true,
                true,
                Some(0),
                Duration::from_secs(2 * 86400),
                false,
                Some(false)
            ),
            Action::Remove
        );
        assert_eq!(
            choose_action(
                true,
                true,
                false,
                Some(0),
                Duration::from_secs(2 * 86400),
                false,
                Some(false)
            ),
            Action::Keep
        );
    }

    #[test]
    fn active_locked_and_unreadable_worktrees_are_kept() {
        let old = Duration::from_secs(10 * 86400);
        for active in [Some(true), None] {
            assert_eq!(
                choose_action(true, true, true, Some(0), old, true, active),
                Action::Keep
            );
        }
        assert_eq!(
            choose_action(false, true, true, Some(0), old, true, Some(false)),
            Action::Keep
        );
        assert_eq!(
            choose_action(true, true, true, None, old, true, Some(false)),
            Action::Keep
        );
    }

    #[test]
    fn a_locked_cargo_profile_cannot_be_deleted() {
        use std::io::{BufRead, Read, Write};
        use std::process::Stdio;

        let target = std::env::temp_dir().join(format!("xtask-build-lock-{}", std::process::id()));
        std::fs::create_dir_all(target.join("debug/.fingerprint")).unwrap();
        let path = target.join("debug").join(seed::LOCKS[0]);
        // A separate owner prevents parallel tests' child processes from inheriting this lock.
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "worktrees::tests::hold_cargo_lock",
                "--nocapture",
            ])
            .env("XTASK_TEST_CARGO_LOCK", &path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
        loop {
            let mut line = String::new();
            assert!(output.read_line(&mut line).unwrap() > 0);
            if line.contains("LOCK_READY") {
                break;
            }
        }
        assert!(!free_build(&target));
        assert!(path.exists());
        child.stdin.take().unwrap().write_all(b"release\n").unwrap();
        output.read_to_string(&mut String::new()).unwrap();
        assert!(child.wait().unwrap().success());
        assert!(free_build(&target));
    }

    #[test]
    fn hold_cargo_lock() {
        let Some(path) = std::env::var_os("XTASK_TEST_CARGO_LOCK") else {
            return;
        };
        let lock = File::create(path).unwrap();
        lock.try_lock().unwrap();
        println!("LOCK_READY");
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        drop(lock);
    }

    #[test]
    fn an_unrecognised_build_folder_is_kept() {
        let target =
            std::env::temp_dir().join(format!("xtask-unknown-build-{}", std::process::id()));
        std::fs::create_dir_all(&target).unwrap();
        let file = target.join("unrecognised-output");
        std::fs::write(&file, b"keep").unwrap();
        assert!(!free_build(&target));
        assert!(file.exists());
        std::fs::remove_dir_all(&target).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn process_inspection_finds_a_session_inside_a_worktree() {
        let dir = std::env::temp_dir().join(format!("xtask-session-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let mut child = Command::new("sleep")
            .arg("30")
            .current_dir(&dir)
            .spawn()
            .unwrap();
        let found =
            process_paths().is_some_and(|paths| paths.iter().any(|path| path.starts_with(&dir)));
        let _ = child.kill();
        let _ = child.wait();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(found);
    }
}
