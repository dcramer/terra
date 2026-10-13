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
use crate::play::Footprint;
use crate::start::Start;
pub(crate) use crate::start::lifted;
use bevy::{light::NotShadowCaster, prelude::*};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::SystemTime;
use terra_build::logs::{Kits, LogKit, Pattern};
use terra_build::ops::{Context, Done, Op, Refusal};
use terra_build::save::Snapshot;
use terra_build::{Base, BuildOp, CELL_M, Kind, Material, Panel, Rules, STOREY_M, Walker, example};
use terra_core::Operation;

pub(crate) mod act;
mod draw;
mod pieces;
pub(crate) mod plans;
pub(crate) mod props;
mod relief;
mod remote;
mod sheet;

use draw::redraw;
use draw::{Ramp, kit_materials};

const REACH_M: f32 = 300.0;
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
    /// The shortcuts that built it, for a build laid whole, as a village's houses are.
    pub operations: Vec<BuildOp>,
    /// The operations done to it since its last snapshot, which are kept and saved with
    /// it (construction-001, gap 4).
    pub snapshot: Option<Snapshot>,
    pub since: Vec<Op>,
    /// Goes up by one every time the build changes shape.
    pub revision: u64,
    /// The revision last drawn.
    drawn: u64,
    /// The cubes lit as what goes in next.
    next: BTreeSet<[i32; 3]>,
    /// Whether it must be drawn again though its shape has not changed.
    stale: bool,
    /// Whether its doors and windows are props fitted into it (the player's base), rather
    /// than drawn by its kit in every opening (a village's houses, the kit sheet).
    pub fitted_props: bool,
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

/// How many operations are kept before they are folded into a snapshot.
const FOLD_AFTER: usize = 400;

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
        .init_resource::<act::BuildRules>()
        .init_resource::<act::Offcuts>()
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

/// The player's base, built by plans (docs/building-plan.md, step 4): acting on it, its
/// fitted doors and windows, and drawing it again as it changes. Only where a player
/// builds, so a run of the build keys alone needs nothing of the player.
pub(crate) fn player_plugin(app: &mut App) {
    act::plugin(app);
    props::plugin(app);
    remote::plugin(app);
    app.add_systems(Update, draw::redraw_changed);
}

/// A point just outside the example base's door, in the base's own space.
pub(super) const OUTSIDE_DOOR: Vec3 = Vec3::new(7.5, 0.0, 9.0);

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
            snapshot: None,
            since: Vec::new(),
            drawn: 0,
            next: BTreeSet::new(),
            stale: false,
            fitted_props: false,
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
        for part in [
            self.structure,
            self.glass,
            self.foundation,
            self.steps,
            self.pieces,
        ] {
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

    /// Does operations to the base as one act, checked against it as it is (D21): all of
    /// them or, if one is refused, none. It is drawn again on the next frame.
    pub(super) fn act(&mut self, ops: &[Op], cx: &Context) -> Result<Done, Refusal> {
        let done = self.base.act_all(ops, cx)?;
        self.since.extend(ops.iter().cloned());
        // Now and then what has been done is folded into a snapshot.
        if self.since.len() > FOLD_AFTER {
            self.fold();
        }
        self.revision += 1;
        Ok(done)
    }

    /// Lights these cubes as what goes in next, drawing it again if that changes them.
    pub(super) fn light(&mut self, next: BTreeSet<[i32; 3]>) {
        if self.next != next {
            self.next = next;
            self.stale = true;
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
        match Base::from_snapshot(snapshot) {
            Ok(base) => self.base = base,
            Err(problem) => {
                warn!("The saved base could not be read: {problem:?}");
                self.base = Base::default();
            }
        }
        self.operations.clear();
        self.snapshot = Some(snapshot.clone());
        self.since.clear();
        let cx = Context::free(rules);
        for op in since {
            match self.base.act(op, &cx) {
                Ok(_) => self.since.push(op.clone()),
                Err(why) => warn!("A saved change to the base no longer applies: {why:?}"),
            }
        }
        self.revision += 1;
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
    // The player's base is built by plans, its doors and windows fitted as props.
    build.fitted_props = driven.is_some();
    if let Some(kits) = look.kits.as_deref() {
        commands.insert_resource(act::BuildRules {
            rules: kits.rules(),
            blocks: kits
                .0
                .iter()
                .map(|kit| kit.kind())
                .filter(|&kind| kits.one_at_a_time(kind))
                .collect(),
            names: kit_names(),
            budgets: Vec::new(),
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
