//! The heap profile (`--features dhat-heap`): dhat-rs (MIT or Apache-2.0, Nicholas
//! Nethercote's port of Valgrind's DHAT) as the global allocator in place of mimalloc, each
//! allocation's backtrace kept, so what holds the process's memory is found by the line of
//! code that made it, without heaptrack or valgrind on the box.
//!
//! The profile is written as the app exits (`AppExit`), with the world still standing, so
//! its end bytes (`eb` in each program point) are what the running game held then, not what
//! a torn-down app leaked; to `TERRA_DHAT_FILE`, or `dhat-heap.json` in the working
//! directory. Open it in DHAT's viewer (dh_view.html), or compare two runs' end bytes by site.
//! Slow: every allocation takes a backtrace and a global lock. Never in a timed build.

use bevy::prelude::*;
use std::sync::Mutex;

#[global_allocator]
static ALLOCATOR: dhat::Alloc = dhat::Alloc;

static PROFILER: Mutex<Option<dhat::Profiler>> = Mutex::new(None);

/// Starts profiling: first thing in `main`.
pub fn start() {
    let file = std::env::var("TERRA_DHAT_FILE").unwrap_or_else(|_| "dhat-heap.json".into());
    let profiler = dhat::Profiler::builder().file_name(file).build();
    *PROFILER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(profiler);
}

/// Writes the profile once the app is told to exit, its world still standing.
pub fn plugin(app: &mut App) {
    app.add_systems(Last, write.run_if(on_message::<AppExit>));
}

/// Writes the profile, if not written yet (also at the end of `main`, for a run that exits
/// another way).
pub fn write() {
    let taken = PROFILER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    if let Some(profiler) = taken {
        info!("Heap profile: writing it with the world standing");
        drop(profiler);
    }
}
