//! The things people put in and around a place they live: furniture, stations, a well.
//!
//! Every one is an item from the builder's catalogue, placed the way a player places one
//! (D108), whoever placed it. An item is solid: its box stops a walker. If the base
//! changes so that an item is no longer indoors, or now stands in a wall, the item is
//! hidden.

use crate::catalogue::{Builds, Item, Light};
use crate::contract;
use crate::interact::{CanInteract, Interactable};
use crate::storage::Storage;
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
use terra_bases::Site;
use terra_build::CELL_M;
use terra_world::models::{Part, Textures, read_model, read_sockets};
use terra_world::planet::Planet;

mod solid;
pub use solid::{Blocked, Standing, blocked, ray_box};

/// A campfire's flames, as the fire drawing draws them with their embers, smoke and light
/// (terra_world::fire): their tips licking 1.3 m over the stones, 0.75 m over its low
/// teepee of logs (0.55 m, tools/props/char.py), as a campfire's do in photographs (at 0.9 m
/// the logs hid all but their tips, fire sheet review 3/10); at 1.5 a 3.3 m card stood them
/// over a seated troll (fire sheet, stage 3).
pub const CAMPFIRE: terra_world::fire::Kindled = terra_world::fire::Kindled {
    tall: 1.3,
    wide: 0.7,
    heat: 0.9,
    logs: terra_world::fire::Logs::CAMPFIRE.below(CAMPFIRE_FOOT_M),
};
/// Its flames stand from so high over its foot, the embers among the stones.
const CAMPFIRE_FOOT_M: f32 = 0.08;
/// The heat a fire gives off as radiation, in watts: a campfire's fifteen kilowatts, a
/// third of it radiated, the rest carried up in its smoke (climate.md).
pub const FIRE_HEAT_W: f32 = 5_000.0;
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
    pub fn boxed(half: Vec3, station: Option<crate::crafting::Station>) -> Self {
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
    /// It has an emissive map, as a campfire's logs' glowing cracks or its painted coals: on
    /// a burning prop they glow as its fire burns now (`Embers`); on any other a crack map
    /// stays dark and painted coals glow steadily.
    pub embers: bool,
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
    app.add_observer(solid::solid_or_not)
        .add_systems(Startup, (load, start_checking))
        .add_systems(Update, (keep_clear, settle, (kindle, smoulder).chain()));
}

/// A burning prop's part whose emissive map glows as its fire burns now, as a campfire's
/// logs' cracks do, on a material of its own (`kindle`): one flicker drives the fire's light,
/// its flames and its embers, as Sons of the Forest links its fires' (D415).
#[derive(Component)]
pub struct Embers;

/// How bright the embers glow at the fire's mean, on their map's colour. At 14 (a burning
/// building's char's, build.wesl) a campfire's cracks and coals went past the tonemapper's
/// shoulder to pink-white; at 2 they read red-orange (fire round 10). Since the firelit
/// shoulder is taken a channel at a time (fire round 28) a brighter ember goes orange, not
/// pink, so 4, with the coal bed's crackle (tools/props/coals.ts, fire round 29).
const EMBER_GLOW: f32 = 4.0;
/// How far a fire lights the embers of what burns: a prop's own fire stands at its foot.
const EMBER_REACH_M: f32 = 2.0;
/// The embers' exposure weight (docs/fire.md, "The exposure rule"): 0, physical, so by day,
/// when the camera takes in 2.8 stops less than at dusk, they glow at a seventh of their
/// night's brightness, as a real fire's coals are dull in sunlight, not red blotches on the
/// logs at noon (fire round 11).
const EMBER_WEIGHT: f32 = 0.0;

/// Each burning part given its own material, so its embers flicker with its own fire, and
/// drawn as a glow (`fire::GlowMaterial`): its glow's share of its light tonemapped as a
/// flame's, out of the night's grade (D415).
fn kindle(
    mut commands: Commands,
    added: Query<(Entity, &MeshMaterial3d<StandardMaterial>), Added<Embers>>,
    standard: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<terra_world::fire::GlowMaterial>>,
) {
    for (entity, material) in &added {
        let Some(own) = standard.get(&material.0).cloned() else {
            continue;
        };
        let glow = terra_world::fire::GlowMaterial {
            base: own,
            extension: terra_world::fire::Glow::default(),
        };
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(materials.add(glow)));
    }
}

