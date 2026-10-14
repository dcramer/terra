//! Drawing a base's sections (docs/building-plan.md, drawing; D211): what `layout` laid
//! and merged on the workers put on screen, a few sections a frame. Each section holds a
//! mesh per kind and material of what is built, near; the next piece lit; a plan's pieces
//! as ghosts, a unit box or prism each stretched over its piece, merged on the worker
//! into a mesh a shade, so a plan of a thousand pieces is two entities rather than a
//! thousand spawned again at every step built; and far off one mesh with its
//! textures baked into its colours, coarser again further still, switched by distance
//! with `VisibilityRange`, as Unreal's HLOD does. Near detail is made only for sections
//! near the camera and dropped when the camera leaves them.

use super::layout::{Done, Drawing, Jobs, Outcome, reach};
use super::store::{self, Drawn, Section, each};
use super::{Build, BuildLook, Built, Site};
use crate::planet::MainCamera;
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::{RenderLayers, VisibilityRange},
    light::NotShadowCaster,
    mesh::Indices,
    prelude::*,
    render::render_resource::{AsBindGroup, PrimitiveTopology},
    shader::ShaderRef,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use terra_build::kit::Kits;
use terra_build::sections::{Drawn as Merged, Ghost, Reach, Shape};
use terra_build::{Kind, Material, Surface};
use terra_world::detail::DetailScale;
use terra_world::grown::SUN_ONLY;
use terra_world::quality::Quality;

/// Where each detail is seen, in metres from a section's middle, each fading into the
/// next over a margin: full detail to here, a few metres off's to `MID_M`, far to `FAR_M`,
/// the proxy to `PROXY_M`; each times the detail scale (D230), which a lower quality preset
/// makes nearer.
const NEAR_M: f32 = 25.0;
const MID_M: f32 = 60.0;
const FAR_M: f32 = 180.0;
/// Where the far detail hands over to the hull at the low preset, as authored: nearer, as its
/// GPU is spent on the far detail's triangles a few pixels across (D278).
const FAR_LOW_M: f32 = 120.0;

/// Where a section's far detail ends at a quality preset.
fn far_end(quality: Quality) -> f32 {
    if quality == Quality::Low {
        FAR_LOW_M
    } else {
        FAR_M
    }
}
const PROXY_M: f32 = 600.0;
/// How far a plan's ghosts are seen.
const GHOST_M: f32 = 90.0;
/// Half the margin each detail fades over, as a share of where it changes.
const FADE: f32 = 0.06;

fn fade(at: f32) -> std::ops::Range<f32> {
    at * (1.0 - FADE)..at * (1.0 + FADE)
}

/// A detail seen from where a finer one ends, or from the camera if there is none, to
/// where it ends; as authored, which the detail scale then scales (D230).
fn range(after: Option<f32>, end: f32) -> VisibilityRange {
    VisibilityRange {
        start_margin: after.map_or(0.0..0.0, fade),
        end_margin: fade(end),
        use_aabb: false,
    }
}

/// Half the margin a section's proxy hands over to its base's cell over (`hlod`), wider than
/// the rest: each is measured from its own middle, a section's and its cell's up to 21 m
/// apart, and over this margin they cross with little of either missing.
const PROXY_FADE: f32 = 0.1;

fn proxy_fade() -> std::ops::Range<f32> {
    PROXY_M * (1.0 - PROXY_FADE)..PROXY_M * (1.0 + PROXY_FADE)
}

/// A section's proxy: from where its far detail ends to where its base's cell takes over.
fn proxy_range(far: f32) -> VisibilityRange {
    VisibilityRange {
        start_margin: fade(far),
        end_margin: proxy_fade(),
        use_aabb: false,
    }
}

/// A base's cell far off (`hlod`): from where its sections' proxies end to the horizon.
pub(super) fn cell_range() -> VisibilityRange {
    VisibilityRange {
        start_margin: proxy_fade(),
        end_margin: fade(super::hlod::HORIZON_M),
        use_aabb: false,
    }
}

/// How far past where it is last seen a section is laid at all, and kept laid, as
/// authored: beyond, its base's cells are all that is drawn of it, and its meshes and
/// pieces are let go (`stream`).
const LAY_M: f32 = 690.0;
const DROP_M: f32 = 740.0;

fn ghost_range() -> VisibilityRange {
    VisibilityRange::abrupt(0.0, GHOST_M)
}

