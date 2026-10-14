//! Seeds a new worktree's build cache from the main checkout's, so its first build compiles
//! only Terra's own crates instead of all of Bevy (D183). A Claude Code hook runs it when a
//! session starts in a worktree with no `target/` yet; `cargo xtask seed` runs it by hand.
//!
//! Only the dependencies' units are copied: their fingerprints are keyed by version and
//! features, so they stay fresh wherever they are. Our own crates' units are left out, since
//! Cargo judges those by file times and could otherwise take the main checkout's code for this
//! one's. It follows Cargo's own locking: the main checkout's build lock is held shared, so a
//! build there finishes first, and this checkout's locks are held for the whole copy, so a
//! `cargo` command started meanwhile waits for it rather than building from nothing.
//!
//! The compiled libraries, most of the size, are hard links to the main checkout's rather than
//! copies, so a worktree costs a few hundred megabytes instead of ten gigabytes: rustc and the
//! linker replace an `.rlib`, `.rmeta` or `.so` with a new file when they build it again, so a
//! rebuild on either side leaves the other's alone. Everything else is copied, since Cargo and
//! rustc rewrite fingerprints, dep-info and build-script output in place.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use super::{output, root};

/// The locks Cargo 1.97 takes in a profile directory: `.cargo-lock` shared, the other two
/// exclusive, for the length of a build. `cargo run` lets them go before the program starts.
pub(super) const LOCKS: [&str; 3] = [".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock"];

/// What a background seed prints to its parent once it holds this checkout's locks.
const LOCKED: &str = "locked";

pub fn seed(args: &[String]) -> bool {
    match args.first().map(String::as_str) {
        Some("--background") => background(),
        Some("--child") => run(true),
        _ => run(false),
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
            "This worktree's target/ is being seeded from the main checkout's compiled \
             dependencies in the background (cargo xtask seed, log in target/seed.log). A cargo \
             command started meanwhile waits for it (\"Blocking waiting for file lock\"), then \
             builds only Terra's own crates."
        );
    }
    true
}

fn run(child: bool) -> bool {
    // A child's stdout is its parent's pipe, read once; everything else goes to the log.
    let say = |text: String| {
        if child {
            eprintln!("{text}");
        } else {
            println!("{text}");
        }
    };
    let here = root();
    let Some(main) = main_checkout() else {
        say("seed: git cannot find the main checkout".into());
        return false;
    };
    if main == here {
        say("seed: this is the main checkout; there is nothing to seed it from".into());
        return true;
    }
    let ours = match workspace_packages(&here) {
        Ok(ours) => ours,
        Err(error) => {
            say(format!(
                "seed: cannot list the workspace's packages: {error}"
            ));
            return false;
        }
    };

    let from_target = main.join("target");
    let to_target = here.join("target");
    let profiles: Vec<PathBuf> = profile_dirs(&from_target)
        .into_iter()
        .filter(|profile| !built(&to_target.join(profile), &ours))
        .collect();

    // This checkout's locks first, so nothing builds here while the copy is under way.
    let mut held = Vec::new();
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
    if child {
        println!("{LOCKED}");
        let _ = std::io::stdout().flush();
    }
    if profiles.is_empty() {
        say(
            "seed: nothing to copy; the main checkout has no build, or this one has its own".into(),
        );
        return true;
    }

    // Then the main checkout's, shared, so a build under way there finishes first.
    for profile in &profiles {
        let lock_file = from_target.join(profile).join(".cargo-build-lock");
        if !lock_file.exists() {
            continue;
        }
        let Ok(file) = File::open(&lock_file) else {
            continue;
        };
        if file.try_lock_shared().is_err() {
            say(format!(
                "seed: waiting for the main checkout's build in {}",
                profile.display()
            ));
            let _ = file.lock_shared();
        }
        held.push(file);
    }

    let started = Instant::now();
    let mut ok = true;
    for profile in &profiles {
        let (from, to) = (from_target.join(profile), to_target.join(profile));
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
    say(format!(
        "seed: done in {:.0} s",
        started.elapsed().as_secs_f64()
    ));
    drop(held);
    ok
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

/// Bytes seeded, as hard links to the main checkout's files and as copies.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn our_units_are_told_from_dependencies() {
        let ours = vec!["terra_core".to_string(), "xtask".to_string()];
        assert!(is_ours("terra_core-29bd634e92f54282", &ours));
        assert!(is_ours("libterra_core-29bd634e92f54282.rlib", &ours));
        assert!(is_ours("terra_core-29bd634e92f54282.d", &ours));
        assert!(is_ours("xtask-0123456789abcdef", &ours));
        assert!(!is_ours("libbevy_ecs-c6abf7f208cd6fe1.rlib", &ours));
        assert!(!is_ours("wayland-client-1a2b3c4d5e6f7a8b", &ours));
        assert!(!is_ours("libterra_core_extra-29bd634e92f54282.rlib", &ours));
    }

    #[test]
    fn only_outputs_replaced_whole_are_linked() {
        assert!(linkable(Path::new(
            "deps/libbevy_ecs-c6abf7f208cd6fe1.rlib"
        )));
        assert!(linkable(Path::new(
            "deps/libbevy_ecs-c6abf7f208cd6fe1.rmeta"
        )));
        assert!(linkable(Path::new(
            "deps/libserde_derive-5dfc8b2696cad4ae.so"
        )));
        assert!(!linkable(Path::new("deps/bevy_ecs-c6abf7f208cd6fe1.d")));
        assert!(!linkable(Path::new(
            ".fingerprint/bevy_ecs-c6abf7f208cd6fe1/lib-bevy_ecs"
        )));
        assert!(!linkable(Path::new("build/blake3-729e203be1ad83f9/output")));
    }

    #[test]
    fn finds_this_workspace_packages() {
        let names = workspace_packages(&root()).unwrap();
        assert!(names.iter().any(|n| n == "xtask"));
        assert!(names.iter().any(|n| n == "terra_game"));
    }
}
