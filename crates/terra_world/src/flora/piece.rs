//! A model of the flora stood somewhere by hand, not grown (a fallen stone in a dungeon),
//! drawn as where it grows: its mesh in our colours and real materials
//! (`model::build_model`) in the flora material, so a rock reads as the same stone indoors
//! as out. Round 18 found the dungeon's rocks drawn from their models as shipped, flat
//! grey without a texture, white lumps under the torches.

use super::super::flora_kinds::Flora;
use super::super::models::read_model;
use super::model::{Drawn, build_model};
use super::{WindMaterial, matte, wind_extension};
use bevy::{pbr::ExtendedMaterial, platform::collections::HashMap, prelude::*};

/// Draws this entity as kind `kind` of the flora (its place in `assets/data/flora.ron`)
/// draws its model `model` (`models/nature/<model>.glb`): `Flora::drawn_as` finds both from
/// a model's path.
#[derive(Component, Clone, Debug)]
pub struct FloraPiece {
    pub kind: usize,
    pub model: String,
}

/// Each piece's mesh, built once a kind and model, and the one material they share: the
/// world's, so its handles go with it (D469).
#[derive(Resource, Default)]
pub(super) struct Built {
    meshes: HashMap<(usize, String), Option<Handle<Mesh>>>,
    material: Option<Handle<WindMaterial>>,
}

/// Gives each new piece its mesh and the flora material.
pub(super) fn draw(
    mut commands: Commands,
    added: Query<(Entity, &FloraPiece), Added<FloraPiece>>,
    flora: Option<Res<Flora>>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WindMaterial>>,
    mut built: ResMut<Built>,
) {
    let Some(flora) = flora else {
        return;
    };
    for (entity, piece) in &added {
        let Some(kind) = flora.kinds().get(piece.kind) else {
            error!("{piece:?} names no kind of the flora");
            continue;
        };
        let mesh = built
            .meshes
            .entry((piece.kind, piece.model.clone()))
            .or_insert_with(
                || match read_model(&format!("nature/{}.glb", piece.model)) {
                    Ok(parts) => match build_model(kind, &parts, &mut meshes).look {
                        Drawn::Mesh(mesh) => Some(mesh),
                        Drawn::Grown(_) => None,
                    },
                    Err(problem) => {
                        error!("Could not read {piece:?}. {problem}");
                        None
                    }
                },
            )
            .clone();
        let Some(mesh) = mesh else {
            continue;
        };
        // Still pieces only (a rock's every vertex is unmoved by the wind), so which way it
        // blows matters not; the world's wind sets it for every flora material anyway.
        let material = built
            .material
            .get_or_insert_with(|| {
                materials.add(ExtendedMaterial {
                    base: matte(Some(bevy::render::render_resource::Face::Back)),
                    extension: wind_extension(&assets, Vec3::X),
                })
            })
            .clone();
        commands
            .entity(entity)
            .insert((Mesh3d(mesh), MeshMaterial3d(material)));
    }
}
