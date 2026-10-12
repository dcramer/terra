//! The kinds of thing, read from the files in `assets/data` (D136): items, stations,
//! recipes, creatures, what grows, what the ground gives and what can be built. Each is
//! read into a registry, a resource the systems that need it read, so the content a game
//! runs on can be replaced as a whole: by a planet's theme, by what a server sends (D51),
//! or by a file edited while the game runs.
//!
//! Files are read from the asset folder Bevy reads from, so a build finds them next to
//! itself and a test finds them in the repository.

use bevy::asset::io::file::FileAssetReader;
use serde::de::DeserializeOwned;
use std::path::PathBuf;

/// Where a data file is.
pub fn path(file: &str) -> PathBuf {
    FileAssetReader::get_base_path()
        .join("assets/data")
        .join(file)
}

/// Reads a data file. A mistake in one is a mistake in the game, so it stops it, saying
/// which file and where.
pub fn read<T: DeserializeOwned>(file: &str) -> T {
    let path = path(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|problem| panic!("Could not read {}: {problem}", path.display()));
    ron::from_str(&text).unwrap_or_else(|problem| panic!("{}: {problem}", path.display()))
}

/// Reads every registry, items first, since the others name items, and stations before
/// what names them.
pub fn plugin(app: &mut bevy::prelude::App) {
    let items = super::inventory::Items::default();
    let stations = super::crafting::Stations::read();
    let recipes = super::crafting::Recipes::read(&stations, &items);
    let builds = super::catalogue::Builds::read(&items, &stations);
    app.insert_resource(items)
        .insert_resource(stations)
        .insert_resource(recipes)
        .insert_resource(builds);
}
