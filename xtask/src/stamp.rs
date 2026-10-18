//! The tree the check before a push last passed on (D413), so the check after a rebase runs
//! again only what the rebase changed. Under load a push's check takes most of an hour, main
//! moves meanwhile, the push is refused, and the check after `git pull --rebase` used to run
//! it all again.
//!
//! The stamp is the git tree that passed: whatever a full check would say of it, it said,
//! taking main as sound as the gate always does. A stage's result depends only on what it
//! reads, so on a tree that differs from the stamped one, a stage needs running again only if
//! the push holds it (`scope::push`) and what it reads changed since the stamp: `since` is
//! that change, scoped as a change is. A stage the push holds whose inputs are as stamped
//! passed on them; one the stamped check left out read then what main holds, which its own
//! push checked. A change to the check itself (xtask) narrows nothing. A missing or unreadable
//! stamp, or one git no longer has, narrows nothing either: the check runs whole.
//!
//! It lives in the worktree's own git folder (`git rev-parse --git-path`), apart from
//! `target/`, so a `cargo clean` keeps it and no other worktree reads it.

use crate::output;
use crate::scope::{self, Scope};
use std::collections::BTreeSet;
use std::path::PathBuf;

/// The stamp's file name in the worktree's git folder.
const FILE: &str = "terra-check-stamp";

fn file() -> Option<PathBuf> {
    output(
        "git",
        &["rev-parse", "--path-format=absolute", "--git-path", FILE],
    )
    .map(PathBuf::from)
}

/// The tree the check is reading, when it is a commit's: HEAD's tree while no tracked file
/// differs from it. A tree with uncommitted changes is not stamped. Files git does not track
/// are in no push and left out, as `scope::push` leaves them.
pub(crate) fn clean_tree() -> Option<String> {
    let tree = output("git", &["rev-parse", "HEAD^{tree}"])?;
    output("git", &["diff", "HEAD", "--quiet"])?;
    Some(tree)
}

/// Records that the check passed on `tree`.
pub(crate) fn write(tree: &str) {
    if let Some(file) = file() {
        let _ = std::fs::write(file, format!("{tree}\n"));
    }
}

/// The stamped tree and what changed since it, up to the working tree, scoped. `None` when
/// there is no stamp, it does not name a tree git has, or git cannot say what changed.
pub(crate) fn since() -> Option<(String, Scope)> {
    let text = std::fs::read_to_string(file()?).ok()?;
    let tree = text.trim();
    if tree.len() < 40 || !tree.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    output("git", &["cat-file", "-e", &format!("{tree}^{{tree}}")])?;
    let files = output("git", &["diff", "--name-only", "--no-renames", tree])?;
    let files: Vec<String> = files
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();
    Some((tree[..10].to_string(), changed(&files, &scope::packages())))
}

/// What changed since the stamp, scoped: as any change is, except that a change to the check
/// itself (the tasks' code or manifest, not their baselines) narrows nothing, since the stamp
/// was a different check's.
fn changed(files: &[String], packages: &BTreeSet<String>) -> Scope {
    if files
        .iter()
        .any(|file| file.starts_with("xtask/src/") || file == "xtask/Cargo.toml")
    {
        return Scope::everything("the check itself changed");
    }
    scope::classify(files, packages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|f| f.to_string()).collect()
    }

    #[test]
    fn a_change_to_the_check_itself_narrows_nothing_and_others_are_scoped() {
        let packages: BTreeSet<String> = ["terra_land", "terra_build"].map(String::from).into();
        assert!(changed(&files(&["docs/a.md", "xtask/src/check.rs"]), &packages).all());
        assert!(changed(&files(&["xtask/Cargo.toml"]), &packages).all());
        // The size ratchet's baseline is the tasks' data, read by a stage that always runs.
        assert!(!changed(&files(&["xtask/sizes-baseline.tsv"]), &packages).all());
        let since = changed(&files(&["crates/terra_land/src/lib.rs"]), &packages);
        assert!(!since.all());
        assert_eq!(since.changed, ["terra_land".to_string()].into());
        assert!(!changed(&files(&[]), &packages).all());
    }
}
