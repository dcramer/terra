//! Benchmarks have the GPU to themselves (D319). On a developer's machine several agents and
//! sessions run scripted shots, films and benchmarks, and a benchmark shared with any of them
//! measures nothing. So a benchmark waits for an exclusive lock on one file, held until it
//! exits, and says that it is waiting; every other unattended run takes the lock shared, so
//! shots and films run side by side, and wait only while a benchmark holds it. Taking turns for
//! every run cost the developers hours of queueing for no gain; only measuring needs the GPU
//! alone. But a dozen at once filled the GPU's memory and crashed, so a shared run also takes
//! one of a few slots (`gpu.slot-<n>` beside the lock, `TERRA_GPU_SLOTS` of them, three by
//! default) and waits for one to be free. A run someone plays is never held back.
//!
//! The lock is the operating system's advisory lock on the whole file, so anything else can take
//! the same turn: `flock ~/.cache/terra/gpu.lock <command>` for a measurement of one's own,
//! `flock --shared` for anything else that renders heavily. A run started under such a turn, as
//! one is when its script is wrapped in `flock`, has it already: on Linux it finds the lock held
//! by one of its own forebears (`/proc/locks`) and goes on, where it once waited on itself.

use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::OnceLock;

/// The lock, held for as long as the program runs.
static TURN: OnceLock<File> = OnceLock::new();
/// A shared run's slot, held for as long as it runs.
static SLOT: OnceLock<File> = OnceLock::new();
/// How many shared runs may draw at once by default: three heavy forests fit a 10 GB GPU.
const SLOTS: usize = 3;

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

/// Takes the GPU's turn until this run ends: alone for a benchmark (`alone`), waiting until no
/// other unattended run holds it; shared otherwise, waiting only while a benchmark has it.
pub fn take(alone: bool) {
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
    if !alone {
        take_slot(&path);
    }
    let free = if alone {
        file.try_lock().is_ok()
    } else {
        file.try_lock_shared().is_ok()
    };
    if !free && !held_above(&file) {
        println!(
            "SANDBOX waiting for the GPU: {} holds {}",
            if alone {
                "another unattended run"
            } else {
                "a benchmark"
            },
            path.display()
        );
        let started = std::time::Instant::now();
        let got = if alone {
            file.lock()
        } else {
            file.lock_shared()
        };
        if got.is_err() {
            return;
        }
        println!(
            "SANDBOX got the GPU after {:.0} s",
            started.elapsed().as_secs_f32()
        );
    }
    let _ = TURN.set(file);
}

/// Waits for one of the shared runs' slots, then holds it until this run ends.
fn take_slot(lock: &std::path::Path) {
    let slots = std::env::var("TERRA_GPU_SLOTS")
        .ok()
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(SLOTS)
        .max(1);
    let mut files: Vec<File> = (0..slots)
        .filter_map(|n| {
            OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(lock.with_file_name(format!("gpu.slot-{n}")))
                .ok()
        })
        .collect();
    if files.is_empty() {
        return;
    }
    let started = std::time::Instant::now();
    let mut said = false;
    loop {
        if let Some(free) = files.iter().position(|file| file.try_lock().is_ok()) {
            if said {
                println!(
                    "SANDBOX got a GPU slot after {:.0} s",
                    started.elapsed().as_secs_f32()
                );
            }
            let _ = SLOT.set(files.swap_remove(free));
            return;
        }
        if !said {
            println!("SANDBOX waiting for a GPU slot: {slots} runs are drawing");
            said = true;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
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
