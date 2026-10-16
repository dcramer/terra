//! A base far off as a few merged meshes (docs/building-plan.md, drawing), as Unreal's
//! hierarchical levels of detail merge a cell's meshes into one: past where a section's own
//! proxy ends (`draw::PROXY_M`) to the horizon, each cell of a base, [`CELL`] sections a side
//! counted from its lowest, is one mesh: the hull of its cubes (`terra_build::hull`), as a
//! section's own proxy is, its materials' colours baked in. A house is one cell, a castle a
//! few dozen. They are made on a worker from the cubes alone, with nothing laid, first when
//! the base appears and again after it changes, once the camera is far enough off that a
//! rebuild cannot be seen; and with them drawn, `draw::stream` lets the sections themselves
//! go when the camera is further off than they are seen, so a base far off holds no more
//! than these.

use super::draw::{self, Uvs};
use super::store::{self, each};
use super::{BuildLook, Built, Site};
use crate::planet::MainCamera;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use std::collections::BTreeMap;
use std::sync::Arc;
use terra_build::kit::Kits;
use terra_world::detail::DetailScale;

/// How many sections a side a cell of a base's far meshes holds: 32 m, as big as its
/// sections can be from its middle and still hand over from their own proxies within the
/// fade (`draw::PROXY_FADE`).
const CELL: i32 = 4;
/// How far a base's cells are seen: the horizon from a rise.
pub(super) const HORIZON_M: f32 = 3000.0;
/// A changed base's cells are made again once the camera is this far off, as authored:
/// well short of where they are first seen, well past where a builder works.
const REMAKE_M: f32 = 300.0;
/// A base's cells are on screen as entities only from this far off, as authored, and let
/// go again inside `CELLS_GO_M`: they are seen from 540 m (`draw::PROXY_M` less its fade),
/// and an entity each nearer would only be Bevy's work every frame for nothing seen.
const CELLS_COME_M: f32 = 300.0;
const CELLS_GO_M: f32 = 270.0;

/// A base's far meshes: the revision they were made for, their entities, and the worker
/// making them again.
#[derive(Default)]
pub(super) struct Cells {
    made: Option<u64>,
    entities: Vec<Entity>,
    /// Its meshes, each about its middle, on screen or not.
    meshes: Vec<(Vec3, Handle<Mesh>)>,
    task: Option<Task<Vec<(Vec3, Option<Mesh>)>>>,
    /// How many bytes its meshes take on the GPU.
    pub gpu_bytes: usize,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, make.in_set(super::layout::Drawing::Upload));
}

/// Starts making each base's cells that needs them, and puts on screen those made.
fn make(
    mut commands: Commands,
    look: Res<BuildLook>,
    scale: Res<DetailScale>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
) {
    let eye = camera.iter().next().map(GlobalTransform::translation);
    let pool = AsyncComputeTaskPool::get();
    let no_kits: Arc<Kits> = Arc::new(Kits(Vec::new()));
    for build in each(&mut site, &mut builds) {
        let revision = build.revision;
        let root = build.root;
        let cells = &mut build.sections.cells;
        if let Some(task) = cells.task.as_mut() {
            let Some(made) = block_on(poll_once(task)) else {
                continue;
            };
            cells.task = None;
            cells.gpu_bytes = 0;
            for entity in cells.entities.drain(..) {
                commands.entity(entity).try_despawn();
            }
            cells.meshes.clear();
            for (centre, mesh) in made {
                let Some(mesh) = mesh else { continue };
                cells.gpu_bytes += draw::gpu_bytes(&mesh);
                cells.meshes.push((centre, meshes.add(mesh)));
            }
            continue;
        }
        // On screen far enough off to be seen, and not nearer.
        let off = eye.map_or(f32::MAX, |eye| build.frame.translation.distance(eye)) / scale.0;
        if cells.entities.is_empty() && off > CELLS_COME_M {
            for (centre, mesh) in &cells.meshes {
                cells.entities.push(
                    commands
                        .spawn((
                            Name::new("Base far off"),
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(look.looks.far.clone()),
                            draw::cell_range(),
                            super::views::Level::Cell,
                            Transform::from_translation(*centre),
                            ChildOf(root),
                        ))
                        .id(),
                );
            }
        } else if !cells.entities.is_empty() && off < CELLS_GO_M {
            for entity in cells.entities.drain(..) {
                commands.entity(entity).try_despawn();
            }
        }
        if cells.made == Some(revision) {
            continue;
        }
        // Made again only once far enough off that it cannot be seen being remade.
        let distance = eye.map_or(f32::MAX, |eye| build.frame.translation.distance(eye)) / scale.0;
        if cells.made.is_some() && distance < REMAKE_M {
            continue;
        }
        cells.made = Some(revision);
        // Not kept: a base far off holds no copy of itself to lay from.
        let snapshot = build
            .sections
            .snapshot
            .clone()
            .unwrap_or_else(|| Arc::new(build.base.clone()));
        let kits = build.kits.clone().unwrap_or_else(|| no_kits.clone());
        let maps = look.maps.clone();
        build.sections.cells.task = Some(pool.spawn(async move {
            // The sections with cubes, by the cell they are in, counted from the lowest.
            let sections: Vec<IVec3> = snapshot.store().sections().collect();
            let low = sections
                .iter()
                .copied()
                .reduce(IVec3::min)
                .unwrap_or_default();
            let mut by_cell: BTreeMap<[i32; 3], Vec<IVec3>> = BTreeMap::new();
            for at in sections {
                let cell = (at - low).div_euclid(IVec3::splat(CELL)).to_array();
                by_cell.entry(cell).or_default().push(at);
            }
            by_cell
                .into_iter()
                .map(|(cell, members)| {
                    let side = terra_build::cubes::SECTION;
                    let from = (low + IVec3::from_array(cell) * CELL) * side;
                    let to = from + IVec3::splat(CELL * side - 1);
                    let hull = terra_build::hull::hull(&snapshot, &kits, &maps, from, to);
                    // Measured from the middle of its sections, as each of them is.
                    let middles: Vec<Vec3> = members.into_iter().map(store::centre).collect();
                    let centre = middles.iter().sum::<Vec3>() / middles.len().max(1) as f32;
                    (
                        centre,
                        draw::mesh(&hull, centre, Uvs::None).map(draw::packed),
                    )
                })
                .collect()
        }));
    }
}
