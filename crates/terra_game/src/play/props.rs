//! The things people put in and around a place they live: furniture, stations, a well.
//!
//! Every one is an item from the builder's catalogue, placed the way a player places one
//! (D108), whoever placed it. An item is solid: its box stops a walker. If the base
//! changes so that an item is no longer indoors, or now stands in a wall, the item is
//! hidden.

use super::Footprint;
use super::catalogue::{Builds, Item, Light};
use super::contract;
use super::flora::Obstacles;
use super::interact::{CanInteract, Interactable};
use super::models::{Part, Textures, read_model, read_sockets};
use super::storage::Storage;
use crate::build::Site;
use crate::planet::Planet;
use bevy::{
    asset::RenderAssetUsages,
    gltf::{GltfAssetLabel, GltfLoaderSettings},
    light::NotShadowCaster,
    mesh::Indices,
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::{Face, PrimitiveTopology},
};
use std::sync::Arc;
use terra_build::{Base, CELL_M};

/// Bright enough to show beside daylight, which is what the camera is exposed for.
const FIRELIGHT_LUMENS: f32 = 500_000.0;
/// The heat a fire gives off as radiation, in watts: a campfire's fifteen kilowatts, a
/// third of it radiated, the rest carried up in its smoke (climate.md).
const FIRE_HEAT_W: f32 = 5_000.0;
/// Anything lower than this is walked over, not round.
const STEP_OVER_M: f32 = 0.25;

/// A placed item that belongs to the world's people, such as a village's well, rather
/// than to the player: it is not saved with the player's things, and is not theirs to
/// take apart.
#[derive(Component)]
pub struct Theirs;

/// An item that has been placed.
#[derive(Component)]
pub struct Prop {
    pub item: Arc<Item>,
    /// Where it was put, in the base's own space.
    pub at: Vec3,
    pub indoors: bool,
    /// How far it was raised above the floor or ground, and how many degrees turned.
    pub lift: f32,
    pub turn: f32,
    /// Half its width, height and depth, in metres: the box a walker cannot enter.
    pub half: Vec3,
}

impl Prop {
    /// Whether a walker has to go round this rather than over it.
    pub fn is_solid(&self) -> bool {
        self.half.y * 2.0 > STEP_OVER_M
    }
}

#[cfg(test)]
impl Prop {
    /// A made-up item, a box so big, standing outdoors at the base's middle: a station
    /// of a kind if it is given one. For tests.
    pub fn boxed(half: Vec3, station: Option<super::crafting::Station>) -> Self {
        let item = Item {
            name: "Box".to_string(),
            file: "box".to_string(),
            dims: half * 2.0,
            cost: Vec::new(),
            light: Light::None,
            station,
            claims: false,
            rest: None,
            holds: None,
        };
        Prop {
            item: Arc::new(item),
            at: Vec3::ZERO,
            indoors: false,
            lift: 0.0,
            turn: 0.0,
            half,
        }
    }
}

#[derive(Component)]
struct Firelight;

/// An item just put down, and for how many seconds: it grows into place over a moment,
/// so a placement is felt as well as seen.
#[derive(Component)]
struct Settling(f32);

const SETTLE_SECONDS: f32 = 0.16;

/// An item's model, ready to draw, as its file has it: at its real size, its origin its
/// anchor (docs/art-style.md, the model contract).
pub struct Model {
    /// What it is drawn as: a textured part in its own textures (D14), the flat-coloured
    /// parts of the older models in one mesh coloured by its points.
    pub pieces: Vec<Piece>,
    /// All of it that does not glow in one mesh, without its textures, for drawing it in one
    /// look, as the builder's ghost of it is.
    pub solid: Option<Handle<Mesh>>,
    /// Half its width, height and depth.
    pub half: Vec3,
    /// The box round it, in its own space.
    pub low: Vec3,
    pub high: Vec3,
    /// Its sockets by name, in its own space: where code attaches to it.
    pub sockets: Vec<(String, Vec3)>,
}

/// One piece of a model and the material it is drawn in.
pub struct Piece {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
    /// It glows, as a forge's coals do, and so casts no shadow.
    pub glows: bool,
}

impl Model {
    /// Its first piece that does not glow, textured if it has any, for where one piece
    /// stands for the whole, as an item rising as it is built, or a dropped bag.
    pub fn body(&self) -> Option<&Piece> {
        self.pieces.iter().find(|piece| !piece.glows)
    }