/// A plan's pieces not yet built, as a blueprint in the air (`shaders/ghost.wesl`): a faint
/// colour, each shape's edges drawn as lines and its outline brighter.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct GhostMaterial {
    #[uniform(0)]
    colour: LinearRgba,
    /// How much an edge line and an outline add to how opaque it is.
    #[uniform(0)]
    edge: f32,
    #[uniform(0)]
    rim: f32,
}

impl bevy::pbr::Material for GhostMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/ghost.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_shadows() -> bool {
        false
    }

    fn enable_prepass() -> bool {
        false
    }
}

/// What builds are drawn with beside their kits' materials: what no kit draws, a plan's
/// ghosts in two shades and its shell, the next piece lit, far off sections in their
/// baked colours.
#[derive(Clone)]
pub(super) struct Looks {
    structure: Handle<StandardMaterial>,
    ghost: [Handle<GhostMaterial>; 2],
    shell: Handle<GhostMaterial>,
    pub next: Handle<StandardMaterial>,
    pub far: Handle<StandardMaterial>,
}

/// A plan's ghosts' colour: a pale blue, clear enough to see what is built through it.
/// Brighter than white, as its lines glow a little.
const GHOST: [f32; 3] = [0.5, 0.75, 1.0];

impl Looks {
    pub fn new(
        materials: &mut Assets<StandardMaterial>,
        ghosts: Option<&mut Assets<GhostMaterial>>,
    ) -> Self {
        let mut ghosts = ghosts;
        let mut ghost = |shade: f32, alpha: f32, edge: f32| {
            ghosts.as_mut().map_or_else(Handle::default, |ghosts| {
                ghosts.add(GhostMaterial {
                    colour: Color::srgba(
                        GHOST[0] * shade,
                        GHOST[1] * shade,
                        GHOST[2] * shade,
                        alpha,
                    )
                    .into(),
                    edge,
                    rim: 0.25,
                })
            })
        };
        Looks {
            ghost: [ghost(1.0, 0.07, 0.55), ghost(0.85, 0.07, 0.55)],
            shell: ghost(1.0, 0.05, 0.45),
            structure: materials.add(StandardMaterial {
                perceptual_roughness: 0.82,
                reflectance: 0.3,
                ..default()
            }),
            next: materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.78, 0.35, 0.75),
                emissive: LinearRgba::rgb(0.6, 0.35, 0.05),
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
            far: materials.add(StandardMaterial {
                perceptual_roughness: 0.88,
                reflectance: 0.25,
                ..default()
            }),
        }
    }
}

/// Each kit's materials, from its textures (D14), so two timber kinds can differ in bark:
/// by kind, and for what is drawn for no one kind (a roof, a floor, a gable's boards) the
/// first kit's to name the material. Kits naming the same textures share one material,
/// so their pieces draw together.
#[derive(Default)]
pub(super) struct KitMaterials {
    by_kind: BTreeMap<(Kind, Material), Handle<StandardMaterial>>,
    first: BTreeMap<Material, Handle<StandardMaterial>>,
}

impl KitMaterials {
    /// Every distinct material the kits are drawn in.
    pub(super) fn all(&self) -> impl Iterator<Item = &Handle<StandardMaterial>> {
        let mut seen = std::collections::BTreeSet::new();
        self.by_kind
            .values()
            .chain(self.first.values())
            .filter(move |handle| seen.insert(handle.id()))
    }

    /// A kind's material, or the first kit's for no kind.
    fn get(&self, kind: Kind, material: Material) -> Option<&Handle<StandardMaterial>> {
        self.by_kind
            .get(&(kind, material))
            .or_else(|| self.first.get(&material))
    }
}

