//! Sweeping Cargo build folders of what no build uses any more (D259). Cargo never deletes
//! anything from `target/`: a dependency built again for a new version, features or flags
//! leaves the old unit beside it, and rustc's incremental cache keeps a folder for every unit it
//! ever compiled incrementally. A build folder that lives long, as the shared seed does (D183),
//! only grows: the seed had reached 282 GB, 178 GB of it incremental caches no worktree reads
//! and 85 GB of units older `Cargo.lock`s built.
//!
//! A unit's outputs, its build-script folder and its fingerprint are all named `<name>-<hash>`,
//! the hash Cargo derives from what decides how it is built, and Cargo's JSON messages name the
//! hash of every unit a build uses, fresh ones included. So a unit whose hash no build names is
//! one Cargo would never read again. This is the rule `cargo-sweep --file` approximates with
//! file times, made exact; Cargo's own garbage collection (`-Zgc`) does not yet reach build
//! folders.

use std::collections::HashSet;
use std::fs::File;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::seed;

/// The unit folders and files of a profile, each named `<name>-<hash>`.
const UNIT_FOLDERS: [&str; 3] = ["deps", "build", ".fingerprint"];

/// Every profile's Cargo locks in a build folder, held, so nothing builds in it meanwhile; none
/// if a build holds one, or a profile has none of the locks Cargo is known to take.
pub(crate) fn hold(target: &Path) -> Option<Vec<File>> {
    let profiles = seed::profile_dirs(target);
    if profiles.is_empty() {
        return None;
    }
    let mut held = Vec::new();
    for profile in profiles {
        let locks: Vec<File> = seed::LOCKS
            .iter()
            .filter_map(|name| File::open(target.join(&profile).join(name)).ok())
            .collect();
        if locks.is_empty() || locks.iter().any(|file| file.try_lock().is_err()) {
            return None;
        }
        held.extend(locks);
    }
    Some(held)
}

/// Deletes a build folder unless a build is under way in it.
pub(crate) fn free_build(target: &Path) -> bool {
    let Some(held) = hold(target) else {
        return false;
    };
    let removed = std::fs::remove_dir_all(target).is_ok();
    drop(held);
    removed
}

/// The unit hashes Cargo's JSON messages name: every `-` and sixteen hex digits in them. A
/// path that happens to match too only keeps more.
pub(crate) fn live_hashes(messages: &str) -> HashSet<String> {
    let bytes = messages.as_bytes();
    let hex = |b: &u8| b.is_ascii_digit() || (b'a'..=b'f').contains(b);
    let mut found = HashSet::new();
    for (at, _) in messages.match_indices('-') {
        let digits = &bytes[at + 1..bytes.len().min(at + 17)];
        if digits.len() == 16
            && digits.iter().all(hex)
            && bytes.get(at + 17).is_none_or(|b| !hex(b))
        {
            found.insert(messages[at + 1..at + 17].to_string());
        }
    }
    found
}

/// A unit's hash from its name in `deps/`, `build/` or `.fingerprint/`, such as
/// `libbevy_ecs-c6abf7f208cd6fe1.rlib`; none for a name without one.
fn unit_hash(file_name: &str) -> Option<&str> {
    let stem = file_name.split('.').next().unwrap_or(file_name);
    let (_, hash) = stem.rsplit_once('-')?;
    (hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hash)
}

/// Deletes a profile's units whose hash `live` lacks, but those `keep` names; what it freed.
/// Names without a hash are kept.
pub(crate) fn stale_units(
    profile: &Path,
    live: &HashSet<String>,
    keep: &dyn Fn(&str) -> bool,
) -> u64 {
    let mut freed = 0;
    for folder in UNIT_FOLDERS {
        for path in children(&profile.join(folder)) {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !keep(&name) && unit_hash(&name).is_some_and(|hash| !live.contains(hash)) {
                freed += delete(&path);
            }
        }
    }
    freed
}

/// Deletes a profile's incremental caches that rustc has not written for `idle`: each unit's
/// folder gets a new session whenever rustc compiles it. Rustc compiles a unit whose cache is
/// gone in full, once. What it freed.
pub(crate) fn incremental(profile: &Path, idle: Duration, now: SystemTime) -> u64 {
    let mut freed = 0;
    for path in children(&profile.join("incremental")) {
        let written = path
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        if now.duration_since(written).unwrap_or_default() >= idle {
            freed += delete(&path);
        }
    }
    freed
}

