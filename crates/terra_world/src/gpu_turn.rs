//! Unattended runs take turns on the GPU (D319). On a developer's machine several agents
//! and sessions run scripted shots, films and benchmarks, and the developer plays on the same
//! GPU: two runs at once each go slower, spoil each other's benchmarks, and a heavy scene no
//! longer fits twice in the GPU's memory. So an unattended run waits for a lock on one file,
//! held until it exits, and says that it is waiting. A run someone plays is never held back.
//!
//! The lock is the operating system's advisory lock on the whole file, so anything else that
//! renders can take the same turn: `flock ~/.cache/terra/gpu.lock blender ...`. A run started
//! under such a turn, as a film's is when its script is wrapped in `flock`, has the turn
//! already: on Linux it finds the lock held by one of its own forebears (`/proc/locks`) and
//! goes on, where it once waited on itself for ever.

use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::OnceLock;

/// The lock, held for as long as the program runs.
static TURN: OnceLock<File> = OnceLock::new();

/// Where the lock is: `TERRA_GPU_LOCK` if set, otherwise `terra/gpu.lock` in the user's
/// cache folder. None where there is no such folder, and then no turn is taken.
pub fn lock_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("TERRA_GPU_LOCK") {
        return Some(PathBuf::from(path));
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))?;
    Some(cache.join("terra").join("gpu.lock"))
}

/// Waits until no other unattended run holds the GPU, then holds it until this one ends.
pub fn take() {
    let Some(path) = lock_path() else {
        return;
    };
    if let Some(folder) = path.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    let Ok(file) = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
    else {
        return;
    };
    if !matches!(file.try_lock(), Ok(())) && !held_above(&file) {
        println!(
            "SANDBOX waiting for the GPU: another unattended run holds {}",
            path.display()
        );
        let started = std::time::Instant::now();
        if file.lock().is_err() {
            return;
        }
        println!(
            "SANDBOX got the GPU after {:.0} s",
            started.elapsed().as_secs_f32()
        );
    }
    let _ = TURN.set(file);
}

/// Whether the lock on a file is held by a forebear of this process: a script that took the
/// turn and started this run. Only Linux says who holds a lock; elsewhere, never.
#[cfg(target_os = "linux")]
fn held_above(file: &File) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(inode) = file.metadata().map(|meta| meta.ino()) else {
        return false;
    };
    let Ok(locks) = std::fs::read_to_string("/proc/locks") else {
        return false;
    };
    let holders = holders_of(&locks, inode);
    let mut pid = std::process::id();
    // Up the line of parents to the first process.
    for _ in 0..64 {
        let Some(parent) = parent_of(pid) else {
            return false;
        };
        if holders.contains(&parent) {
            return true;
        }
        if parent <= 1 {
            return false;
        }
        pid = parent;
    }
    false
}

#[cfg(not(target_os = "linux"))]
fn held_above(_file: &File) -> bool {
    false
}

/// The processes holding a lock on a file by its inode, from `/proc/locks`: lines such as
/// `33: FLOCK  ADVISORY  WRITE 704977 00:1c:7468718 0 EOF`; those waiting are marked `->`.
#[cfg(any(target_os = "linux", test))]
fn holders_of(locks: &str, inode: u64) -> Vec<u32> {
    locks
        .lines()
        .filter(|line| !line.contains("->"))
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (pid, device) = (fields.get(4)?, fields.get(5)?);
            (device.rsplit(':').next()?.parse::<u64>().ok()? == inode)
                .then(|| pid.parse().ok())
                .flatten()
        })
        .collect()
}

/// A process's parent, from `/proc/<pid>/stat`, whose fourth field it is, after the name in
/// brackets, which may hold spaces.
#[cfg(target_os = "linux")]
fn parent_of(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = stat.rsplit_once(')')?.1;
    after.split_whitespace().nth(1)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_holders_of_a_lock_are_read_and_those_waiting_left_out() {
        let locks = "33: FLOCK  ADVISORY  WRITE 704977 00:1c:7468718 0 EOF\n\
                     33: -> FLOCK  ADVISORY  WRITE 766125 00:1c:7468718 0 EOF\n\
                     34: POSIX  ADVISORY  WRITE 1200 103:02:555 0 EOF\n";
        assert_eq!(holders_of(locks, 7468718), [704977]);
        assert_eq!(holders_of(locks, 555), [1200]);
        assert!(holders_of(locks, 1).is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_process_knows_its_parent() {
        let parent = parent_of(std::process::id()).expect("a parent");
        assert!(parent > 0 && parent != std::process::id());
    }
}
