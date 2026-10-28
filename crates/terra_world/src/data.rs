//! The world's files in `assets/data` (D136), and reading them: what grows (`flora.ron`) is
//! read here into its registry, `Flora`; the kinds gameplay runs on are read by
//! `registries`, with the same `read`.
//!
//! Files are read from the asset folder Bevy reads from, so a build finds them next to
//! itself and a test finds them in the repository. The flora file, saved while the game
//! runs, is read again within a second; a change that adds, takes away or reorders kinds
//! needs a restart, and so does a mistake: either is said in the log, and the kinds stay as
//! they were.

use super::flora_kinds::Flora;
use bevy::asset::io::file::FileAssetReader;
use bevy::prelude::*;
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use std::time::SystemTime;

/// How often the flora file is looked at for changes, in seconds.
const LOOK_EVERY_S: f32 = 1.0;

/// The asset folder: `assets` where Bevy looks for it (`BEVY_ASSET_ROOT`, which Cargo sets,
/// or beside the program); failing that, in the checkout found above, which holds `assets`
/// and `Cargo.toml`. A build run straight from `target/release/` by a script or an agent,
/// outside Cargo, otherwise found no files and failed once it had its turn on the GPU.
pub fn assets() -> PathBuf {
    let base = FileAssetReader::get_base_path();
    if base.join("assets").is_dir() {
        return base.join("assets");
    }
    base.ancestors()
        .find(|dir| dir.join("assets").is_dir() && dir.join("Cargo.toml").is_file())
        .map_or_else(|| base.join("assets"), |dir| dir.join("assets"))
}

/// The checkout's assets: `assets()`, or, for a Windows copy run from WSL (`cargo xtask win`),
/// the source's own, which it names in `TERRA_SOURCE_ASSETS`, so a tool writing assets (the
/// icon baker) writes them into the checkout and not into the copy.
pub fn source_assets() -> PathBuf {
    std::env::var_os("TERRA_SOURCE_ASSETS")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .unwrap_or_else(assets)
}

/// The asset source of pictures read on the CPU only, such as building's texture arrays read
/// them (D290): the same folder as `assets()`, as `cpu://<path>`. Bevy keeps one asset per
/// path, and the first load's settings win, so a picture read CPU-only and the same picture
/// drawn by a material were one asset that never reached the GPU: in the play experiment the
/// lush forest's log ends and stumps, which share `materials/log_end` with the log kits, were
/// never drawn and held the loading screen up to its cap. From their own source they are two
/// assets.
pub const CPU_ONLY: &str = "cpu";

/// A picture under `assets()` as read on the CPU only (`CPU_ONLY`).
pub fn cpu_only(path: &str) -> String {
    format!("{CPU_ONLY}://{path}")
}

/// Where a data file is.
pub fn path(file: &str) -> PathBuf {
    assets().join("data").join(file)
}

/// Reads a data file. A mistake in one is a mistake in the game, so it stops it, saying
/// which file and where.
pub fn read<T: DeserializeOwned>(file: &str) -> T {
    let path = path(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|problem| panic!("Could not read {}: {problem}", path.display()));
    ron::from_str(&text).unwrap_or_else(|problem| panic!("{}: {problem}", path.display()))
}

/// When the flora file was last changed, as last looked.
#[derive(Resource)]
struct Watched {
    changed: Option<SystemTime>,
    since: f32,
}

fn changed() -> Option<SystemTime> {
    std::fs::metadata(path(Flora::FILE))
        .and_then(|it| it.modified())
        .ok()
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Flora::read())
        .insert_resource(Watched {
            changed: changed(),
            since: 0.0,
        })
        .add_systems(Last, reload);
}

/// Reads the flora file again when it has changed, if its kinds are the same ones.
fn reload(time: Res<Time<Real>>, mut watched: ResMut<Watched>, mut flora: ResMut<Flora>) {
    watched.since += time.delta_secs();
    if watched.since < LOOK_EVERY_S {
        return;
    }
    watched.since = 0.0;
    let now = changed();
    if now == watched.changed {
        return;
    }
    watched.changed = now;
    let Ok(fresh) = std::panic::catch_unwind(Flora::read) else {
        warn!("The flora file has a mistake, said above; the kinds stay as they were.");
        return;
    };
    let names = |flora: &Flora| -> Vec<String> {
        flora.kinds().iter().map(|kind| kind.name.clone()).collect()
    };
    if names(&flora) != names(&fresh) {
        warn!("The flora file changed which kinds there are; restart to see it.");
        return;
    }
    *flora = fresh;
    info!("Read the flora file again.");
}