/// The kits' materials, each from its textures: colour, normal, roughness and occlusion,
/// and parallax where it is given a depth (D14).
pub(super) fn kit_materials(
    kits: &Kits,
    assets: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> Arc<KitMaterials> {
    let linear = |path: String| -> Handle<Image> {
        assets
            .load_builder()
            .with_settings(|settings: &mut bevy::image::ImageLoaderSettings| {
                settings.is_srgb = false;
            })
            .load(path)
    };
    let mut made: BTreeMap<(String, [u32; 2]), Handle<StandardMaterial>> = BTreeMap::new();
    let mut out = KitMaterials::default();
    for kit in &kits.0 {
        for (material, texture) in &kit.textures {
            let path = &texture.path;
            let handle = made
                .entry((
                    path.clone(),
                    [texture.depth.to_bits(), texture.reflectance.to_bits()],
                ))
                .or_insert_with(|| {
                    let arm: Handle<Image> = linear(format!("{path}/arm.jpg"));
                    let parallax = texture.depth > 0.0;
                    materials.add(StandardMaterial {
                        base_color_texture: Some(assets.load(format!("{path}/color.jpg"))),
                        normal_map_texture: Some(linear(format!("{path}/normal.jpg"))),
                        metallic_roughness_texture: Some(arm.clone()),
                        occlusion_texture: Some(arm),
                        perceptual_roughness: 1.0,
                        metallic: 1.0,
                        reflectance: texture.reflectance,
                        depth_map: parallax.then(|| linear(format!("{path}/height.jpg"))),
                        parallax_depth_scale: texture.depth,
                        parallax_mapping_method:
                            bevy::pbr::ParallaxMappingMethod::DEFAULT_RELIEF_MAPPING,
                        max_parallax_layer_count: 16.0,
                        ..default()
                    })
                })
                .clone();
            out.by_kind.insert((kit.kind(), *material), handle.clone());
            out.first.entry(*material).or_insert(handle);
        }
    }
    Arc::new(out)
}

/// A section ready to put on screen, made on a worker: its meshes, about its middle.
pub(super) struct Parts {
    near: Vec<(Kind, Material, Mesh)>,
    mid: Vec<(Kind, Material, Mesh)>,
    plain: Option<Mesh>,
    next: Vec<Mesh>,
    shell: Option<Mesh>,
    /// A plan's ghosts, a mesh for each of their two shades.
    ghosts: [Option<Mesh>; 2],
    far: Option<Mesh>,
    proxy: Option<Mesh>,
    centre: Vec3,
}

/// Turns a section drawn into meshes, about its middle.
pub(super) fn parts(drawn: Merged, centre: Vec3) -> Parts {
    // A surface whose colours came out near black, as a mask reading the real maps can make
    // it, is drawn black in any light: said once a run (`terra_build::inspect`).
    static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    for (&(kind, material), surface) in &drawn.near {
        if let Some(dark) = terra_build::inspect::black(surface)
            && !SAID.swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            warn!(
                "Build: kind {} {material:?} near {centre} drawn black: {dark} corners near black",
                kind.0
            );
        }
    }
    let textured = |surface: &Surface| mesh(surface, centre, Uvs::Mapped);
    let plain = |surface: &Surface| mesh(surface, centre, Uvs::None);
    Parts {
        near: drawn
            .near
            .iter()
            .filter_map(|(&(kind, material), surface)| {
                textured(surface).map(|mesh| (kind, material, mesh))
            })
            .collect(),
        mid: drawn
            .mid
            .iter()
            .filter_map(|(&(kind, material), surface)| {
                textured(surface).map(|mesh| (kind, material, mesh))
            })
            .collect(),
        plain: plain(&drawn.plain),
        next: drawn.next.0.values().filter_map(plain).collect(),
        shell: mesh(&drawn.shell, centre, Uvs::Only),
        ghosts: [false, true].map(|shade| ghost_mesh(&drawn.ghosts, shade, centre)),
        far: plain(&drawn.far),
        proxy: plain(&drawn.proxy),
        centre,
    }
}

/// The shapes a ghost is drawn as, a unit box and an eight-sided prism along X, each
/// face's texture running 0 to 1 across it, which the ghost shader draws the edges of.
fn ghost_shapes() -> &'static [Mesh; 2] {
    static SHAPES: std::sync::OnceLock<[Mesh; 2]> = std::sync::OnceLock::new();
    SHAPES.get_or_init(|| {
        [
            Mesh::from(Cuboid::new(1.0, 1.0, 1.0)),
            Mesh::from(Cylinder::new(0.5, 1.0).mesh().resolution(8))
                .rotated_by(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        ]
    })
}

