//! Construction: a base you shape, drawn from the rules in `terra_build`.
//!
//! Everything built is half-metre cubes (D181), drawn plain or by its kind's kit
//! (docs/building-kits.md): walls of logs as logs.
//!
//! Point the middle of the view at the base. V cycles the wall you are looking at through
//! solid, window, tall window and door. Point at open ground and B adds a room there.
//!
//! The base starts as a small example, built through the same operations the keys use.
//!
//! With the `play` experiment on, a player's builder tool drives this instead.

use crate::planet::MainCamera;
use crate::planet::Planet;
use crate::play::{Footprint, Steps};
use crate::start::Start;
pub(crate) use crate::start::lifted;
use bevy::{
    asset::RenderAssetUsages, light::NotShadowCaster, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::SystemTime;
use terra_build::logs::{self, Kits, LogKit, Pattern};
use terra_build::{Base, BuildOp, CELL_M, Kind, Panel, STOREY_M, Surface, Walker, example};
use terra_build::{Material, Surfaces};
use terra_core::Operation;

mod pieces;
mod relief;
mod sheet;

const REACH_M: f32 = 300.0;
/// The foundation stands this far out from the walls, and reaches this far into the ground.
const FOUNDATION_LEDGE_M: f32 = 0.14;
const FOUNDATION_DEPTH_M: f32 = 0.7;
/// Half the width of the footing under a wall that stands on the ground.
const WALL_FOOTING_HALF_M: f32 = 0.1;
/// Steps run this far out for every metre they have to climb.
const RAMP_RUN_PER_RISE: f32 = 2.4;
/// Steps are this much wider than the doorway they lead to.
const STEPS_WIDER_M: f32 = 0.5;
const STEP_RISE_M: f32 = 0.17;
const CONCRETE: Color = Color::srgb(0.5, 0.48, 0.47);

/// Present when something else is driving construction, such as a player holding a
/// builder tool. The keys and camera here then stand aside.
#[derive(Resource)]
pub(super) struct DrivenByPlayer;

/// Something built from parts (D108): its shape, the operations that made it, where it
/// stands, and what is drawn for it. The player's own is the [`Site`]; anyone else's,
/// such as a village house, is [`Built`]. Both are made, drawn and walked on the same way.
pub(super) struct Build {
    pub base: Base,
    /// Places the build's own space on the planet.
    pub frame: Transform,
    pub operations: Vec<BuildOp>,
    /// Goes up by one every time the build changes shape.
    pub revision: u64,
    /// Steps from each ground-floor door down to the ground. A walker treats each flight
    /// as a ramp.
    ramps: Vec<Ramp>,
    /// Where it stands on the ground, as the world sees it, remade with every redraw.
    pub footprint: Footprint,
    structure: Entity,
    glass: Entity,
    foundation: Entity,
    steps: Entity,
    /// With kits, walls of their kinds are laid as their pieces (docs/building-kits.md), drawn as
    /// they stand.
    pub kits: Option<Arc<Kits>>,
    pub shown: Shown,
    /// What stands of it while it goes up, which is what a walker meets: `None` once it is
    /// all built, when that is the base itself.
    standing: Option<Base>,
    /// What a kit's pieces and materials are drawn under.
    pieces: Entity,
    /// The kit's pieces' models, once loaded, and the copies of them placed.
    kit_pieces: Option<Arc<pieces::KitPieces>>,
    model_entities: Vec<Entity>,
    /// The block kinds' relief, once loaded.
    relief: Option<Arc<relief::Relief>>,
    /// What pieces not yet in, and the next, are drawn in.
    ghost_material: Handle<StandardMaterial>,
    next_material: Handle<StandardMaterial>,
    /// The kit's materials, textured, and what is drawn plain; and what is drawn in them.
    materials: Arc<BTreeMap<Material, Handle<StandardMaterial>>>,
    plain_material: Handle<StandardMaterial>,
    textured_entities: Vec<Entity>,
}

/// How far a build's pieces are in (D139, D177): built, or a plan with so much of it in,
/// from nothing, a blueprint, to all of it. Plans proper come with step 3 of
/// docs/building-tech.md; until then, the kit sheet shows each state this way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shown {
    Built,
    Plan(f32),
}

/// How much ground a room set into it clears round its walls.
const ROOM_CLEARANCE_M: f32 = 0.2;

/// The build under construction by the player, and where it stands.
#[derive(Resource, Deref, DerefMut)]
pub(super) struct Site {
    #[deref]
    pub build: Build,
    camera_placed: bool,
}

/// Marks the [`Footprint`] the world sees of the player's base.
#[derive(Component)]
struct SiteFootprint;

/// Gives the world the base's footprint whenever the base changes shape or place.
fn keep_footprint(
    site: Res<Site>,
    mut seen: Local<Option<u64>>,
    mut footprint: Single<&mut Footprint, With<SiteFootprint>>,
) {
    if *seen != Some(site.revision) {
        *seen = Some(site.revision);
        footprint.set_if_neq(site.footprint.clone());
    }
}

/// A build that is not the player's, such as a house in a village.
#[derive(Component, Deref, DerefMut)]
pub(super) struct Built(pub Build);

