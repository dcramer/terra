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

use crate::planet::{MainCamera, Planet};
use crate::play::Footprint;
use crate::start::Start;
use bevy::prelude::*;
pub(crate) use footing::lifted;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::SystemTime;
use terra_build::kit::{Kit, Kits, Pattern};
use terra_build::ops::{Context, Done, Op, Refusal};
use terra_build::save::Snapshot;
use terra_build::{Base, BuildOp, CELL_M, Kind, Panel, Rules, STOREY_M, Walker, example};
use terra_core::Operation;

pub(crate) mod act;
mod arrays;
mod atlases;
mod draw;
mod footing;
pub(crate) use draw::PlanGhost;
pub(crate) mod fire;
mod going_up;
mod hlod;
mod layout;
mod light;
mod loss;
mod maps;
mod memory;
pub(crate) use terra_build::plans;
pub(crate) mod props;
mod remote;
mod sheet;
mod store;
mod views;
mod village;

use draw::{KitMaterials, kit_materials};
use terra_build::relief::Maps;
use terra_build::sections::Progress;

const REACH_M: f32 = 300.0;

/// Present when something else is driving construction, such as a player holding a
/// builder tool. The keys and camera here then stand aside.
#[derive(Resource)]
pub(super) struct DrivenByPlayer;

/// Something built from parts (D108): its shape, the operations that made it, where it
/// stands, and how it is drawn. The player's own is the [`Site`]; anyone else's, such as a
/// village house, is [`Built`]. Both are made, drawn and walked on the same way: the base
/// is an entity placed at `frame`, its sections of cubes its children, each laid and
/// merged on the workers as it changes (`store`, `layout`, `draw`).
pub(super) struct Build {
    pub base: Base,
    /// Places the build's own space on the planet.
    pub frame: Transform,
    /// The shortcuts that built it, for a build laid whole, as a village's houses are.
    pub operations: Vec<BuildOp>,
    /// The operations done to it since its last snapshot, which are kept and saved with
    /// it (construction-001, gap 4).
    pub snapshot: Option<Snapshot>,
    pub since: Vec<Op>,
    /// Goes up by one every time the build changes shape.
    pub revision: u64,
    /// The revision its steps and footprint were last worked out for.
    refreshed: u64,
    /// The cubes lit as what goes in next.
    next: BTreeSet<[i32; 3]>,
    /// Whether it is drawn as far as each of its cubes is built (the player's base, the
    /// kit sheet's rows of cubes in states), rather than whole (a village's houses).
    pub by_cubes: bool,
    /// Whether it is built by plans, as the player's base is, rather than laid whole by the
    /// shortcuts: it lays its own footings as plans.
    pub by_plans: bool,
    /// Where it stands on the ground, as the world sees it, remade as it changes.
    pub footprint: Footprint,
    /// With kits, walls of their kinds are laid as their pieces (docs/building-kits.md), drawn as
    /// they stand.
    pub kits: Option<Arc<Kits>>,
    pub shown: Shown,
    /// What stands of it while it goes up, which is what a walker meets: `None` once it is
    /// all built, when that is the base itself.
    standing: Option<Base>,
    /// The base's entity, at `frame`: its sections are its children.
    pub root: Entity,
    /// Its sections: what is drawn of each, and what is to be laid again.
    sections: store::Sections,
    /// What came down since it was last burst into fragments (`loss`).
    pub fallen: Vec<loss::Fallen>,
}

/// How far a build's pieces are in (D139, D177): built, or a plan with so much of it in,
/// from nothing, a blueprint, to all of it. Plans proper come with step 3 of
/// docs/building-tech.md; until then, the kit sheet shows each state this way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shown {
    Built,
    Plan(f32),
}

/// How many operations are kept before they are folded into a snapshot.
const FOLD_AFTER: usize = 400;

/// An act worked out against a copy of a base, on a worker or at once, ready to be swapped
/// in (`Build::swap`): the base after it, what it did, what came down or was chipped off,
/// the cubes it only repainted, and its operations.
pub(super) struct Applied {
    pub base: Base,
    pub done: Done,
    pub fallen: Vec<loss::Fallen>,
    pub repainted: Vec<([i32; 3], Kind, Kind)>,
    pub ops: Vec<Op>,
}

