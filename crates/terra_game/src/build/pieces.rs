//! A kit's pieces drawn from its models (D177; docs/building-kits.md): the layout says where each
//! piece goes, and a copy of its model is placed there, fitted to its place. A log is
//! stretched along its length and turned about it and end for end by its seed, so
//! neighbours differ; a board fills its box; a door or window its opening; footings and
//! ridge caps are laid along their lines a metre or so at a time; roof and floor panels
//! fill their squares; stone fills its box, turned by its seed. Pieces not yet in are the same models in the ghost's material, and
//! the next one lit.

use super::{Build, BuildLook, Built, Shown, Site, redraw};
use crate::planet::Planet;
use bevy::asset::LoadState;
use bevy::camera::primitives::MeshAabb;
use bevy::gltf::{Gltf, GltfMesh};
use bevy::prelude::*;
use std::collections::BTreeMap;
use std::f32::consts::{PI, TAU};
use std::sync::Arc;
use terra_build::logs::{KitModels, LogKit, Piece, Split};
use terra_build::masonry::Stone;

/// How tall and deep a footing is, its top at the wall's foot.
const FOOTING_HIGH_M: f32 = 0.5;
const FOOTING_DEEP_M: f32 = 0.6;
/// How thick a roof's covering of shakes is, its courses standing proud of one another.
const SHAKES_THICK_M: f32 = 0.08;
/// About how long each copy of a footing or ridge cap is, laid along its line.
const STRETCH_M: f32 = 1.0;

/// A model, ready to place: each of its parts, and the box round it all, in its own space.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    parts: Vec<(Handle<Mesh>, Handle<StandardMaterial>)>,
    centre: Vec3,
    size: Vec3,
}

/// A kit's pieces' models, those that have loaded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KitPieces {
    logs: Vec<Model>,
    long_log: Option<Model>,
    long_from: f32,
    board: Option<Model>,
    door: Option<Model>,
    window: Option<Model>,
    footing: Vec<Model>,
    shakes: Option<Model>,
    ridge: Option<Model>,
    floor: Option<Model>,
    stone: Vec<Model>,
    stone_half: Vec<Model>,
    stone_corner: Vec<Model>,
}

impl KitPieces {
    /// Whether a model draws a piece, so code need not.
    pub(super) fn draws(&self, piece: &Piece) -> bool {
        match piece {
            Piece::Log { .. } => !self.logs.is_empty(),
            Piece::Board { .. } => self.board.is_some(),
            _ => true,
        }
    }
}

/// The models being loaded, by path, for the kit's models as they were asked for.
#[derive(Resource, Default)]
pub(super) struct Loading {
    wanted: Option<KitModels>,
    handles: BTreeMap<String, Handle<Gltf>>,
    done: bool,
}

/// Loads the log kit's models when it names them, and when they are ready draws every
/// build with them.
#[allow(clippy::too_many_arguments)]
pub(super) fn load(
    mut loading: ResMut<Loading>,
    mut look: ResMut<BuildLook>,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut commands: Commands,
    planet: Res<Planet>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let wanted = look.kits.as_deref().map(merged).unwrap_or_default();
    if loading.wanted.as_ref() != Some(&wanted) {
        let paths = wanted
            .logs
            .iter()
            .chain(&wanted.footing)
            .chain(wanted.long_log.iter())
            .chain(wanted.board.iter())
            .chain(wanted.door.iter())
            .chain(wanted.window.iter())
            .chain(wanted.shakes.iter())
            .chain(wanted.ridge.iter())
            .chain(wanted.floor.iter())
            .chain(&wanted.stone)
            .chain(&wanted.stone_half)
            .chain(&wanted.stone_corner);
        loading.handles = paths
            .map(|path| (path.clone(), assets.load(path.clone())))
            .collect();
        loading.wanted = Some(wanted);
        loading.done = false;
        return;
    }
    if loading.done {
        return;
    }
    let mut models: BTreeMap<String, Model> = BTreeMap::new();
    for (path, handle) in &loading.handles {
        if let Some(LoadState::Failed(problem)) = assets.get_load_state(handle) {
            error!("A kit's model did not load: {path}: {problem}");
            continue;
        }
        let Some(model) = resolve(path, handle, &assets, &gltfs, &gltf_meshes, &meshes) else {
            // Not loaded yet.
            return;
        };
        models.insert(path.clone(), model);
    }
    let wanted = loading.wanted.clone().unwrap_or_default();
    let one = |path: &Option<String>| path.as_ref().and_then(|path| models.get(path).cloned());
    let many = |paths: &[String]| -> Vec<Model> {
        paths
            .iter()
            .filter_map(|path| models.get(path).cloned())
            .collect()
    };
    let pieces = KitPieces {
        logs: many(&wanted.logs),
        long_log: one(&wanted.long_log),
        long_from: wanted.long_from,
        board: one(&wanted.board),
        door: one(&wanted.door),
        window: one(&wanted.window),
        footing: many(&wanted.footing),
        shakes: one(&wanted.shakes),
        ridge: one(&wanted.ridge),
        floor: one(&wanted.floor),
        stone: many(&wanted.stone),
        stone_half: many(&wanted.stone_half),
        stone_corner: many(&wanted.stone_corner),
    };
    loading.done = true;
    info!("The kits draw their pieces with {} models.", models.len());
    let pieces = (!models.is_empty()).then(|| Arc::new(pieces));
    look.kit_pieces = pieces.clone();
    for build in
        std::iter::once(&mut site.build).chain(builds.iter_mut().map(|b| &mut b.into_inner().0))
    {
        if build.kits.is_some() {
            build.kit_pieces = pieces.clone();
            redraw(&mut commands, build, &planet, &mut meshes);
        }
    }
}

