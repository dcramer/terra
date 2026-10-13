//! Reading the shapes out of model files.
//!
//! The free models used here (see THIRD_PARTY.md) are flat-coloured: each part has one
//! material with one colour and no textures. We read the triangles and the material's
//! name and choose the colours ourselves, so everything in a scene shares one palette.

use bevy::{
    asset::{RenderAssetUsages, io::file::FileAssetReader},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

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

/// Bakes a model's parts into one mesh along +Y, scaled to its length, with the place it
/// is held at the origin, as an arrow in flight is drawn. Grey parts, the metal, take the head's colour if it has one.
pub fn bake(parts: &[Part], length: f32, held_from_end: f32, head: Option<Color>) -> Mesh {
    let all = || parts.iter().flat_map(|part| part.positions.iter().copied());
    let low = all().fold(Vec3::MAX, Vec3::min);
    let high = all().fold(Vec3::MIN, Vec3::max);
    let scale = length / (high.y - low.y).max(1e-3);
    let foot = Vec3::new((low.x + high.x) * 0.5, low.y, (low.z + high.z) * 0.5);
    let (mut positions, mut normals, mut colours, mut indices) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for part in parts {
        let own = part.colour.to_srgba();
        let greyish = (own.red - own.green).abs() < 0.08 && (own.green - own.blue).abs() < 0.08;
        let colour = match head {
            Some(head) if greyish && own.red > 0.25 => head,
            _ => part.colour,
        };
        let first = positions.len() as u32;
        positions.extend(
            part.positions
                .iter()
                .map(|&p| (p - foot) * scale - Vec3::Y * held_from_end),
        );
        normals.extend(part.normals.iter().copied());
        colours.extend(std::iter::repeat_n(
            colour.to_linear().to_f32_array(),
            part.positions.len(),
        ));
        indices.extend(part.indices.iter().map(|index| first + index));
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(Indices::U32(indices))
}
