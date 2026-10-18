//! At most a few push checks build and test at once on one machine (D413). Every session and
//! worktree shares the machine, and four checks at once ran 54 to 80 minutes each, where one
//! alone takes a few: on 10 cores and 32 GB they swapped rather than shared. So a check with
//! crates to build or test, or building's checks to run, waits for one of a few slots
//! (`check.slot-<n>` in `~/.cache/terra`, `TERRA_CHECK_SLOTS` of them, two by default), says
//! that it waits and who holds them, and holds its slot until it ends. A check with nothing
//! heavy to run never waits. The slots are the operating system's advisory locks on the
//! files, as the GPU's turn is (D319), so a check that dies frees its slot.

use crate::{root, stage};
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How many checks build and test at once by default.
const SLOTS: usize = 2;

/// The user's cache folder for Terra, as `terra_world::cache::folder` finds it (the tasks
/// depend on no crate).
fn folder() -> Option<PathBuf> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(cache.join("terra"))
}

/// Waits for a slot and holds it for as long as the file returned is kept. `None`, and no
/// wait, where no slot file can be opened: the slots share the machine, they guard nothing.
pub(crate) fn take() -> Option<File> {
    let folder = folder()?;
    std::fs::create_dir_all(&folder).ok()?;
    let slots = std::env::var("TERRA_CHECK_SLOTS")
        .ok()
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(SLOTS)
        .max(1);
    let started = Instant::now();
    let mut said = false;
    loop {
        match try_take(&folder, slots) {
            Taken::Slot(file) => {
                if said {
                    println!(
                        "  got a check slot after {}",
                        stage::duration(started.elapsed())
                    );
                }
                return Some(file);
            }
            Taken::NoFiles => return None,
            Taken::Busy => {}
        }
        if !said {
            println!(
                "  waiting for a check slot: {slots} checks are building and testing ({}; TERRA_CHECK_SLOTS sets how many)",
                holders(&folder, slots).join(", ")
            );
            said = true;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

enum Taken {
    Slot(File),
    Busy,
    NoFiles,
}

/// The first free slot of `slots` in `folder`, marked with who holds it.
fn try_take(folder: &Path, slots: usize) -> Taken {
    let mut opened = false;
    for n in 0..slots {
        let Ok(mut file) = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(folder.join(format!("check.slot-{n}")))
        else {
            continue;
        };
        opened = true;
        if file.try_lock().is_ok() {
            let _ = file.set_len(0);
            let _ = write!(file, "{} (pid {})", root().display(), std::process::id());
            return Taken::Slot(file);
        }
    }
    if opened { Taken::Busy } else { Taken::NoFiles }
}

/// Who holds the slots, as each holder wrote it: its checkout's folder name and pid.
fn holders(folder: &Path, slots: usize) -> Vec<String> {
    (0..slots)
        .filter_map(|n| std::fs::read_to_string(folder.join(format!("check.slot-{n}"))).ok())
        .map(|text| {
            let name = text.rsplit('/').next().unwrap_or(&text);
            name.trim().to_string()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_slot_is_not_taken_again_and_is_free_once_let_go() {
        let folder = std::env::temp_dir().join(format!("terra-slots-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let Taken::Slot(first) = try_take(&folder, 2) else {
            panic!("the first slot is free")
        };
        let Taken::Slot(second) = try_take(&folder, 2) else {
            panic!("the second slot is free")
        };
        assert!(matches!(try_take(&folder, 2), Taken::Busy));
        assert_eq!(holders(&folder, 2).len(), 2);
        assert!(holders(&folder, 2)[0].contains(&format!("pid {}", std::process::id())));
        drop(first);
        // A child another test forks holds the lock with its copy of the file until it runs
        // its program, as a check's own children do for a moment: `take` waits that out.
        let since = Instant::now();
        let freed = loop {
            if let Taken::Slot(file) = try_take(&folder, 2) {
                break Some(file);
            }
            if since.elapsed() > Duration::from_secs(10) {
                break None;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(freed.is_some(), "the slot let go is free");
        drop(freed);
        drop(second);
        let _ = std::fs::remove_dir_all(&folder);
        assert!(matches!(try_take(&folder, 2), Taken::NoFiles));
    }
}
