//! The things people put in and around a place they live: furniture indoors, and
//! outdoors crates, fuel, and a fire with logs to sit on.
//!
//! Every one is an item from the builder's catalogue, placed the way a player places
//! one. The example base starts furnished through that same call. An item is solid: its
//! box stops a walker. If the base is reshaped so that an item is no longer indoors, or
//! now stands in a wall, the item is hidden.

use super::catalogue::{self, Item, Light};
use super::models::{Part, read_model};
use crate::experiments::build::Site;
use crate::planet::Planet;
use bevy::{
    asset::RenderAssetUsages, light::NotShadowCaster, mesh::Indices,
    platform::collections::HashMap, prelude::*, render::render_resource::PrimitiveTopology,
};
use terra_build::TILE_M;

/// Bright enough to show beside daylight, which is what the camera is exposed for.
const FIRELIGHT_LUMENS: f32 = 500_000.0;
const LAMPLIGHT_LUMENS: f32 = 60_000.0;
/// Anything lower than this is walked over, not round.
const STEP_OVER_M: f32 = 0.25;

/// Where the example base's fire is, which its logs are arranged around.
const FIRE: Vec3 = Vec3::new(4.0, 0.0, 15.0);

/// The example base's furniture: an item's model, where the middle of its foot stands in
/// the base's own space, how far it is raised above the floor or ground, how many degrees
/// it is turned, and whether it is indoors.
const EXAMPLE: &[(&str, f32, f32, f32, f32, bool)] = &[
    // The main room: somewhere to sit, looking out at the water.
    ("Carpet_Round", 1.5, -2.2, 0.0, 0.0, true),
    ("Couch_Medium1", 1.5, -4.9, 0.0, 0.0, true),
    ("Table_RoundSmall", 1.5, -2.2, 0.0, 0.0, true),
    ("Stool", 3.0, -1.6, 0.0, 30.0, true),
    ("Light_Floor1", -0.4, -5.2, 0.0, 0.0, true),
    ("Bookshelf", -5.55, -4.4, 0.0, 90.0, true),
    ("Houseplant_7", -5.1, 2.2, 0.0, 0.0, true),
    ("Houseplant_3", 5.2, -5.3, 0.0, 0.0, true),
    ("Props_Computer", 8.45, -4.6, 0.0, -90.0, true),
    ("Props_Shelf_Tall", 8.5, -1.8, 0.0, -90.0, true),
    ("Chair_1", 7.4, -4.6, 0.0, 90.0, true),
    // The far room: a bunk and a kitchen.
    ("Carpet_1", 18.0, -1.0, 0.0, 90.0, true),
    ("Bed_Single", 17.2, -4.8, 0.0, 90.0, true),
    ("NightStand_1", 19.0, -5.45, 0.0, 0.0, true),
    ("Houseplant_1", 19.0, -5.45, 0.55, 0.0, true),
    ("Kitchen_Fridge", 15.55, 2.35, 0.0, 90.0, true),
    ("Kitchen_Sink", 15.5, 1.2, 0.0, 90.0, true),
    ("Kitchen_2Drawers", 15.5, 0.3, 0.0, 90.0, true),
    ("Houseplant_7", 20.2, 2.2, 0.0, 0.0, true),
    // By the door: what has not been carried in yet.
    ("Props_Crate", 10.6, 4.3, 0.0, 12.0, false),
    ("Props_Crate", 11.6, 4.6, 0.0, -8.0, false),
    ("Props_Crate", 11.1, 4.45, 0.78, 30.0, false),
    ("Props_CrateLong", 12.6, 5.6, 0.0, 70.0, false),
    ("Props_ContainerFull", 4.2, 4.4, 0.0, 5.0, false),
    ("PropaneTank", 13.0, 0.75, 0.0, 0.0, false),
    ("PropaneTank", 13.9, 0.8, 0.0, 40.0, false),
    ("GasCan", 9.9, 5.4, 0.0, 60.0, false),
    ("Radio", 12.6, 5.6, 0.74, 110.0, false),
    ("Shovel", 3.0, 3.25, 0.0, 0.0, false),
    // Out front: a fire, and logs to sit on.
    ("Bonfire_Fire", FIRE.x, FIRE.z, 0.0, 0.0, false),
    ("WoodLog", FIRE.x - 2.0, FIRE.z + 0.4, 0.0, 80.0, false),
    ("WoodLog", FIRE.x + 1.3, FIRE.z + 1.7, 0.0, -40.0, false),
    ("WoodLog", FIRE.x + 1.6, FIRE.z - 1.5, 0.0, 35.0, false),
];

/// An item that has been placed.
#[derive(Component)]
pub struct Prop {
    pub item: &'static Item,
    /// Where it was put, in the base's own space.
    pub at: Vec3,
    pub indoors: bool,
    /// Half its width, height and depth, in metres: the box a walker cannot enter.
    pub half: Vec3,
}

