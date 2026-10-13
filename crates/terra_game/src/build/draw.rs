//! Drawing a base's sections (docs/building-plan.md, drawing; D211): what `layout` laid
//! and merged on the workers put on screen, a few sections a frame. Each section holds a
//! mesh per kind and material of what is built, near; the next piece lit; a plan's pieces
//! as ghosts, a unit box or prism each, all sharing one mesh and material so Bevy draws
//! them as one batch; its doors and windows, fitted; and far off one mesh with its
//! textures baked into its colours, coarser again further still, switched by distance
//! with `VisibilityRange`, as Unreal's HLOD does. Near detail is made only for sections
//! near the camera and dropped when the camera leaves them.

use super::layout::{Done, Drawing, Jobs, Outcome, reach};
use super::store::{self, Drawn, Section, each};
use super::{Build, BuildLook, Built, Site, pieces};
use crate::planet::MainCamera;
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::VisibilityRange,
    light::NotShadowCaster,
    mesh::Indices,
    prelude::*,
    render::render_resource::{AsBindGroup, PrimitiveTopology},
    shader::ShaderRef,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use terra_build::kit::Kits;
use terra_build::layout::Piece;
use terra_build::sections::{Drawn as Merged, Ghost, Reach, Shape, Stage};
use terra_build::{Kind, Material, Prop, Surface};

/// Where each detail is seen, in metres from a section's middle, each fading into the
/// next over a margin: full detail to here, a few metres off's to `MID_M`, far to `FAR_M`,
/// the proxy to `PROXY_M`.
const NEAR_M: f32 = 25.0;
const MID_M: f32 = 60.0;
const FAR_M: f32 = 180.0;
const PROXY_M: f32 = 600.0;
/// How far a plan's ghosts are seen.
const GHOST_M: f32 = 90.0;
/// Half the margin each detail fades over, as a share of where it changes.
const FADE: f32 = 0.06;

fn fade(at: f32) -> std::ops::Range<f32> {
    at * (1.0 - FADE)..at * (1.0 + FADE)
}

/// A detail seen from where a finer one ends, or from the camera if there is none, to
/// where it ends.
fn range(after: Option<f32>, end: f32) -> VisibilityRange {
    VisibilityRange {
        start_margin: after.map_or(0.0..0.0, fade),
        end_margin: fade(end),
        use_aabb: false,
    }
}

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

/// What builds are drawn with beside their kits' materials: what no kit draws, glass, a
/// plan's ghosts in two shades and its shell, the next piece lit, far off sections in
/// their baked colours; and the shapes a ghost is drawn as.
#[derive(Clone)]
pub(super) struct Looks {
    structure: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    ghost: [Handle<GhostMaterial>; 2],
    shell: Handle<GhostMaterial>,
    pub next: Handle<StandardMaterial>,
    far: Handle<StandardMaterial>,
    cube: Handle<Mesh>,
    prism: Handle<Mesh>,
}

/// A plan's ghosts' colour: a pale blue, clear enough to see what is built through it.
/// Brighter than white, as its lines glow a little.
const GHOST: [f32; 3] = [0.5, 0.75, 1.0];

