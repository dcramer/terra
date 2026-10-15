//! Many models loading at once never nest deep on an IO thread's stack (gltf_gate.rs).
//!
//! Its own test program, as the IO pool is one per process and this one must be made with
//! the stack the test gives it before anything else makes one. Without the gate Bevy's glTF
//! loader nests a load inside another's scope for every model loading at once: the game's
//! models went 25 deep, about 650 KiB of stack, and this test died of a stack overflow every
//! time. With it a thread holds at most as many loads as the gate lets through (four, about
//! 80 KiB), and every model loads.

use bevy::{
    app::ScheduleRunnerPlugin,
    asset::{AssetPlugin, LoadState},
    gltf::Gltf,
    prelude::*,
    tasks::{IoTaskPool, TaskPoolBuilder},
};
use std::path::{Path, PathBuf};
use terra_world::gltf_gate::GatedGltfPlugin;

/// Every model under `dir`, by its path within the assets folder.
fn models(assets: &Path, dir: &Path, found: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            models(assets, &path, found);
        } else if path.extension().is_some_and(|e| e == "glb") {
            let within = path.strip_prefix(assets).unwrap().to_string_lossy();
            found.push(within.replace('\\', "/"));
        }
    }
}

#[test]
fn every_model_loads_at_once_on_small_stacks() {
    // Four IO threads, as many as Bevy gives a big machine, each with 256 KiB of stack: an
    // eighth of the 2 MiB Bevy's pool threads get, enough for the gate's four loads and an
    // image decoding on top, too little for loads nested without it.
    IoTaskPool::get_or_init(|| {
        TaskPoolBuilder::new()
            .thread_name("IO Task Pool".to_string())
            .num_threads(4)
            .stack_size(256 << 10)
            .build()
    });
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut paths = Vec::new();
    models(&assets, &assets.join("models"), &mut paths);
    assert!(paths.len() > 50, "the game's models, all asked for at once");

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins.set(ScheduleRunnerPlugin::run_once()),
        AssetPlugin {
            file_path: assets.to_string_lossy().into_owned(),
            ..default()
        },
        ImagePlugin::default(),
        bevy::mesh::MeshPlugin,
        bevy::scene::ScenePlugin,
        bevy::world_serialization::WorldSerializationPlugin,
        bevy::animation::AnimationPlugin,
        GatedGltfPlugin::default(),
    ));
    app.finish();
    app.cleanup();
    let server = app.world().resource::<AssetServer>().clone();
    let handles: Vec<Handle<Gltf>> = paths.iter().map(|path| server.load(path.clone())).collect();
    let start = std::time::Instant::now();
    loop {
        app.update();
        let waiting = handles
            .iter()
            .filter(|handle| matches!(server.load_state(handle.id()), LoadState::Loading))
            .count();
        if waiting == 0 {
            break;
        }
        assert!(
            start.elapsed().as_secs() < 120,
            "{waiting} models still loading"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    for (path, handle) in paths.iter().zip(&handles) {
        assert!(
            server.is_loaded(handle.id()),
            "{path}: {:?}",
            server.load_state(handle.id())
        );
    }
}