/// What every build is drawn with: its structure, its glass, and the concrete of its
/// foundations and steps. Colour comes from each triangle, so these serve every build.
#[derive(Resource, Clone)]
pub(super) struct BuildLook {
    structure: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    concrete: Handle<StandardMaterial>,
    /// A plan's pieces not yet in, and the next one.
    ghost: Handle<StandardMaterial>,
    next: Handle<StandardMaterial>,
    /// The kits, each drawing the cubes of its kind: logs, stone (D179).
    pub kits: Option<Arc<Kits>>,
    /// The kind of construction walls, floors and roofs are laid in (`--walls`).
    pub laying: Kind,
    /// The kits' pieces' models, once loaded.
    kit_pieces: Option<Arc<pieces::KitPieces>>,
    /// The block kinds' relief, once loaded.
    relief: Option<Arc<relief::Relief>>,
    /// The kits' materials, textured (D14).
    materials: Arc<BTreeMap<Material, Handle<StandardMaterial>>>,
}

/// The kits' files, `assets/data/kits/`, read again when one is saved so a wall's look can
/// be tuned while the game runs.
#[derive(Resource)]
struct KitFile {
    modified: Option<SystemTime>,
    timer: Timer,
}

const KITS: &str = "assets/data/kits";
/// The kit things are laid in by default, which comes first.
const FIRST_KIT: &str = "logs.ron";

/// Every kit's file: the first kit, then the rest by name.
fn kit_paths() -> Vec<std::path::PathBuf> {
    let folder = bevy::asset::io::file::FileAssetReader::get_base_path().join(KITS);
    let mut paths: Vec<_> = std::fs::read_dir(&folder)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|e| e == "ron"))
                .collect()
        })
        .unwrap_or_default();
    paths.sort_by_key(|path| (!path.ends_with(FIRST_KIT), path.clone()));
    paths
}

/// When a kit's file last changed.
fn modified() -> Option<SystemTime> {
    kit_paths()
        .iter()
        .filter_map(|path| std::fs::metadata(path).and_then(|m| m.modified()).ok())
        .max()
}

/// Reads every kit. A mistake in one is said in the log and that kit left out; `None` if
/// none could be read.
fn read_kits() -> Option<Kits> {
    let kits: Vec<LogKit> = kit_paths()
        .iter()
        .filter_map(|path| read_kit(path))
        .collect();
    (!kits.is_empty()).then_some(Kits(kits))
}

fn read_kit(path: &std::path::Path) -> Option<LogKit> {
    let text = std::fs::read_to_string(path)
        .map_err(|problem| error!("Could not read {}: {problem}", path.display()))
        .ok()?;
    let kit: LogKit = ron::from_str(&text)
        .map_err(|problem| error!("{}: {problem}", path.display()))
        .ok()?;
    match (kit.pattern, kit.courses()) {
        (Pattern::Masonry, _) | (_, (_, true)) => Some(kit),
        _ => {
            error!(
                "{}: a course of {} m does not divide the storey",
                path.display(),
                kit.course
            );
            None
        }
    }
}

#[derive(Component)]
struct Readout;

/// Marks that the construction systems are in place, so asking for them twice is safe.
#[derive(Resource)]
struct Added;

pub fn plugin(app: &mut App) {
    if app.world().contains_resource::<Added>() {
        return;
    }
    let free = not(resource_exists::<DrivenByPlayer>);
    app.insert_resource(Added)
        .add_systems(Startup, setup)
        .add_systems(
            PostStartup,
            keep_footprint.before(crate::play::FloraSystems::Plant),
        )
        .add_systems(
            Update,
            keep_footprint.before(crate::play::FloraSystems::Clear),
        )
        .init_resource::<pieces::Loading>()
        .init_resource::<relief::Loading>()
        .add_systems(Update, (reload_kit, sheet::lay, pieces::load, relief::load))
        .add_systems(
            Update,
            (
                place_camera.run_if(free.clone()),
                edit.run_if(free.clone()),
                update_readout.run_if(free),
            )
                .chain(),
        );
}

/// A point just outside the example base's door, in the base's own space.
pub(super) const OUTSIDE_DOOR: Vec3 = Vec3::new(7.5, 0.0, 9.0);

/// A sloping walkway from a door's sill down to the ground, in the base's own space.
struct Ramp {
    /// The middle of the door's sill.
    top: Vec3,
    /// Level, pointing away from the door.
    outward: Vec3,
    length: f32,
    /// How far the far end is below the sill.
    drop: f32,
    width: f32,
}

impl Ramp {
    /// The ramp drawn as a flight of steps, each a block reaching down into the ground.
    fn steps(&self, surface: &mut Blocks) {
        let across = self.outward.cross(Vec3::Y) * (self.width * 0.5);
        let count = (self.drop / STEP_RISE_M).ceil().max(1.0);
        let tread = self.length / count;
        for step in 0..count as usize {
            let near = self.top + self.outward * (tread * step as f32);
            let far = near + self.outward * tread;
            // Level with the ramp at the middle of the tread.
            let top = self.top.y - self.drop * (step as f32 + 0.5) / count;
            let bottom = self.top.y - self.drop - FOUNDATION_DEPTH_M;
            surface.block(
                (near - across).min(far + across).with_y(bottom),
                (near - across).max(far + across).with_y(top),
            );
        }
    }