    /// Where its socket called `name` is, in its own space, if it has one.
    pub fn socket(&self, name: &str) -> Option<Vec3> {
        socket(&self.sockets, name)
    }
}

/// Where the socket called `name` is among `sockets`.
pub fn socket(sockets: &[(String, Vec3)], name: &str) -> Option<Vec3> {
    sockets
        .iter()
        .find(|(each, _)| each == name)
        .map(|&(_, at)| at)
}

/// The model for every item in the catalogue.
#[derive(Resource)]
pub struct Models {
    models: HashMap<String, Model>,
}

impl Models {
    pub fn of(&self, item: &Item) -> Option<&Model> {
        self.models.get(&item.file)
    }
}

/// The base's shape the props were last checked against.
#[derive(Resource)]
struct CheckedFor(u64);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, (load, start_checking))
        .add_systems(Update, (flicker, keep_clear, settle));
}

/// Our colour for a flat-coloured part of a prop, as it should look on screen, and whether
/// it glows. A part named for a palette role (`tools/props`) has that role's colour in its
/// file; the free models' wood is given ours.
fn colour_of(part: &Part) -> (Color, bool) {
    let name = part.material.split('.').next().unwrap_or("");
    let colour = match name {
        "Wood" => Color::srgb(0.56, 0.39, 0.25),
        "LightWood" => Color::srgb(0.74, 0.56, 0.37),
        "Fire" => return (Color::srgb(1.0, 0.72, 0.35), true),
        _ => part.colour,
    };
    (colour, false)
}

/// A model read from its file, as it is: at its real size, its origin its anchor and its
/// front +Z, as the model contract says (docs/art-style.md), so nothing about it is fixed
/// up here.
pub struct Shape {
    parts: Vec<Part>,
    /// The box round it, in its own space.
    pub low: Vec3,
    pub high: Vec3,
    /// Its sockets by name, in its own space.
    pub sockets: Vec<(String, Vec3)>,
}

#[cfg(test)]
impl Shape {
    /// A shape that is only its box and sockets, for testing what is said of models.
    pub fn from_box(low: Vec3, high: Vec3, sockets: Vec<(String, Vec3)>) -> Self {
        Self {
            parts: Vec::new(),
            low,
            high,
            sockets,
        }
    }
}

impl Shape {
    /// Half its width, height and depth.
    pub fn half(&self) -> Vec3 {
        (self.high - self.low) * 0.5
    }
}

/// The model in `assets/models/<file>`, such as `props/Chest.glb`.
pub fn read_shape(file: &str) -> Result<Shape, String> {
    let parts = read_model(file)?;
    let all = || parts.iter().flat_map(|part| part.positions.iter().copied());
    let low = all().fold(Vec3::MAX, Vec3::min);
    let high = all().fold(Vec3::MIN, Vec3::max);
    if low.cmpgt(high).any() {
        return Err(format!("{file} has no triangles"));
    }
    Ok(Shape {
        parts,
        low,
        high,
        sockets: read_sockets(file)?,
    })
}

/// Where a catalogue item's model is, under `assets/models`.
pub fn item_file(item: &Item) -> String {
    format!("props/{}.glb", item.file)
}

/// What making a prop's model to draw needs: where its meshes and materials go, the asset
/// server that loads its textures, and the materials its flat-coloured parts share.
pub struct Making<'a> {
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a mut Assets<StandardMaterial>,
    pub assets: &'a AssetServer,
    pub lit: Handle<StandardMaterial>,
    pub glowing: Handle<StandardMaterial>,
}

/// One mesh of `parts`, each point coloured by its part, or nothing if there are none.
fn coloured(parts: &[&Part], meshes: &mut Assets<Mesh>) -> Option<Handle<Mesh>> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colours = Vec::new();
    let mut indices = Vec::new();
    for part in parts {
        let first = positions.len() as u32;
        // A textured part drawn without its textures takes its material's own colour.
        let colour = if part.textures.is_some() {
            part.colour
        } else {
            colour_of(part).0
        };
        positions.extend_from_slice(&part.positions);
        normals.extend_from_slice(&part.normals);
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
}

