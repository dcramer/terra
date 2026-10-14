//! Lists this repository's worktrees and clears away the ones agents are done with (D259).
//! Claude Code removes a subagent's worktree when the agent made no changes, and otherwise only
//! after `cleanupPeriodDays`, a month by default, while each built worktree holds gigabytes.
//! `cargo xtask worktrees` shows what each holds and what `--prune` would do; a Claude Code hook
//! runs `--prune` in the background at most once an hour.
//!
//! Only worktrees under `.claude/worktrees/` are touched, never the one this runs in, never one
//! locked (Claude Code locks a subagent's while it runs) and never one used recently:
//! - a subagent's (`agent-…`) whose commits are all on `main` and that has no changes, untracked
//!   files included, is removed with its branch once it has been idle a day;
//! - any other's build folder is deleted once it has been idle three days, since it can be
//!   built again (`cargo xtask seed` makes that quick). Its code is never touched.
//!
//! Idle is measured from the last thing that touched it: git (a commit or a checkout, not the
//! index, which `git status` rewrites) or a build. Every rule fails toward keeping: a worktree
//! it cannot read, or whose name or build locks it does not recognise, is left alone.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use super::{output, root, seed};

const HOUR: Duration = Duration::from_secs(60 * 60);
/// How long a finished subagent's worktree is kept, in case its agent is sent another message.
const REMOVE_AFTER: Duration = Duration::from_secs(24 * 60 * 60);
/// How long an idle worktree keeps its build folder.
const FREE_AFTER: Duration = Duration::from_secs(3 * 24 * 60 * 60);

pub fn worktrees(args: &[String]) -> bool {
    let prune = match args {
        [] => false,
        [flag] if flag == "--prune" => true,
        _ => {
            eprintln!("usage: cargo xtask worktrees [--prune]");
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
        let idle = now
            .duration_since(last_used(&tree.path))
            .unwrap_or_default();
        let ours = tree.path.starts_with(&managed) && tree.path != root() && !tree.locked;
        let action = if !ours {
            Action::Keep
        } else if name.starts_with("agent-") && merged && changes == Some(0) && idle >= REMOVE_AFTER
        {
            Action::Remove
        } else if target.is_dir() && idle >= FREE_AFTER {
            Action::FreeBuild
        } else {
            Action::Keep
        };

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
        .collect::<Vec<_>>()
        .join(", ");
        let build = own_bytes(&target);
        let checkout: u64 = children(&tree.path)
            .iter()
            .filter(|p| **p != target)
            .map(|p| own_bytes(p))
            .sum();
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
             clean and idle a day, and deletes any other's build after three idle days."
        );
    }
    ok
}

#[derive(Clone, Copy)]
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
    let mut held = Vec::new();
    for profile in seed::profile_dirs(target) {
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

/// Bytes under a folder that deleting it would free: files no other folder links to.
fn own_bytes(dir: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(dir) else {
        return 0;
    };
    if meta.is_dir() {
        children(dir).iter().map(|p| own_bytes(p)).sum()
    } else if shared(&meta) {
        0
    } else {
        meta.len()
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

    #[test]
    fn idle_reads_in_the_largest_unit_that_fits() {
        assert_eq!(age(Duration::from_secs(5 * 60)), "5m");
        assert_eq!(age(Duration::from_secs(30 * 60 * 60)), "30h");
        assert_eq!(age(Duration::from_secs(4 * 24 * 60 * 60)), "4d");
    }
}
