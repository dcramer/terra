//! What is in hand, on the figure: the tool in the picked slot of the bar, as its own
//! model in the right hand, swapped as soon as another slot is picked.
//!
//! The tools are free models (see THIRD_PARTY.md) in their own colours, baked once into a
//! mesh each. There is no free pickaxe, so it is made here from a few faceted pieces;
//! stone and iron tools differ by the colour of their heads.

use super::inventory::{Belongings, Tool};
use super::models::{Part, read_model};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

/// Where the figure's figure.rs hangs things in its right hand. Held tools are its
/// children.
#[derive(Component)]
pub struct Grip;

/// One tool's look: its mesh, how long it is, and how far up its handle it is held.
struct Look {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct Looks {
    tools: Vec<(Tool, Look)>,
    /// What is shown in hand now, so it changes only when the tool does.
    showing: Option<Option<Tool>>,
}

/// A tool shown in hand.
#[derive(Component)]
struct Shown;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load).add_systems(Update, show);
}

/// A tool's model, how long it is drawn, and how far from its end it is held.
fn about(tool: Tool) -> (Option<&'static str>, f32, f32) {
    match tool {
        Tool::Hammer => (Some("tools/Hammer_Small.glb"), 0.42, 0.06),
        Tool::StoneAxe | Tool::IronAxe => (Some("tools/Axe.glb"), 0.68, 0.08),
        Tool::Knife => (Some("tools/Knife.glb"), 0.26, 0.05),
        Tool::Spear => (Some("tools/Spear.glb"), 1.8, 0.75),
        Tool::StonePickaxe | Tool::IronPickaxe => (None, 0.72, 0.08),
    }
}

fn load(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material = materials.add(StandardMaterial {
        perceptual_roughness: 0.7,
        reflectance: 0.3,
        ..default()
    });
    let mut tools = Vec::new();
    for tool in Tool::ALL {
        let (file, length, held_from_end) = about(tool);
        let parts = match file {
            Some(file) => match read_model(file) {
                Ok(parts) => parts,
                Err(problem) => {
                    error!("Could not read a tool. {problem}");
                    continue;
                }
            },
            None => pickaxe(tool == Tool::IronPickaxe),
        };
        let mesh = bake(&parts, length, held_from_end, head_colour(tool));
        tools.push((
            tool,
            Look {
                mesh: meshes.add(mesh),
                material: material.clone(),
            },
        ));
    }
    commands.insert_resource(Looks {
        tools,
        showing: None,
    });
}

/// The colour of the working end, where a tool's material shows: stone or iron.
fn head_colour(tool: Tool) -> Option<Color> {
    match tool {
        Tool::StoneAxe | Tool::StonePickaxe => Some(Color::srgb(0.55, 0.53, 0.5)),
        Tool::IronAxe | Tool::IronPickaxe => Some(Color::srgb(0.38, 0.42, 0.48)),
        _ => None,
    }
}

/// Bakes a tool's parts into one mesh along +Y, scaled to its length, with the place it
/// is held at the origin. Grey parts, the metal, take the head's colour if it has one.
fn bake(parts: &[Part], length: f32, held_from_end: f32, head: Option<Color>) -> Mesh {
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

/// A pickaxe from a few faceted pieces: a handle, and a head that tapers to two points.
fn pickaxe(iron: bool) -> Vec<Part> {
    let wood = Color::srgb(0.48, 0.32, 0.2);
    let head = if iron {
        Color::srgb(0.6, 0.6, 0.62)
    } else {
        Color::srgb(0.6, 0.58, 0.55)
    };
    let piece = |mesh: Mesh, at: Transform, colour: Color| {
        let mesh = mesh.transformed_by(at);
        let positions: Vec<Vec3> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(values) => values
                .as_float3()
                .map(|all| all.iter().map(|&p| Vec3::from(p)).collect())
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let normals: Vec<Vec3> = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(values) => values
                .as_float3()
                .map(|all| all.iter().map(|&n| Vec3::from(n)).collect())
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let indices = match mesh.indices() {
            Some(indices) => indices.iter().map(|index| index as u32).collect(),
            None => Vec::new(),
        };
        Part {
            material: String::new(),
            colour,
            positions,
            normals,
            indices,
        }
    };
    vec![
        piece(
            Cylinder::new(0.025, 1.0).mesh().resolution(6).into(),
            Transform::from_xyz(0.0, 0.5, 0.0),
            wood,
        ),
        piece(
            Cuboid::new(0.12, 0.08, 0.07).into(),
            Transform::from_xyz(0.0, 0.95, 0.0),
            head,
        ),
        piece(
            Cone::new(0.04, 0.32).mesh().resolution(4).into(),
            Transform::from_xyz(0.2, 0.93, 0.0).with_rotation(Quat::from_rotation_z(-1.75)),
            head,
        ),
        piece(
            Cone::new(0.04, 0.32).mesh().resolution(4).into(),
            Transform::from_xyz(-0.2, 0.93, 0.0).with_rotation(Quat::from_rotation_z(1.75)),
            head,
        ),
    ]
}

/// Puts the tool in the picked slot in the figure's hand, whenever it changes.
fn show(
    mut commands: Commands,
    belongings: Belongings,
    grip: Option<Single<Entity, With<Grip>>>,
    shown: Query<Entity, With<Shown>>,
    looks: Option<ResMut<Looks>>,
) {
    let (Some(grip), Some(mut looks)) = (grip, looks) else {
        return;
    };
    let holding = belongings.holding();
    if looks.showing == Some(holding) {
        return;
    }
    looks.showing = Some(holding);
    for entity in &shown {
        commands.entity(entity).despawn();
    }
    let Some(look) = holding.and_then(|tool| looks.tools.iter().find(|(each, _)| *each == tool))
    else {
        return;
    };
    commands.spawn((
        Shown,
        Mesh3d(look.1.mesh.clone()),
        MeshMaterial3d(look.1.material.clone()),
        Transform::default(),
        ChildOf(*grip),
    ));
}