/// A textured part's mesh, with its places on its textures and, for its normal map, its
/// tangents: the file's, as `tools/props` exports them, or made here if it has none.
fn textured_mesh(part: &Part, textures: &Textures) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, part.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, part.normals.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, part.uvs.clone().unwrap_or_default())
    .with_inserted_indices(Indices::U32(part.indices.clone()));
    if textures.normal.is_some() {
        match &part.tangents {
            Some(tangents) => mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents.clone()),
            None => {
                if let Err(problem) = mesh.generate_tangents() {
                    warn!("A prop's part has no tangents for its normal map. {problem}");
                }
            }
        }
    }
    mesh
}

/// How bright a glowing textured part is: its colour texture times this, so painted coals
/// glow as brightly as the flat glow (`glowing`).
const GLOW_SCALE: f32 = 8.0;

/// A textured part's material: its own textures, loaded from its file through Bevy's glTF
/// loader by their labels, and its own factors. Loading them that way gives them mipmaps
/// and filtering (`terra_world::mipmaps`), and each texture is loaded once however many
/// pieces draw it.
fn textured_material(
    path: &str,
    textures: &Textures,
    glows: bool,
    assets: &AssetServer,
) -> StandardMaterial {
    let load = |index: usize| -> Handle<Image> {
        assets
            .load_builder()
            .with_settings(|settings: &mut GltfLoaderSettings| {
                // Only its textures are wanted: the meshes are made here.
                settings.load_meshes = RenderAssetUsages::empty();
                settings.load_cameras = false;
                settings.load_lights = false;
                settings.load_animations = false;
            })
            .load(GltfAssetLabel::Texture(index).from_asset(path.to_string()))
    };
    let colour = load(textures.colour);
    StandardMaterial {
        base_color_texture: Some(colour.clone()),
        normal_map_texture: textures.normal.map(load),
        metallic_roughness_texture: textures.metallic_roughness.map(load),
        occlusion_texture: textures.occlusion.map(load),
        metallic: textures.metallic,
        perceptual_roughness: textures.roughness,
        double_sided: textures.double_sided,
        cull_mode: (!textures.double_sided).then_some(Face::Back),
        emissive: if glows {
            LinearRgba::rgb(GLOW_SCALE, GLOW_SCALE, GLOW_SCALE)
        } else {
            LinearRgba::BLACK
        },
        emissive_texture: glows.then_some(colour),
        ..lit()
    }
}

/// An item's model made to draw from its shape: its textured parts each in their own
/// material, its flat-coloured parts in one mesh lit and one glowing.
fn make(file: &str, shape: Shape, making: &mut Making) -> Model {
    let path = format!("models/{file}");
    let glows = |part: &Part| colour_of(part).1;
    let mut pieces = Vec::new();
    // Its textured parts first, so its body is one of them where it has any.
    for part in &shape.parts {
        let Some(textures) = &part.textures else {
            continue;
        };
        let glowing = glows(part);
        let material = textured_material(&path, textures, glowing, making.assets);
        pieces.push(Piece {
            mesh: making.meshes.add(textured_mesh(part, textures)),
            material: making.materials.add(material),
            glows: glowing,
        });
    }
    for glowing in [false, true] {
        let flat: Vec<&Part> = shape
            .parts
            .iter()
            .filter(|part| part.textures.is_none() && glows(part) == glowing)
            .collect();
        if let Some(mesh) = coloured(&flat, making.meshes) {
            pieces.push(Piece {
                mesh,
                material: if glowing {
                    making.glowing.clone()
                } else {
                    making.lit.clone()
                },
                glows: glowing,
            });
        }
    }
    let textured = shape.parts.iter().any(|part| part.textures.is_some());
    let solid = if textured {
        let lit: Vec<&Part> = shape.parts.iter().filter(|part| !glows(part)).collect();
        coloured(&lit, making.meshes)
    } else {
        pieces
            .iter()
            .find(|piece| !piece.glows)
            .map(|piece| piece.mesh.clone())
    };
    Model {
        pieces,
        solid,
        half: shape.half(),
        low: shape.low,
        high: shape.high,
        sockets: shape.sockets,
    }
}

/// The model in `assets/models/<file>`, such as `props/Chest.glb`, ready to draw.
pub fn read_look(file: &str, making: &mut Making) -> Result<Model, String> {
    Ok(make(file, read_shape(file)?, making))
}

