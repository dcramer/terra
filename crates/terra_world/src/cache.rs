//! The user's cache folder for what the game makes once and keeps (`terra/` in it): the GPU
//! turn's lock (`gpu_turn`), compressed texture sets (D362). XDG's on Linux, `~/.cache`
//! otherwise where there is a home, Windows' local application data.

use std::path::PathBuf;

/// `terra/` in the user's cache folder; none where there is no such folder.
pub fn folder() -> Option<PathBuf> {
    terra_core::cooked::cache_folder()
}