    /// The height of the ramp's surface under a point, if the point is over the ramp.
    fn height_at(&self, point: Vec3) -> Option<f32> {
        let offset = point - self.top;
        let along = offset.dot(self.outward);
        let across = offset.dot(self.outward.cross(Vec3::Y));
        let on_ramp = (0.0..=self.length).contains(&along) && across.abs() <= self.width * 0.5;
        on_ramp.then(|| self.top.y - self.drop * along / self.length)
    }
}

impl Build {
    /// A build with nothing in it yet, standing at `frame`, with the entities its parts
    /// are drawn on.
    pub(super) fn new(
        commands: &mut Commands,
        look: &BuildLook,
        frame: Transform,
        name: &str,
    ) -> Self {
        let mut part = |what: &str, material: &Handle<StandardMaterial>| {
            commands
                .spawn((
                    Name::new(format!("{name} {what}")),
                    frame,
                    // Seen, and so are what is drawn under it, with or without a mesh.
                    Visibility::default(),
                    MeshMaterial3d(material.clone()),
                ))
                .id()
        };
        let structure = part("structure", &look.structure);
        let foundation = part("foundation", &look.concrete);
        let steps = part("steps", &look.concrete);
        let glass = part("glass", &look.glass);
        let pieces = part("pieces", &look.structure);
        commands.entity(glass).insert(NotShadowCaster);
        Self {
            base: Base::default(),
            frame,
            operations: Vec::new(),
            revision: 0,
            ramps: Vec::new(),
            footprint: Footprint {
                frame,
                cell_m: CELL_M,
                ..default()
            },
            structure,
            glass,
            foundation,
            steps,
            kits: look.kits.clone(),
            shown: Shown::Built,
            standing: None,
            pieces,
            kit_pieces: look.kit_pieces.clone(),
            relief: look.relief.clone(),
            model_entities: Vec::new(),
            materials: look.materials.clone(),
            plain_material: look.structure.clone(),
            textured_entities: Vec::new(),
            ghost_material: look.ghost.clone(),
            next_material: look.next.clone(),
        }
    }

    /// A point on the ground a little beyond the foot of the first door's ramp, facing
    /// which a walker would climb the ramp into the base. Returns the point and the
    /// direction to face, both in the base's own space.
    pub(super) fn approach(&self) -> (Vec3, Vec3) {
        match self.ramps.first() {
            Some(ramp) => (ramp.top + ramp.outward * (ramp.length + 2.5), -ramp.outward),
            None => (OUTSIDE_DOOR, Vec3::NEG_Z),
        }
    }

    /// The height of whatever the base offers to stand on at a point in its own space: a
    /// floor, a roof or a ramp, whichever is highest within a step of the feet.
    pub(super) fn support(&self, point: Vec3, walker: &Walker) -> Option<f32> {
        let ramp = self
            .ramps
            .iter()
            .filter_map(|ramp| ramp.height_at(point))
            .filter(|&height| height <= point.y + walker.step_up)
            .max_by(|a, b| a.total_cmp(b));
        match (self.walkable().support(point, walker), ramp) {
            (Some(floor), Some(ramp)) => Some(floor.max(ramp)),
            (floor, ramp) => floor.or(ramp),
        }
    }

    /// What a walker meets: what stands of the base so far.
    pub(super) fn walkable(&self) -> &Base {
        self.standing.as_ref().unwrap_or(&self.base)
    }

    /// Whether a point in the world is under the base, or within a margin of it: in a room,
    /// on a floor, or by any wall, such as one standing alone.
    pub(super) fn covers(&self, point: Vec3, margin: f32) -> bool {
        self.footprint.covers(point, margin)
    }

    /// Moves the build to stand somewhere else, as when a base is begun where its claim
    /// is: its parts are drawn there from then on.
    pub(super) fn move_to(
        &mut self,
        frame: Transform,
        commands: &mut Commands,
        planet: &Planet,
        meshes: &mut Assets<Mesh>,
    ) {
        self.frame = frame;
        for part in [self.structure, self.glass, self.foundation, self.steps] {
            commands.entity(part).insert(frame);
        }
        self.revision += 1;
        redraw(commands, self, planet, meshes);
    }

    /// Applies several operations as one act, and redraws the base once. Returns whether
    /// anything changed.
    /// The ground a room from one tile to another would cut away to stand in it: its
    /// whole space from the floor to above the roof, and a little round its walls, so a
    /// room set into a slope has room. What is under the floor stays, for the foundation.
    pub(super) fn cut_for_room(&self, min: IVec3, max: IVec3) -> Operation {
        let low = Vec3::new(
            min.x as f32 * CELL_M,
            min.y as f32 * STOREY_M,
            min.z as f32 * CELL_M,
        ) - Vec3::new(ROOM_CLEARANCE_M, 0.05, ROOM_CLEARANCE_M);
        let high = Vec3::new(
            (max.x + 1) as f32 * CELL_M,
            (max.y + 1) as f32 * STOREY_M + 0.6,
            (max.z + 1) as f32 * CELL_M,
        ) + Vec3::new(ROOM_CLEARANCE_M, 0.0, ROOM_CLEARANCE_M);
        Operation::DigBox {
            centre: self.frame.transform_point((low + high) * 0.5),
            half: (high - low) * 0.5,
            rotation: self.frame.rotation,
        }
    }