/// The material a prop's flat-coloured parts are drawn with: their colours are its
/// vertices'.
pub fn lit() -> StandardMaterial {
    StandardMaterial {
        perceptual_roughness: 0.8,
        reflectance: 0.3,
        ..default()
    }
}

fn load(
    mut commands: Commands,
    builds: Res<Builds>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let lit = materials.add(lit());
    let glowing = materials.add(glowing());
    let mut making = Making {
        meshes: &mut meshes,
        materials: &mut materials,
        assets: &assets,
        lit,
        glowing,
    };
    let mut models = HashMap::default();
    for item in builds.items() {
        match read_look(&item_file(item), &mut making) {
            Ok(model) => {
                models.insert(item.file.clone(), model);
            }
            Err(problem) => error!("Could not read a model. {problem}"),
        }
    }
    commands.insert_resource(Models { models });
}

/// The material a prop's flat glowing parts are drawn with, such as a forge's coals.
pub fn glowing() -> StandardMaterial {
    StandardMaterial {
        emissive: LinearRgba::rgb(14.0, 6.0, 1.6),
        ..default()
    }
}

/// An item, door or shutters of the builder's catalogue as it is drawn, for looking at
/// models outside play, as the sandbox's workbench does: with its entry's size and anchor.
pub struct Look {
    pub entry: contract::Entry,
    pub model: Model,
}

/// The looks of the catalogue's items, doors and shutters called `names`, or of all for
/// `all`, in the catalogue's order then. Each is the model as its file has it, or why it
/// could not be read.
pub fn looks(names: &[String], making: &mut Making) -> Vec<Result<Look, String>> {
    let builds = Builds::read(
        &super::inventory::Items::default(),
        &super::crafting::Stations::read(),
    );
    let entries = contract::entries(&builds);
    let wanted: Vec<&contract::Entry> = if names.iter().any(|name| name == "all") {
        entries.iter().collect()
    } else {
        names
            .iter()
            .filter_map(|name| entries.iter().find(|entry| entry.name == *name))
            .collect()
    };
    let mut found: Vec<Result<Look, String>> = names
        .iter()
        .filter(|name| *name != "all" && !entries.iter().any(|entry| entry.name == **name))
        .map(|name| Err(format!("There is no item called {name} in the catalogue.")))
        .collect();
    found.extend(wanted.into_iter().map(|entry| {
        read_look(&entry.file, making).map(|model| Look {
            entry: entry.clone(),
            model,
        })
    }));
    found
}

/// Stands an item in the world. `at` is in the base's own space; outdoors only its east
/// and south count, and the item is stood on the ground there. Returns nothing if the
/// item's model is missing.
/// Who put something where it stands, when a player did.
#[derive(Component, Clone, Copy, Debug)]
pub struct PlacedBy(pub Entity);

