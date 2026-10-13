//! A kit's pieces drawn from its models (D177; docs/building-kits.md): the layout says where each
//! log goes, and a copy of the kit's log model is placed there, stretched along its length
//! to fit and turned about it and end for end by the log's seed, so neighbours differ.
//! Pieces not yet in are the same model in the ghost's material, and the next one lit.

use super::{Build, BuildLook, Built, Shown, Site, redraw};
use crate::planet::Planet;
use bevy::camera::primitives::MeshAabb;
use bevy::gltf::{Gltf, GltfMesh};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};
use terra_build::logs::{LogKit, Piece, Split};

/// A kit's log model, ready to place: each of its parts, and the box round it all, in its
/// own space, its length along X.
#[derive(Clone, Debug, PartialEq)]
pub struct LogModel {
    pub path: String,
    parts: Vec<(Handle<Mesh>, Handle<StandardMaterial>)>,
    centre: Vec3,
    size: Vec3,
}

/// The log model being loaded, by its path.
#[derive(Resource, Default)]
pub(super) struct Loading(Option<(String, Handle<Gltf>)>);

/// Loads the log kit's model when the kit names one, and when it is ready draws every
/// build with it.
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
    let wanted = look.logs.as_ref().and_then(|kit| kit.log_model.clone());
    if wanted.as_deref() != loading.0.as_ref().map(|(path, _)| path.as_str()) {
        loading.0 = wanted.map(|path| (path.clone(), assets.load(path)));
        if loading.0.is_none() && look.log_model.is_some() {
            look.log_model = None;
            set(
                None,
                &mut commands,
                &planet,
                &mut site,
                &mut builds,
                &mut meshes,
            );
        }
        return;
    }
    let Some((path, handle)) = &loading.0 else {
        return;
    };
    if look
        .log_model
        .as_ref()
        .is_some_and(|model| &model.path == path)
    {
        return;
    }
    let Some(gltf) = gltfs.get(handle) else {
        return;
    };
    let mut parts = Vec::new();
    let (mut low, mut high) = (Vec3::MAX, Vec3::MIN);
    for primitive in gltf
        .meshes
        .iter()
        .filter_map(|mesh| gltf_meshes.get(mesh))
        .flat_map(|mesh| &mesh.primitives)
    {
        let Some(aabb) = meshes
            .get(&primitive.mesh)
            .and_then(|mesh| mesh.compute_aabb())
        else {
            return;
        };
        low = low.min(Vec3::from(aabb.min()));
        high = high.max(Vec3::from(aabb.max()));
        // The loader keeps each material as a standard one too, under its label and "/std".
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
    if parts.is_empty() {
        return;
    }
    info!("The log kit draws its logs with {path}.");
    let model = LogModel {
        path: path.clone(),
        parts,
        centre: (low + high) * 0.5,
        size: high - low,
    };
    look.log_model = Some(model.clone());
    set(
        Some(model),
        &mut commands,
        &planet,
        &mut site,
        &mut builds,
        &mut meshes,
    );
}

/// Draws every build with a log model, or without one.
fn set(
    model: Option<LogModel>,
    commands: &mut Commands,
    planet: &Planet,
    site: &mut Site,
    builds: &mut Query<&mut Built>,
    meshes: &mut Assets<Mesh>,
) {
    for build in
        std::iter::once(&mut site.build).chain(builds.iter_mut().map(|b| &mut b.into_inner().0))
    {
        if build.logs.is_some() {
            build.log_model = model.clone();
            redraw(commands, build, planet, meshes);
        }
    }
}

/// Places a copy of the log model for each log among a build's pieces, in the order they
/// go in: those in drawn as the model is, the next lit and the rest a ghost. The copies
/// are children of the entity the build's other pieces are drawn on.
pub(super) fn place(commands: &mut Commands, build: &mut Build, pieces: &[Piece], kit: &LogKit) {
    for entity in build.log_entities.drain(..) {
        commands.entity(entity).despawn();
    }
    let Some(model) = &build.log_model else {
        return;
    };
    let done = match build.shown {
        Shown::Built => pieces.len(),
        Shown::Plan(fraction) => {
            ((pieces.len() as f32 * fraction.clamp(0.0, 1.0)) as usize).min(pieces.len())
        }
    };
    for (index, piece) in pieces.iter().enumerate() {
        let Piece::Log {
            from,
            to,
            split,
            seed,
            ..
        } = *piece
        else {
            continue;
        };
        let at = fit(model, from, to, split, seed, kit.diameter);
        for (mesh, material) in &model.parts {
            let material = if index < done {
                material.clone()
            } else if index == done {
                build.next_material.clone()
            } else {
                build.ghost_material.clone()
            };
            let entity = commands
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    at,
                    ChildOf(build.pieces),
                ))
                .id();
            build.log_entities.push(entity);
        }
    }
}

/// Where a copy of the model goes for a log from one point to another: its length along
/// the log, as thick as the kit's logs, turned about its length and end for end by its
/// seed. A split log is the model squashed to its half, flat side to the wall's foot or
/// top.
fn fit(
    model: &LogModel,
    from: Vec3,
    to: Vec3,
    split: Split,
    seed: u32,
    diameter: f32,
) -> Transform {
    let length = from.distance(to).max(1e-3);
    let axis = (to - from) / length;
    let thick = model.size.y.max(model.size.z).max(1e-3);
    let mut scale = Vec3::new(
        length / model.size.x.max(1e-3),
        diameter / thick,
        diameter / thick,
    );
    let (spin, flip) = match split {
        Split::Whole => ((seed & 0xffff) as f32 / 65536.0 * TAU, seed & 0x10000 != 0),
        _ => (0.0, seed & 0x10000 != 0),
    };
    let mut middle = (from + to) * 0.5;
    match split {
        Split::Whole => {}
        Split::Top => {
            scale.y *= 0.5;
            middle.y += diameter * 0.25;
        }
        Split::Bottom => {
            scale.y *= 0.5;
            middle.y -= diameter * 0.25;
        }
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