/// Every kit's models as one set: lists joined, and of one model the first kit's to name
/// it.
fn merged(kits: &terra_build::logs::Kits) -> KitModels {
    let mut all = KitModels::default();
    for kit in &kits.0 {
        let m = &kit.models;
        all.logs.extend(m.logs.iter().cloned());
        all.footing.extend(m.footing.iter().cloned());
        all.stone.extend(m.stone.iter().cloned());
        all.stone_half.extend(m.stone_half.iter().cloned());
        all.stone_corner.extend(m.stone_corner.iter().cloned());
        for (into, from) in [
            (&mut all.long_log, &m.long_log),
            (&mut all.board, &m.board),
            (&mut all.door, &m.door),
            (&mut all.window, &m.window),
            (&mut all.shakes, &m.shakes),
            (&mut all.ridge, &m.ridge),
            (&mut all.floor, &m.floor),
        ] {
            if into.is_none() {
                into.clone_from(from);
            }
        }
        if all.long_from == 0.0 {
            all.long_from = m.long_from;
        }
    }
    all
}

/// How many of a kit's pieces are in, as it is shown.
pub(super) fn done(count: usize, shown: Shown) -> usize {
    match shown {
        Shown::Built => count,
        Shown::Plan(fraction) => ((count as f32 * fraction.clamp(0.0, 1.0)) as usize).min(count),
    }
}

/// A glTF's parts and the box round them, once it and its meshes have loaded.
fn resolve(
    path: &str,
    handle: &Handle<Gltf>,
    assets: &AssetServer,
    gltfs: &Assets<Gltf>,
    gltf_meshes: &Assets<GltfMesh>,
    meshes: &Assets<Mesh>,
) -> Option<Model> {
    let gltf = gltfs.get(handle)?;
    let mut parts = Vec::new();
    let (mut low, mut high) = (Vec3::MAX, Vec3::MIN);
    for mesh in &gltf.meshes {
        for primitive in &gltf_meshes.get(mesh)?.primitives {
            let aabb = meshes.get(&primitive.mesh)?.get_aabb()?;
            low = low.min(Vec3::from(aabb.min()));
            high = high.max(Vec3::from(aabb.max()));
            // The loader keeps each material as a standard one too, under its label and
            // "/std".
            let label = match primitive
                .material
                .as_ref()
                .and_then(|material| gltf.materials.iter().position(|each| each == material))
            {
                Some(index) => format!("Material{index}/std"),
                None => "DefaultMaterial/std".to_string(),
            };
            parts.push((
                primitive.mesh.clone(),
                assets.load(format!("{path}#{label}")),
            ));
        }
    }
    (!parts.is_empty()).then(|| Model {
        parts,
        centre: (low + high) * 0.5,
        size: (high - low).max(Vec3::splat(1e-3)),
    })
}

/// Places a copy of a model for each piece a model draws, in the order they go in: those
/// in drawn as the model is, the next lit and the rest a ghost. The copies are children of
/// the entity the build's other pieces are drawn on.
pub(super) fn place(commands: &mut Commands, build: &mut Build, laid: &[(&LogKit, &[Piece])]) {
    for entity in build.model_entities.drain(..) {
        commands.entity(entity).despawn();
    }
    let Some(kit) = build.kit_pieces.clone() else {
        return;
    };
    for &(own, pieces) in laid {
        place_kit(commands, build, &kit, pieces, own.diameter);
    }
}