/// A plan's ghosts of one shade merged into one mesh about a point: each a unit shape
/// turned and stretched over its piece, its normals turned with it.
fn ghost_mesh(ghosts: &[Ghost], shade: bool, centre: Vec3) -> Option<Mesh> {
    let shapes = ghost_shapes();
    let read = |mesh: &Mesh| {
        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(bevy::mesh::VertexAttributeValues::Float32x3(v)) => v.clone(),
            _ => Vec::new(),
        };
        let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(bevy::mesh::VertexAttributeValues::Float32x3(v)) => v.clone(),
            _ => Vec::new(),
        };
        let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(bevy::mesh::VertexAttributeValues::Float32x2(v)) => v.clone(),
            _ => Vec::new(),
        };
        let indices: Vec<u32> = mesh
            .indices()
            .map_or(Vec::new(), |i| i.iter().map(|i| i as u32).collect());
        (positions, normals, uvs, indices)
    };
    let units = [read(&shapes[0]), read(&shapes[1])];
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for ghost in ghosts.iter().filter(|ghost| ghost.shade == shade) {
        let unit = &units[usize::from(ghost.shape == Shape::Round)];
        let first = positions.len() as u32;
        let size = ghost.size.max(Vec3::splat(1e-4));
        for (p, n) in unit.0.iter().zip(&unit.1) {
            let p = ghost.rotation * (Vec3::from_array(*p) * ghost.size) + ghost.centre - centre;
            // Stretched, a normal is scaled by the inverse.
            let n = (ghost.rotation * (Vec3::from_array(*n) / size)).normalize_or_zero();
            positions.push(p.to_array());
            normals.push(n.to_array());
        }
        uvs.extend(&unit.2);
        indices.extend(unit.3.iter().map(|i| first + i));
    }
    if indices.is_empty() {
        return None;
    }
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices)),
    )
}

/// What a mesh is given of where its corners fall on a texture.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Uvs {
    None,
    /// Where, for a ghost's lines.
    Only,
    /// Where, and the tangents a normal map needs.
    Mapped,
}

/// A surface as a mesh about a point, its corners shared, its colours linear.
pub(super) fn mesh(surface: &Surface, centre: Vec3, uvs: Uvs) -> Option<Mesh> {
    let all = surface.indexed();
    if all.indices.is_empty() {
        return None;
    }
    // The base's colours are as they should look on screen; a mesh wants them linear.
    // Runs of corners share a colour, a log's or a face's, so each is turned once.
    let mut last: Option<([f32; 3], [f32; 4])> = None;
    let colours: Vec<[f32; 4]> = all
        .colours
        .iter()
        .map(|c| match last {
            Some((from, to)) if from == *c => to,
            _ => {
                let to = Color::srgb(c[0], c[1], c[2]).to_linear().to_f32_array();
                last = Some((*c, to));
                to
            }
        })
        .collect();
    let positions: Vec<Vec3> = all.positions.iter().map(|p| *p - centre).collect();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, all.normals.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    if uvs == Uvs::Mapped {
        mesh = mesh.with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, all.tangents());
    }
    if uvs != Uvs::None {
        mesh = mesh.with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, all.uvs);
    }
    Some(mesh.with_inserted_indices(Indices::U32(all.indices)))
}

pub fn plugin(app: &mut App) {
    // Only where things are drawn: a run with no renderer has no materials.
    if app.is_plugin_added::<bevy::pbr::PbrPlugin>() {
        app.add_plugins(MaterialPlugin::<GhostMaterial>::default());
    }
    app.add_systems(
        Update,
        (follow_quality, upload, stream)
            .chain()
            .in_set(Drawing::Upload),
    );
}

/// How many corners are put on screen a frame, at most, beyond the first section: a
/// section near is a few hundred thousand, far a few thousand.
const UPLOAD_CORNERS: usize = 200_000;

/// How many frames a build's laid sections are held back for the rest sent out with them at
/// most, before they are shown as they are, so a build always shows its changes.
const HOLD_FRAMES: u32 = 30;

