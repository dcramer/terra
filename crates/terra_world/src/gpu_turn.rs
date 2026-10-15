//! Unattended runs take turns on the GPU (D319). On a developer's machine several agents
//! and sessions run scripted shots, films and benchmarks, and the developer plays on the same
//! GPU: two runs at once each go slower, spoil each other's benchmarks, and a heavy scene no
//! longer fits twice in the GPU's memory. So an unattended run waits for a lock on one file,
//! held until it exits, and says that it is waiting. A run someone plays is never held back.
//!
//! The lock is the operating system's advisory lock on the whole file, so anything else that
//! renders can take the same turn: `flock ~/.cache/terra/gpu.lock blender ...`.

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
    if !matches!(file.try_lock(), Ok(())) {
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