/// Does operations to a copy of a base as one act (D21): all of them or, if one is refused,
/// none; with what harm chipped off and brought down, and the cubes only repainted (D298).
/// Touches nothing else, so it runs on a worker against the copy (`act::Applying`).
pub(super) fn work_out(
    mut base: Base,
    kits: Option<Arc<Kits>>,
    ops: Vec<Op>,
    cx: &Context,
) -> Result<Applied, Refusal> {
    let named: BTreeSet<[i32; 3]> = ops.iter().flat_map(op_cubes).collect();
    let before: Vec<([i32; 3], Option<terra_build::Cube>)> = named
        .iter()
        .map(|&at| (at, base.cube(IVec3::from_array(at))))
        .collect();
    // What harm is aimed at, and how much of each it has lost already.
    let rules = kits.as_deref().map(Kits::rules).unwrap_or_default();
    let harmed_before: Vec<(IVec3, u8)> = ops
        .iter()
        .filter_map(|op| match op {
            Op::Damage { cubes, .. } => Some(cubes.iter().copied()),
            _ => None,
        })
        .flatten()
        .filter_map(|at| {
            let at = IVec3::from_array(at);
            let cube = base.cube(at)?;
            let toughness = rules.toughness(cube.kind);
            Some((at, toughness.quarters(base.damage(at))))
        })
        .collect();
    let done = base.act_all(&ops, cx)?;
    let mut fallen = Vec::new();
    // What came off what harm hit and still stands (D305).
    let mut chipped = Vec::new();
    for (at, before) in harmed_before {
        let Some(cube) = base.cube(at) else {
            continue;
        };
        let now = rules.toughness(cube.kind).quarters(base.damage(at));
        if now <= before {
            continue;
        }
        // Its open faces' way, or up.
        let out = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::Y]
            .into_iter()
            .filter(|&step| base.cube(at + step).is_none())
            .map(|step| step.as_vec3())
            .sum::<Vec3>()
            .normalize_or(Vec3::Y);
        chipped.push((at, cube, before, now, out));
    }
    if !chipped.is_empty() {
        fallen.push(loss::Fallen {
            chipped,
            ..default()
        });
    }
    // What came down, for its fragments: what harm destroyed, and what hung from it.
    if !done.fell.is_empty() {
        let hit: BTreeSet<[i32; 3]> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Damage { cubes, .. } => Some(cubes.iter().copied()),
                _ => None,
            })
            .flatten()
            .collect();
        fallen.push(loss::Fallen {
            destroyed: done
                .fell
                .iter()
                .map(|(at, _)| at.to_array())
                .filter(|at| hit.contains(at))
                .collect(),
            fell: done.fell.clone(),
            ..default()
        });
    }
    // A change of material and nothing else: the drawing looks it up at once (D298).
    let mut repainted = Vec::new();
    let mut only_kind = true;
    for (at, was) in before {
        let now = base.cube(IVec3::from_array(at));
        match (was, now) {
            (Some(was), Some(now))
                if (was.fill, was.shape, was.state) == (now.fill, now.shape, now.state) =>
            {
                if was.kind != now.kind {
                    repainted.push((at, was.kind, now.kind));
                }
            }
            _ => only_kind = false,
        }
    }
    if !only_kind {
        repainted.clear();
    }
    Ok(Applied {
        base,
        done,
        fallen,
        repainted,
        ops,
    })
}

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

/// Gives the world the footprint of each build that is not the player's as it is set
/// down, a village's houses and the kit sheet's, so what grows keeps out of it and a margin
/// round it, as it does of the player's (D108): the same for every build, from where it
/// stands, so a village is cleared the same way every time it is made from its seed.
fn give_footprints(
    mut commands: Commands,
    added: Query<(Entity, &Built), bevy::ecs::query::Added<Built>>,
) {
    for (entity, built) in &added {
        commands.entity(entity).insert(built.footprint.clone());
    }
}

/// A build that is not the player's, such as a house in a village.
#[derive(Component, Deref, DerefMut)]
pub(super) struct Built(pub Build);

