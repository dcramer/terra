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

/// A part made from one of Bevy's shapes rather than read from a file, placed by `at`.
pub fn piece(mesh: Mesh, at: Transform, material: &str, colour: Color) -> Part {
    let mesh = mesh.transformed_by(at);
    let float3 = |attribute| -> Vec<Vec3> {
        mesh.attribute(attribute)
            .and_then(|values| values.as_float3())
            .map(|all| all.iter().map(|&p| Vec3::from(p)).collect())
            .unwrap_or_default()
    };
    Part {
        material: material.to_string(),
        colour,
        positions: float3(Mesh::ATTRIBUTE_POSITION),
        normals: float3(Mesh::ATTRIBUTE_NORMAL),
        indices: mesh
            .indices()
            .map(|indices| indices.iter().map(|index| index as u32).collect())
            .unwrap_or_default(),
    }
}

/// Models shaped here from simple pieces, for things no free model covers. An item names
/// one with a file of `shaped:<name>`.
pub fn shaped(name: &str) -> Option<Vec<Part>> {
    match name {
        "forge" => Some(forge()),
        "smelter" => Some(smelter()),
        _ => None,
    }
}

/// A forge: a stone hearth with glowing coals under a hood, and an anvil on a stump beside
/// it. About two metres across. A stand-in until there is a model of one.
fn forge() -> Vec<Part> {
    let stone = Color::srgb(0.5, 0.48, 0.46);
    let dark = Color::srgb(0.3, 0.29, 0.29);
    let iron = Color::srgb(0.22, 0.23, 0.26);
    let wood = Color::srgb(0.45, 0.31, 0.2);
    let at = Transform::from_xyz;
    vec![
        // The hearth, its rim, and the coals in it.
        piece(
            Cuboid::new(1.2, 0.8, 0.9).into(),
            at(-0.35, 0.4, 0.0),
            "",
            stone,
        ),
        piece(
            Cuboid::new(1.2, 0.12, 0.12).into(),
            at(-0.35, 0.86, 0.39),
            "",
            dark,
        ),
        piece(
            Cuboid::new(1.2, 0.12, 0.12).into(),
            at(-0.35, 0.86, -0.39),
            "",
            dark,
        ),
        piece(
            Cuboid::new(0.9, 0.06, 0.62).into(),
            at(-0.35, 0.83, 0.0),
            "Fire",
            stone,
        ),
        // The hood and its chimney.
        piece(
            Cone::new(0.62, 0.6).mesh().resolution(4).into(),
            at(-0.35, 1.55, 0.0).with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4)),
            "",
            dark,
        ),
        piece(
            Cuboid::new(0.3, 1.0, 0.3).into(),
            at(-0.35, 2.1, 0.0),
            "",
            dark,
        ),
        piece(
            Cuboid::new(0.1, 0.75, 0.1).into(),
            at(-0.85, 1.2, 0.35),
            "",
            wood,
        ),
        piece(
            Cuboid::new(0.1, 0.75, 0.1).into(),
            at(0.15, 1.2, 0.35),
            "",
            wood,
        ),
        // The anvil on its stump.
        piece(
            Cylinder::new(0.22, 0.5).mesh().resolution(8).into(),
            at(0.75, 0.25, 0.0),
            "",
            wood,
        ),
        piece(
            Cuboid::new(0.24, 0.12, 0.2).into(),
            at(0.75, 0.56, 0.0),
            "",
            iron,
        ),
        piece(
            Cuboid::new(0.5, 0.12, 0.2).into(),
            at(0.78, 0.68, 0.0),
            "",
            iron,
        ),
        piece(
            Cone::new(0.06, 0.18).mesh().resolution(4).into(),
            at(1.11, 0.68, 0.0).with_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)),
            "",
            iron,
        ),
    ]
}

/// A smelter: a squat stone furnace, wider at the foot, with a glowing mouth and a short
/// chimney. A stand-in until there is a model of one.
fn smelter() -> Vec<Part> {
    let stone = Color::srgb(0.52, 0.5, 0.47);
    let dark = Color::srgb(0.32, 0.3, 0.3);
    let at = Transform::from_xyz;
    let square = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
    vec![
        piece(
            Cylinder::new(0.8, 0.5).mesh().resolution(4).into(),
            at(0.0, 0.25, 0.0).with_rotation(square),
            "",
            stone,
        ),
        piece(
            ConicalFrustum {
                radius_top: 0.42,
                radius_bottom: 0.68,
                height: 1.1,
            }
            .mesh()
            .resolution(4)
            .into(),
            at(0.0, 1.05, 0.0).with_rotation(square),
            "",
            stone,
        ),
        piece(
            Cuboid::new(0.36, 0.7, 0.36).into(),
            at(0.0, 1.95, 0.0),
            "",
            dark,
        ),
        piece(
            Cuboid::new(0.4, 0.3, 0.06).into(),
            at(0.0, 0.75, 0.47),
            "Fire",
            stone,
        ),
    ]
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