/// Puts what the workers have laid on screen, a few builds' changes a frame. What a build
/// sent out is swapped in whole, every section of it in the same frame once all are back,
/// the old meshes drawn until then, as a chunked world double-buffers its chunks: a piece
/// belongs to one section, and a block surface is rounded with its neighbours' cubes, so a
/// section shown new beside one still old could show a piece twice or not at all, or a seam,
/// for a frame. Only sections held `HOLD_FRAMES` are shown as they are, and say so.
#[allow(clippy::too_many_arguments)]
fn upload(
    mut commands: Commands,
    mut jobs: ResMut<Jobs>,
    look: Res<BuildLook>,
    quality: Res<Quality>,
    options: Res<crate::shell::Options>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
) {
    let all = options.scripted() && !options.bench;
    // The builds whose changes are all laid, and those held back too long.
    let roots: BTreeSet<Entity> = jobs.ready.iter().map(|done| done.root).collect();
    let (mut complete, mut partial) = (BTreeSet::new(), BTreeSet::new());
    let standing: BTreeSet<Entity> = each(&mut site, &mut builds).map(|b| b.root).collect();
    // What was laid for a build since taken away is dropped.
    jobs.ready.retain(|done| standing.contains(&done.root));
    jobs.held.retain(|root, _| standing.contains(root));
    for build in each(&mut site, &mut builds) {
        if !roots.contains(&build.root) {
            continue;
        }
        // Every section sent out has come back: what came back is the latest of each, laid
        // from one base, to show together. Sections still waiting to be sent, for a newer
        // change, are shown when theirs come back.
        let laid = !jobs.laying(build.root);
        let held = jobs.held.entry(build.root).or_default();
        *held += 1;
        if laid {
            complete.insert(build.root);
        } else if *held > HOLD_FRAMES {
            partial.insert(build.root);
        }
    }
    let mut corners = 0;
    let mut rest = std::collections::VecDeque::new();
    let mut shown: BTreeSet<Entity> = BTreeSet::new();
    while let Some(done) = jobs.ready.pop_front() {
        let whole = complete.contains(&done.root);
        if !whole && !partial.contains(&done.root) {
            rest.push_back(done);
            continue;
        }
        let size = match &done.outcome {
            Outcome::Drawn { parts, .. } => parts.corners(),
            Outcome::Same => 0,
        };
        // A change begun this frame is finished this frame, however big; another build's
        // waits for the next once this frame's corners are spent.
        if !all && !shown.contains(&done.root) && corners > 0 && corners + size > UPLOAD_CORNERS {
            rest.push_back(done);
            continue;
        }
        corners += size;
        if shown.insert(done.root) && !whole {
            warn!(
                "Build: sections shown before the rest sent out with them came back, after {HOLD_FRAMES} frames"
            );
        }
        let Some(build) = each(&mut site, &mut builds).find(|build| build.root == done.root) else {
            continue;
        };
        if let Some(change) = build.sections.change.as_mut() {
            match &done.outcome {
                Outcome::Drawn { took_ms, .. } => {
                    change.merged += 1;
                    change.slowest_ms = change.slowest_ms.max(*took_ms);
                }
                Outcome::Same => change.same += 1,
            }
            change.back_ms = change.since.elapsed().as_secs_f32() * 1000.0;
        }
        show(&mut commands, &mut meshes, &look, build, done, *quality);
    }
    jobs.ready = rest;
    for root in shown {
        if !jobs.ready.iter().any(|done| done.root == root) {
            jobs.held.remove(&root);
        }
    }
    // A change is on screen once nothing of its build is waiting, being laid or out.
    for build in each(&mut site, &mut builds) {
        let sections = &build.sections;
        let done = sections.waiting.is_empty()
            && sections.whole.is_some()
            && sections
                .tickets
                .iter()
                .all(|(at, ticket)| sections.drawn.get(at).is_some_and(|d| d.ticket == *ticket));
        if done && let Some(at) = build.sections.changed_at.take() {
            jobs.timings.edits.push(at.elapsed().as_secs_f32() * 1000.0);
        }
    }
    let waiting = each(&mut site, &mut builds).all(|build| build.sections.waiting.is_empty());
    // A change all on screen: how long from its being noticed, and what it set going.
    if jobs.quiet() {
        for build in each(&mut site, &mut builds) {
            if !build.sections.waiting.is_empty() || build.sections.laying.is_some() {
                continue;
            }
            if let Some(change) = build.sections.change.take() {
                info!(
                    "Build: change on screen in {:.1} ms: {} sections set waiting, {} merged anew, {} laid the same; the base laid again by {:.1} ms, the slowest section {:.1} ms on its worker, the last back at {:.1} ms.",
                    change.since.elapsed().as_secs_f32() * 1000.0,
                    change.waiting,
                    change.merged,
                    change.same,
                    change.laid_ms,
                    change.slowest_ms,
                    change.back_ms,
                );
            }
        }
    }
    jobs.settle(waiting);
}

impl Parts {
    /// Whether nothing of it is drawn.
    pub(super) fn is_empty(&self) -> bool {
        self.corners() == 0
    }

    /// How many corners its meshes have.
    fn corners(&self) -> usize {
        self.sum(Mesh::count_vertices)
    }