pub fn place(
    commands: &mut Commands,
    models: &Models,
    site: &Site,
    planet: &Planet,
    item: &Arc<Item>,
    at: Vec3,
    lift: f32,
    turn_degrees: f32,
    indoors: bool,
) -> Option<Entity> {
    let model = models.of(item)?;
    let mut prop = commands.spawn((
        Name::new(item.name.clone()),
        Prop {
            item: item.clone(),
            at,
            indoors,
            lift,
            turn: turn_degrees,
            half: model.half,
        },
        standing(site, planet, at, lift, turn_degrees, indoors),
        Visibility::default(),
        Settling(0.0),
    ));
    if let Some(station) = item.station {
        prop.insert(CanInteract(Interactable::Station(station)));
    } else if item.claims {
        prop.insert(CanInteract(Interactable::ClaimStone));
    } else if let Some((w, h)) = item.holds {
        prop.insert((
            CanInteract(Interactable::Storage),
            Storage(terra_items::Grid::new(w, h)),
        ));
    }
    for piece in &model.pieces {
        let drawn = (
            Mesh3d(piece.mesh.clone()),
            MeshMaterial3d(piece.material.clone()),
        );
        if piece.glows {
            prop.with_child((drawn, NotShadowCaster));
        } else {
            prop.with_child(drawn);
        }
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
        Light::Fire => {
            prop.with_child((
                Firelight,
                terra_world::climate::Heat { watts: FIRE_HEAT_W },
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
    let turned = site.frame.rotation * Quat::from_rotation_y(turn_degrees.to_radians());
    if indoors {
        return Transform {
            translation: floor + *site.frame.up() * lift,
            rotation: turned,
            scale: Vec3::ONE,
        };
    }
    // Outdoors it stands upright where it is, which far from the base is not quite the
    // base's up, the planet being round.
    let stood = planet.ground_at(floor);
    let up = stood.normalize();
    Transform {
        translation: stood + up * lift,
        rotation: Quat::from_rotation_arc(*site.frame.up(), up) * turned,
        scale: Vec3::ONE,
    }
}

/// Nothing has been checked against the base yet.
fn start_checking(mut commands: Commands) {
    commands.insert_resource(CheckedFor(u64::MAX));
}

fn settle(
    mut commands: Commands,
    time: Res<Time>,
    mut settling: Query<(Entity, &mut Settling, &mut Transform)>,
) {
    for (entity, mut settling, mut transform) in &mut settling {
        settling.0 += time.delta_secs();
        let done = (settling.0 / SETTLE_SECONDS).min(1.0);
        // Quick at first, easing to rest.
        let eased = 1.0 - (1.0 - done) * (1.0 - done);
        transform.scale = Vec3::splat(0.8 + 0.2 * eased);
        if done >= 1.0 {
            commands.entity(entity).remove::<Settling>();
        }
    }
}

/// Firelight is never steady.
fn flicker(time: Res<Time>, mut fires: Query<&mut PointLight, With<Firelight>>) {
    let t = time.elapsed_secs();
    let waver = 0.82 + 0.1 * libm::sinf(t * 11.0) + 0.08 * libm::sinf(t * 23.0 + 1.3);
    for mut fire in &mut fires {
        fire.intensity = FIRELIGHT_LUMENS * waver;
    }
}

/// Whether an item put at a place in the base's own space would still be where it claims
/// to be: under a roof if indoors, clear of the base if not.
fn still_fits(site: &Site, at: Vec3, indoors: bool) -> bool {
    let tile = (at / CELL_M).floor().as_ivec3();
    if indoors {
        site.base.contains(tile.with_y(0))
    } else {
        !site.covers(site.frame.transform_point(at), 0.3)
    }
}

/// An item's box as it would stand: where, how big, and how turned, in the base's own
/// space.
#[derive(Clone, Copy)]
pub struct Standing {
    pub at: Vec3,
    pub lift: f32,
    pub turn: f32,
    pub half: Vec3,
    pub indoors: bool,
}

impl Standing {
    pub fn of(prop: &Prop) -> Self {
        Standing {
            at: prop.at,
            lift: prop.lift,
            turn: prop.turn,
            half: prop.half,
            indoors: prop.indoors,
        }
    }

    /// The four corners of its foot, east and south.
    fn corners(&self) -> [Vec2; 4] {
        let turned = Quat::from_rotation_y(self.turn.to_radians());
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(x, z)| {
            let corner = self.at + turned * Vec3::new(x * self.half.x, 0.0, z * self.half.z);
            corner.xz()
        })
    }

    /// How far its box reaches from its middle in a level direction.
    pub fn reach_toward(&self, direction: Vec3) -> f32 {
        let turned = Quat::from_rotation_y(self.turn.to_radians());
        self.half.x * (turned * Vec3::X).dot(direction).abs()
            + self.half.z * (turned * Vec3::Z).dot(direction).abs()
    }

    fn is_solid(&self) -> bool {
        self.half.y * 2.0 > STEP_OVER_M
    }

    /// Whether a wall runs through the box's foot, near enough: whether any tile its
    /// outline covers is a wall (D181).
    fn crosses_a_wall(&self, base: &Base) -> bool {
        let corners = self.corners();
        let low = corners
            .iter()
            .fold(Vec2::MAX, |low, corner| low.min(*corner));
        let high = corners
            .iter()
            .fold(Vec2::MIN, |high, corner| high.max(*corner));
        let cell = |at: Vec2| (at / CELL_M).floor().as_ivec2();
        let (low, high) = (cell(low + 0.02), cell(high - 0.02));
        let storey = (self.at.y / terra_build::STOREY_M).round() as i32;
        (low.x..=high.x)
            .any(|x| (low.y..=high.y).any(|z| base.filling(IVec3::new(x, storey, z)).is_some()))
    }

    /// Whether two boxes share space. Boxes that only touch do not.
    pub fn overlaps(&self, other: &Standing) -> bool {
        const TOUCHING: f32 = 0.03;
        let (low, high) = (self.lift, self.lift + self.half.y * 2.0);
        let (other_low, other_high) = (other.lift, other.lift + other.half.y * 2.0);
        if self.indoors != other.indoors
            || high < other_low + TOUCHING
            || other_high < low + TOUCHING
        {
            return false;
        }
        // Two rectangles are apart if some edge of either has the other wholly beyond it.
        let (ours, theirs) = (self.corners(), other.corners());
        let apart_along = |corners: &[Vec2; 4]| {
            (0..2).any(|edge| {
                let axis = (corners[edge + 1] - corners[edge])
                    .perp()
                    .normalize_or_zero();
                let span = |shape: &[Vec2; 4]| {
                    shape
                        .iter()
                        .map(|corner| corner.dot(axis))
                        .fold((f32::MAX, f32::MIN), |(low, high), at| {
                            (low.min(at), high.max(at))
                        })
                };
                let (a, b) = (span(&ours), span(&theirs));
                a.1 < b.0 + TOUCHING || b.1 < a.0 + TOUCHING
            })
        };
        !apart_along(&ours) && !apart_along(&theirs)
    }

    /// Whether a circle on the ground, given in the base's own space, reaches into the
    /// box's foot.
    fn meets_circle(&self, middle: Vec3, radius: f32) -> bool {
        let turned_back = Quat::from_rotation_y(-self.turn.to_radians());
        let local = turned_back * (middle - self.at);
        let outside = (Vec2::new(local.x.abs(), local.z.abs())
            - Vec2::new(self.half.x, self.half.z))
        .max(Vec2::ZERO);
        outside.length() < radius
    }
}

/// Why an item cannot stand where it is offered.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Blocked {
    /// It would stick through a wall, or stand half in the base and half out.
    Structure,
    /// Another item is there.
    Item(Entity),
    /// A tree or a boulder is there.
    Nature,
    Water,
}

/// What stops an item standing somewhere in a base, if anything, given the base's shape
/// and where it stands on the ground. It has to be wholly indoors or wholly clear of the
/// base, and not in the same space as another item, a tree or a boulder. Rugs and other
/// flat things share space with anything.
pub fn blocked<'a>(
    (base, footprint): (&Base, &Footprint),
    standing: &Standing,
    others: impl Iterator<Item = (Entity, &'a Prop)>,
    obstacles: &Obstacles,
) -> Option<Blocked> {
    let frame = footprint.frame;
    let placed = standing.corners().iter().all(|corner| {
        let at = Vec3::new(corner.x, 0.0, corner.y);
        if standing.indoors {
            base.contains((at / CELL_M).floor().as_ivec3().with_y(0))
        } else {
            !footprint.covers(frame.transform_point(at), 0.05)
        }
    });
    if !placed || standing.crosses_a_wall(base) {
        return Some(Blocked::Structure);
    }
    if !standing.is_solid() {
        return None;
    }
    if let Some((entity, _)) = others
        .filter(|(_, other)| other.is_solid())
        .find(|(_, other)| standing.overlaps(&Standing::of(other)))
    {
        return Some(Blocked::Item(entity));
    }
    let to_base = frame.compute_affine().inverse();
    let middle = frame.transform_point(standing.at);
    (!standing.indoors
        && obstacles
            .near(middle, standing.half.length())
            .any(|(at, radius)| {
                standing.meets_circle(to_base.transform_point3(at).with_y(0.0), radius)
            }))
    .then_some(Blocked::Nature)
}

/// Where a ray first meets an item's box, as the distance along the ray and which way
/// the side it met faces. The ray and the answer are in the world's own space.
pub fn ray_box(
    transform: &Transform,
    half: Vec3,
    origin: Vec3,
    direction: Vec3,
) -> Option<(f32, Vec3)> {
    let turned_back = transform.rotation.inverse();
    let from = turned_back * (origin - transform.translation);
    let along = turned_back * direction;
    let low = Vec3::new(-half.x, 0.0, -half.z);
    let high = Vec3::new(half.x, half.y * 2.0, half.z);
    let (mut near, mut far, mut side) = (0.0_f32, f32::MAX, Vec3::ZERO);
    for axis in 0..3 {
        if along[axis].abs() < 1e-6 {
            if from[axis] < low[axis] || from[axis] > high[axis] {
                return None;
            }
            continue;
        }
        let enter = (low[axis] - from[axis]) / along[axis];
        let leave = (high[axis] - from[axis]) / along[axis];
        let (enter, leave, facing) = if enter <= leave {
            (enter, leave, -1.0)
        } else {
            (leave, enter, 1.0)
        };
        if enter > near {
            near = enter;
            side = Vec3::ZERO;
            side[axis] = facing;
        }
        far = far.min(leave);
        if near > far {
            return None;
        }
    }
    // Starting inside the box meets no side of it.
    (side != Vec3::ZERO).then(|| (near, transform.rotation * side))
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
        *visibility = if still_fits(&site, prop.at, prop.indoors) {
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

    fn box_at_origin() -> (Prop, Transform, Visibility) {
        let prop = Prop::boxed(Vec3::splat(0.4), None);
        (prop, Transform::IDENTITY, Visibility::Inherited)
    }

    #[test]
    fn a_walker_is_pushed_out_of_an_item_by_its_nearest_side() {
        let (prop, transform, visibility) = box_at_origin();
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
        let (mut prop, transform, visibility) = box_at_origin();
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

    fn standing(east: f32, turn: f32) -> Standing {
        Standing {
            at: Vec3::new(east, 0.0, 0.0),
            lift: 0.0,
            turn,
            half: Vec3::new(1.0, 0.4, 0.3),
            indoors: true,
        }
    }

    #[test]
    fn boxes_overlap_only_when_they_share_space() {
        let here = standing(0.0, 0.0);
        assert!(here.overlaps(&standing(1.5, 0.0)));
        // Side by side and touching is not overlapping.
        assert!(!here.overlaps(&standing(2.0, 0.0)));
        // Turned a quarter circle it is narrow, and fits closer.
        assert!(!here.overlaps(&standing(1.4, 90.0)));
        assert!(here.overlaps(&standing(1.2, 90.0)));
        // Stood on top of it, or in a different place altogether.
        let above = Standing { lift: 0.8, ..here };
        assert!(!here.overlaps(&above));
        let outside = Standing {
            indoors: false,
            ..here
        };
        assert!(!here.overlaps(&outside));
    }

    #[test]
    fn a_box_knows_how_far_it_reaches_and_what_circles_touch_it() {
        let here = standing(0.0, 0.0);
        assert!((here.reach_toward(Vec3::X) - 1.0).abs() < 1e-5);
        assert!((here.reach_toward(Vec3::Z) - 0.3).abs() < 1e-5);
        assert!((standing(0.0, 90.0).reach_toward(Vec3::X) - 0.3).abs() < 1e-5);
        assert!(here.meets_circle(Vec3::new(1.2, 0.0, 0.0), 0.3));
        assert!(!here.meets_circle(Vec3::new(1.4, 0.0, 0.0), 0.3));
        assert!(!here.meets_circle(Vec3::new(0.0, 0.0, 0.7), 0.3));
    }

    #[test]
    fn a_ray_meets_the_nearest_side_of_a_box() {
        let stood = Transform::from_xyz(5.0, 0.0, 0.0);
        let half = Vec3::new(0.5, 0.4, 0.5);
        // Level, from the west: its west side, half a metre short of its middle.
        let (distance, side) = ray_box(&stood, half, Vec3::new(0.0, 0.4, 0.0), Vec3::X).unwrap();
        assert!((distance - 4.5).abs() < 1e-5 && side.distance(Vec3::NEG_X) < 1e-5);
        // From above: its top.
        let (distance, side) =
            ray_box(&stood, half, Vec3::new(5.0, 3.0, 0.0), Vec3::NEG_Y).unwrap();
        assert!((distance - 2.2).abs() < 1e-5 && side.distance(Vec3::Y) < 1e-5);
        // Past it, or away from it.
        assert!(ray_box(&stood, half, Vec3::new(0.0, 0.4, 2.0), Vec3::X).is_none());
        assert!(ray_box(&stood, half, Vec3::new(0.0, 0.4, 0.0), Vec3::NEG_X).is_none());
    }

    /// A room ten tiles by ten outside, eight by eight inside, at the world's middle,
    /// with a stub of wall standing alone in it.
    fn room() -> (Base, Footprint) {
        use terra_build::{BuildOp, Kind, Panel};
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 9],
            kind: Kind::PLAIN,
        });
        base.apply(BuildOp::SetWall {
            tile: [5, 0, 5],
            panel: Some(Panel::Solid),
            kind: Kind::PLAIN,
        });
        assert!(base.contains(IVec3::new(4, 0, 4)) && !base.contains(IVec3::new(5, 0, 5)));
        let footprint = Footprint {
            frame: Transform::IDENTITY,
            cell_m: CELL_M,
            ground: base.ground_columns().collect(),
            standing: base.occupied_columns().collect(),
        };
        (base, footprint)
    }

    /// A box half a metre across and a metre high, its middle at a place.
    fn boxed(east: f32, south: f32, indoors: bool) -> Standing {
        Standing {
            at: Vec3::new(east, 0.0, south),
            lift: 0.0,
            turn: 0.0,
            half: Vec3::new(0.25, 0.5, 0.25),
            indoors,
        }
    }

    #[test]
    fn an_item_stands_wholly_indoors_or_wholly_out() {
        let (base, footprint) = room();
        let blocked = |standing| {
            blocked(
                (&base, &footprint),
                &standing,
                std::iter::empty(),
                &Obstacles::default(),
            )
        };
        // Inside the room, clear of the walls; and well out of the base.
        assert_eq!(blocked(boxed(1.5, 1.5, true)), None);
        assert_eq!(blocked(boxed(10.0, 10.0, false)), None);
        // Through the room's wall, from either side, and outdoors in the room.
        assert_eq!(blocked(boxed(0.6, 1.5, true)), Some(Blocked::Structure));
        assert_eq!(blocked(boxed(-0.1, 1.5, false)), Some(Blocked::Structure));
        // Indoors where there is no room.
        assert_eq!(blocked(boxed(10.0, 10.0, true)), Some(Blocked::Structure));
        assert_eq!(blocked(boxed(1.5, 1.5, false)), Some(Blocked::Structure));
        // Round a wall standing in the room, every corner of it in the room.
        let over_the_stub = Standing {
            half: Vec3::new(0.6, 0.5, 0.6),
            ..boxed(2.75, 2.75, true)
        };
        assert_eq!(blocked(over_the_stub), Some(Blocked::Structure));
    }

    #[test]
    fn a_solid_item_is_kept_out_of_another_but_a_rug_is_not() {
        let (base, footprint) = room();
        let mut world = World::new();
        let mut put = |at: Vec3, half: Vec3| {
            let prop = Prop {
                at,
                indoors: true,
                ..Prop::boxed(half, None)
            };
            world.spawn(prop).id()
        };
        let chest = put(Vec3::new(1.5, 0.0, 1.5), Vec3::new(0.25, 0.5, 0.25));
        // Low enough to step over, though not so low it shares no space with a box.
        let rug = put(Vec3::new(3.5, 0.0, 1.5), Vec3::new(0.5, 0.1, 0.5));
        let props: Vec<(Entity, &Prop)> = [chest, rug]
            .map(|entity| (entity, world.get::<Prop>(entity).unwrap()))
            .to_vec();
        let blocked = |standing| {
            blocked(
                (&base, &footprint),
                &standing,
                props.iter().copied(),
                &Obstacles::default(),
            )
        };
        assert_eq!(blocked(boxed(1.7, 1.5, true)), Some(Blocked::Item(chest)));
        assert_eq!(blocked(boxed(3.5, 1.5, true)), None, "on the rug");
        let flat = Standing {
            half: Vec3::new(0.25, 0.1, 0.25),
            ..boxed(1.5, 1.5, true)
        };
        assert_eq!(blocked(flat), None, "a rug under the chest");
    }

    #[test]
    fn an_item_is_kept_out_of_a_tree_outdoors() {
        let (base, footprint) = room();
        let mut trees = Obstacles::default();
        trees.add(Vec3::new(10.0, 0.0, 10.0), 0.3);
        let blocked =
            |standing| blocked((&base, &footprint), &standing, std::iter::empty(), &trees);
        assert_eq!(blocked(boxed(10.4, 10.0, false)), Some(Blocked::Nature));
        assert_eq!(blocked(boxed(11.0, 10.0, false)), None);
    }
}
