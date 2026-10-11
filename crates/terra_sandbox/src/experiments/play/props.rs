//! The things people leave around a place they live: furniture indoors, and outdoors
//! crates, fuel, and a fire with logs to sit on.
//!
//! All of it is set dressing, placed by hand for the example base. Nothing here can be
//! picked up or used. If the base is reshaped so that a piece is no longer indoors, or
//! now stands in a wall, the piece is hidden.

use super::models::{Part, read_model};
use crate::experiments::build::Site;
use crate::planet::Planet;
use bevy::{
    asset::RenderAssetUsages, light::NotShadowCaster, mesh::Indices, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use terra_build::TILE_M;

/// One piece and where it goes, in the base's own space, in metres.
struct Placing {
    file: &'static str,
    /// Where the middle of its foot stands. Outdoors only east and south count; it is
    /// stood on the ground there.
    at: Vec3,
    /// Raised this far above the floor or ground, for things stood on other things.
    lift: f32,
    /// Turned this many degrees, anticlockwise seen from above.
    turn: f32,
    /// Its longest dimension.
    size: f32,
    indoors: bool,
}

const fn indoors(file: &'static str, x: f32, z: f32, turn: f32, size: f32) -> Placing {
    Placing {
        file,
        at: Vec3::new(x, 0.0, z),
        lift: 0.0,
        turn,
        size,
        indoors: true,
    }
}

const fn outdoors(file: &'static str, x: f32, z: f32, turn: f32, size: f32) -> Placing {
    Placing {
        file,
        at: Vec3::new(x, 0.0, z),
        lift: 0.0,
        turn,
        size,
        indoors: false,
    }
}

const fn raised(placing: Placing, lift: f32) -> Placing {
    Placing { lift, ..placing }
}

/// Where the fire is, which the logs and the light are arranged around.
const FIRE: Vec3 = Vec3::new(4.0, 0.0, 15.0);

const LAYOUT: &[Placing] = &[
    // The main room: somewhere to sit, looking out at the water.
    raised(indoors("Carpet_Round", 1.5, -2.2, 0.0, 3.2), 0.015),
    indoors("Couch_Medium1", 1.5, -4.9, 0.0, 2.2),
    indoors("Table_RoundSmall", 1.5, -2.2, 0.0, 1.1),
    indoors("Stool", 3.0, -1.6, 30.0, 0.5),
    indoors("Light_Floor1", -0.4, -5.2, 0.0, 1.6),
    indoors("Bookshelf", -5.55, -4.4, 90.0, 2.0),
    indoors("Houseplant_7", -5.1, 2.2, 0.0, 1.1),
    indoors("Houseplant_3", 5.2, -5.3, 0.0, 0.9),
    indoors("Props_Computer", 8.45, -4.6, -90.0, 1.8),
    indoors("Props_Shelf_Tall", 8.5, -1.8, -90.0, 2.0),
    indoors("Chair_1", 7.4, -4.6, 90.0, 0.95),
    // The far room: a bunk and a kitchen.
    raised(indoors("Carpet_1", 18.0, -1.0, 90.0, 3.0), 0.015),
    indoors("Bed_Single", 17.2, -4.8, 90.0, 2.15),
    indoors("NightStand_1", 19.0, -5.45, 0.0, 0.55),
    raised(indoors("Houseplant_1", 19.0, -5.45, 0.0, 0.45), 0.55),
    indoors("Kitchen_Fridge", 15.55, 2.35, 90.0, 1.85),
    indoors("Kitchen_Sink", 15.5, 1.2, 90.0, 1.08),
    indoors("Kitchen_2Drawers", 15.5, 0.3, 90.0, 0.92),
    indoors("Houseplant_7", 20.2, 2.2, 0.0, 1.0),
    // By the door: what has not been carried in yet.
    outdoors("Props_Crate", 10.6, 4.3, 12.0, 0.8),
    outdoors("Props_Crate", 11.6, 4.6, -8.0, 0.8),
    raised(outdoors("Props_Crate", 11.1, 4.45, 30.0, 0.8), 0.78),
    outdoors("Props_CrateLong", 12.6, 5.6, 70.0, 1.2),
    outdoors("Props_ContainerFull", 4.2, 4.4, 5.0, 1.3),
    outdoors("PropaneTank", 13.0, 0.75, 0.0, 1.3),
    outdoors("PropaneTank", 13.9, 0.8, 40.0, 1.3),
    outdoors("GasCan", 9.9, 5.4, 60.0, 0.45),
    raised(outdoors("Radio", 12.6, 5.6, 110.0, 0.4), 0.74),
    outdoors("Shovel", 3.0, 3.25, 0.0, 1.3),
    // Out front: a fire, and logs to sit on.
    outdoors("Bonfire_Fire", FIRE.x, FIRE.z, 0.0, 1.3),
    outdoors("WoodLog", FIRE.x - 2.0, FIRE.z + 0.4, 80.0, 1.7),
    outdoors("WoodLog", FIRE.x + 1.3, FIRE.z + 1.7, -40.0, 1.7),
    outdoors("WoodLog", FIRE.x + 1.6, FIRE.z - 1.5, 35.0, 1.5),
];

/// A piece of set dressing, and where it was put in the base's own space.
#[derive(Component)]
struct Prop {
    at: Vec3,
    indoors: bool,
}

#[derive(Component)]
struct Firelight;

/// The base's shape the props were last checked against.
#[derive(Resource)]
struct CheckedFor(u64);

pub fn plugin(app: &mut App) {
    app.add_systems(PostStartup, furnish)
        .add_systems(Update, (flicker, keep_clear));
}

/// Our colour for a part of a prop, as it should look on screen, and whether it glows.
fn colour_of(part: &Part) -> (Color, bool) {
    let name = part.material.split('.').next().unwrap_or("");
    let colour = match name {
        "White" | "Main" => Color::srgb(0.9, 0.87, 0.8),
        "Wood" | "Brown" => Color::srgb(0.56, 0.39, 0.25),
        "Wood_Light" | "LightWood" => Color::srgb(0.74, 0.56, 0.37),
        "Wood_Dark" | "DarkWood" => Color::srgb(0.37, 0.26, 0.19),
        "Grey" | "LightGrey" | "Metal" | "LightMetal" => Color::srgb(0.62, 0.63, 0.66),
        "DarkGrey" => Color::srgb(0.3, 0.31, 0.36),
        "Black" => Color::srgb(0.14, 0.14, 0.17),
        "Red" => Color::srgb(0.84, 0.27, 0.2),
        "DarkRed" => Color::srgb(0.62, 0.2, 0.17),
        "Orange" | "Accent" => Color::srgb(0.9, 0.46, 0.17),
        "DarkAccent" => Color::srgb(0.62, 0.31, 0.13),
        "LightOrange" => Color::srgb(0.95, 0.66, 0.36),
        "Couch_Blue" => Color::srgb(0.2, 0.45, 0.56),
        "Cushin" => Color::srgb(0.82, 0.74, 0.62),
        "Kitchen" => Color::srgb(0.3, 0.5, 0.5),
        "KitchenTop" => Color::srgb(0.84, 0.81, 0.75),
        "Plant_Green" => Color::srgb(0.42, 0.6, 0.26),
        "DarkGreen" => Color::srgb(0.27, 0.46, 0.25),
        "Light" | "Fire" => return (Color::srgb(1.0, 0.72, 0.35), true),
        _ => part.colour,
    };
    (colour, false)
}

/// A prop's two meshes: what is lit, and what glows. Stood on its foot, centred, and
/// scaled so its longest dimension is `size`.
fn build(parts: &[Part], size: f32) -> [Option<Mesh>; 2] {
    let all = || parts.iter().flat_map(|part| part.positions.iter().copied());
    let low = all().fold(Vec3::MAX, Vec3::min);
    let high = all().fold(Vec3::MIN, Vec3::max);
    let scale = size / (high - low).max_element().max(1e-3);
    let foot = Vec3::new((low.x + high.x) * 0.5, low.y, (low.z + high.z) * 0.5);
    [false, true].map(|glowing| {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut colours = Vec::new();
        let mut indices = Vec::new();
        for part in parts {
            let (colour, glows) = colour_of(part);
            if glows != glowing {
                continue;
            }
            let first = positions.len() as u32;
            positions.extend(part.positions.iter().map(|&p| (p - foot) * scale));
            normals.extend(part.normals.iter().copied());
            colours.extend(std::iter::repeat_n(
                colour.to_linear().to_f32_array(),
                part.positions.len(),
            ));
            indices.extend(part.indices.iter().map(|index| first + index));
        }
        (!positions.is_empty()).then(|| {
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
            .with_inserted_indices(Indices::U32(indices))
        })
    })
}

fn furnish(
    mut commands: Commands,
    planet: Res<Planet>,
    site: Res<Site>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let lit = materials.add(StandardMaterial {
        perceptual_roughness: 0.8,
        reflectance: 0.3,
        ..default()
    });
    let glowing = materials.add(StandardMaterial {
        emissive: LinearRgba::rgb(14.0, 6.0, 1.6),
        ..default()
    });
    for placing in LAYOUT {
        let parts = match read_model(&format!("props/{}.glb", placing.file)) {
            Ok(parts) => parts,
            Err(problem) => {
                error!("Could not read a model. {problem}");
                continue;
            }
        };
        let floor = site.frame.transform_point(placing.at);
        let stood = if placing.indoors {
            floor
        } else {
            planet.drawn(floor.normalize())
        };
        let [solid, glow] = build(&parts, placing.size);
        let mut prop = commands.spawn((
            Name::new(placing.file),
            Prop {
                at: placing.at,
                indoors: placing.indoors,
            },
            Transform {
                translation: stood + *site.frame.up() * placing.lift,
                rotation: site.frame.rotation * Quat::from_rotation_y(placing.turn.to_radians()),
                scale: Vec3::ONE,
            },
            Visibility::default(),
        ));
        if let Some(mesh) = solid {
            prop.with_child((Mesh3d(meshes.add(mesh)), MeshMaterial3d(lit.clone())));
        }
        if let Some(mesh) = glow {
            prop.with_child((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(glowing.clone()),
                NotShadowCaster,
            ));
        }
        if placing.file == "Bonfire_Fire" {
            prop.with_child((
                Firelight,
                PointLight {
                    color: Color::srgb(1.0, 0.55, 0.22),
                    intensity: FIRELIGHT_LUMENS,
                    range: 16.0,
                    radius: 0.25,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.7, 0.0),
            ));
        }
    }
    commands.insert_resource(CheckedFor(u64::MAX));
}

/// Bright enough to show beside daylight, which is what the camera is exposed for.
const FIRELIGHT_LUMENS: f32 = 500_000.0;

/// Firelight is never steady.
fn flicker(time: Res<Time>, mut fires: Query<&mut PointLight, With<Firelight>>) {
    let t = time.elapsed_secs();
    let waver = 0.82 + 0.1 * libm::sinf(t * 11.0) + 0.08 * libm::sinf(t * 23.0 + 1.3);
    for mut fire in &mut fires {
        fire.intensity = FIRELIGHT_LUMENS * waver;
    }
}

/// Hides whatever the base's current shape leaves out in the open or shut in a wall.
fn keep_clear(
    site: Res<Site>,
    checked: Option<ResMut<CheckedFor>>,
    mut props: Query<(&Prop, &mut Visibility)>,
) {
    let Some(mut checked) = checked else { return };
    if checked.0 == site.revision {
        return;
    }
    checked.0 = site.revision;
    for (prop, mut visibility) in &mut props {
        let tile = (prop.at / TILE_M).floor().as_ivec3();
        let under_a_roof = site.base.contains(tile.with_y(0));
        let fits = if prop.indoors {
            under_a_roof
        } else {
            !site.covers(site.frame.transform_point(prop.at), 0.3)
        };
        *visibility = if fits {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}