/// Deletes a file or folder; the bytes that freed, those no other folder links to.
fn delete(path: &Path) -> u64 {
    let bytes = unlinked(path);
    let gone = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    if gone.is_ok() { bytes } else { 0 }
}

/// The bytes under a path no other folder links to.
pub(crate) fn unlinked(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_dir() {
        children(path).iter().map(|p| unlinked(p)).sum()
    } else if shared(&meta) {
        0
    } else {
        meta.len()
    }
}

#[cfg(unix)]
pub(crate) fn shared(meta: &std::fs::Metadata) -> bool {
    std::os::unix::fs::MetadataExt::nlink(meta) > 1
}

#[cfg(not(unix))]
pub(crate) fn shared(_: &std::fs::Metadata) -> bool {
    false
}

fn children(dir: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_are_named_by_their_hash() {
        for name in [
            "libbevy_ecs-c6abf7f208cd6fe1.rlib",
            "bevy_ecs-c6abf7f208cd6fe1.d",
            "bevy_ecs-c6abf7f208cd6fe1",
            "wayland-client-c6abf7f208cd6fe1",
            "terra_sandbox-c6abf7f208cd6fe1.exe",
        ] {
            assert_eq!(unit_hash(name), Some("c6abf7f208cd6fe1"), "{name}");
        }
        for name in [
            "terra_sandbox",
            "wayland-client",
            "terra_world-2x7sd224sewjq",
        ] {
            assert_eq!(unit_hash(name), None, "{name}");
        }
    }

    #[test]
    fn a_build_names_every_unit_it_uses() {
        let messages = r#"{"reason":"compiler-artifact","package_id":"registry+https://github.com/rust-lang/crates.io-index#wayland-client@0.31.2","filenames":["/t/debug/deps/libwayland_client-0123456789abcdef.rlib"],"fresh":true}
{"reason":"build-script-executed","out_dir":"/t/debug/build/blake3-fedcba9876543210/out"}
{"reason":"compiler-artifact","executable":"/t/debug/deps/terra_core-00112233445566aa","fresh":false}"#;
        let live = live_hashes(messages);
        for hash in ["0123456789abcdef", "fedcba9876543210", "00112233445566aa"] {
            assert!(live.contains(hash), "{hash}");
        }
        assert_eq!(live.len(), 3);
    }

    #[test]
    fn only_units_no_build_names_are_swept() {
        let profile = std::env::temp_dir().join(format!("xtask-sweep-{}", std::process::id()));
        let live_unit = profile.join("deps/libfoo-0123456789abcdef.rlib");
        let stale_unit = profile.join("deps/libfoo-fedcba9876543210.rlib");
        let stale_folder = profile.join(".fingerprint/foo-fedcba9876543210/lib-foo");
        let unnamed = profile.join("deps/unnamed");
        let kept = profile.join("deps/ours-fedcba9876543210");
        for file in [&live_unit, &stale_unit, &stale_folder, &unnamed, &kept] {
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, b"1234").unwrap();
        }
        let live = HashSet::from(["0123456789abcdef".to_string()]);
        let keep = |name: &str| name.starts_with("ours-");
        assert_eq!(stale_units(&profile, &live, &keep), 8);
        assert!(live_unit.exists() && unnamed.exists() && kept.exists());
        assert!(!stale_unit.exists() && !stale_folder.parent().unwrap().exists());
        std::fs::remove_dir_all(&profile).unwrap();
    }

    #[test]
    fn incremental_caches_go_once_unwritten_for_their_idle_time() {
        let profile = std::env::temp_dir().join(format!("xtask-incr-{}", std::process::id()));
        let unit = profile.join("incremental/foo-2x7sd224sewjq");
        std::fs::create_dir_all(&unit).unwrap();
        std::fs::write(unit.join("s-1"), b"12").unwrap();
        let now = SystemTime::now();
        assert_eq!(incremental(&profile, Duration::from_secs(3600), now), 0);
        assert!(unit.exists());
        let later = now + Duration::from_secs(3600);
        assert_eq!(incremental(&profile, Duration::from_secs(3600), later), 2);
        assert!(!unit.exists());
        std::fs::remove_dir_all(&profile).unwrap();
    }
}
