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
use bevy::{
    asset::RenderAssetUsages, light::NotShadowCaster, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use std::sync::Arc;
use std::time::SystemTime;
use terra_build::logs::{self, LogKit};
use terra_build::{Base, BuildOp, CELL_M, Kind, Panel, STOREY_M, Side, Surface, Walker};
use terra_core::Operation;

mod pieces;
mod sheet;

const REACH_M: f32 = 300.0;
/// The base's floor clears the highest ground under it by this much.
const FLOOR_CLEARANCE_M: f32 = 0.12;
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
    structure: Entity,
    glass: Entity,
    foundation: Entity,
    steps: Entity,
    /// With a kit, its walls are laid as its pieces (docs/building-kits.md), drawn as they stand.
    pub logs: Option<Arc<LogKit>>,
    pub shown: Shown,
    /// The pieces that are in, those still a ghost, and the next to go in.
    pieces: Entity,
    ghost: Entity,
    next: Entity,
    /// The kit's log model, once loaded, and the copies of it placed for each log.
    log_model: Option<pieces::LogModel>,
    log_entities: Vec<Entity>,
    /// What pieces not yet in, and the next, are drawn in.
    ghost_material: Handle<StandardMaterial>,
    next_material: Handle<StandardMaterial>,
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
    /// Where the base first stood, before any claim moved it: "the start", the fixed place
    /// that every position given as metres east and south is measured from, by scripts,
    /// the map and the log, wherever the base is later begun.
    pub start: Transform,
    camera_placed: bool,
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
    /// The log kit, which draws the cubes of its kind as logs.
    pub logs: Option<Arc<LogKit>>,
    /// The kind of construction walls, floors and roofs are laid in (`--walls`).
    pub laying: Kind,
    /// The log kit's log model, once loaded.
    log_model: Option<pieces::LogModel>,
}

/// The log kit's file, `assets/data/kits/logs.ron`, read again when it is saved so a log
/// wall's look can be tuned while the game runs.
#[derive(Resource)]
struct KitFile {
    modified: Option<SystemTime>,
    timer: Timer,
}

const LOG_KIT: &str = "assets/data/kits/logs.ron";

fn kit_path() -> std::path::PathBuf {
    bevy::asset::io::file::FileAssetReader::get_base_path().join(LOG_KIT)
}

fn modified() -> Option<SystemTime> {
    std::fs::metadata(kit_path())
        .and_then(|m| m.modified())
        .ok()
}

