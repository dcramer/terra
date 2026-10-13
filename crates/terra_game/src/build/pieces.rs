//! A kit's props drawn from their models (D177; docs/building-kits.md): the layout says
//! where each door and window goes, and a copy of its model is fitted to its opening. Long
//! members and boards are made from the kit's numbers (`terra_build::timber`, D200) and
//! block kinds are their cubes' surface (`terra_build::surface`, D197), so neither is here.

use super::{BuildLook, Built, Site};
use bevy::asset::LoadState;
use bevy::camera::primitives::MeshAabb;
use bevy::gltf::{Gltf, GltfMesh};
use bevy::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;
use terra_build::kit::{KitModels, Kits};
use terra_build::layout::{Piece, Place};

/// A model, ready to place: each of its parts, and the box round it all, in its own space;
/// for a door cut into its frame and its leaf (`tools/props/split.py`), which parts are the
/// leaf, and the box round the leaf, whose low edge across is its hinge.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    parts: Vec<(Handle<Mesh>, Handle<StandardMaterial>, bool)>,
    centre: Vec3,
    size: Vec3,
    leaf: Option<(Vec3, Vec3)>,
}

/// What a door's leaf, which swings, is called in its model; the rest of it is its frame.
const LEAF: &str = "Leaf";

/// A kit's props' models, those that have loaded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KitPieces {
    door: Option<Model>,
    window: Option<Model>,
}

/// The models being loaded, by path, for the kit's models as they were asked for.
#[derive(Resource, Default)]
pub(super) struct Loading {
    wanted: Option<KitModels>,
    handles: BTreeMap<String, Handle<Gltf>>,
    done: bool,
}

/// Loads the kits' models when they name them, and when they are ready draws every
/// build with them.
#[allow(clippy::too_many_arguments)]
pub(super) fn load(
    mut loading: ResMut<Loading>,
    mut look: ResMut<BuildLook>,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
    meshes: Res<Assets<Mesh>>,
) {
    let wanted = look.kits.as_deref().map(merged).unwrap_or_default();
    if loading.wanted.as_ref() != Some(&wanted) {
        let paths = wanted.door.iter().chain(wanted.window.iter());
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
    let pieces = KitPieces {
        door: one(&wanted.door),
        window: one(&wanted.window),
    };
    loading.done = true;
    info!("The kits draw their pieces with {} models.", models.len());
    let pieces = (!models.is_empty()).then(|| Arc::new(pieces));
    look.kit_pieces = pieces.clone();
    for build in super::store::each(&mut site, &mut builds) {
        if build.kits.is_some() {
            build.redraw_all();
        }
    }
}

/// Every kit's models as one set: of each, the first kit's to name it.
fn merged(kits: &Kits) -> KitModels {
    let mut all = KitModels::default();
    for kit in &kits.0 {
        let m = &kit.models;
        for (into, from) in [(&mut all.door, &m.door), (&mut all.window, &m.window)] {
            if into.is_none() {
                into.clone_from(from);
            }
        }
    }
    all
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
    let mut leaf: Option<(Vec3, Vec3)> = None;
    for mesh in &gltf.meshes {
        let gltf_mesh = gltf_meshes.get(mesh)?;
        let is_leaf = gltf_mesh.name == LEAF;
        for primitive in &gltf_mesh.primitives {
            let aabb = meshes.get(&primitive.mesh)?.get_aabb()?;
            let (min, max) = (Vec3::from(aabb.min()), Vec3::from(aabb.max()));
            low = low.min(min);
            high = high.max(max);
            if is_leaf {
                leaf = Some(leaf.map_or((min, max), |(l, h)| (l.min(min), h.max(max))));
            }
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
                is_leaf,
            ));
        }
    }
    (!parts.is_empty()).then(|| Model {
        parts,
        centre: (low + high) * 0.5,
        size: (high - low).max(Vec3::splat(1e-3)),
        leaf,
    })
}

/// A door's or window's parts, fitted to the opening a piece is, each with its material and
/// where it goes in the base's own space.
pub(super) fn fitted_parts(
    kit: &KitPieces,
    piece: &Piece,
) -> Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)> {
    fitted(kit, piece)
        .into_iter()
        .flat_map(|(model, at)| {
            model
                .parts
                .iter()
                .map(move |(mesh, material, _)| (mesh.clone(), material.clone(), at))
        })
        .collect()
}

/// The kit's door or window fitted to an opening's box, in the base's own space: each
/// part's mesh and material, and where it goes. A door's leaf is turned `swing` radians
/// about the up axis, on its hinge, the low edge of the leaf across the opening; its frame,
/// and a door with no leaf of its own, stay as they are fitted.
pub(super) fn opening(
    kit: &KitPieces,
    min: Vec3,
    max: Vec3,
    along_x: bool,
    door: bool,
    swing: f32,
) -> Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)> {
    let piece = Piece::Opening {
        min: Place::near(min),
        max: Place::near(max),
        along_x,
        door,
    };
    fitted(kit, &piece)
        .into_iter()
        .flat_map(|(model, at)| {
            let turned = model.leaf.map_or(at, |(low, high)| {
                let hinge = at.transform_point(Vec3::new(low.x, low.y, (low.z + high.z) * 0.5));
                Transform::from_translation(hinge)
                    * Transform::from_rotation(Quat::from_rotation_y(swing))
                    * Transform::from_translation(-hinge)
                    * at
            });
            model.parts.iter().map(move |(mesh, material, leaf)| {
                (
                    mesh.clone(),
                    material.clone(),
                    if *leaf { turned } else { at },
                )
            })
        })
        .collect()
}

/// The model a door or window is drawn with, and where it goes.
fn fitted<'a>(kit: &'a KitPieces, piece: &Piece) -> Vec<(&'a Model, Transform)> {
    let Piece::Opening {
        min,
        max,
        along_x,
        door,
    } = *piece
    else {
        return Vec::new();
    };
    let model = if door { &kit.door } else { &kit.window };
    model
        .iter()
        .map(|model| {
            let x = if along_x { Vec3::X } else { Vec3::Z };
            let (min, max) = (min.metres(), max.metres());
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