/// What every build is drawn with: its kits and their materials and maps, what no kit
/// draws, a plan's ghosts and the next piece lit, far off sections, and the shapes
/// a ghost is drawn as (`draw`).
#[derive(Resource, Clone)]
pub(super) struct BuildLook {
    looks: draw::Looks,
    /// The kits, each drawing the cubes of its kind: logs, stone (D179).
    pub kits: Option<Arc<Kits>>,
    /// The kind of construction walls, floors and roofs are laid in (`--walls`).
    pub laying: Kind,
    /// The height and colour maps the kits' pieces are made with, as they load.
    maps: Arc<Maps>,
    /// The kits' materials, textured (D14).
    materials: Arc<KitMaterials>,
    /// The kits' texture sets as layers of the texture arrays (D290).
    layers: Arc<arrays::Layers>,
}

/// The kits' files, `assets/data/kits/`, read again when one is saved so a wall's look can
/// be tuned while the game runs.
///
/// Whether one has changed is asked on a worker, never the main thread: from a Windows copy
/// run from WSL the files are over WSL's share, and listing them took 140 ms a second there
/// when the machine was busy, a hitch every second (`cargo xtask qa`'s trace).
#[derive(Resource)]
struct KitFile {
    modified: Option<SystemTime>,
    timer: Timer,
    asking: Option<bevy::tasks::Task<Option<SystemTime>>>,
}

const KITS: &str = "data/kits";

/// The assets the kits' files are read from: those next to the program, or, for a Windows
/// copy run from WSL (`cargo xtask win`), the source's own, which it names in
/// `TERRA_SOURCE_ASSETS`, so a kit's file saved while it runs is seen.
fn kit_assets() -> std::path::PathBuf {
    std::env::var_os("TERRA_SOURCE_ASSETS")
        .map(std::path::PathBuf::from)
        .filter(|path| path.join(KITS).is_dir())
        .unwrap_or_else(terra_world::data::assets)
}

/// Every material's file, by name; `read_kits` puts them in order of id.
fn kit_paths() -> Vec<std::path::PathBuf> {
    let folder = kit_assets().join(KITS);
    let mut paths: Vec<_> = std::fs::read_dir(&folder)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|e| e == "ron"))
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

/// When a material's file, a pattern's or a pack's last changed (D290).
fn modified() -> Option<SystemTime> {
    let assets = kit_assets();
    let folders = [
        terra_build::kit::MATERIALS,
        terra_build::layers::PATTERNS,
        terra_build::layers::PACKS,
    ];
    folders
        .iter()
        .filter_map(|folder| std::fs::read_dir(assets.join(folder)).ok())
        .flat_map(|entries| entries.flatten())
        .filter_map(|entry| entry.metadata().and_then(|m| m.modified()).ok())
        .max()
}

/// Each kit's kind, by its file's name: `logs`, `stone`.
fn kit_names() -> BTreeMap<Kind, String> {
    kit_paths()
        .iter()
        .filter_map(|path| {
            let kit = read_kit(path)?;
            let name = path.file_stem()?.to_string_lossy().to_string();
            Some((kit.kind(), name))
        })
        .collect()
}

/// Reads every kit. A mistake in one is said in the log and that kit left out; `None` if
/// none could be read.
fn read_kits() -> Option<Kits> {
    let mut kits: Vec<Kit> = kit_paths()
        .iter()
        .filter_map(|path| read_kit(path))
        .collect();
    // By id: the first, which things are laid in by default, the lowest (D290).
    kits.sort_by_key(|kit| kit.id);
    (!kits.is_empty()).then_some(Kits(kits))
}

/// Every material read again, or none if one does not read: a material saved half-edited
/// while the game runs keeps the kits as they were, rather than taking it out from under
/// the bases built of it.
fn read_kits_again() -> Option<Kits> {
    let paths = kit_paths();
    let mut kits: Vec<Kit> = paths.iter().filter_map(|path| read_kit(path)).collect();
    if kits.len() < paths.len() {
        warn!("Build: a material in {KITS} does not read; the kits are kept as they were.");
        return None;
    }
    // By id: the first, which things are laid in by default, the lowest (D290).
    kits.sort_by_key(|kit| kit.id);
    (!kits.is_empty()).then_some(Kits(kits))
}

