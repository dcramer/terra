//! The seed's path dependencies outside the workspace: the Bevy crates we carry a change to in
//! `patches/` (D184). Their units are seeded like any dependency's, but Cargo judges a path
//! package fresh by its files' modification times, and every file of a new checkout is newer
//! than the seed's build of it: each fresh worktree rebuilt `bevy_mesh` and, from it, Bevy's
//! whole render stack (`bevy_render`, `bevy_pbr` and 30 more, over a minute on the critical
//! path of every build it made). So a seeded worktree's file whose bytes match the seed's copy
//! takes the seed's time, as fresh as the seed's build of it; a file that differs keeps its own
//! and is rebuilt, as it must be.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The folders, relative to the root, of the path dependencies the root manifest names that are
/// not workspace members.
pub(super) fn foreign(root: &Path, ours: &[String]) -> Vec<PathBuf> {
    let Ok(manifest) = std::fs::read_to_string(root.join("Cargo.toml")) else {
        return Vec::new();
    };
    manifest
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.split_once('=')?;
            let (_, after) = rest.split_once("path")?;
            let path = after.trim_start().strip_prefix('=')?.split('"').nth(1)?;
            let name = name.trim();
            (!ours.iter().any(|o| o == name)).then(|| PathBuf::from(path))
        })
        .collect()
}

/// Gives each file under `dirs` in `here` whose bytes match the same file under `seed` the
/// seed's file's modification time. Returns how many files took it and how many differ.
pub(super) fn backdate(seed: &Path, here: &Path, dirs: &[PathBuf]) -> (usize, usize) {
    let mut counts = (0, 0);
    for dir in dirs {
        walk(&seed.join(dir), &here.join(dir), &mut counts);
    }
    counts
}

fn walk(from: &Path, to: &Path, counts: &mut (usize, usize)) {
    if from.is_dir() {
        let Ok(entries) = std::fs::read_dir(from) else {
            return;
        };
        for entry in entries.flatten() {
            walk(&entry.path(), &to.join(entry.file_name()), counts);
        }
        return;
    }
    match same_dated(from, to) {
        Some(true) => counts.0 += 1,
        Some(false) => counts.1 += 1,
        None => {}
    }
}

/// Dates `to` as `from` when their bytes match; checks again after, since a `git pull` running
/// meanwhile may have written the file between the reading and the dating, and puts such a
/// file's time back to now. None when either is missing.
fn same_dated(from: &Path, to: &Path) -> Option<bool> {
    let theirs = std::fs::read(from).ok()?;
    if std::fs::read(to).ok()? != theirs {
        return Some(false);
    }
    let time = std::fs::metadata(from).and_then(|m| m.modified()).ok()?;
    let set = |time: SystemTime| {
        std::fs::File::options()
            .write(true)
            .open(to)
            .and_then(|file| file.set_modified(time))
    };
    set(time).ok()?;
    if std::fs::read(to).ok().as_ref() != Some(&theirs) {
        let _ = set(SystemTime::now());
        return Some(false);
    }
    Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_patched_bevy_crates_are_found_and_members_are_not() {
        let ours = vec!["terra_core".to_string()];
        let dirs = foreign(&crate::root(), &ours);
        assert!(
            dirs.contains(&PathBuf::from("patches/bevy_mesh")),
            "{dirs:?}"
        );
        assert!(
            dirs.contains(&PathBuf::from("patches/bevy_pbr")),
            "{dirs:?}"
        );
        assert!(!dirs.contains(&PathBuf::from("crates/terra_core")));
    }

    /// A file like the seed's takes its time, so Cargo finds the seeded unit fresh; one edited
    /// keeps its own, so Cargo rebuilds it.
    #[test]
    fn only_files_like_the_seeds_take_its_time() {
        let base = crate::root().join("target/seed-paths-test");
        let _ = std::fs::remove_dir_all(&base);
        let (seed, here) = (base.join("seed"), base.join("here"));
        for dir in [&seed, &here] {
            std::fs::create_dir_all(dir.join("p/src")).unwrap();
        }
        std::fs::write(seed.join("p/src/a.rs"), "same").unwrap();
        std::fs::write(seed.join("p/src/b.rs"), "seed's").unwrap();
        std::fs::write(here.join("p/src/a.rs"), "same").unwrap();
        std::fs::write(here.join("p/src/b.rs"), "edited").unwrap();
        let old = SystemTime::now() - Duration::from_secs(86_400);
        for name in ["a.rs", "b.rs"] {
            let file = std::fs::File::options()
                .write(true)
                .open(seed.join("p/src").join(name))
                .unwrap();
            file.set_modified(old).unwrap();
        }
        let counts = backdate(&seed, &here, &[PathBuf::from("p")]);
        assert_eq!(counts, (1, 1));
        let time = |name: &str| {
            std::fs::metadata(here.join("p/src").join(name))
                .and_then(|m| m.modified())
                .unwrap()
        };
        assert_eq!(time("a.rs"), old);
        assert!(time("b.rs") > old + Duration::from_secs(3_600));
        let _ = std::fs::remove_dir_all(&base);
    }
}