impl Prop {
    /// Whether a walker has to go round this rather than over it.
    pub fn is_solid(&self) -> bool {
        self.half.y * 2.0 > STEP_OVER_M
    }
}

#[derive(Component)]
struct Firelight;

/// An item's shapes, ready to draw: what is lit and what glows, stood on its foot,
/// centred, and at its real size.
pub struct Model {
    pub solid: Option<Handle<Mesh>>,
    pub glow: Option<Handle<Mesh>>,
    pub half: Vec3,
}

/// The model for every item in the catalogue.
#[derive(Resource)]
pub struct Models {
    models: HashMap<&'static str, Model>,
    lit: Handle<StandardMaterial>,
    glowing: Handle<StandardMaterial>,
}

impl Models {
    pub fn of(&self, item: &Item) -> Option<&Model> {
        self.models.get(item.file)
    }
}

/// The base's shape the props were last checked against.
#[derive(Resource)]
struct CheckedFor(u64);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load)
        .add_systems(PostStartup, furnish)
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

/// An item's meshes and its size, from the parts of its model file.
fn build(parts: &[Part], size: f32, meshes: &mut Assets<Mesh>) -> Model {
    let all = || parts.iter().flat_map(|part| part.positions.iter().copied());
    let low = all().fold(Vec3::MAX, Vec3::min);
    let high = all().fold(Vec3::MIN, Vec3::max);
    let scale = size / (high - low).max_element().max(1e-3);
    let foot = Vec3::new((low.x + high.x) * 0.5, low.y, (low.z + high.z) * 0.5);
    let mut mesh_of = |glowing: bool| {
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
            meshes.add(
                Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
                .with_inserted_indices(Indices::U32(indices)),
            )
        })
    };
    Model {
        solid: mesh_of(false),
        glow: mesh_of(true),
        half: (high - low) * scale * 0.5,
    }
}

fn load(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut models = HashMap::default();
    for item in catalogue::items() {
        match read_model(&format!("props/{}.glb", item.file)) {
            Ok(parts) => {
                models.insert(item.file, build(&parts, item.size, &mut meshes));
            }
            Err(problem) => error!("Could not read a model. {problem}"),
        }
    }
    commands.insert_resource(Models {
        models,
        lit: materials.add(StandardMaterial {
            perceptual_roughness: 0.8,
            reflectance: 0.3,
            ..default()
        }),
        glowing: materials.add(StandardMaterial {
            emissive: LinearRgba::rgb(14.0, 6.0, 1.6),
            ..default()
        }),
    });
}

/// Stands an item in the world. `at` is in the base's own space; outdoors only its east
/// and south count, and the item is stood on the ground there. Returns nothing if the
/// item's model is missing.
pub fn place(
    commands: &mut Commands,
    models: &Models,
    site: &Site,
    planet: &Planet,
    item: &'static Item,
    at: Vec3,
    lift: f32,
    turn_degrees: f32,
    indoors: bool,
) -> Option<Entity> {
    let model = models.of(item)?;
    let mut prop = commands.spawn((
        Name::new(item.name),
        Prop {
            item,
            at,
            indoors,
            half: model.half,
        },
        standing(site, planet, at, lift + item.lift, turn_degrees, indoors),
        Visibility::default(),
    ));
    if let Some(mesh) = &model.solid {
        prop.with_child((Mesh3d(mesh.clone()), MeshMaterial3d(models.lit.clone())));
    }
    if let Some(mesh) = &model.glow {
        prop.with_child((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(models.glowing.clone()),
            NotShadowCaster,
        ));
    }
    let glow = |colour: Color, lumens: f32, range: f32| PointLight {
        color: colour,
        intensity: lumens,
        range,
        radius: 0.25,
        shadow_maps_enabled: false,
        ..default()
    };
    match item.light {
        Light::None => {}
        Light::Lamp => {
            prop.with_child((
                glow(Color::srgb(1.0, 0.8, 0.56), LAMPLIGHT_LUMENS, 7.0),
                Transform::from_xyz(0.0, model.half.y * 2.0 - 0.15, 0.0),
            ));
        }
        Light::Fire => {
            prop.with_child((
                Firelight,
                glow(Color::srgb(1.0, 0.55, 0.22), FIRELIGHT_LUMENS, 16.0),
                Transform::from_xyz(0.0, 0.7, 0.0),
            ));
        }
    }
    Some(prop.id())
}

/// Where in the world something stands, given where it is in the base's own space.
pub fn standing(
    site: &Site,
    planet: &Planet,
    at: Vec3,
    lift: f32,
    turn_degrees: f32,
    indoors: bool,
) -> Transform {
    let floor = site.frame.transform_point(at);
    let stood = if indoors {
        floor
    } else {
        planet.drawn(floor.normalize())
    };
    Transform {
        translation: stood + *site.frame.up() * lift,
        rotation: site.frame.rotation * Quat::from_rotation_y(turn_degrees.to_radians()),
        scale: Vec3::ONE,
    }
}

