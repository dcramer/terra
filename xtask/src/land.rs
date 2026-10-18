//! `cargo xtask land` (D413): lands the branch on main without being overtaken. Every agent
//! pushes straight to main (D375), and a push's check under load takes longer than main stays
//! still, so a push checked against one main was refused because another had landed meanwhile,
//! and its retry was overtaken again. Landing takes turns, as a merge queue does:
//!
//! 1. Rebase onto origin/main and run the push's check, unlocked, beside everyone else's (the
//!    slow part, which stamps the tree it passed on, `stamp`).
//! 2. Take the turn to land: one machine-wide lock (`land.lock` in `~/.cache/terra`), waiting
//!    while another checkout holds it and saying who.
//! 3. Holding it, fetch and rebase again and push. The pre-push hook's check then runs only
//!    what landed since the stamp (often nothing heavy), against the main it lands on, and no
//!    push from this machine can land in between. Refused all the same (a push from elsewhere),
//!    it fetches, rebases and pushes again, still holding the turn.
//!
//! A plain `git push` to main takes the same turn in its hook (`hold_for_push`): if another
//! checkout holds it, or main has moved past what it was rebased on, it fails at once rather
//! than check for most of an hour and be refused. Only pushes from this machine take turns; one
//! from elsewhere is still refused and retried, the stamp keeping the retry short.

use crate::{Task, output, root, slot, stage};
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::process::Command;
use std::time::Instant;

/// Set by `land` for the push it makes, so the hook's check knows the turn is held for it.
const HELD: &str = "TERRA_LANDING";

/// How many times `land` pushes, holding the turn, when pushes from elsewhere keep landing.
const TRIES: usize = 3;

/// The task this module runs (D410).
pub(crate) const TASKS: &[Task] = &[Task {
    name: "land",
    takes: "",
    does: "lands the branch on main without being overtaken (D413): rebases and runs the push's check, then takes this machine's turn to land, rebases again and pushes, the hook's check running only what landed meanwhile",
    group: "Play and check",
    run: land,
}];

pub(crate) fn land(args: &[String]) -> bool {
    if !args.is_empty() {
        eprintln!("usage: cargo xtask land");
        return false;
    }
    if output("git", &["diff", "HEAD", "--quiet"]).is_none() {
        println!("✗ land: tracked files have uncommitted changes; commit them first");
        return false;
    }
    if !rebase() {
        return false;
    }
    if ahead() == Some(0) {
        println!("land: nothing to land, HEAD is on origin/main");
        return true;
    }
    println!("land: checking before taking the turn to land");
    if !crate::check::check(&[]) {
        return false;
    }
    let Some(_turn) = take_turn() else {
        println!("✗ land: cannot open the turn's lock in ~/.cache/terra");
        return false;
    };
    for _ in 0..TRIES {
        if !rebase() {
            return false;
        }
        let pushed = Command::new("git")
            .args(["push", "origin", "HEAD:main"])
            .env(HELD, "1")
            .current_dir(root())
            .status()
            .is_ok_and(|status| status.success());
        if pushed {
            let head = output("git", &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
            println!("✓ land: {head} is on main");
            return true;
        }
        // Refused because main moved (a push from elsewhere): again. Otherwise the check or
        // git failed, and what it printed says why.
        if !main_moved() {
            println!("✗ land: the push failed; fix what it says and land again");
            return false;
        }
        println!("land: main moved under the push (a push from elsewhere); again");
    }
    println!("✗ land: main moved under {TRIES} pushes in a row; land again");
    false
}

/// Brings origin/main in and rebases onto it, as the routine does.
fn rebase() -> bool {
    let ok = Command::new("git")
        .args(["pull", "-q", "--rebase", "origin", "main"])
        .current_dir(root())
        .status()
        .is_ok_and(|status| status.success());
    if !ok {
        println!(
            "✗ land: the rebase onto origin/main stopped (a conflict?): resolve it, `git rebase --continue`, then land again"
        );
    }
    ok
}

/// How many commits HEAD has that origin/main has not.
fn ahead() -> Option<usize> {
    output("git", &["rev-list", "--count", "origin/main..HEAD"])?
        .parse()
        .ok()
}

/// Whether origin/main, fetched again, is no longer under HEAD.
fn main_moved() -> bool {
    let _ = output("git", &["fetch", "-q", "origin", "main"]);
    output(
        "git",
        &["merge-base", "--is-ancestor", "origin/main", "HEAD"],
    )
    .is_none()
}

fn lock() -> Option<File> {
    let folder = slot::folder()?;
    std::fs::create_dir_all(&folder).ok()?;
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(folder.join("land.lock"))
        .ok()
}

/// Who holds the turn, as they wrote it: their checkout's folder name and pid.
fn holder() -> String {
    slot::folder()
        .and_then(|folder| std::fs::read_to_string(folder.join("land.lock")).ok())
        .map(|text| text.rsplit('/').next().unwrap_or(&text).trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "another checkout".into())
}

fn mark(file: &mut File) {
    let _ = file.set_len(0);
    let _ = write!(file, "{} (pid {})", root().display(), std::process::id());
}

/// Waits for the turn to land and holds it while the file returned is kept.
fn take_turn() -> Option<File> {
    let mut file = lock()?;
    if file.try_lock().is_err() {
        println!("land: waiting for the turn to land: {} holds it", holder());
        let started = Instant::now();
        file.lock().ok()?;
        println!(
            "land: got the turn after {}",
            stage::duration(started.elapsed())
        );
    }
    mark(&mut file);
    Some(file)
}

/// For the check a push to main runs (`--pushing`): the turn to land, held until the check
/// ends, or why the push should stop before checking. `Ok(None)` when `land` holds it for
/// this push. Fails when another checkout holds it, or main has moved past what HEAD was
/// rebased on, since git would refuse the push after the check.
pub(crate) fn hold_for_push() -> Result<Option<File>, String> {
    if std::env::var_os(HELD).is_some() {
        return Ok(None);
    }
    let Some(mut file) = lock() else {
        return Ok(None);
    };
    if file.try_lock().is_err() {
        return Err(format!(
            "{} is landing on main; `cargo xtask land` waits its turn, rebases and pushes (D413)",
            holder()
        ));
    }
    mark(&mut file);
    if main_moved() {
        return Err(
            "main has moved since this was rebased, so the push would be refused after its check: \
             `cargo xtask land` (D413)"
                .into(),
        );
    }
    Ok(Some(file))
}
