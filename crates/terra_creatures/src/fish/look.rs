//! How a fish is drawn: its model's own textures (D14), swum by a wave down its body in
//! the vertex shader (`assets/shaders/fish.wesl`), so a school of thirty is thirty
//! instances of one mesh in one material, with no skeleton and nothing animated here.

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use terra_survival::props::{textured_material, textured_mesh};
use terra_world::models::read_model;

/// The swimming a fish's material adds. Field order must match `Swim` in fish.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Swim {
    /// Where its nose is along the model's z, how long the model is, how many strokes a
    /// second its tail beats, and how far it swings, as a share of its length.
    #[uniform(100)]
    pub body: Vec4,
}

impl MaterialExtension for Swim {
    fn vertex_shader() -> ShaderRef {
        "shaders/fish.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/fish.wesl".into()
    }
}

pub type FishMaterial = ExtendedMaterial<StandardMaterial, Swim>;

/// A kind's look: its mesh, its material, and how long the model is as made, which a fish
/// of another length is scaled from.
pub struct Look {
    pub mesh: Handle<Mesh>,
    pub material: Handle<FishMaterial>,
    pub long_m: f32,
}

/// The look of the model in `assets/models/<file>`, beating so many strokes a second and
/// swinging so far: its first textured part, facing +Z, as `tools/props` makes it (D303).
pub fn look(
    file: &str,
    (stroke_hz, bend): (f32, f32),
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<FishMaterial>,
) -> Result<Look, String> {
    let parts = read_model(file)?;
    let (part, textures) = parts
        .iter()
        .find_map(|part| Some((part, part.textures.as_ref()?)))
        .ok_or_else(|| format!("{file} has no textured part: make it through tools/props."))?;
    let (nose, tail) = part
        .positions
        .iter()
        .fold((f32::MIN, f32::MAX), |(nose, tail), at| {
            (nose.max(at.z), tail.min(at.z))
        });
    let long_m = nose - tail;
    if long_m <= 0.0 {
        return Err(format!("{file} has no length along its z."));
    }
    let base = textured_material(&format!("models/{file}"), textures, None, assets);
    Ok(Look {
        mesh: meshes.add(textured_mesh(part, textures)),
        material: materials.add(FishMaterial {
            base,
            extension: Swim {
                body: Vec4::new(nose, long_m, stroke_hz, bend),
            },
        }),
        long_m,
    })
}