/// Reads the log kit. A mistake in it is said in the log, and `None` returned.
fn read_kit() -> Option<LogKit> {
    let path = kit_path();
    let text = std::fs::read_to_string(&path)
        .map_err(|problem| error!("Could not read {}: {problem}", path.display()))
        .ok()?;
    let kit: LogKit = ron::from_str(&text)
        .map_err(|problem| error!("{}: {problem}", path.display()))
        .ok()?;
    match kit.courses() {
        (_, true) => Some(kit),
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
        .init_resource::<pieces::Loading>()
        .add_systems(Update, (reload_kit, sheet::lay, pieces::load))
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
                    MeshMaterial3d(material.clone()),
                ))
                .id()
        };
        let structure = part("structure", &look.structure);
        let foundation = part("foundation", &look.concrete);
        let steps = part("steps", &look.concrete);
        let glass = part("glass", &look.glass);
        let pieces = part("pieces", &look.structure);
        let ghost = part("ghost", &look.ghost);
        let next = part("next", &look.next);
        commands.entity(glass).insert(NotShadowCaster);
        commands.entity(ghost).insert(NotShadowCaster);
        commands.entity(next).insert(NotShadowCaster);
        Self {
            base: Base::default(),
            frame,
            operations: Vec::new(),
            revision: 0,
            ramps: Vec::new(),
            structure,
            glass,
            foundation,
            steps,
            logs: look.logs.clone(),
            shown: Shown::Built,
            pieces,
            ghost,
            next,
            log_model: look.log_model.clone(),
            log_entities: Vec::new(),
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
        match (self.base.support(point, walker), ramp) {
            (Some(floor), Some(ramp)) => Some(floor.max(ramp)),
            (floor, ramp) => floor.or(ramp),
        }
    }

    /// Whether a point in the world is under the base, or within a margin of it.
    pub(super) fn covers(&self, point: Vec3, margin: f32) -> bool {
        let local = self
            .frame
            .compute_affine()
            .inverse()
            .transform_point3(point);
        let reach = [-margin, 0.0, margin];
        reach.iter().any(|dx| {
            reach.iter().any(|dz| {
                let tile = ((local + Vec3::new(*dx, 0.0, *dz)) / CELL_M)
                    .floor()
                    .as_ivec3();
                self.base.contains(tile.with_y(0)) || self.base.has_floor(tile.with_y(0))
            })
        }) || self.ramps.iter().any(|ramp| {
            let offset = local - ramp.top;
            let along = offset.dot(ramp.outward);
            let across = offset.dot(ramp.outward.cross(Vec3::Y));
            (-margin..=ramp.length + margin).contains(&along)
                && across.abs() <= ramp.width * 0.5 + margin
        })
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

/// The example base: two rooms joined by a corridor, a small upper storey, windows on
/// every side and a door. Written in metres, so it is the same base whatever the grid.
fn example(kind: Kind) -> Vec<BuildOp> {
    // A block of tiles from one outside corner in metres to the other, on a storey. Rooms
    // that meet share the row of tiles between them, so they open into one another.
    let room = |west: f32, east: f32, north: f32, south: f32, storey: i32| BuildOp::AddRoom {
        min: [cells_from(west), storey, cells_from(north)],
        max: [cells_from(east) - 1, storey, cells_from(south) - 1],
        kind,
    };
    let mut ops = vec![
        room(-6.0, 9.0, -6.0, 3.0, 0),
        room(8.5, 15.5, -3.0, 0.0, 0),
        room(15.0, 21.0, -6.0, 3.0, 0),
        room(-3.0, 3.0, -6.0, 0.0, 1),
    ];
    // An opening in a wall: which way the wall faces, where the wall is, and the stretch
    // of it to open, all in metres.
    let mut cut = |side: Side, wall_at: f32, from: f32, to: f32, storey: i32, panel: Panel| {
        // The tile of the wall, just inside its outer face.
        let inside = match side {
            Side::East | Side::South => cells_from(wall_at) - 1,
            _ => cells_from(wall_at),
        };
        for along in cells_from(from)..cells_from(to) {
            let cell = match side {
                Side::East | Side::West => IVec3::new(inside, storey, along),
                _ => IVec3::new(along, storey, inside),
            };
            ops.push(BuildOp::SetWall {
                tile: cell.to_array(),
                panel: Some(panel),
                kind,
            });
        }
    };
    cut(Side::South, 3.0, -3.0, 6.0, 0, Panel::WINDOW);
    cut(Side::South, 3.0, 7.0, 8.5, 0, Panel::Door);
    cut(Side::West, -6.0, -2.5, -0.5, 0, Panel::WINDOW);
    cut(Side::South, 0.0, -2.5, 2.5, 1, Panel::TALL_WINDOW);
    cut(Side::South, 0.0, 12.5, 14.5, 0, Panel::WINDOW);
    cut(Side::South, 3.0, 18.5, 20.5, 0, Panel::WINDOW);
    cut(Side::East, 21.0, -2.5, -0.5, 0, Panel::TALL_WINDOW);
    cut(Side::North, -6.0, -5.5, -3.5, 0, Panel::WINDOW);
    cut(Side::North, -6.0, 6.5, 8.5, 0, Panel::WINDOW);
    cut(Side::North, -6.0, 18.5, 20.5, 0, Panel::WINDOW);
    cut(Side::North, -6.0, -2.5, -0.5, 1, Panel::WINDOW);
    ops
}

/// The grid line a distance in metres falls on. Unlike [`cells`], which counts, this
/// places, so it can be negative or zero.
fn cells_from(metres: f32) -> i32 {
    (metres / CELL_M).round() as i32
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

/// Where to stand a base: the most level ground near the start that is clear of the sea,
/// turned so its door looks out over the water if there is any.
fn choose_site(planet: &Planet, base: &Base) -> Transform {
    let start = planet.start();
    let up = start.normalize();
    let east = up.cross(Vec3::X).normalize();
    let north = east.cross(up);
    let sea = planet.sea_radius_m();

    let ground_floor = || base.tiles().filter(|(tile, _)| tile.y == 0);
    let low = ground_floor().fold(IVec3::MAX, |low, (tile, _)| low.min(tile));
    let high = ground_floor().fold(IVec3::MIN, |high, (tile, _)| high.max(tile));
    let (low, high) = (
        low.as_vec3() * CELL_M - 3.0,
        (high + 1).as_vec3() * CELL_M + 3.0,
    );
    let middle = (low + high) * 0.5;

    let mut best: Option<(f32, Transform)> = None;
    for ring in 3..22 {
        for spoke in 0..24 {
            let angle = spoke as f32 / 24.0 * std::f32::consts::TAU;
            let offset =
                (east * libm::cosf(angle) + north * libm::sinf(angle)) * (ring as f32 * 6.0);
            let anchor = planet.ground_at(start + offset);
            let anchor_up = anchor.normalize();
            for turn in 0..8 {
                let facing =
                    Quat::from_axis_angle(anchor_up, turn as f32 * std::f32::consts::TAU / 8.0)
                        * east.reject_from(anchor_up).normalize();
                let mut frame = Transform::from_translation(anchor).looking_to(facing, anchor_up);
                // Put the middle of the base, not its corner, on the chosen spot.
                frame.translation -= frame.rotation * Vec3::new(middle.x, 0.0, middle.z);

                // How far the ground under the base rises above and falls below the frame.
                let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
                let mut x = low.x;
                while x <= high.x {
                    let mut z = low.z;
                    while z <= high.z {
                        let height = -planet.distance(frame.transform_point(Vec3::new(x, 0.0, z)));
                        lowest = lowest.min(height);
                        highest = highest.max(height);
                        z += 3.0;
                    }
                    x += 3.0;
                }
                let above_sea = anchor.length() + lowest - sea;
                if above_sea < 1.6 {
                    continue;
                }
                if highest - lowest > 2.5 {
                    continue;
                }
                // Doors are on the base's +Z side; water a short walk out that way is
                // worth a little slope.
                let water_ahead = [40.0, 60.0, 85.0]
                    .iter()
                    .filter(|&&ahead| {
                        let point = frame.transform_point(Vec3::new(middle.x, 0.0, ahead));
                        planet.under_sea(point)
                    })
                    .count();
                // Grass to stand on, at the base and out in front of it.
                let grassy: f32 = [0.0, 12.0, 24.0]
                    .iter()
                    .map(|&ahead| {
                        let point = frame.transform_point(Vec3::new(middle.x, 0.0, ahead));
                        let point = planet.ground_at(point);
                        planet.cover(point, planet.normal(point)).grass
                    })
                    .sum();
                let score = (highest - lowest) - 0.8 * water_ahead as f32 - 0.5 * grassy
                    + 0.04 * (above_sea - 4.0).abs();
                if best.as_ref().is_none_or(|(least, _)| score < *least) {
                    best = Some((score, frame));
                }
            }
        }
    }
    best.map(|(_, frame)| frame).unwrap_or_else(|| {
        let anchor = planet.ground_at(start + east * 60.0);
        Transform::from_translation(anchor).looking_to(north, anchor.normalize())
    })
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

/// Raises a build's frame until its floor clears the highest ground under it. The
/// stand-in planet cannot be dug, so a build has to stay out of the ground's way; a
/// foundation and steps make up the difference.
pub(super) fn lifted(mut frame: Transform, base: &Base, planet: &Planet) -> Transform {
    let highest_ground = base
        .tiles()
        .filter(|(tile, _)| tile.y == 0)
        .flat_map(|(tile, _)| {
            [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (0.5, 0.5)].map(|(dx, dz)| {
                let on_floor = Vec3::new(
                    (tile.x as f32 + dx) * CELL_M,
                    0.0,
                    (tile.z as f32 + dz) * CELL_M,
                );
                -planet.distance(frame.transform_point(on_floor))
            })
        })
        .fold(f32::MIN, f32::max);
    let up = *frame.up();
    frame.translation += up * (highest_ground + FLOOR_CLEARANCE_M);
    frame
}

fn setup(
    mut commands: Commands,
    planet: Res<Planet>,
    options: Res<crate::shell::Options>,
    driven: Option<Res<DrivenByPlayer>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // The example base decides where the base stands, whether or not it is built: a
    // player starts with nothing built, on the same spot. On the yard, the flat test
    // level, the base's grid is laid at the start itself, facing east.
    let mut example_base = Base::default();
    for op in example(Kind::PLAIN) {
        example_base.apply(op);
    }
    let frame = if options.yard() {
        let start = planet.start();
        let up = start.normalize();
        let east = up.cross(Vec3::X).normalize();
        Transform::from_translation(start + up * FLOOR_CLEARANCE_M).looking_to(east, up)
    } else {
        lifted(choose_site(&planet, &example_base), &example_base, &planet)
    };
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
        logs: read_kit().map(Arc::new),
        laying: Kind::PLAIN,
        log_model: None,
    };
    let look = BuildLook {
        laying: match (options.walls.as_deref(), &look.logs) {
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
    commands.insert_resource(Site {
        build,
        start: frame,
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
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, surface.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, surface.normals.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours),
    )
}

/// Rebuilds everything drawn for a build from its current shape.
fn redraw(commands: &mut Commands, site: &mut Build, planet: &Planet, meshes: &mut Assets<Mesh>) {
    let (mesh, pieces) = match site.logs.clone() {
        Some(kit) => {
            let (mesh, layout) = site.base.mesh_with_logs(&kit);
            let modelled = site.log_model.is_some();
            pieces::place(commands, site, &layout.pieces, &kit);
            (
                mesh,
                draw_pieces(&layout.pieces, &kit, site.shown, modelled),
            )
        }
        None => {
            pieces::place(commands, site, &[], &LogKit::default());
            (site.base.mesh(|_| None), [None, None, None])
        }
    };
    let mut show = |entity: Entity, mesh: Option<Mesh>| {
        match mesh {
            Some(mesh) => commands.entity(entity).insert(Mesh3d(meshes.add(mesh))),
            None => commands.entity(entity).remove::<Mesh3d>(),
        };
    };
    let [placed, ghost, next] = pieces;
    show(site.structure, to_mesh(&mesh.structure));
    show(site.glass, to_mesh(&mesh.glass));
    show(site.pieces, placed);
    show(site.ghost, ghost);
    show(site.next, next);

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
}

/// Colour of a plan's pieces not yet in: each a shade apart, so they can be counted.
const GHOST: [f32; 3] = [0.62, 0.84, 1.0];

/// A kit's pieces as three meshes: those in, those still a ghost, and the next to go in.
/// Logs are left out when a model draws them.
fn draw_pieces(
    pieces: &[logs::Piece],
    kit: &LogKit,
    shown: Shown,
    modelled: bool,
) -> [Option<Mesh>; 3] {
    let done = match shown {
        Shown::Built => pieces.len(),
        Shown::Plan(fraction) => {
            ((pieces.len() as f32 * fraction.clamp(0.0, 1.0)) as usize).min(pieces.len())
        }
    };
    let mut placed = Surface::default();
    let mut ghost = Surface::default();
    let mut next = Surface::default();
    for (index, piece) in pieces.iter().enumerate() {
        let surface = if index < done {
            &mut placed
        } else if index == done {
            &mut next
        } else {
            &mut ghost
        };
        let from = surface.colours.len();
        if modelled && matches!(piece, logs::Piece::Log { .. }) {
            continue;
        }
        logs::draw(piece, kit, surface);
        if index > done {
            let shade = if index % 2 == 0 { 1.0 } else { 0.86 };
            let colour = GHOST.map(|c| c * shade);
            surface.colours[from..].fill(colour);
        }
    }
    [to_mesh(&placed), to_mesh(&ghost), to_mesh(&next)]
}

/// Reads the log kit again when its file is saved, and draws every build in logs again.
fn reload_kit(
    time: Res<Time>,
    mut file: ResMut<KitFile>,
    mut look: ResMut<BuildLook>,
    mut commands: Commands,
    planet: Res<Planet>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if !file.timer.tick(time.delta()).just_finished() {
        return;
    }
    let now = modified();
    if now == file.modified {
        return;
    }
    file.modified = now;
    let Some(kit) = read_kit().map(Arc::new) else {
        return;
    };
    info!("Read {LOG_KIT} again.");
    if look.logs.is_some() {
        look.logs = Some(kit.clone());
    }
    for build in
        std::iter::once(&mut site.build).chain(builds.iter_mut().map(|b| &mut b.into_inner().0))
    {
        if build.logs.is_some() {
            build.logs = Some(kit.clone());
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
