//! The repository's kits as the tests over building read them, in every building crate
//! (D407: a crate's tests see only the crates below it, so what they share is here, not in a
//! test module): not the game's, which loads them through its asset server.

use crate::kit::{Kit, MATERIALS};

/// The repository's `assets/` folder.
pub fn assets() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// A shipped kit by its file's name, without its modelled pieces.
pub fn shipped(name: &str) -> Kit {
    let read = |path: &str| {
        std::fs::read_to_string(assets().join(path)).map_err(|why| format!("{path}: {why}"))
    };
    let mut kit = Kit::read(&format!("{MATERIALS}/{name}"), &read).unwrap();
    kit.dress.made = None;
    kit
}
