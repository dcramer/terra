//! Pictures read cooked (D188, `terra_core::cooked`): a material's picture loaded from its
//! block-compressed copy with mipmaps where the bake (`cargo xtask cook`) has made one
//! for those very bytes and the GPU samples BC, from the picture as made otherwise. Models
//! are read cooked by the glTF gate (`gltf_gate.rs`).

use bevy::asset::AssetPath;
use bevy::image::{CompressedImageFormats, ImageLoaderSettings};
use bevy::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use terra_core::cooked::{self, Cook};

/// The asset source cooked files are read from: the cache folder they are kept in.
pub const SOURCE: &str = "cooked";

/// The cache folder, where the GPU samples BC; set once the renderer knows.
static FOLDER: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Each picture's cooked name, or none, as found the first time it was asked for.
static NAMES: Mutex<Option<HashMap<(String, Cook), Option<String>>>> = Mutex::new(None);

/// Registers [`SOURCE`]; before the asset server is made.
pub fn register(app: &mut App) {
    let folder = cooked::folder().unwrap_or_default();
    let _ = std::fs::create_dir_all(&folder);
    app.register_asset_source(
        SOURCE,
        bevy::asset::io::AssetSourceBuilder::platform_default(&folder.to_string_lossy(), None),
    );
}

/// Turns cooked reading on where the GPU samples `formats`; once, as the renderer starts.
pub fn enable(formats: CompressedImageFormats) -> Option<PathBuf> {
    FOLDER
        .get_or_init(|| {
            formats
                .contains(CompressedImageFormats::BC)
                .then(cooked::folder)
                .flatten()
        })
        .clone()
}

/// The picture at `path` under the assets, cooked as `cook` where a cooked copy is there,
/// otherwise read as made with `settings`, which say how to make it what `cook` names.
pub fn picture(
    assets: &AssetServer,
    path: &str,
    cook: Cook,
    settings: impl Fn(&mut ImageLoaderSettings) + Send + Sync + 'static,
) -> Handle<Image> {
    match name(path, cook) {
        Some(name) => assets.load(AssetPath::from(name).with_source(SOURCE)),
        None => assets
            .load_builder()
            .with_settings(settings)
            .load(path.to_string()),
    }
}

/// The cache folder where cooked reading is on; none before the renderer starts or where
/// the GPU samples no BC.
pub fn folder() -> Option<PathBuf> {
    FOLDER.get().cloned().flatten()
}

fn name(path: &str, cook: Cook) -> Option<String> {
    let folder = FOLDER.get()?.as_ref()?;
    let mut names = NAMES.lock().ok()?;
    names
        .get_or_insert_with(HashMap::new)
        .entry((path.to_string(), cook))
        .or_insert_with(|| {
            let bytes = std::fs::read(crate::data::assets().join(path)).ok()?;
            let name = cooked::picture_name(&bytes, cook);
            cooked::find(folder, &name).map(|_| name)
        })
        .clone()
}
