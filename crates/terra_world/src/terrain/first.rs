//! The first view of the ground, meshed on entering a world (`super::FirstView`): every chunk
//! the camera wants but the coarsest levels (`super::STREAM_FROM`), on every core.
//!
//! Behind a loading screen it is meshed on a thread of its own, as the land is (D623), and
//! put in once it comes back: 6.5 to 14.5 s of a 30 km world's load was this one frame, the
//! window blank. The loading screen says how many chunks are shaped meanwhile, and waits for
//! them (`Terrain::coming_near`). A run with no loading screen, a layer's test, meshes it in
//! the frame the world is entered, as before.

use super::{Shown, Terrain, mesh_chunk, show, to_mesh, wanted};
use crate::loading::Screen;
use crate::planet::{GroundLook, MainCamera, Planet};
use bevy::platform::time::Instant;
use bevy::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::JoinHandle;
use terra_core::ChunkId;
use terra_voxel::Sides;

/// A chunk of the first view meshed: its mesh, if it has a surface, the seconds it took to
/// shape and to colour, and its vertices, for the log.
type Meshed = (ChunkId, Sides, Option<Mesh>, [f32; 2], usize);

/// The first view being meshed on its own thread.
pub(super) struct Meshing {
    thread: Option<JoinHandle<Vec<Meshed>>>,
    /// How many chunks, and how many of them are meshed so far.
    count: usize,
    done: Arc<AtomicUsize>,
    began: Instant,
    /// The seconds choosing them took, on the main thread, and the threads meshing them.
    chosen: f32,
    threads: usize,
    /// Whether it is meshed behind the loading screen, on its thread.
    behind: bool,
}

impl Meshing {
    /// The chunks still being meshed.
    pub(super) fn left(&self) -> usize {
        self.count - self.done.load(Ordering::Relaxed).min(self.count)
    }

    /// The chunks meshed so far.
    pub(super) fn done(&self) -> usize {
        self.count - self.left()
    }
}

/// Chooses the first view's chunks from where the camera stands and meshes them: on a thread
/// of its own behind a loading screen, at once without one.
pub(super) fn first_view(
    mut commands: Commands,
    planet: Res<Planet>,
    look: Res<GroundLook>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
    asked: Option<Res<crate::quality::Asked>>,
    screen: Option<Res<State<Screen>>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain: ResMut<Terrain>,
) {
    let began = Instant::now();
    let viewer = camera.translation();
    // The preset is chosen in the first frame; one asked for is known already.
    let reach = asked
        .and_then(|asked| asked.0)
        .unwrap_or_default()
        .ground_reach();
    let wanted = wanted(&planet, viewer, reach);
    let chosen = began.elapsed().as_secs_f32();
    let (chunks, later): (Vec<(ChunkId, Sides)>, Vec<(ChunkId, Sides)>) = wanted
        .iter()
        .map(|(&id, &sides)| (id, sides))
        .partition(|(id, _)| id.level < super::STREAM_FROM);
    terrain.later = later.into_iter().collect();
    terrain.wanted = wanted;
    terrain.chosen_from = Some(viewer);
    terrain.chosen_reach = reach;
    terrain.edits_seen = planet.voxels().edit_count();
    let done = Arc::new(AtomicUsize::new(0));
    let mut meshing = Meshing {
        thread: None,
        count: chunks.len(),
        done: Arc::clone(&done),
        began,
        chosen,
        threads: terra_gen::par::width(),
        behind: screen.is_some(),
    };
    if screen.is_none() {
        let meshed = mesh_all(&planet, &chunks, &done);
        put_in(
            &mut commands,
            &mut meshes,
            &look,
            &mut terrain,
            &meshing,
            meshed,
        );
        return;
    }
    let planet = planet.clone();
    let thread = std::thread::Builder::new()
        .name("terra-first-view".into())
        .spawn(move || mesh_all(&planet, &chunks, &done))
        .unwrap_or_else(|why| {
            panic!("the first view's thread could not be started ({why}); the machine is out of threads or memory")
        });
    meshing.thread = Some(thread);
    terrain.first = Some(meshing);
}

/// The first view, once meshed on its thread, put in; a panic meshing it goes on here.
pub(super) fn come_back(
    mut commands: Commands,
    look: Option<Res<GroundLook>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain: ResMut<Terrain>,
) {
    let Some(look) = look else { return };
    let finished = terrain
        .first
        .as_ref()
        .and_then(|first| first.thread.as_ref())
        .is_some_and(JoinHandle::is_finished);
    if !finished {
        return;
    }
    let Some(mut first) = terrain.first.take() else {
        return;
    };
    let Some(thread) = first.thread.take() else {
        return;
    };
    let meshed = thread
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    put_in(
        &mut commands,
        &mut meshes,
        &look,
        &mut terrain,
        &first,
        meshed,
    );
}

/// Every chunk shaped and coloured, on every core, counting them as they are done.
fn mesh_all(planet: &Planet, chunks: &[(ChunkId, Sides)], done: &AtomicUsize) -> Vec<Meshed> {
    terra_gen::par::map(chunks, |&(id, sides)| {
        let at = std::time::Instant::now();
        let chunk = mesh_chunk(&**planet.voxels(), id, sides);
        let shaped = at.elapsed().as_secs_f32();
        let vertices = chunk.positions.len();
        let mesh = to_mesh(planet, id, chunk);
        let coloured = at.elapsed().as_secs_f32() - shaped;
        done.fetch_add(1, Ordering::Relaxed);
        (id, sides, mesh, [shaped, coloured], vertices)
    })
}

/// The meshed chunks shown, and how long they took said.
fn put_in(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    look: &GroundLook,
    terrain: &mut Terrain,
    meshing: &Meshing,
    meshed: Vec<Meshed>,
) {
    let count = meshed.len();
    let mut levels: std::collections::BTreeMap<u8, (usize, f32, f32, usize)> = default();
    for (id, sides, mesh, [shaped, coloured], vertices) in meshed {
        let level = levels.entry(id.level).or_default();
        *level = (
            level.0 + 1,
            level.1 + shaped,
            level.2 + coloured,
            level.3 + vertices,
        );
        let entity = show(commands, meshes, look, id, mesh);
        terrain.shown.insert(id, Shown { sides, entity });
    }
    // How long, on how many threads: what a change to a chunk's meshing or colouring costs.
    let secs = meshing.began.elapsed().as_secs_f32();
    let by_level: Vec<String> = levels
        .iter()
        .map(|(level, (n, shaped, coloured, vertices))| {
            format!("level {level} {n} chunks, {vertices} vertices, {shaped:.1} s shaped and {coloured:.1} s coloured")
        })
        .collect();
    let behind = if meshing.behind {
        " on its own thread, behind the loading screen"
    } else {
        ""
    };
    info!(
        "Meshed the first view of the ground{behind}: {count} chunks in {secs:.2} s on {} threads, {:.2} s of it choosing them, {} far ones of level {} and up left for after the loading screen; {} (summed over threads).",
        meshing.threads,
        meshing.chosen,
        terrain.later.len(),
        super::STREAM_FROM,
        by_level.join("; ")
    );
}