    /// How many bytes its meshes take on the GPU: their corners and indices.
    pub(super) fn bytes(&self) -> usize {
        self.sum(gpu_bytes)
    }

    /// Something of each of its meshes, summed.
    fn sum(&self, count: impl Fn(&Mesh) -> usize) -> usize {
        self.near
            .iter()
            .chain(&self.mid)
            .map(|(_, _, mesh)| count(mesh))
            .sum::<usize>()
            + self.next.iter().map(&count).sum::<usize>()
            + [
                &self.plain,
                &self.shell,
                &self.ghosts[0],
                &self.ghosts[1],
                &self.far,
                &self.proxy,
            ]
            .into_iter()
            .flatten()
            .map(count)
            .sum::<usize>()
    }
}

/// How many bytes a mesh takes on the GPU: its corners and its indices.
pub(super) fn gpu_bytes(mesh: &Mesh) -> usize {
    mesh.get_vertex_buffer_size() + mesh.get_index_buffer_bytes().map_or(0, <[u8]>::len)
}

/// Puts a section laid on screen, in place of what was drawn of it.
fn show(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    look: &BuildLook,
    build: &mut Build,
    done: Done,
    quality: Quality,
) {
    let coarse_shadows = quality != Quality::High;
    // An answer to an older job is dropped: the section has been sent again since.
    if build.sections.tickets.get(&done.section) != Some(&done.ticket) {
        return;
    }
    let Outcome::Drawn {
        key,
        reach,
        parts,
        drawing,
        ..
    } = done.outcome
    else {
        // Laid as it was: drawn as it is, and no longer being laid.
        if let Some(drawn) = build.sections.drawn.get_mut(&done.section) {
            drawn.ticket = done.ticket;
        }
        return;
    };
    if let Some(entity) = build
        .sections
        .drawn
        .remove(&done.section)
        .and_then(|old| old.entity)
    {
        commands.entity(entity).despawn();
    }
    let at = IVec3::from_array(done.section);
    let mut drawn = Drawn {
        entity: None,
        ticket: done.ticket,
        key,
        reach,
        drawing,
        gpu_bytes: parts.bytes(),
    };
    // Nothing in it: nothing drawn, but what it was laid as kept.
    if parts.is_empty() {
        build.sections.drawn.insert(done.section, drawn);
        return;
    }
    let looks = &look.looks;
    let entity = commands
        .spawn((
            Name::new(format!("Section {at}")),
            Section { at },
            Transform::from_translation(parts.centre),
            Visibility::default(),
            ChildOf(build.root),
        ))
        .id();
    drawn.entity = Some(entity);
    // Each detail is seen from where the finer one drawn ends.
    let near = reach == Reach::Near;
    let mid_from = near.then_some(NEAR_M);
    let far_from = (reach >= Reach::Mid).then_some(MID_M);
    let mut add = |mesh: Mesh, material: Handle<StandardMaterial>, range: VisibilityRange| {
        let mesh = meshes.add(mesh);
        let id = commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                range,
                Transform::IDENTITY,
                ChildOf(entity),
            ))
            .id();
        (id, mesh)
    };
    let material = |kind: Kind, material: Material| {
        look.materials
            .get(kind, material)
            .cloned()
            .unwrap_or_else(|| looks.structure.clone())
    };
    // Below the high preset the sun sees each section at its middle detail from the
    // camera out, as shipped games cast shadows from a coarser level of detail: the
    // finest detail casts none, and the middle one's shadow is cast by a copy of it only
    // the sun sees, in a plain material.
    let mut quiet: Vec<Entity> = Vec::new();
    let mut casters: Vec<Handle<Mesh>> = Vec::new();
    for (kind, which, mesh) in parts.near {
        let (id, _) = add(mesh, material(kind, which), range(None, NEAR_M));
        quiet.push(id);
    }
    for (kind, which, mesh) in parts.mid {
        let (id, mesh) = add(mesh, material(kind, which), range(mid_from, MID_M));
        quiet.push(id);
        casters.push(mesh);
    }
    // What goes in next, as long as the finest detail drawn is seen.
    let next_to = if near { NEAR_M } else { MID_M };
    for mesh in parts.next {
        add(mesh, looks.next.clone(), range(None, next_to));
    }
    if let Some(mesh) = parts.plain {
        add(mesh, looks.structure.clone(), range(None, far_end(quality)));
    }
    if let Some(mesh) = parts.far {
        add(mesh, looks.far.clone(), range(far_from, far_end(quality)));
    }
    if let Some(mesh) = parts.proxy {
        add(mesh, looks.far.clone(), proxy_range(far_end(quality)));
    }
    if coarse_shadows {
        for id in quiet {
            commands.entity(id).insert(NotShadowCaster);
        }
        for mesh in casters {
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(looks.far.clone()),
                range(None, MID_M),
                RenderLayers::layer(SUN_ONLY),
                Transform::IDENTITY,
                ChildOf(entity),
            ));
        }
    }
    if let Some(mesh) = parts.shell {
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(looks.shell.clone()),
            ghost_range(),
            NotShadowCaster,
            Transform::IDENTITY,
            ChildOf(entity),
        ));
    }
    // A plan's pieces, a mesh for each shade.
    for (shade, mesh) in parts.ghosts.into_iter().enumerate() {
        if let Some(mesh) = mesh {
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(looks.ghost[shade].clone()),
                Transform::IDENTITY,
                ghost_range(),
                NotShadowCaster,
                ChildOf(entity),
            ));
        }
    }
    build.sections.drawn.insert(done.section, drawn);
}