/// The embers brighten and dim with the nearest fire's flicker, by three times as much as
/// its light, as a burning building's char does (build.wesl `fire_glow`); dark with no fire
/// near.
fn smoulder(
    embers: Query<
        (
            &MeshMaterial3d<terra_world::fire::GlowMaterial>,
            &GlobalTransform,
        ),
        With<Embers>,
    >,
    glows: Option<Res<terra_world::fire::FireGlows>>,
    camera: Query<&bevy::camera::Exposure, With<terra_world::planet::MainCamera>>,
    mut materials: ResMut<Assets<terra_world::fire::GlowMaterial>>,
) {
    let Some(glows) = glows else {
        return;
    };
    // The camera's exposure multiplier in stops, as Bevy makes it from EV100; never brighter
    // than at dusk, where the embers were tuned.
    let daylit = camera.single().map_or(1.0, |exposure| {
        let stops = -exposure.ev100 - libm::log2f(1.2);
        let rule = terra_world::fire::FireExposure {
            weight: EMBER_WEIGHT,
            ..default()
        };
        rule.factor(stops).min(1.0)
    });
    for (material, at) in &embers {
        let here = at.translation();
        let glow = glows
            .glows
            .iter()
            .filter(|fire| fire[3] > 0.0)
            .map(|fire| (Vec3::new(fire[0], fire[1], fire[2]).distance(here), fire[3]))
            .filter(|(d, _)| *d < EMBER_REACH_M)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(0.0, |(_, glow)| (1.0 + 3.0 * (glow - 1.0)).max(0.15));
        // Its colour scaled, not its alpha: `LinearRgba * f32` scales all four.
        let strength = EMBER_GLOW * glow * daylit;
        let emissive = LinearRgba::rgb(strength, strength, strength);
        // Only when it changed, so a material is not written again for nothing.
        let changed = materials
            .get(&material.0)
            .is_some_and(|m| m.base.emissive != emissive);
        if changed && let Some(mut m) = materials.get_mut(&material.0) {
            m.base.emissive = emissive;
        }
    }
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
    as_prop(&StandardMaterial {
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
        // A part's own emissive map, dark until a fire tells it how bright (`smoulder`).
        emissive_texture: if glows {
            Some(colour)
        } else {
            textures.emissive.map(load)
        },
        ..default()
    })
}

/// A textured part's material as a prop draws it: its own maps and factors, the rest the
/// props' light response (`lit`). The one path for a prop's textured part, placed
/// (`textured_material`) or staged from its glTF (`staged::quench`): Bevy's glTF loader gives
/// a reflectance of 0.5, which drew a staged campfire's char brighter than a placed one's 0.3
/// (fire round 37).
pub fn as_prop(own: &StandardMaterial) -> StandardMaterial {
    StandardMaterial {
        base_color: own.base_color,
        base_color_texture: own.base_color_texture.clone(),
        normal_map_texture: own.normal_map_texture.clone(),
        metallic_roughness_texture: own.metallic_roughness_texture.clone(),
        occlusion_texture: own.occlusion_texture.clone(),
        emissive: own.emissive,
        emissive_texture: own.emissive_texture.clone(),
        metallic: own.metallic,
        perceptual_roughness: own.perceptual_roughness,
        alpha_mode: own.alpha_mode,
        double_sided: own.double_sided,
        cull_mode: own.cull_mode,
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
            // A glowing part's emissive map is its own colour (`textured_material`).
            embers: glowing || textures.emissive.is_some(),
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
                embers: false,
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
        &crate::inventory::Items::default(),
        &crate::crafting::Stations::read(),
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
    let burning = matches!(item.light, Light::Fire);
    for piece in &model.pieces {
        // A burning prop's flat-coloured flame gives way to the fire drawn in its place; its
        // painted glowing parts are its coals and burning faces (a campfire's `Fire` faces
        // are cut from its logs, fire round 10), which glow with its fire.
        if piece.glows && burning && !piece.embers {
            continue;
        }
        let drawn = (
            Mesh3d(piece.mesh.clone()),
            MeshMaterial3d(piece.material.clone()),
        );
        match (piece.glows, piece.embers && burning) {
            (true, true) => prop.with_child((drawn, NotShadowCaster, Embers)),
            (true, false) => prop.with_child((drawn, NotShadowCaster)),
            (false, true) => prop.with_child((drawn, Embers)),
            (false, false) => prop.with_child(drawn),
        };
    }
    match item.light {
        Light::None => {}
        Light::Fire => {
            prop.with_child((
                CAMPFIRE,
                terra_world::climate::Heat { watts: FIRE_HEAT_W },
                Transform::from_xyz(0.0, CAMPFIRE_FOOT_M, 0.0),
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

/// Hides whatever the base's current shape leaves out in the open or shut in a wall.
fn keep_clear(
    site: Res<Site>,
    checked: Option<ResMut<CheckedFor>>,
    // Only the player's: what a town placed is not in their base.
    mut props: Query<(&Prop, &mut Visibility), Without<Theirs>>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_staged_part_from_its_gltf_is_lit_as_a_placed_prop() {
        // As Bevy's glTF loader makes a part: reflectance 0.5, the file's maps and factors.
        let map = Handle::<Image>::default();
        let loaded = StandardMaterial {
            base_color_texture: Some(map.clone()),
            emissive_texture: Some(map.clone()),
            metallic_roughness_texture: Some(map.clone()),
            perceptual_roughness: 0.95,
            reflectance: 0.5,
            ..default()
        };
        let drawn = as_prop(&loaded);
        assert_eq!(drawn.reflectance, lit().reflectance);
        assert!(drawn.reflectance < 0.5);
        // Its own maps and roughness kept.
        assert_eq!(drawn.base_color_texture, Some(map.clone()));
        assert_eq!(drawn.emissive_texture, Some(map.clone()));
        assert_eq!(drawn.metallic_roughness_texture, Some(map));
        assert_eq!(drawn.perceptual_roughness, 0.95);
    }
}