    pub(super) fn apply_all(
        &mut self,
        ops: &[BuildOp],
        commands: &mut Commands,
        planet: &Planet,
        meshes: &mut Assets<Mesh>,
    ) -> bool {
        let mut changed = false;
        for &op in ops {
            if self.base.apply(op) {
                self.operations.push(op);
                changed = true;
            }
        }
        if changed {
            self.revision += 1;
            info!("Applied {} operations, such as {:?}", ops.len(), ops[0]);
            redraw(commands, self, planet, meshes);
        }
        changed
    }

    /// Starts the base again from a list of operations, as when a saved world is loaded.
    pub(super) fn replace_with(
        &mut self,
        ops: &[BuildOp],
        commands: &mut Commands,
        planet: &Planet,
        meshes: &mut Assets<Mesh>,
    ) {
        self.base = Base::default();
        self.operations.clear();
        if !self.apply_all(ops, commands, planet, meshes) {
            self.revision += 1;
            redraw(commands, self, planet, meshes);
        }
    }

    /// Applies an operation and redraws the base. Returns whether anything changed.
    pub(super) fn apply(
        &mut self,
        op: BuildOp,
        commands: &mut Commands,
        planet: &Planet,
        meshes: &mut Assets<Mesh>,
    ) -> bool {
        let changed = self.base.apply(op);
        if changed {
            self.operations.push(op);
            self.revision += 1;
            info!("Applied {op:?}");
            redraw(commands, self, planet, meshes);
        } else {
            info!("{op:?} changed nothing.");
        }
        changed
    }
}

/// Axis-aligned boxes in the base's own space, as flat-faced triangles of one colour.
#[derive(Default)]
struct Blocks {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
}

impl Blocks {
    fn block(&mut self, min: Vec3, max: Vec3) {
        let corner = |x: bool, y: bool, z: bool| {
            Vec3::new(
                if x { max.x } else { min.x },
                if y { max.y } else { min.y },
                if z { max.z } else { min.z },
            )
        };
        // Each side as four corners, anticlockwise seen from outside.
        let sides = [
            (
                Vec3::X,
                [
                    (true, false, true),
                    (true, false, false),
                    (true, true, false),
                    (true, true, true),
                ],
            ),
            (
                Vec3::NEG_X,
                [
                    (false, false, false),
                    (false, false, true),
                    (false, true, true),
                    (false, true, false),
                ],
            ),
            (
                Vec3::Y,
                [
                    (false, true, true),
                    (true, true, true),
                    (true, true, false),
                    (false, true, false),
                ],
            ),
            (
                Vec3::NEG_Y,
                [
                    (false, false, false),
                    (true, false, false),
                    (true, false, true),
                    (false, false, true),
                ],
            ),
            (
                Vec3::Z,
                [
                    (false, false, true),
                    (true, false, true),
                    (true, true, true),
                    (false, true, true),
                ],
            ),
            (
                Vec3::NEG_Z,
                [
                    (true, false, false),
                    (false, false, false),
                    (false, true, false),
                    (true, true, false),
                ],
            ),
        ];
        for (normal, corners) in sides {
            let [a, b, c, d] = corners.map(|(x, y, z)| corner(x, y, z));
            self.positions.extend([a, b, c, a, c, d]);
            self.normals.extend([normal; 6]);
        }
    }

    fn into_mesh(self) -> Option<Mesh> {
        if self.positions.is_empty() {
            return None;
        }
        Some(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals),
        )
    }
}

/// Where a base begun at a point on the ground stands: level there, its floor at the ground,
/// and turned as `like` is, so its grid keeps the same heading wherever it is begun (D57).
pub(super) fn frame_at(point: Vec3, like: &Transform) -> Transform {
    let up = point.normalize();
    Transform {
        translation: point,
        rotation: Quat::from_rotation_arc(*like.up(), up) * like.rotation,
        scale: Vec3::ONE,
    }
}

fn setup(
    mut commands: Commands,
    planet: Res<Planet>,
    start: Res<Start>,
    options: Res<crate::shell::Options>,
    driven: Option<Res<DrivenByPlayer>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    // The base is first laid at the start, whether or not anything is built there.
    let frame = **start;
    let look = BuildLook {
        structure: materials.add(StandardMaterial {
            perceptual_roughness: 0.82,
            reflectance: 0.3,
            ..default()
        }),
        glass: materials.add(StandardMaterial {
            base_color: Color::srgba(0.62, 0.86, 0.95, 0.3),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.06,
            reflectance: 0.7,
            cull_mode: None,
            double_sided: true,
            ..default()
        }),
        concrete: materials.add(StandardMaterial {
            base_color: CONCRETE,
            perceptual_roughness: 0.95,
            ..default()
        }),
        ghost: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.38),
            emissive: LinearRgba::rgb(0.05, 0.12, 0.25),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.6,
            ..default()
        }),
        next: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.78, 0.35, 0.75),
            emissive: LinearRgba::rgb(0.6, 0.35, 0.05),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        kits: read_kits().map(Arc::new),
        laying: Kind::PLAIN,
        kit_pieces: None,
        relief: None,
        materials: Arc::default(),
    };
    let materials_of_kit = look
        .kits
        .as_deref()
        .map(|kits| kit_materials(kits, &assets, &mut materials))
        .unwrap_or_default();
    let look = BuildLook {
        materials: materials_of_kit,
        laying: match (
            options.walls.as_deref(),
            look.kits.as_deref().and_then(|k| k.0.first()),
        ) {
            (Some("plain"), _) | (_, None) => Kind::PLAIN,
            (_, Some(kit)) => kit.kind(),
        },
        ..look
    };
    commands.insert_resource(KitFile {
        modified: modified(),
        timer: Timer::from_seconds(1.0, TimerMode::Repeating),
    });
    let mut build = Build::new(&mut commands, &look, frame, "Base");
    let operations = if driven.is_some() {
        Vec::new()
    } else {
        example(look.laying)
    };
    build.replace_with(&operations, &mut commands, &planet, &mut meshes);
    commands.insert_resource(look);
    commands.spawn((
        Name::new("Base footprint"),
        SiteFootprint,
        build.footprint.clone(),
    ));
    commands.insert_resource(Site {
        build,
        camera_placed: false,
    });

    // A player brings their own readout and aiming mark.
    if driven.is_some() {
        return;
    }
    commands.spawn((
        Readout,
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12),
            left: px(12),
            ..default()
        },
    ));
    // A mark in the middle of the view, which is what the build keys aim with.
    commands.spawn((
        Text::new("+"),
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            margin: UiRect {
                left: px(-5),
                top: px(-12),
                ..default()
            },
            ..default()
        },
    ));
}

