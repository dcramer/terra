//! Reading the shapes out of model files.
//!
//! The free models used here (see THIRD_PARTY.md) are flat-coloured: each part has one
//! material with one colour and no textures. We read the triangles and the material's
//! name and choose the colours ourselves, so everything in a scene shares one palette.

use bevy::{asset::io::file::FileAssetReader, prelude::*};

/// One part of a model: the triangles that share a material.
pub struct Part {
    pub material: String,
    /// The material's own colour, as it should look on screen.
    pub colour: Color,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub indices: Vec<u32>,
}

/// Reads a model's triangles from a file in `assets/models`, with +Y up and at the size
/// its file says, whatever axes and scale the file is stored in.
pub fn read_model(file: &str) -> Result<Vec<Part>, String> {
    let path = FileAssetReader::get_base_path()
        .join("assets/models")
        .join(file);
    let problem = |what: String| format!("{}: {what}", path.display());
    let bytes = std::fs::read(&path).map_err(|error| problem(error.to_string()))?;
    let gltf = gltf::Gltf::from_slice(&bytes).map_err(|error| problem(error.to_string()))?;
    let blob = gltf.blob.as_deref();
    let mut parts = Vec::new();
    let mut waiting: Vec<(gltf::Node, Mat4)> = gltf
        .scenes()
        .flat_map(|scene| scene.nodes())
        .map(|node| (node, Mat4::IDENTITY))
        .collect();
    while let Some((node, parent)) = waiting.pop() {
        let placed = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        waiting.extend(node.children().map(|child| (child, placed)));
        let Some(mesh) = node.mesh() else { continue };
        let turned = Mat3::from_mat4(placed).inverse().transpose();
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|_| blob);
            let positions: Vec<Vec3> = reader
                .read_positions()
                .ok_or_else(|| problem("a mesh has no positions".into()))?
                .map(|p| placed.transform_point3(Vec3::from(p)))
                .collect();
            let normals: Vec<Vec3> = reader
                .read_normals()
                .ok_or_else(|| problem("a mesh has no normals".into()))?
                .map(|n| (turned * Vec3::from(n)).normalize_or(Vec3::Y))
                .collect();
            let indices = reader
                .read_indices()
                .map(|indices| indices.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());
            let [red, green, blue, _] = primitive
                .material()
                .pbr_metallic_roughness()
                .base_color_factor();
            parts.push(Part {
                material: primitive.material().name().unwrap_or("").to_string(),
                colour: Color::linear_rgb(red, green, blue),
                positions,
                normals,
                indices,
            });
        }
    }
    Ok(parts)
}