fn place_kit(
    commands: &mut Commands,
    build: &mut Build,
    kit: &KitPieces,
    pieces: &[Piece],
    diameter: f32,
) {
    let done = done(pieces.len(), build.shown);
    for (index, piece) in pieces.iter().enumerate() {
        // A door or window is not part of the plan: it is fitted into its opening once
        // the building is up, so a plan or a building going up shows only the opening.
        if matches!(piece, Piece::Opening { .. }) && !matches!(build.shown, Shown::Built) {
            continue;
        }
        let state = index.cmp(&done);
        for (model, at) in fitted(kit, piece, diameter) {
            for (mesh, material) in &model.parts {
                let material = match state {
                    std::cmp::Ordering::Less => material.clone(),
                    std::cmp::Ordering::Equal => build.next_material.clone(),
                    std::cmp::Ordering::Greater => build.ghost_material.clone(),
                };
                let entity = commands
                    .spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material),
                        at,
                        ChildOf(build.pieces),
                    ))
                    .id();
                build.model_entities.push(entity);
            }
        }
    }
}

/// The models a piece is drawn with, and where each copy goes.
fn fitted<'a>(kit: &'a KitPieces, piece: &Piece, diameter: f32) -> Vec<(&'a Model, Transform)> {
    match *piece {
        Piece::Log {
            from,
            to,
            split,
            seed,
            ..
        } => {
            let length = from.distance(to);
            let model = match &kit.long_log {
                Some(long) if length > kit.long_from => Some(long),
                _ if kit.logs.is_empty() => None,
                _ => Some(&kit.logs[seed as usize % kit.logs.len()]),
            };
            model
                .map(|model| vec![(model, log(model, from, to, split, seed, diameter))])
                .unwrap_or_default()
        }
        Piece::Board { min, max } => kit
            .board
            .iter()
            .map(|model| (model, board(model, min, max)))
            .collect(),
        Piece::Opening {
            min,
            max,
            along_x,
            door,
        } => {
            let model = if door { &kit.door } else { &kit.window };
            model
                .iter()
                .map(|model| {
                    let x = if along_x { Vec3::X } else { Vec3::Z };
                    let size = max - min;
                    let width = if along_x { size.x } else { size.z };
                    let depth = if along_x { size.z } else { size.x };
                    (
                        model,
                        fill(
                            model,
                            (min + max) * 0.5,
                            x,
                            Vec3::Y,
                            Vec3::new(width, size.y, depth),
                        ),
                    )
                })
                .collect()
        }
        Piece::Footing { from, to, seed } => {
            if kit.footing.is_empty() {
                return Vec::new();
            }
            along(from, to)
                .into_iter()
                .enumerate()
                .map(|(step, (middle, x, length))| {
                    let model = &kit.footing[(seed as usize + step) % kit.footing.len()];
                    let centre = middle + Vec3::Y * (0.02 - FOOTING_HIGH_M * 0.5);
                    (
                        model,
                        fill(
                            model,
                            centre,
                            x,
                            Vec3::Y,
                            Vec3::new(length + 0.02, FOOTING_HIGH_M, FOOTING_DEEP_M),
                        ),
                    )
                })
                .collect()
        }
        Piece::Shakes {
            corners: [a, b, c, d],
        } => kit
            .shakes
            .iter()
            .map(|model| {
                // The panel's rows' lower edges, its +Z, down the slope.
                let across = b - a;
                let down = a - d;
                let up = across.cross(-down).normalize_or_zero();
                let up = if up.y < 0.0 { -up } else { up };
                let centre = (a + b + c + d) * 0.25 + up * (SHAKES_THICK_M * 0.5);
                (
                    model,
                    fill(
                        model,
                        centre,
                        up.cross(down.normalize_or_zero()),
                        up,
                        Vec3::new(across.length() + 0.04, SHAKES_THICK_M, down.length() + 0.04),
                    ),
                )
            })
            .collect(),
        Piece::Ridge { from, to } => kit
            .ridge
            .iter()
            .flat_map(|model| {
                along(from, to).into_iter().map(move |(middle, x, length)| {
                    (
                        model,
                        fill(
                            model,
                            middle,
                            x,
                            Vec3::Y,
                            Vec3::new(length + 0.02, model.size.y, model.size.z),
                        ),
                    )
                })
            })
            .collect(),
        Piece::Floor { min, max } => kit
            .floor
            .iter()
            .map(|model| {
                let size = max - min;
                let centre = (min + max) * 0.5 - Vec3::Y * (model.size.y * 0.5 - 0.01);
                (
                    model,
                    fill(
                        model,
                        centre,
                        Vec3::X,
                        Vec3::Y,
                        Vec3::new(size.x, model.size.y, size.z),
                    ),
                )
            })
            .collect(),
        Piece::Stone {
            shape,
            centre,
            size,
            along_x,
            seed,
        } => {
            let models = match shape {
                Stone::Wall => &kit.stone,
                Stone::Half => &kit.stone_half,
                Stone::Corner => &kit.stone_corner,
            };
            if models.is_empty() {
                return Vec::new();
            }
            let model = &models[seed as usize % models.len()];
            // A section is turned end for end, showing its other face; a block or a corner
            // any of four ways, as it is square.
            let turns = match shape {
                Stone::Wall => 2 * ((seed >> 8) % 2),
                Stone::Half | Stone::Corner => (seed >> 8) % 4,
            };
            let length = if along_x { Vec3::X } else { Vec3::Z };
            let x = Quat::from_rotation_y(turns as f32 * PI * 0.5) * length;
            let extents = if along_x {
                size
            } else {
                Vec3::new(size.z, size.y, size.x)
            };
            vec![(model, fill(model, centre, x, Vec3::Y, extents))]
        }
    }
}