fn to_mesh(surface: &Surface) -> Option<Mesh> {
    if surface.positions.is_empty() {
        return None;
    }
    // The base's colours are as they should look on screen; a mesh wants them linear.
    let colours: Vec<[f32; 4]> = surface
        .colours
        .iter()
        .map(|c| Color::srgb(c[0], c[1], c[2]).to_linear().to_f32_array())
        .collect();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, surface.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, surface.normals.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    // Textured: where each corner falls on its texture, and the tangents its normal map
    // needs.
    if surface.uvs.len() == surface.positions.len() {
        let count = surface.positions.len() as u32;
        mesh = mesh
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, surface.uvs.clone())
            .with_inserted_indices(bevy::mesh::Indices::U32((0..count).collect()));
        if let Err(problem) = mesh.generate_tangents() {
            warn!("No tangents for a textured surface: {problem}");
        }
    }
    Some(mesh)
}

/// The kits' materials, each from its textures: colour, normal, roughness and occlusion,
/// and parallax where it is given a depth (D14). Where two kits name the same material,
/// the first's textures are used.
fn kit_materials(
    kits: &Kits,
    assets: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> Arc<BTreeMap<Material, Handle<StandardMaterial>>> {
    let linear = |path: String| -> Handle<Image> {
        assets
            .load_builder()
            .with_settings(|settings: &mut bevy::image::ImageLoaderSettings| {
                settings.is_srgb = false;
            })
            .load(path)
    };
    let mut textures = BTreeMap::new();
    for kit in &kits.0 {
        for (material, texture) in &kit.textures {
            textures.entry(*material).or_insert(texture);
        }
    }
    Arc::new(
        textures
            .into_iter()
            .map(|(material, texture)| {
                let path = &texture.path;
                let arm: Handle<Image> = linear(format!("{path}/arm.jpg"));
                let parallax = texture.depth > 0.0;
                let standard = StandardMaterial {
                    base_color_texture: Some(assets.load(format!("{path}/color.jpg"))),
                    normal_map_texture: Some(linear(format!("{path}/normal.jpg"))),
                    metallic_roughness_texture: Some(arm.clone()),
                    occlusion_texture: Some(arm),
                    perceptual_roughness: 1.0,
                    metallic: 1.0,
                    depth_map: parallax.then(|| linear(format!("{path}/height.jpg"))),
                    parallax_depth_scale: texture.depth,
                    parallax_mapping_method:
                        bevy::pbr::ParallaxMappingMethod::DEFAULT_RELIEF_MAPPING,
                    max_parallax_layer_count: 16.0,
                    ..default()
                };
                (material, materials.add(standard))
            })
            .collect(),
    )
}

/// Rebuilds everything drawn for a build from its current shape.
fn redraw(commands: &mut Commands, site: &mut Build, planet: &Planet, meshes: &mut Assets<Mesh>) {
    let (mesh, states) = match site.kits.clone() {
        Some(kits) => {
            let (mesh, layouts) = site.base.mesh_with_kits(&kits);
            let laid: Vec<(&LogKit, &[logs::Piece])> = kits
                .0
                .iter()
                .zip(&layouts)
                .map(|(kit, layout)| (kit, layout.pieces.as_slice()))
                .collect();
            pieces::place(commands, site, &laid);
            site.standing = match site.shown {
                Shown::Built => None,
                shown => Some(site.base.standing(
                    laid.iter().flat_map(|&(_, pieces)| {
                        pieces[..pieces::done(pieces.len(), shown)].iter()
                    }),
                    false,
                )),
            };
            let models = site.kit_pieces.clone();
            let drawn = draw_pieces(
                &laid,
                site.shown,
                models.as_deref(),
                &site.base,
                site.relief.as_deref(),
            );
            (mesh, drawn)
        }
        None => {
            pieces::place(commands, site, &[]);
            site.standing = None;
            (site.base.mesh(|_| None), Default::default())
        }
    };
    // What is drawn in each material, built and planned: a mesh for each, under the
    // entity the pieces are drawn on.
    for entity in site.textured_entities.drain(..) {
        commands.entity(entity).despawn();
    }
    let [placed, ghost, next] = states;
    let built = [&mesh.textured, &placed];
    for (surfaces, state) in built
        .into_iter()
        .map(|surfaces| (surfaces, 0))
        .chain([(&ghost, 1), (&next, 2)])
    {
        for (material, surface) in &surfaces.0 {
            let Some(drawn) = to_mesh(surface) else {
                continue;
            };
            let look = match state {
                1 => site.ghost_material.clone(),
                2 => site.next_material.clone(),
                _ => site
                    .materials
                    .get(material)
                    .cloned()
                    .unwrap_or_else(|| site.plain_material.clone()),
            };
            let entity = commands
                .spawn((
                    Mesh3d(meshes.add(drawn)),
                    MeshMaterial3d(look),
                    Transform::IDENTITY,
                    ChildOf(site.pieces),
                ))
                .id();
            if state == 1 {
                commands.entity(entity).insert(NotShadowCaster);
            }
            site.textured_entities.push(entity);
        }
    }
    let mut show = |entity: Entity, mesh: Option<Mesh>| {
        match mesh {
            Some(mesh) => commands.entity(entity).insert(Mesh3d(meshes.add(mesh))),
            None => commands.entity(entity).remove::<Mesh3d>(),
        };
    };
    show(site.structure, to_mesh(&mesh.structure));
    show(site.glass, to_mesh(&mesh.glass));

    // A foundation under every ground-floor cell with a floor, down into the ground,
    // standing a little proud of the walls.
    let mut foundation = Blocks::default();
    for tile in site.base.floors().filter(|tile| tile.y == 0) {
        let ledge = |step: IVec3| {
            if site.base.has_floor(tile + step) {
                0.0
            } else {
                FOUNDATION_LEDGE_M
            }
        };
        let min =
            tile.as_vec3() * CELL_M - Vec3::new(ledge(IVec3::NEG_X), 0.0, ledge(IVec3::NEG_Z));
        let max = (tile + IVec3::new(1, 0, 1)).as_vec3() * CELL_M
            + Vec3::new(ledge(IVec3::X), 0.0, ledge(IVec3::Z));
        let gap = [
            (min.x, min.z),
            (max.x, min.z),
            (min.x, max.z),
            (max.x, max.z),
        ]
        .map(|(x, z)| planet.distance(site.frame.transform_point(Vec3::new(x, 0.0, z))))
        .into_iter()
        .fold(0.0, f32::max);
        foundation.block(min.with_y(-gap - FOUNDATION_DEPTH_M), max.with_y(-0.004));
    }
    // A footing under every ground-floor wall with no floor beside it, as a free-standing
    // wall's, down into the ground.
    for (tile, _) in site.base.walls().filter(|(tile, _)| tile.y == 0) {
        if site.base.has_floor(tile) {
            continue;
        }
        let middle = (tile.with_y(0).as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL_M;
        let half = Vec3::new(CELL_M * 0.5, 0.0, CELL_M * 0.5);
        let reach = Vec3::new(WALL_FOOTING_HALF_M, 0.0, WALL_FOOTING_HALF_M) - half;
        let (min, max) = (middle - half - reach.max(Vec3::ZERO), middle + half);
        let gap = [min, max, middle.with_x(min.x), middle.with_z(max.z)]
            .map(|at| planet.distance(site.frame.transform_point(at.with_y(0.0))))
            .into_iter()
            .fold(0.0, f32::max);
        foundation.block(min.with_y(-gap - FOUNDATION_DEPTH_M), max.with_y(0.02));
    }
    show(site.foundation, foundation.into_mesh());

    // Steps from every ground-floor door in an outside wall down to the ground.
    site.ramps.clear();
    let mut steps = Blocks::default();
    // One flight for each doorway, however many cells wide it is.
    let doorways: Vec<Vec<[i32; 3]>> = site
        .base
        .walls()
        .filter(|&(tile, panel)| tile.y == 0 && panel == Panel::Door)
        .map(|(tile, _)| {
            let run = site.base.run(tile);
            run.iter().map(|tile| tile.to_array()).collect()
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    for doorway in &doorways {
        let (first, last) = (
            IVec3::from_array(doorway[0]),
            IVec3::from_array(doorway[doorway.len() - 1]),
        );
        let Some(outward) = site.base.outward(first) else {
            continue;
        };
        let outward = outward.as_vec3();
        // The foot of the doorway's outer face, from one side to the other.
        let foot = |tile: IVec3| {
            (tile.with_y(0).as_vec3() + Vec3::new(0.5, 0.0, 0.5) + outward * 0.5) * CELL_M
        };
        let along = (last - first).as_vec3().normalize_or_zero() * CELL_M * 0.5;
        let (near, far) = (foot(first) - along, foot(last) + along);
        let top = (near + far) * 0.5;
        // Long enough for the climb, measured where the steps start and where they end.
        let rise_here = planet.distance(site.frame.transform_point(top)).max(0.0);
        let length = (rise_here * RAMP_RUN_PER_RISE).clamp(1.2, 16.0);
        let far_end = site.frame.transform_point(top + outward * length);
        let drop = planet.distance(far_end).max(0.0) + 0.05;
        let ramp = Ramp {
            top,
            outward,
            length,
            drop,
            width: near.distance(far) + STEPS_WIDER_M,
        };
        ramp.steps(&mut steps);
        site.ramps.push(ramp);
    }
    show(site.steps, steps.into_mesh());
    site.footprint = Footprint {
        frame: site.frame,
        cell_m: CELL_M,
        ground: site.base.ground_columns().collect(),
        standing: site.base.occupied_columns().collect(),
        steps: site
            .ramps
            .iter()
            .map(|ramp| Steps {
                top: ramp.top,
                outward: ramp.outward,
                length: ramp.length,
                width: ramp.width,
            })
            .collect(),
    };
}

/// Colour of a plan's pieces not yet in: each a shade apart, so they can be counted.
const GHOST: [f32; 3] = [0.62, 0.84, 1.0];

/// A kit's pieces as three sets of surfaces by material: those in, those still a ghost,
/// and the next to go in. Pieces a model draws are left out, but for the daub under a log.
fn draw_pieces(
    laid: &[(&LogKit, &[logs::Piece])],
    shown: Shown,
    models: Option<&pieces::KitPieces>,
    base: &Base,
    relief: Option<&relief::Relief>,
) -> [Surfaces; 3] {
    let [mut placed, mut ghost, mut next] = [
        Surfaces::default(),
        Surfaces::default(),
        Surfaces::default(),
    ];
    for &(kit, pieces) in laid {
        let [p, g, n] = draw_kit(pieces, kit, shown, models);
        let [bp, bg, bn] = draw_blocks(pieces, kit, shown, base, relief);
        let (p, g, n) = (joined(p, bp), joined(g, bg), joined(n, bn));
        for (into, from) in [(&mut placed, p), (&mut ghost, g), (&mut next, n)] {
            for (material, surface) in from.0 {
                let to = into.of(material);
                to.positions.extend(surface.positions);
                to.normals.extend(surface.normals);
                to.colours.extend(surface.colours);
                to.uvs.extend(surface.uvs);
            }
        }
    }
    [placed, ghost, next]
}

/// Two sets of surfaces as one.
fn joined(mut a: Surfaces, b: Surfaces) -> Surfaces {
    for (material, surface) in b.0 {
        let to = a.of(material);
        to.positions.extend(surface.positions);
        to.normals.extend(surface.normals);
        to.colours.extend(surface.colours);
        to.uvs.extend(surface.uvs);
    }
    a
}

/// A block kind's cubes as its surface (D197): those in, those not yet, and the next, each
/// closed where it meets the others but for what is already built.
fn draw_blocks(
    pieces: &[logs::Piece],
    kit: &LogKit,
    shown: Shown,
    base: &Base,
    relief: Option<&relief::Relief>,
) -> [Surfaces; 3] {
    if kit.pattern != Pattern::Masonry {
        return Default::default();
    }
    let done = pieces::done(pieces.len(), shown);
    let mut sets: [std::collections::BTreeSet<[i32; 3]>; 3] = Default::default();
    for (index, piece) in pieces.iter().enumerate() {
        if let logs::Piece::Block { cube } = piece {
            let set = match index.cmp(&done) {
                std::cmp::Ordering::Less => 0,
                std::cmp::Ordering::Equal => 2,
                std::cmp::Ordering::Greater => 1,
            };
            sets[set].insert(cube.to_array());
        }
    }
    let look = terra_build::surface::Look {
        bevel: kit.bevel,
        relief: kit.relief,
        steps: SURFACE_STEPS,
        metres: kit
            .textures
            .get(&Material::Masonry)
            .map_or([2.0, 2.0], |texture| texture.metres),
    };
    let height = |uv: [f32; 2]| relief.map_or(0.5, |relief| relief.at(uv));
    // Floors, and walls of kinds drawn as boxes, hide a face; walls of other kits do not,
    // being thinner than the cube.
    let hides_always = |at: IVec3| {
        base.cube(at).is_some_and(|cube| {
            cube.fill != terra_build::Fill::Open
                && cube.kind != kit.kind()
                && (!base.in_wall(at) || cube.kind == Kind::PLAIN)
        })
    };
    let mut out: [Surfaces; 3] = Default::default();
    for (set, colour) in [(0, [1.0; 3]), (1, GHOST), (2, [1.0; 3])] {
        // What is built hides what is not; the rest show their own faces.
        let built = &sets[0];
        let hides = |at: IVec3| hides_always(at) || (set != 0 && built.contains(&at.to_array()));
        terra_build::surface::draw(
            &sets[set],
            &hides,
            &look,
            &height,
            colour,
            out[set].of(Material::Masonry),
        );
    }
    out
}

/// How finely a block kind's cube faces are divided: a step every 5 cm or so.
const SURFACE_STEPS: u32 = 10;

/// One kit's pieces: those in, those not yet, and the next.
fn draw_kit(
    pieces: &[logs::Piece],
    kit: &LogKit,
    shown: Shown,
    models: Option<&pieces::KitPieces>,
) -> [Surfaces; 3] {
    let done = pieces::done(pieces.len(), shown);
    let [mut placed, mut ghost, mut next] = [
        Surfaces::default(),
        Surfaces::default(),
        Surfaces::default(),
    ];
    for (index, piece) in pieces.iter().enumerate() {
        let surfaces = if index < done {
            &mut placed
        } else if index == done {
            &mut next
        } else {
            &mut ghost
        };
        let before: BTreeMap<Material, usize> = surfaces
            .0
            .iter()
            .map(|(material, surface)| (*material, surface.colours.len()))
            .collect();
        if models.is_some_and(|models| models.draws(piece)) {
            logs::draw_chinking(piece, kit, surfaces);
        } else {
            logs::draw(piece, kit, surfaces);
        }
        if index > done {
            let shade = if index % 2 == 0 { 1.0 } else { 0.86 };
            let colour = GHOST.map(|c| c * shade);
            for (material, surface) in surfaces.0.iter_mut() {
                let from = before.get(material).copied().unwrap_or(0);
                surface.colours[from..].fill(colour);
            }
        }
    }
    [placed, ghost, next]
}

/// Reads the kits again when one's file is saved, and draws every build in them again.
#[allow(clippy::too_many_arguments)]
fn reload_kit(
    time: Res<Time>,
    mut file: ResMut<KitFile>,
    mut look: ResMut<BuildLook>,
    mut commands: Commands,
    planet: Res<Planet>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
    mut meshes: ResMut<Assets<Mesh>>,
    (assets, mut materials): (Res<AssetServer>, ResMut<Assets<StandardMaterial>>),
) {
    if !file.timer.tick(time.delta()).just_finished() {
        return;
    }
    let now = modified();
    if now == file.modified {
        return;
    }
    file.modified = now;
    let Some(kits) = read_kits().map(Arc::new) else {
        return;
    };
    info!("Read the kits in {KITS} again.");
    let textured = kit_materials(&kits, &assets, &mut materials);
    look.materials = textured.clone();
    if look.kits.is_some() {
        look.kits = Some(kits.clone());
    }
    for build in
        std::iter::once(&mut site.build).chain(builds.iter_mut().map(|b| &mut b.into_inner().0))
    {
        if build.kits.is_some() {
            build.kits = Some(kits.clone());
            build.materials = textured.clone();
            redraw(&mut commands, build, &planet, &mut meshes);
        }
    }
}

/// Moves the camera to a good view of the base, once.
fn place_camera(mut site: ResMut<Site>, mut camera: Single<&mut Transform, With<MainCamera>>) {
    if site.camera_placed {
        return;
    }
    site.camera_placed = true;
    let frame = site.frame;
    let eye = frame.transform_point(Vec3::new(4.0, 6.5, 26.0));
    let target = frame.transform_point(Vec3::new(6.0, 2.0, -3.0));
    **camera = Transform::from_translation(eye).looking_at(target, *frame.up());
}

fn edit(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
    planet: Res<Planet>,
    mut site: ResMut<Site>,
    look: Res<BuildLook>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut gizmos: Gizmos,
) {
    let site = &mut *site;
    // Aim with the middle of the view, in the base's own space.
    let to_base = site.frame.compute_affine().inverse();
    let origin = to_base.transform_point3(camera.translation());
    let direction = to_base.transform_vector3(*camera.forward()).normalize();
    let aimed = site.base.raycast(origin, direction, REACH_M);

    // Show what a key press would act on: the wall cell aimed at.
    if let Some(hit) = aimed {
        let corners = site
            .base
            .corners(hit.face.tile())
            .map(|corner| site.frame.transform_point(corner));
        gizmos.linestrip(
            [corners[0], corners[1], corners[2], corners[3], corners[0]],
            Color::srgb(1.0, 0.85, 0.2),
        );
    }

    let op = if let Some(hit) = aimed.filter(|hit| hit.wall) {
        keys.just_pressed(KeyCode::KeyV).then(|| {
            let tile = hit.face.tile();
            BuildOp::SetWall {
                tile: tile.to_array(),
                panel: Some(site.base.panel(tile).next()),
                kind: site.base.kind(tile).unwrap_or(look.laying),
            }
        })
    } else if keys.just_pressed(KeyCode::KeyB) {
        // A new room, three cells square, on the ground being looked at.
        planet
            .raycast(camera.translation(), *camera.forward(), REACH_M)
            .map(|ground| {
                let local = to_base.transform_point3(ground.point);
                let (x, z) = (
                    (local.x / CELL_M).floor() as i32,
                    (local.z / CELL_M).floor() as i32,
                );
                BuildOp::AddRoom {
                    min: [x - 2, 0, z - 2],
                    max: [x + 2, 0, z + 2],
                    kind: look.laying,
                }
            })
    } else {
        None
    };

    if let Some(op) = op {
        site.apply(op, &mut commands, &planet, &mut meshes);
    }
}

fn update_readout(site: Res<Site>, mut readout: Single<&mut Text, With<Readout>>) {
    let windows = site
        .base
        .walls()
        .filter(|(_, panel)| matches!(panel, Panel::Window { .. }))
        .count();
    readout.0 = format!(
        "Base: {} cells in rooms, {} wall cells, {} window cells, {} operations\n\
         Aim with the + mark. V window or door, B new room on open ground.",
        site.base.tile_count(),
        site.base.walls().count(),
        windows,
        site.operations.len()
    );
}