/// Furnishes the example base, through the same call a player's builder makes.
fn furnish(mut commands: Commands, models: Res<Models>, planet: Res<Planet>, site: Res<Site>) {
    for &(file, east, south, lift, turn, indoors) in EXAMPLE {
        let Some(item) = catalogue::item_from(file) else {
            error!("The example base wants {file}, which is not in the catalogue.");
            continue;
        };
        let at = Vec3::new(east, 0.0, south);
        place(
            &mut commands,
            &models,
            &site,
            &planet,
            item,
            at,
            lift,
            turn,
            indoors,
        );
    }
    commands.insert_resource(CheckedFor(u64::MAX));
}

/// Firelight is never steady.
fn flicker(time: Res<Time>, mut fires: Query<&mut PointLight, With<Firelight>>) {
    let t = time.elapsed_secs();
    let waver = 0.82 + 0.1 * libm::sinf(t * 11.0) + 0.08 * libm::sinf(t * 23.0 + 1.3);
    for mut fire in &mut fires {
        fire.intensity = FIRELIGHT_LUMENS * waver;
    }
}

/// Whether an item put at a place in the base's own space would be where it claims to
/// be: under a roof if indoors, clear of the base if not.
pub fn fits(site: &Site, at: Vec3, indoors: bool) -> bool {
    let tile = (at / TILE_M).floor().as_ivec3();
    if indoors {
        site.base.contains(tile.with_y(0))
    } else {
        !site.covers(site.frame.transform_point(at), 0.3)
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
        *visibility = if fits(&site, prop.at, prop.indoors) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Moves a walker's next position out of any item it would be standing inside. A
/// walker is a circle on the ground; an item is its box.
pub fn push_out<'a>(
    mut target: Vec3,
    walker_radius: f32,
    props: impl Iterator<Item = (&'a Prop, &'a Transform, &'a Visibility)>,
) -> Vec3 {
    for (prop, transform, visibility) in props {
        if !prop.is_solid() || *visibility == Visibility::Hidden {
            continue;
        }
        let turned_back = transform.rotation.inverse();
        let mut local = turned_back * (target - transform.translation);
        // Only if the feet are between its foot and its top.
        if local.y < -0.5 || local.y > prop.half.y * 2.0 - 0.05 {
            continue;
        }
        let reach = Vec2::new(prop.half.x, prop.half.z) + walker_radius;
        let inside = reach - Vec2::new(local.x.abs(), local.z.abs());
        if inside.x <= 0.0 || inside.y <= 0.0 {
            continue;
        }
        // Out by the nearest side.
        if inside.x < inside.y {
            local.x = reach.x.copysign(local.x);
        } else {
            local.z = reach.y.copysign(local.z);
        }
        target = transform.translation + transform.rotation * local;
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crate_at_origin() -> (Prop, Transform, Visibility) {
        let prop = Prop {
            item: catalogue::item_from("Props_Crate").unwrap(),
            at: Vec3::ZERO,
            indoors: false,
            half: Vec3::new(0.4, 0.4, 0.4),
        };
        (prop, Transform::IDENTITY, Visibility::Inherited)
    }

    #[test]
    fn a_walker_is_pushed_out_of_an_item_by_its_nearest_side() {
        let (prop, transform, visibility) = crate_at_origin();
        let push =
            |target: Vec3| push_out(target, 0.3, [(&prop, &transform, &visibility)].into_iter());
        // Just inside the east side: out to the east, a radius clear of the box.
        let out = push(Vec3::new(0.5, 0.0, 0.1));
        assert!((out.x - 0.7).abs() < 1e-5 && (out.z - 0.1).abs() < 1e-5);
        // Clear of it already, or standing on top of it: left alone.
        for free in [Vec3::new(0.8, 0.0, 0.0), Vec3::new(0.0, 0.8, 0.0)] {
            assert_eq!(push(free), free);
        }
    }

    #[test]
    fn a_rug_and_a_hidden_item_are_walked_over() {
        let (mut prop, transform, visibility) = crate_at_origin();
        let inside = Vec3::new(0.1, 0.0, 0.1);
        assert_eq!(
            push_out(
                inside,
                0.3,
                [(&prop, &transform, &Visibility::Hidden)].into_iter()
            ),
            inside
        );
        prop.half.y = 0.01;
        assert!(!prop.is_solid());
        assert_eq!(
            push_out(inside, 0.3, [(&prop, &transform, &visibility)].into_iter()),
            inside
        );
    }

    #[test]
    fn the_example_base_is_furnished_from_the_catalogue() {
        for (file, ..) in EXAMPLE {
            assert!(
                catalogue::item_from(file).is_some(),
                "{file} is not an item"
            );
        }
    }
}