/// A line cut into about metre lengths: the middle of each, the way along, and its length.
fn along(from: Vec3, to: Vec3) -> Vec<(Vec3, Vec3, f32)> {
    let length = from.distance(to);
    if length < 1e-3 {
        return Vec::new();
    }
    let x = (to - from) / length;
    let count = (length / STRETCH_M).round().max(1.0) as usize;
    let each = length / count as f32;
    (0..count)
        .map(|step| (from + x * (each * (step as f32 + 0.5)), x, each))
        .collect()
}

/// A model turned so its X, Y and Z lie along `x`, `y` and the way that makes them
/// right-handed, scaled to fill `extents` along them, its middle at `centre`.
fn fill(model: &Model, centre: Vec3, x: Vec3, y: Vec3, extents: Vec3) -> Transform {
    let x = x.normalize_or(Vec3::X);
    let y = (y - x * y.dot(x)).normalize_or(Vec3::Y);
    let z = x.cross(y);
    let rotation = Quat::from_mat3(&Mat3::from_cols(x, y, z));
    let scale = extents / model.size;
    Transform {
        translation: centre - rotation * (model.centre * scale),
        rotation,
        scale,
    }
}

/// A board filling its box: its length along the box's longest side, its thickness its
/// shortest.
fn board(model: &Model, min: Vec3, max: Vec3) -> Transform {
    let size = max - min;
    let mut axes = [(size.x, Vec3::X), (size.y, Vec3::Y), (size.z, Vec3::Z)];
    axes.sort_by(|a, b| b.0.total_cmp(&a.0));
    let [(long, x), (middle, _), (short, y)] = axes;
    fill(
        model,
        (min + max) * 0.5,
        x,
        y,
        Vec3::new(long, short, middle),
    )
}

/// Where a copy of a log goes from one point to another: its length along the log, as
/// thick as the kit's logs, turned about its length and end for end by its seed. The split
/// log at a wall's foot is a whole one sunk into the footing; the one on top, squashed to
/// its half.
fn log(model: &Model, from: Vec3, to: Vec3, split: Split, seed: u32, diameter: f32) -> Transform {
    let length = from.distance(to).max(1e-3);
    let axis = (to - from) / length;
    let thick = model.size.y.max(model.size.z);
    let mut scale = Vec3::new(length / model.size.x, diameter / thick, diameter / thick);
    let flip = seed & 0x10000 != 0;
    let mut spin = (seed & 0xffff) as f32 / 65536.0 * TAU;
    let mut middle = (from + to) * 0.5;
    if split == Split::Bottom {
        scale.y *= 0.5;
        middle.y -= diameter * 0.25;
        spin = 0.0;
    }
    let turn = Quat::from_rotation_arc(Vec3::X, axis)
        * Quat::from_rotation_x(spin)
        * Quat::from_rotation_y(if flip { PI } else { 0.0 });
    Transform {
        translation: middle - turn * (model.centre * scale),
        rotation: turn,
        scale,
    }
}