/// Keeps each section drawn as finely as the camera's distance asks: one coming nearer
/// is laid again with finer detail, one gone further off with coarser, a little after it
/// crosses, so one on the line is not laid over and over.
fn stream(
    scale: Res<DetailScale>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
) {
    let Some(eye) = camera.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    for build in each(&mut site, &mut builds) {
        let frame = build.frame;
        let sections = &mut build.sections;
        for (&at, drawn) in &sections.drawn {
            let distance = frame
                .transform_point(store::centre(IVec3::from_array(at)))
                .distance(eye);
            let laying = sections.waiting.contains_key(&at)
                || sections.tickets.get(&at) != Some(&drawn.ticket);
            let wanted = reach(distance / scale.0, Some(drawn.reach));
            if wanted != drawn.reach && !laying {
                sections.waiting.insert(
                    at,
                    store::Waiting {
                        force: true,
                        reach: Some(wanted),
                    },
                );
            }
        }
    }
}

/// Streams sections by distance, as open worlds stream their cells: one further off than
/// it is seen at all, past [`DROP_M`], is let go, its entity and meshes despawned and its
/// base's cells (`hlod`) all that is drawn of it; one waiting to be laid there is set aside
/// rather than laid; and one set aside that the camera comes back within [`LAY_M`] of waits
/// to be laid again. A base with nothing laid or waiting lets go of what it was laid from.
pub(super) fn aside(
    mut commands: Commands,
    scale: Res<DetailScale>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
) {
    let Some(eye) = camera.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    for build in each(&mut site, &mut builds) {
        let frame = build.frame;
        let distance = |at: [i32; 3]| {
            frame
                .transform_point(store::centre(IVec3::from_array(at)))
                .distance(eye)
                / scale.0
        };
        let sections = &mut build.sections;
        let far: Vec<[i32; 3]> = sections
            .waiting
            .keys()
            .chain(sections.drawn.keys())
            .copied()
            .filter(|&at| distance(at) > DROP_M)
            .collect();
        for at in far {
            sections.waiting.remove(&at);
            sections.tickets.remove(&at);
            if let Some(entity) = sections.drawn.remove(&at).and_then(|drawn| drawn.entity) {
                commands.entity(entity).try_despawn();
            }
            sections.far_off.insert(at);
        }
        let back: Vec<[i32; 3]> = sections
            .far_off
            .iter()
            .copied()
            .filter(|&at| distance(at) < LAY_M)
            .collect();
        for at in back {
            sections.far_off.remove(&at);
            sections.waiting.insert(
                at,
                store::Waiting {
                    force: true,
                    reach: None,
                },
            );
        }
        if sections.drawn.is_empty() && sections.waiting.is_empty() && sections.laying.is_none() {
            sections.whole = None;
            sections.snapshot = None;
        }
    }
}

/// Draws every build again when the quality preset changes, so each detail is seen as far
/// as the new one has it.
fn follow_quality(quality: Res<Quality>, mut site: ResMut<Site>, mut builds: Query<&mut Built>) {
    if !quality.is_changed() || quality.is_added() {
        return;
    }
    for build in each(&mut site, &mut builds) {
        build.redraw_all();
    }
}