fn read_kit(path: &std::path::Path) -> Option<Kit> {
    // A material over its pattern and its pack, and its dressed pieces (D215, D290).
    let assets = kit_assets();
    let read = |file: &str| {
        std::fs::read_to_string(assets.join(file)).map_err(|why| format!("{file}: {why}"))
    };
    let name = path.file_name()?.to_string_lossy().to_string();
    let kit = Kit::read(&format!("{}/{name}", terra_build::kit::MATERIALS), &read)
        .map_err(|problem| error!("{problem}"))
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

/// The cubes an operation names, if it names any.
fn op_cubes(op: &Op) -> Vec<[i32; 3]> {
    match op {
        Op::LayPlan { cubes, .. } => cubes.iter().map(|planned| planned.at).collect(),
        Op::Unplan { cubes }
        | Op::Fill { cubes }
        | Op::Empty { cubes }
        | Op::SetShape { cubes, .. }
        | Op::Replace { cubes, .. }
        | Op::Swap { cubes }
        | Op::Changed { cubes, .. }
        | Op::Carve { cubes }
        | Op::Cancel { cubes }
        | Op::Open { cubes }
        | Op::Close { cubes }
        | Op::Regroup { cubes, .. } => cubes.clone(),
        _ => Vec::new(),
    }
}

/// Makes the texture arrays the kits' surfaces are drawn from, where things are drawn (D290).
fn make_arrays(world: &mut World) {
    let Some(layers) = world
        .get_resource::<BuildLook>()
        .map(|look| look.layers.clone())
    else {
        return;
    };
    let assets = kit_assets();
    let exists = Arc::new(move |path: &str| assets.join(path).is_file());
    if let Some(arrays) = arrays::setup(world, layers, exists) {
        world.insert_resource(arrays);
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
        .init_resource::<act::BuildRules>()
        .init_resource::<act::Offcuts>()
        .add_systems(Startup, (setup, make_arrays).chain())
        .add_systems(
            PostStartup,
            keep_footprint.before(crate::play::FloraSystems::Plant),
        )
        .add_systems(
            Update,
            (keep_footprint, give_footprints).before(crate::play::FloraSystems::Clear),
        )
        .init_resource::<maps::Loading>()
        .init_resource::<atlases::Atlases>()
        .add_systems(
            Update,
            (
                reload_kit,
                sheet::lay,
                village::lay,
                village::lay_scale,
                maps::load,
                arrays::stream.run_if(resource_exists::<arrays::Arrays>),
                atlases::compress.run_if(resource_exists::<BuildLook>),
            ),
        );
    arrays::plugin(app);
    store::plugin(app);
    layout::plugin(app);
    draw::plugin(app);
    hlod::plugin(app);
    memory::plugin(app);
    light::plugin(app);
    fire::plugin(app);
    views::plugin(app);
    app.add_systems(
        Update,
        (
            place_camera.run_if(free.clone()),
            edit.run_if(free.clone()),
            update_readout.run_if(free),
        )
            .chain(),
    );
}

/// The player's base, built by plans (docs/building-plan.md, step 4): acting on it, its
/// doors and shutters and every village house's, and drawing it again as it changes. Only where a player
/// builds, so a run of the build keys alone needs nothing of the player.
pub(crate) fn player_plugin(app: &mut App) {
    act::plugin(app);
    props::plugin(app);
    loss::plugin(app);
    remote::plugin(app);
    memory::remote(app);
    going_up::plugin(app);
}

/// A point just outside the example base's door, in the base's own space.
pub(super) const OUTSIDE_DOOR: Vec3 = Vec3::new(7.5, 0.0, 9.0);

impl Build {
    /// A build with nothing in it yet, standing at `frame`, with the entity it is drawn
    /// under.
    pub(super) fn new(
        commands: &mut Commands,
        look: &BuildLook,
        frame: Transform,
        name: &str,
    ) -> Self {
        let root = commands
            .spawn((
                Name::new(name.to_string()),
                store::BaseRoot,
                frame,
                // Seen, and so are its sections, with or without a mesh.
                Visibility::default(),
            ))
            .id();
        Self {
            base: Base::default(),
            frame,
            operations: Vec::new(),
            snapshot: None,
            since: Vec::new(),
            next: BTreeSet::new(),
            by_cubes: false,
            by_plans: false,
            revision: 0,
            refreshed: u64::MAX,
            footprint: Footprint {
                frame,
                cell_m: CELL_M,
                ..default()
            },
            kits: look.kits.clone(),
            shown: Shown::Built,
            standing: None,
            root,
            sections: store::Sections::default(),
            fallen: Vec::new(),
        }
    }

    /// How far its pieces are in, as its sections are laid.
    fn progress(&self) -> Progress {
        match self.shown {
            Shown::Built if self.by_cubes => Progress::Cubes {
                next: self.next.clone(),
            },
            Shown::Built => Progress::Whole,
            Shown::Plan(fraction) => Progress::Fraction(fraction),
        }
    }

    /// Works out again where it stands on the ground, now its shape or place has changed:
    /// its footprint, its door steps among what stands (D261). Its sections are laid again
    /// by `store`.
    fn refresh(&mut self, _planet: &Planet) {
        self.refreshed = self.revision;
        self.footprint = Footprint {
            frame: self.frame,
            cell_m: CELL_M,
            ground: self.base.ground_columns().collect(),
            standing: self.base.occupied_columns().collect(),
        };
    }

    /// Lays every section again, as when it has been replaced or its kits have changed.
    pub(super) fn redraw_all(&mut self) {
        self.sections.everything = true;
        self.revision += 1;
    }

    /// A point on the ground a few metres out from the first doorway, facing which a walker
    /// would climb its steps into the base. Returns the point and the direction to face,
    /// both in the base's own space.
    pub(super) fn approach(&self) -> (Vec3, Vec3) {
        match self.base.doorways(&BTreeSet::new()).first() {
            Some(door) => {
                let out = door.outward.as_vec3();
                let middle = (door.at.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL_M;
                (middle + out * 4.0, -out)
            }
            None => (OUTSIDE_DOOR, Vec3::NEG_Z),
        }
    }

    /// The height of whatever the base offers to stand on at a point in its own space: a
    /// floor, a roof, a stair or a slope, whichever is highest within a step of the feet.
    pub(super) fn support(&self, point: Vec3, walker: &Walker) -> Option<f32> {
        self.walkable().support(point, walker)
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
        _meshes: &mut Assets<Mesh>,
    ) {
        self.frame = frame;
        commands.entity(self.root).insert(frame);
        self.redraw_all();
        self.refresh(planet);
    }

    /// Applies several operations as one act, and redraws the base once. Returns whether
    /// anything changed.
    /// The ground a room from one tile to another would cut away to stand in it: its
    /// whole space from the floor to above the roof, within its walls' outer faces and no
    /// further, since the ground outside a building is not changed (D131, D193). What is
    /// under the floor stays, for its footings.
    pub(super) fn cut_for_room(&self, min: IVec3, max: IVec3) -> Operation {
        let low = Vec3::new(
            min.x as f32 * CELL_M,
            min.y as f32 * STOREY_M,
            min.z as f32 * CELL_M,
        ) - Vec3::new(0.0, 0.05, 0.0);
        let high = Vec3::new(
            (max.x + 1) as f32 * CELL_M,
            (max.y + 1) as f32 * STOREY_M + 0.6,
            (max.z + 1) as f32 * CELL_M,
        );
        Operation::DigBox {
            centre: self.frame.transform_point((low + high) * 0.5),
            half: (high - low) * 0.5,
            rotation: self.frame.rotation,
        }
    }

    pub(super) fn apply_all(
        &mut self,
        ops: &[BuildOp],
        _commands: &mut Commands,
        planet: &Planet,
        _meshes: &mut Assets<Mesh>,
    ) -> bool {
        let mut changed = false;
        for (&op, applied) in ops.iter().zip(self.base.apply_shortcuts(ops)) {
            if applied {
                self.operations.push(op);
                changed = true;
            }
        }
        if changed {
            self.lay_footings(planet);
            self.revision += 1;
            info!("Applied {} operations, such as {:?}", ops.len(), ops[0]);
            self.refresh(planet);
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
        self.redraw_all();
        self.apply_all(ops, commands, planet, meshes);
        self.refresh(planet);
    }

    /// Puts a building up from what the shortcuts say it is, by the same checked operations
    /// a player's are (D108, D280): laid as the shortcuts lay it, its footings and door
    /// steps from the planet, then laid again as plans and built a layer at a time, every
    /// step checked, in its kits' terms. Returns the operation refused and why, if one is,
    /// and keeps the shortcuts' base then.
    pub(super) fn put_up(
        &mut self,
        ops: &[BuildOp],
        commands: &mut Commands,
        planet: &Planet,
        meshes: &mut Assets<Mesh>,
    ) -> Result<(), String> {
        self.replace_with(ops, commands, planet, meshes);
        let rules = self.kits.as_deref().map(Kits::rules).unwrap_or_default();
        match self.base.put_up(&rules) {
            Ok(base) => {
                self.base = base;
                self.redraw_all();
                self.refresh(planet);
                Ok(())
            }
            Err((op, why)) => Err(format!("{} ({op:?})", why.message())),
        }
    }

    /// Readies a building to go up as builders put it up (D381): what the shortcuts say it
    /// is, its footings and door steps from the planet, kept as the whole it will be, and
    /// the build left empty, drawn as far as each of its cubes is in.
    pub(crate) fn ready_to_rise(
        &mut self,
        ops: &[BuildOp],
        commands: &mut Commands,
        planet: &Planet,
        meshes: &mut Assets<Mesh>,
    ) -> Base {
        self.replace_with(ops, commands, planet, meshes);
        let whole = std::mem::take(&mut self.base);
        self.operations.clear();
        self.by_cubes = true;
        self.redraw_all();
        self.refresh(planet);
        whole
    }

    /// Does operations to the base as one act, checked against it as it is (D21): all of
    /// them or, if one is refused, none. It is drawn again on the next frame.
    pub(super) fn act(&mut self, ops: &[Op], cx: &Context) -> Result<Done, Refusal> {
        let applied = work_out(self.base.clone(), self.kits.clone(), ops.to_vec(), cx)?;
        let done = applied.done.clone();
        self.swap(applied, std::time::Instant::now());
        Ok(done)
    }

    /// Takes in an act worked out on a worker (`work_out`): its base, what came down and
    /// what was only repainted, kept to be saved with the base. Its change to the screen is
    /// timed from when the act was `made`.
    pub(super) fn swap(&mut self, applied: Applied, made: std::time::Instant) {
        if !self.sections.drawn.is_empty() {
            self.sections.changed_at.get_or_insert(made);
        }
        // What the old base alone held is let go on a worker, not the main thread.
        store::let_go(std::mem::replace(&mut self.base, applied.base));
        self.fallen.extend(applied.fallen);
        if !applied.repainted.is_empty() {
            self.sections.repaints.push(store::Repaint {
                cubes: applied.repainted,
                since: std::time::Instant::now(),
            });
        }
        self.since.extend(applied.ops);
        // Now and then what has been done is folded into a snapshot.
        if self.since.len() > FOLD_AFTER {
            self.fold();
        }
        self.revision += 1;
    }

    /// Lights these cubes as what goes in next, drawing their sections again if that
    /// changes them.
    pub(super) fn light(&mut self, next: BTreeSet<[i32; 3]>) {
        if self.next != next {
            self.sections
                .relight
                .extend(self.next.symmetric_difference(&next).copied());
            self.next = next;
            self.revision += 1;
        }
    }

    /// Takes a snapshot of the base as it is, and forgets the operations before it.
    pub(super) fn fold(&mut self) {
        self.snapshot = Some(self.base.snapshot());
        self.since.clear();
    }

    /// Starts the base again from a snapshot and the operations since, as when a saved
    /// game is loaded. Operations that no longer apply are left out, and said.
    pub(super) fn restore(&mut self, snapshot: &Snapshot, since: &[Op], rules: &Rules) {
        let cx = Context::free(rules);
        match terra_build::save::restore(snapshot, since, &cx) {
            Ok((base, applied, left_out)) => {
                self.base = base;
                self.snapshot = Some(snapshot.clone());
                self.since = applied;
                for why in left_out {
                    warn!("A saved change to the base no longer applies: {why}");
                }
            }
            Err(problem) => {
                warn!("The saved base could not be read: {problem}");
                self.base = Base::default();
                self.snapshot = None;
                self.since.clear();
            }
        }
        self.sections.everything = true;
        self.operations.clear();
        self.revision += 1;
    }

    /// Applies an operation and redraws the base. Returns whether anything changed.
    pub(super) fn apply(
        &mut self,
        op: BuildOp,
        _commands: &mut Commands,
        planet: &Planet,
        _meshes: &mut Assets<Mesh>,
    ) -> bool {
        let changed = self.base.apply(op);
        if changed {
            self.operations.push(op);
            self.lay_footings(planet);
            self.revision += 1;
            info!("Applied {op:?}");
            self.refresh(planet);
        } else {
            info!("{op:?} changed nothing.");
        }
        changed
    }

    /// Footings under what the shortcuts built, down to the planet's ground (D194), as
    /// built cubes of the footings' kind. The player's base, built by plans, lays its
    /// footings as plans instead.
    fn lay_footings(&mut self, planet: &Planet) {
        if self.by_plans {
            return;
        }
        let Some(kits) = self.kits.as_deref() else {
            return;
        };
        let Some(kind) = floor_footing(&self.base, kits).or_else(|| footing_kind(kits)) else {
            return;
        };
        let frame = self.frame;
        self.base
            .lay_footings(kind, |column| ground_cm(&frame, planet, column));
        // Steps from its doorways down to the ground, of the footings' kind (D261).
        self.base
            .lay_door_steps(kind, |column| ground_cm(&frame, planet, column));
        // The ground under every column, as a plan samples it, which the roles a building's
        // edges are dressed by are read from (D215): where its feet are, how its plinth steps.
        let samples: Vec<terra_build::Sample> = self
            .base
            .occupied_columns()
            .map(|column| {
                let middle = Vec3::new(
                    (column.x as f32 + 0.5) * CELL_M,
                    0.0,
                    (column.y as f32 + 0.5) * CELL_M,
                );
                let height = -planet.distance(frame.transform_point(middle));
                terra_build::Sample::metres([column.x, column.y], height)
            })
            .collect();
        self.base.sample_ground(samples);
    }
}

/// The kind a build's floor on the ground asks its footings to be (D194): its own, where
/// most of it is of a block kind, as a stone floor stands on stone; none where it is timber
/// or there is none, so the footings take their default.
fn floor_footing(base: &Base, kits: &Kits) -> Option<Kind> {
    let floor = terra_build::floor_layer(0);
    let mut counts: BTreeMap<Kind, usize> = BTreeMap::new();
    for (at, cube) in base.cubes().filter(|(at, _)| at.y == floor) {
        if !base.in_wall(at.with_y(terra_build::wall_layer(0))) {
            *counts.entry(cube.kind).or_default() += 1;
        }
    }
    let (kind, _) = counts
        .into_iter()
        .max_by_key(|&(kind, count)| (count, kind))?;
    kits.of(kind)
        .is_some_and(|kit| kit.pattern == Pattern::Masonry && kit.framing.is_none())
        .then_some(kind)
}

/// The kinds a village is built of (`terra_build::village`): each role's material as the
/// settlement's data names it, and the roof each takes (D290).
/// Where the player's base stands, its own space placed on the planet, if there is one: for
/// what is told places in the base's own metres, as a script tells the camera.
pub(crate) fn base_frame(world: &World) -> Option<Transform> {
    world.get_resource::<Site>().map(|site| site.frame)
}

pub(crate) fn village_kinds(kits: &Kits) -> Option<terra_build::village::Palette> {
    let assets = kit_assets();
    let read = |file: &str| {
        std::fs::read_to_string(assets.join(file)).map_err(|why| format!("{file}: {why}"))
    };
    let names = kit_names();
    let named = |name: &str| {
        names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(k, _)| *k)
    };
    terra_build::village::Palette::read(&read, &named, kits)
        .map_err(|why| error!("{why}"))
        .ok()
}

/// The kind footings are laid in (D194): stone, the first block kind by its id that is not
/// framed.
pub(super) fn footing_kind(kits: &Kits) -> Option<Kind> {
    kits.footing().map(|kit| kit.kind())
}

/// How far a footing reaches below the lowest ground under its column, so no gap shows
/// under it on a slope.
const FOOTING_INTO_GROUND_M: f32 = 0.04;

/// The ground's height under a column of a build, in centimetres in its own space: the
/// lowest under its corners and its middle, a little lower still, so a footing down to it
/// is set into the ground all round.
pub(super) fn ground_cm(frame: &Transform, planet: &Planet, column: [i32; 2]) -> i16 {
    let lowest = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (0.5, 0.5)]
        .map(|(dx, dz)| {
            let at = Vec3::new(
                (column[0] as f32 + dx) * CELL_M,
                0.0,
                (column[1] as f32 + dz) * CELL_M,
            );
            -planet.distance(frame.transform_point(at))
        })
        .into_iter()
        .fold(f32::MAX, f32::min);
    terra_build::Sample::metres(column, lowest - FOOTING_INTO_GROUND_M).height_cm
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
    mut ghosts: Option<ResMut<Assets<draw::GhostMaterial>>>,
    assets: Res<AssetServer>,
) {
    // The base is first laid at the start, whether or not anything is built there.
    let frame = **start;
    let look = BuildLook {
        looks: draw::Looks::new(&mut materials, ghosts.as_deref_mut()),
        kits: read_kits().map(Arc::new),
        laying: Kind::PLAIN,
        maps: Arc::default(),
        materials: Arc::default(),
        layers: Arc::default(),
    };
    let materials_of_kit = look
        .kits
        .as_deref()
        .map(|kits| kit_materials(kits, &assets, &mut materials))
        .unwrap_or_default();
    let layers = Arc::new(
        look.kits
            .as_deref()
            .map(arrays::Layers::of)
            .unwrap_or_default(),
    );
    let look = BuildLook {
        materials: materials_of_kit,
        layers,
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
        asking: None,
    });
    let mut build = Build::new(&mut commands, &look, frame, "Base");
    // The player's base is built by plans, drawn as far as each cube is in.
    build.by_cubes = driven.is_some();
    build.by_plans = driven.is_some();
    if let Some(kits) = look.kits.as_deref() {
        commands.insert_resource(act::BuildRules {
            rules: kits.rules(),
            blocks: kits
                .0
                .iter()
                .map(|kit| kit.kind())
                .filter(|&kind| kits.one_at_a_time(kind))
                .collect(),
            footed: kits
                .0
                .iter()
                .filter(|kit| kit.pattern == Pattern::Masonry && kit.framing.is_none())
                .map(|kit| kit.kind())
                .collect(),
            names: kit_names(),
            budgets: Vec::new(),
            footing: footing_kind(kits),
            families: kits
                .0
                .iter()
                .map(|kit| (kit.kind(), plans::family(kit)))
                .collect(),
        });
    }
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

/// Reads the kits again when one's file is saved, and draws every build in them again.
fn reload_kit(
    time: Res<Time>,
    mut file: ResMut<KitFile>,
    mut look: ResMut<BuildLook>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
    (assets, mut materials): (Res<AssetServer>, ResMut<Assets<StandardMaterial>>),
    mut arrays: Option<ResMut<arrays::Arrays>>,
) {
    if file.timer.tick(time.delta()).just_finished() && file.asking.is_none() {
        file.asking = Some(bevy::tasks::IoTaskPool::get().spawn(async { modified() }));
    }
    let Some(task) = file.asking.as_mut() else {
        return;
    };
    let Some(now) = bevy::tasks::block_on(bevy::tasks::poll_once(task)) else {
        return;
    };
    file.asking = None;
    if now == file.modified {
        return;
    }
    file.modified = now;
    let Some(kits) = read_kits_again().map(Arc::new) else {
        return;
    };
    info!("Read the kits in {KITS} again.");
    look.materials = kit_materials(&kits, &assets, &mut materials);
    look.layers = Arc::new(arrays::Layers::of(&kits));
    if let Some(arrays) = arrays.as_mut() {
        arrays.relayer(look.layers.clone());
    }
    if look.kits.is_some() {
        look.kits = Some(kits.clone());
    }
    for build in
        std::iter::once(&mut site.build).chain(builds.iter_mut().map(|b| &mut b.into_inner().0))
    {
        if build.kits.is_some() {
            build.kits = Some(kits.clone());
            build.redraw_all();
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
    mut meshes: ResMut<Assets<Mesh>>,
    keys: Res<ButtonInput<KeyCode>>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
    planet: Res<Planet>,
    mut site: ResMut<Site>,
    look: Res<BuildLook>,
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
        "Base: {} cubes in rooms, {} wall cells, {} window cells, {} operations\n\
         Aim with the + mark. V window or door, B new room on open ground.",
        site.base.room_cubes(),
        site.base.walls().count(),
        windows,
        site.operations.len()
    );
}
