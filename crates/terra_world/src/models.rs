//! Reading the shapes out of model files.
//!
//! A part is the triangles that share a material. The free models (see THIRD_PARTY.md) are
//! flat-coloured: each part's material has one colour and no textures, and the game may
//! choose the colour by the material's name. A model made through `tools/props` keeps its
//! textures (D14): its parts say where on them each point is and which of the file's
//! textures their material draws, for the game to load and draw them.

use bevy::{
    asset::RenderAssetUsages,
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
    /// Where on its textures each point is, when its material has any.
    pub uvs: Option<Vec<Vec2>>,
    /// Each point's tangent, which a normal map needs, when the file has them.
    pub tangents: Option<Vec<Vec4>>,
    /// Its material's textures, when it has any.
    pub textures: Option<Textures>,
}

/// A part's material when it is textured: which of the file's textures it draws for each
/// map, by their index in the file, as Bevy's glTF loader labels them
/// (`GltfAssetLabel::Texture`), and the factors they are scaled by.
#[derive(Clone, Debug, PartialEq)]
pub struct Textures {
    pub colour: usize,
    pub normal: Option<usize>,
    /// Roughness in green and metalness in blue, as glTF packs them.
    pub metallic_roughness: Option<usize>,
    pub occlusion: Option<usize>,
    /// What glows of it, as a burning log's cracks do: drawn only by what is told how bright
    /// its fire burns now (the game's burning props), dark otherwise.
    pub emissive: Option<usize>,
    pub metallic: f32,
    pub roughness: f32,
    /// Drawn from both sides, as a thin cloth or an open shell must be.
    pub double_sided: bool,
}

/// The textures a material draws from the first set of texture places, if it has a colour
/// texture there.
fn textures_of(material: &gltf::Material) -> Option<Textures> {
    let first = |index: usize, place: u32| (place == 0).then_some(index);
    let pbr = material.pbr_metallic_roughness();
    let colour = pbr
        .base_color_texture()
        .and_then(|info| first(info.texture().index(), info.tex_coord()))?;
    Some(Textures {
        colour,
        normal: material
            .normal_texture()
            .and_then(|info| first(info.texture().index(), info.tex_coord())),
        metallic_roughness: pbr
            .metallic_roughness_texture()
            .and_then(|info| first(info.texture().index(), info.tex_coord())),
        occlusion: material
            .occlusion_texture()
            .and_then(|info| first(info.texture().index(), info.tex_coord())),
        emissive: material
            .emissive_texture()
            .and_then(|info| first(info.texture().index(), info.tex_coord())),
        metallic: pbr.metallic_factor(),
        roughness: pbr.roughness_factor(),
        double_sided: material.double_sided(),
    })
}

/// What names a socket in a model's file: an empty, a node with no mesh, called
/// `socket.<name>` (docs/art-style.md, the model contract).
pub const SOCKET: &str = "socket.";

/// A model's sockets, the named points code attaches to (a door's hinge, a chair's seat),
/// from a file in `assets/models`, by name, in the model's own space.
pub fn read_sockets(file: &str) -> Result<Vec<(String, Vec3)>, String> {
    let path = crate::data::assets().join("models").join(file);
    let problem = |what: String| format!("{}: {what}", path.display());
    let bytes = std::fs::read(&path).map_err(|error| problem(error.to_string()))?;
    let gltf = gltf::Gltf::from_slice(&bytes).map_err(|error| problem(error.to_string()))?;
    let mut sockets = Vec::new();
    let mut waiting: Vec<(gltf::Node, Mat4)> = gltf
        .scenes()
        .flat_map(|scene| scene.nodes())
        .map(|node| (node, Mat4::IDENTITY))
        .collect();
    while let Some((node, parent)) = waiting.pop() {
        let placed = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        waiting.extend(node.children().map(|child| (child, placed)));
        if node.mesh().is_none()
            && let Some(name) = node.name().and_then(|name| name.strip_prefix(SOCKET))
        {
            sockets.push((name.to_string(), placed.transform_point3(Vec3::ZERO)));
        }
    }
    sockets.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(sockets)
}

/// Reads a model's triangles from a file in `assets/models`, with +Y up and at the size
/// its file says, whatever axes and scale the file is stored in.
pub fn read_model(file: &str) -> Result<Vec<Part>, String> {
    let path = crate::data::assets().join("models").join(file);
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
            let material = primitive.material();
            let [red, green, blue, _] = material.pbr_metallic_roughness().base_color_factor();
            let uvs: Option<Vec<Vec2>> = reader
                .read_tex_coords(0)
                .map(|uvs| uvs.into_f32().map(Vec2::from).collect());
            // Only a part with places on its textures can draw them.
            let textures = uvs.as_ref().and_then(|_| textures_of(&material));
            let along = Mat3::from_mat4(placed);
            let tangents = textures.as_ref().and_then(|_| {
                reader.read_tangents().map(|tangents| {
                    tangents
                        .map(|[x, y, z, w]| {
                            (along * Vec3::new(x, y, z)).normalize_or(Vec3::X).extend(w)
                        })
                        .collect()
                })
            });
            parts.push(Part {
                material: material.name().unwrap_or("").to_string(),
                colour: Color::linear_rgb(red, green, blue),
                positions,
                normals,
                indices,
                uvs: textures.as_ref().and(uvs),
                tangents,
                textures,
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