impl Looks {
    pub fn new(
        meshes: &mut Assets<Mesh>,
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
            glass: materials.add(StandardMaterial {
                base_color: Color::srgba(0.62, 0.86, 0.95, 0.3),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 0.06,
                reflectance: 0.7,
                cull_mode: None,
                double_sided: true,
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
            cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            // Eight sides round, along X as a log lies.
            prism: meshes.add(
                Mesh::from(Cylinder::new(0.5, 1.0).mesh().resolution(8))
                    .rotated_by(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            ),
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
    glass: Option<Mesh>,
    next: Vec<Mesh>,
    shell: Option<Mesh>,
    ghosts: Vec<Ghost>,
    far: Option<Mesh>,
    proxy: Option<Mesh>,
    openings: Vec<(Piece, Stage)>,
    centre: Vec3,
}

/// Turns a section drawn into meshes, about its middle.
pub(super) fn parts(drawn: Merged, centre: Vec3) -> Parts {
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
        glass: plain(&drawn.glass),
        next: drawn.next.0.values().filter_map(plain).collect(),
        shell: mesh(&drawn.shell, centre, Uvs::Only),
        ghosts: drawn.ghosts,
        far: plain(&drawn.far),
        proxy: plain(&drawn.proxy),
        openings: drawn.openings,
        centre,
    }
}

/// What a mesh is given of where its corners fall on a texture.
#[derive(Clone, Copy, PartialEq)]
enum Uvs {
    None,
    /// Where, for a ghost's lines.
    Only,
    /// Where, and the tangents a normal map needs.
    Mapped,
}

/// A surface as a mesh about a point, its corners shared, its colours linear.
fn mesh(surface: &Surface, centre: Vec3, uvs: Uvs) -> Option<Mesh> {
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
    app.add_systems(Update, (upload, stream).chain().in_set(Drawing::Upload));
}

/// How many corners are put on screen a frame, at most, beyond the first section: a
/// section near is a few hundred thousand, far a few thousand.
const UPLOAD_CORNERS: usize = 200_000;

/// Puts what the workers have laid on screen, a few sections a frame.
fn upload(
    mut commands: Commands,
    mut jobs: ResMut<Jobs>,
    look: Res<BuildLook>,
    options: Res<crate::shell::Options>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
) {
    let all = options.scripted() && !options.bench;
    let mut corners = 0;
    while let Some(done) = jobs.ready.pop_front() {
        let size = match &done.outcome {
            Outcome::Drawn { parts, .. } => parts.corners(),
            Outcome::Same => 0,
        };
        if !all && corners > 0 && corners + size > UPLOAD_CORNERS {
            jobs.ready.push_front(done);
            break;
        }
        corners += size;
        let Some(build) = each(&mut site, &mut builds).find(|build| build.root == done.root) else {
            continue;
        };
        show(&mut commands, &mut meshes, &look, build, done);
    }
    let waiting = each(&mut site, &mut builds).all(|build| build.sections.waiting.is_empty());
    jobs.settle(waiting);
}

impl Parts {
    /// Whether nothing of it is drawn.
    pub(super) fn is_empty(&self) -> bool {
        self.corners() == 0 && self.ghosts.is_empty() && self.openings.is_empty()
    }

    /// How many corners its meshes have.
    fn corners(&self) -> usize {
        let count = |mesh: &Mesh| mesh.count_vertices();
        self.near
            .iter()
            .chain(&self.mid)
            .map(|(_, _, mesh)| count(mesh))
            .sum::<usize>()
            + self.next.iter().map(count).sum::<usize>()
            + [
                &self.plain,
                &self.glass,
                &self.shell,
                &self.far,
                &self.proxy,
            ]
            .into_iter()
            .flatten()
            .map(count)
            .sum::<usize>()
    }
}

/// Puts a section laid on screen, in place of what was drawn of it.
fn show(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    look: &BuildLook,
    build: &mut Build,
    done: Done,
) {
    // An answer to an older job is dropped: the section has been sent again since.
    if build.sections.tickets.get(&done.section) != Some(&done.ticket) {
        return;
    }
    let Outcome::Drawn {
        key, reach, parts, ..
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
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            range,
            Transform::IDENTITY,
            ChildOf(entity),
        ));
    };
    let material = |kind: Kind, material: Material| {
        look.materials
            .get(kind, material)
            .cloned()
            .unwrap_or_else(|| looks.structure.clone())
    };
    for (kind, which, mesh) in parts.near {
        add(mesh, material(kind, which), range(None, NEAR_M));
    }
    for (kind, which, mesh) in parts.mid {
        add(mesh, material(kind, which), range(mid_from, MID_M));
    }
    // What goes in next, as long as the finest detail drawn is seen.
    let next_to = if near { NEAR_M } else { MID_M };
    for mesh in parts.next {
        add(mesh, looks.next.clone(), range(None, next_to));
    }
    if let Some(mesh) = parts.plain {
        add(mesh, looks.structure.clone(), range(None, FAR_M));
    }
    if let Some(mesh) = parts.far {
        add(mesh, looks.far.clone(), range(far_from, FAR_M));
    }
    if let Some(mesh) = parts.proxy {
        add(mesh, looks.far.clone(), range(Some(FAR_M), PROXY_M));
    }
    if let Some(mesh) = parts.glass {
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(looks.glass.clone()),
            range(None, FAR_M),
            NotShadowCaster,
            Transform::IDENTITY,
            ChildOf(entity),
        ));
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
    // A plan's pieces, each a unit shape stretched over it: one mesh and two materials
    // for them all, which Bevy batches.
    for ghost in &parts.ghosts {
        let shape = match ghost.shape {
            Shape::Box => looks.cube.clone(),
            Shape::Round => looks.prism.clone(),
        };
        commands.spawn((
            Mesh3d(shape),
            MeshMaterial3d(looks.ghost[usize::from(ghost.shade)].clone()),
            Transform {
                translation: ghost.centre - parts.centre,
                rotation: ghost.rotation,
                scale: ghost.size,
            },
            ghost_range(),
            NotShadowCaster,
            ChildOf(entity),
        ));
    }
    // Doors and windows the kit draws in its openings, and the props fitted into a build
    // that is not the player's.
    if let Some(kit) = look.kit_pieces.as_deref() {
        for (piece, stage) in &parts.openings {
            for (mesh, material, placed) in pieces::fitted_parts(kit, piece) {
                let mut part = commands.spawn((
                    Mesh3d(mesh),
                    Transform::from_translation(-parts.centre) * placed,
                    range(None, FAR_M),
                    ChildOf(entity),
                ));
                match stage {
                    Stage::Next => part.insert(MeshMaterial3d(looks.next.clone())),
                    Stage::Ghost => part.insert(MeshMaterial3d(looks.ghost[0].clone())),
                    _ => part.insert(MeshMaterial3d(material)),
                };
            }
        }
        if build.fitted_props && !build.by_plans {
            let props: Vec<Prop> = build
                .base
                .props()
                .filter(|prop| terra_build::cubes::section_of(IVec3::from_array(prop.min)) == at)
                .copied()
                .collect();
            for prop in props {
                let (_, parts_of) = super::props::parts(kit, &build.base, &prop);
                for (mesh, material, placed) in parts_of {
                    commands.spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(material),
                        Transform::from_translation(-parts.centre) * placed,
                        range(None, FAR_M),
                        ChildOf(entity),
                    ));
                }
            }
        }
    }
    build.sections.drawn.insert(done.section, drawn);
}

/// Keeps each section drawn as finely as the camera's distance asks: one coming nearer
/// is laid again with finer detail, one gone further off with coarser, a little after it
/// crosses, so one on the line is not laid over and over.
fn stream(
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
            let wanted = reach(distance, Some(drawn.reach));
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
