//! Drawing a base's sections (docs/building-plan.md, drawing; D211): what `layout` laid
//! and merged on the workers put on screen, a few sections a frame. Each section holds a
//! mesh per kind and material of what is built, near; the next piece lit; a plan's pieces
//! as ghosts, a unit box or prism each stretched over its piece, merged on the worker
//! into a mesh a shade, so a plan of a thousand pieces is two entities rather than a
//! thousand spawned again at every step built; and far off one mesh with its
//! textures baked into its colours, coarser again further still, switched by distance
//! with `VisibilityRange`, as Unreal's HLOD does. Near detail is made only for sections
//! near the camera and dropped when the camera leaves them.

use super::arrays::{Arrays, BuildMaterial, Layers};
use super::layout::{Done, Drawing, Jobs, Outcome, close, reach, suffices};
use super::store::{self, Drawn, Section, each};
use super::{Build, BuildLook, Built, Site};
use crate::planet::MainCamera;
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::{RenderLayers, VisibilityRange},
    light::NotShadowCaster,
    mesh::{AttributeQuantization, Indices, MeshAttributeCompressionFlags, MeshCompressionArgs},
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
/// Past this its hull: the far detail's pieces were two thirds of a town's triangles a
/// frame, most of them houses a hundred metres off and more (D278).
pub(super) const FAR_M: f32 = 120.0;
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

/// A mesh of a plan's ghosts or its shell, a section's child: what a build that shows no
/// blueprint, such as a settlement rising in a time-lapse (D381), keeps out of sight.
#[derive(Component)]
pub(crate) struct PlanGhost;

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
    /// The ghost a plan's pieces are drawn in, for anything else drawn as one.
    pub(super) fn ghost(&self) -> Handle<GhostMaterial> {
        self.ghost[0].clone()
    }

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

/// Each kit's pieces' atlas as a material, from its textures (D14); the rest of its texture
/// sets are layers of the texture arrays (`arrays`, D290). So two timber kinds can differ:
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
    pub(super) fn get(&self, kind: Kind, material: Material) -> Option<&Handle<StandardMaterial>> {
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
            // Every other texture set is a layer of the texture arrays (D290); only a pack's
            // own pieces' atlas, read by their own UVs, is a material of its own.
            if !matches!(material, Material::Piece | Material::Trim) {
                continue;
            }
            let path = &texture.path;
            let depth = kit.depth(*material);
            let handle = made
                .entry((
                    path.clone(),
                    [depth.to_bits(), texture.reflectance.to_bits()],
                ))
                .or_insert_with(|| {
                    let arm: Handle<Image> = linear(format!("{path}/arm.jpg"));
                    let parallax = depth > 0.0;
                    materials.add(StandardMaterial {
                        base_color_texture: Some(assets.load(format!("{path}/color.jpg"))),
                        normal_map_texture: Some(linear(format!("{path}/normal.jpg"))),
                        metallic_roughness_texture: Some(arm.clone()),
                        occlusion_texture: Some(arm),
                        perceptual_roughness: 1.0,
                        metallic: 1.0,
                        reflectance: texture.reflectance,
                        depth_map: parallax.then(|| linear(format!("{path}/height.jpg"))),
                        parallax_depth_scale: depth,
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

/// A section ready to put on screen, made on a worker: its meshes, about its middle; its
/// near and middle detail a part at a time, `None` for a part left as it is on screen.
pub(super) struct Parts {
    /// Every layer of the texture arrays its parts drawn again wear (D290).
    pub layers: Vec<u16>,
    parts: Vec<Option<Part>>,
    /// What each part was drawn from (`sections::cluster_keys`).
    keys: Vec<u64>,
    plain: Option<Staged>,
    next: Vec<Staged>,
    shell: Option<Staged>,
    /// A plan's ghosts, a mesh for each of their two shades.
    ghosts: [Option<Staged>; 2],
    far: Option<Staged>,
    proxy: Option<Staged>,
    centre: Vec3,
    /// A group's sections with anything in them, as a box in its base's own space.
    pub bounds: Option<(Vec3, Vec3)>,
}

/// A part of a section's near and middle detail as meshes: its surfaces in the texture
/// arrays one mesh a detail, each corner carrying its layer, and the layers they wear (D290);
/// what the arrays do not hold, a pack's pieces' atlas, by kind and material.
struct Part {
    arrays: [Option<Staged>; 2],
    layers: Vec<u16>,
    near: Vec<(Kind, Material, Staged)>,
    mid: Vec<(Kind, Material, Staged)>,
}

/// A mesh made on a worker, and, once it is put among the meshes ahead of being shown, its
/// handle: a change is put on the GPU over a few frames, within a budget a frame, and only
/// swapped in once all of it is there, as a streaming world uploads a chunk before it
/// shows it (D308). A tower's fall put every section it reached on the GPU in the frame it
/// was shown, 20 to 55 ms of Bevy preparing and placing meshes.
pub(super) enum Staged {
    Made(Box<Mesh>),
    Put {
        handle: Handle<Mesh>,
        corners: usize,
        bytes: usize,
    },
}

impl Staged {
    fn corners(&self) -> usize {
        match self {
            Staged::Made(mesh) => mesh.count_vertices(),
            Staged::Put { corners, .. } => *corners,
        }
    }

    fn bytes(&self) -> usize {
        match self {
            Staged::Made(mesh) => gpu_bytes(mesh),
            Staged::Put { bytes, .. } => *bytes,
        }
    }

    /// Puts it among the meshes, kept on the CPU too if `cpu`; how many corners it put,
    /// counted for the run too ([`PUT`]).
    fn put(&mut self, meshes: &mut Assets<Mesh>, cpu: bool) -> usize {
        let Staged::Made(mesh) = self else {
            return 0;
        };
        let mut mesh = *std::mem::replace(
            mesh,
            Box::new(Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )),
        );
        if cpu {
            mesh.asset_usage |= RenderAssetUsages::MAIN_WORLD;
        }
        let (corners, bytes) = (mesh.count_vertices(), gpu_bytes(&mesh));
        PUT[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        PUT[1].fetch_add(corners, std::sync::atomic::Ordering::Relaxed);
        *self = Staged::Put {
            handle: meshes.add(mesh),
            corners,
            bytes,
        };
        corners
    }

    fn handle(mut self, meshes: &mut Assets<Mesh>, cpu: bool) -> Handle<Mesh> {
        self.put(meshes, cpu);
        match self {
            Staged::Put { handle, .. } => handle,
            Staged::Made(_) => unreachable!("put above"),
        }
    }
}

/// A detail's surfaces as one mesh in the texture arrays, each corner carrying its layer
/// (`uv_b`'s first), with the rest left to draw in their own materials.
#[allow(clippy::type_complexity)]
pub(super) fn in_arrays(
    surfaces: &BTreeMap<(Kind, Material), Surface>,
    layers: &Layers,
    centre: Vec3,
    worn: &BTreeMap<[i32; 3], u8>,
    used: &mut BTreeSet<u16>,
) -> (Option<Mesh>, Vec<(Kind, Material, Mesh)>) {
    in_arrays_scorched(surfaces, layers, centre, worn, &BTreeMap::new(), used)
}

/// As [`in_arrays`], each corner's layer carrying how blackened it is by fire as well
/// (D377, `terra_build::fire::charred`), which the arrays' shader chars, soots and glows by.
#[allow(clippy::type_complexity)]
pub(super) fn in_arrays_scorched(
    surfaces: &BTreeMap<(Kind, Material), Surface>,
    layers: &Layers,
    centre: Vec3,
    worn: &BTreeMap<[i32; 3], u8>,
    scorched: &BTreeMap<[i32; 3], terra_build::Scorch>,
    used: &mut BTreeSet<u16>,
) -> (Option<Mesh>, Vec<(Kind, Material, Mesh)>) {
    let mut rest = Vec::new();
    let mut all = terra_build::Indexed::default();
    let mut tangents: Vec<[f32; 4]> = Vec::new();
    let mut ids: Vec<[f32; 2]> = Vec::new();
    for (&(kind, material), surface) in surfaces {
        let Some(id) = layers.get(kind, material) else {
            if let Some(mesh) = mesh(surface, centre, Uvs::Mapped) {
                rest.push((kind, material, mesh));
            }
            continue;
        };
        let part = surface.indexed().welded();
        if part.indices.is_empty() {
            continue;
        }
        used.insert(id);
        tangents.extend(part.tangents());
        // Each corner's layer, and how much of its cube's health is lost, which reveals the
        // shared damage mask (D290, D305).
        ids.extend(part.positions.iter().zip(&part.normals).map(|(p, n)| {
            let lost = if worn.is_empty() {
                0.0
            } else {
                terra_build::loss::struck(worn, *p, *n)
            };
            let char = terra_build::fire::charred(scorched, *p, *n);
            [f32::from(id) + terra_build::fire::CHAR_SHARE * char, lost]
        }));
        all.append(part);
    }
    let mesh = mesh_of(all, centre, Uvs::Mapped).map(|mesh| {
        mesh.with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, tangents)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, ids)
    });
    (mesh, rest)
}

/// How many meshes of builds, and corners in them, have been put on the GPU in the run: what
/// an edit costs to show, which `memory` says as the run ends.
pub(super) static PUT: [std::sync::atomic::AtomicUsize; 2] = [
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
];

/// A mesh's corners packed as shipped games pack them, by Bevy's own compression, which
/// its shaders and the arrays' unpack: normals and tangents octahedral in 16 bits, texture
/// places 16-bit within their range, colours 8-bit (tints and baked shades, smooth enough
/// at 256 steps), and indices 16-bit where they
/// fit. A textured corner goes from 76 bytes to 32; one far off, its textures baked into
/// its colour, from 44 to 20. Positions stay 32-bit: Bevy 0.20's 16-bit positions, unpacked
/// by each mesh's bounds, drew every building nowhere here (tried on the 5090, the town
/// gone from the screen with its triangles still counted), which is Bevy's to look into.
pub(super) fn packed(mesh: Mesh) -> Mesh {
    let args = MeshCompressionArgs {
        compress_indices: true,
        quantize_attributes: (&[(Mesh::ATTRIBUTE_COLOR.id, AttributeQuantization::Unorm8)]).into(),
        compress_attributes: MeshAttributeCompressionFlags::all()
            - MeshAttributeCompressionFlags::COMPRESS_POSITION,
    };
    match mesh.compressed_mesh(&args) {
        Ok(mesh) | Err((mesh, _)) => mesh,
    }
}

/// Turns a section drawn into meshes, about its middle: of its parts, only those `only`
/// says, which were drawn; its surfaces in the texture arrays by `layers`; packed
/// ([`packed`]) unless it is kept on the CPU to repaint in place, as a base being built is.
pub(super) fn parts(
    drawn: Merged,
    centre: Vec3,
    only: &[bool],
    keys: Vec<u64>,
    layers: &Layers,
    pack: bool,
) -> Parts {
    let pack = |mesh: Mesh| Staged::Made(Box::new(if pack { packed(mesh) } else { mesh }));
    // A surface whose colours came out near black, as a mask reading the real maps can make
    // it, is drawn black in any light: said once a run (`terra_build::inspect`).
    static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    for (&(kind, material), surface) in drawn.near_by.iter().flatten() {
        if let Some(dark) = terra_build::inspect::black(surface)
            && !SAID.swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            warn!(
                "Build: kind {} {material:?} near {centre} drawn black: {dark} corners near black",
                kind.0
            );
        }
    }
    let plain = |surface: &Surface| mesh(surface, centre, Uvs::None).map(pack);
    let none = BTreeMap::new();
    let worn: BTreeMap<[i32; 3], u8> = drawn.worn.iter().copied().collect();
    let scorched: BTreeMap<[i32; 3], terra_build::Scorch> =
        drawn.scorched.iter().copied().collect();
    let count = drawn.near_by.len().max(drawn.mid_by.len());
    let part = |c: usize| {
        only.get(c).copied().unwrap_or(true).then(|| {
            let mut used = BTreeSet::new();
            let (near_arrays, near) = in_arrays_scorched(
                drawn.near_by.get(c).unwrap_or(&none),
                layers,
                centre,
                &worn,
                &scorched,
                &mut used,
            );
            let (mid_arrays, mid) = in_arrays_scorched(
                drawn.mid_by.get(c).unwrap_or(&none),
                layers,
                centre,
                &worn,
                &scorched,
                &mut used,
            );
            let packs = |all: Vec<(Kind, Material, Mesh)>| {
                all.into_iter()
                    .map(|(kind, material, mesh)| (kind, material, pack(mesh)))
                    .collect()
            };
            Part {
                arrays: [near_arrays.map(pack), mid_arrays.map(pack)],
                layers: used.into_iter().collect(),
                near: packs(near),
                mid: packs(mid),
            }
        })
    };
    // A section's parts drawn again together are made into meshes side by side, on the
    // threads spare (`terra_build::sections::Spare`): welding, tangents and packing are
    // most of a near section's time on its worker, and the parts are apart (D308).
    let chosen: Vec<usize> = (0..count)
        .filter(|&c| only.get(c).copied().unwrap_or(true))
        .collect();
    let spare = terra_build::sections::Spare::take(chosen.len().saturating_sub(1));
    let mut parts: Vec<Option<Part>> = (0..count).map(|_| None).collect();
    if spare.count() == 0 {
        for c in chosen {
            parts[c] = part(c);
        }
    } else {
        let shares = terra_build::sections::shares(&chosen, spare.count() + 1);
        let made: Vec<(usize, Option<Part>)> = std::thread::scope(|scope| {
            let threads: Vec<_> = shares[1..]
                .iter()
                .map(|share| {
                    scope.spawn(move || share.iter().map(|&c| (c, part(c))).collect::<Vec<_>>())
                })
                .collect();
            let mut all: Vec<(usize, Option<Part>)> =
                shares[0].iter().map(|&c| (c, part(c))).collect();
            for thread in threads {
                all.extend(thread.join().expect("a share of parts is made into meshes"));
            }
            all
        });
        for (c, made) in made {
            parts[c] = made;
        }
    }
    drop(spare);
    let layers: BTreeSet<u16> = parts
        .iter()
        .flatten()
        .flat_map(|part| part.layers.iter().copied())
        .collect();
    Parts {
        parts,
        layers: layers.into_iter().collect(),
        keys,
        plain: plain(&drawn.plain),
        next: drawn.next.0.values().filter_map(plain).collect(),
        shell: mesh(&drawn.shell, centre, Uvs::Only).map(|mesh| Staged::Made(Box::new(mesh))),
        ghosts: [false, true].map(|shade| {
            ghost_mesh(&drawn.ghosts, shade, centre).map(|mesh| Staged::Made(Box::new(mesh)))
        }),
        far: plain(&drawn.far),
        proxy: plain(&drawn.proxy),
        centre,
        bounds: None,
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
    let all = surface.indexed().welded();
    let tangents = (uvs == Uvs::Mapped).then(|| all.tangents());
    let mesh = mesh_of(all, centre, uvs)?;
    Some(match tangents {
        Some(tangents) => mesh.with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, tangents),
        None => mesh,
    })
}

/// How many steps between 0 and 1 an sRGB channel is read off at when made linear.
const LINEAR_STEPS: usize = 4096;

/// An sRGB channel made linear, as Bevy's `Srgba::gamma_function` makes it, read from a
/// table between 0 and 1 and eased between its steps, to well under an eighth of a
/// byte's step: a masked surface's corners each have a colour of their own, and working
/// the power out for every channel of every corner was most of a near section's meshing.
fn linear(c: f32) -> f32 {
    let exact = |c: f32| {
        if c <= 0.0 {
            c
        } else if c <= 0.04045 {
            c / 12.92
        } else {
            libm::powf((c + 0.055) / 1.055, 2.4)
        }
    };
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    if !(0.0..1.0).contains(&c) {
        return exact(c);
    }
    let table = TABLE.get_or_init(|| {
        (0..LINEAR_STEPS)
            .map(|i| exact(i as f32 / (LINEAR_STEPS - 1) as f32))
            .collect()
    });
    let at = c * (LINEAR_STEPS - 1) as f32;
    let i = at as usize;
    let f = at - i as f32;
    table[i] + (table[(i + 1).min(LINEAR_STEPS - 1)] - table[i]) * f
}

/// Triangles with shared corners as a mesh about a point, its colours linear; its tangents
/// are the caller's.
fn mesh_of(all: terra_build::Indexed, centre: Vec3, uvs: Uvs) -> Option<Mesh> {
    if all.indices.is_empty() {
        return None;
    }
    // The base's colours are as they should look on screen; a mesh wants them linear.
    // Runs of corners share a colour, a log's or a face's, so each is turned once.
    let mut last: Option<([f32; 3], [f32; 4])> = None;
    // Its alpha is how unworn it is (`Indexed::wear`): the texture arrays' shader shows the
    // set a layer wears down to as it falls; opaque materials take no other note of it.
    let colours: Vec<[f32; 4]> = all
        .colours
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut to = match last {
                Some((from, to)) if from == *c => to,
                _ => {
                    let to = [linear(c[0]), linear(c[1]), linear(c[2]), 1.0];
                    last = Some((*c, to));
                    to
                }
            };
            to[3] = 1.0 - all.wear.get(i).copied().unwrap_or(0.0);
            to
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
        (follow_quality, repaint, upload, stream)
            .chain()
            .in_set(Drawing::Upload),
    );
}

/// How many frames a repaint waits for its new texture sets to be resident before it is
/// left to the sections laid again.
const REPAINT_WAIT_FRAMES: u32 = 30;

/// A change of material and nothing else, looked up in what is drawn (D298): each corner of
/// the changed cubes in the texture arrays' meshes takes the new kind's layer for the
/// material it wore, so the change is on screen the next frame, as a texture set swapped on
/// geometry that stays (Space Engineers' skins, Teardown's palettes). The sections are laid
/// again behind it, as for any change, and replace it when they come back, with the new
/// kind's weathering and how it meets its neighbours. A corner's cube is the one it lies in,
/// a little in from its face.
fn repaint(
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
    look: Res<BuildLook>,
    arrays: Option<ResMut<Arrays>>,
    assets: Res<AssetServer>,
    time: Res<Time<Real>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut waited: Local<u32>,
) {
    let Some(mut arrays) = arrays else {
        return;
    };
    let now = time.elapsed_secs();
    let layers = look.layers.clone();
    for build in each(&mut site, &mut builds) {
        if build.sections.repaints.is_empty() {
            continue;
        }
        // Those whose layers are ready are looked up together, in one walk of what is
        // drawn: a dozen made in a frame cost one walk, not a dozen.
        let mut ready = Vec::new();
        for repaint in std::mem::take(&mut build.sections.repaints) {
            // Each kind's layer for a material, old to new.
            let mut map: BTreeMap<(Kind, u16), u16> = BTreeMap::new();
            let mut wanted = Vec::new();
            for &(_, from, to) in &repaint.cubes {
                for material in MATERIALS {
                    if let (Some(old), Some(new)) =
                        (layers.own(from, material), layers.own(to, material))
                    {
                        map.entry((to, old)).or_insert(new);
                        wanted.push(new);
                    }
                }
            }
            if !arrays.ready(&wanted) {
                arrays.want(&wanted, &assets, now);
                *waited += 1;
                if *waited < REPAINT_WAIT_FRAMES {
                    build.sections.repaints.push(repaint);
                }
                continue;
            }
            *waited = 0;
            let to: BTreeMap<[i32; 3], Kind> =
                repaint.cubes.iter().map(|&(at, _, to)| (at, to)).collect();
            ready.push((repaint, to, map));
        }
        if ready.is_empty() {
            continue;
        }
        let started = std::time::Instant::now();
        // The box round every cube repainted: a corner outside it is passed over at once.
        let (low, high) = ready
            .iter()
            .flat_map(|(repaint, ..)| &repaint.cubes)
            .map(|&(at, ..)| IVec3::from_array(at))
            .fold((IVec3::MAX, IVec3::MIN), |(low, high), at| {
                (low.min(at), high.max(at))
            });
        let (mut rewritten, mut touched) = (0usize, 0usize);
        for (&at, drawn) in &build.sections.drawn {
            let centre = store::centre(IVec3::from_array(at));
            for handle in drawn.parts.iter().flat_map(|part| &part.arrays) {
                let Some(mesh) = meshes.get(handle) else {
                    continue;
                };
                let (
                    Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)),
                    Some(bevy::mesh::VertexAttributeValues::Float32x3(normals)),
                ) = (
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION),
                    mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
                )
                else {
                    continue;
                };
                let cubes: Vec<Option<[i32; 3]>> = positions
                    .iter()
                    .zip(normals)
                    .map(|(p, n)| {
                        let inside = centre + Vec3::from_array(*p)
                            - Vec3::from_array(*n) * (0.04 * terra_build::CELL_M);
                        let cube = (inside / terra_build::CELL_M).floor().as_ivec3();
                        let cube = (cube.cmpge(low).all() && cube.cmple(high).all())
                            .then_some(cube.to_array())?;
                        ready
                            .iter()
                            .any(|(_, to, _)| to.contains_key(&cube))
                            .then_some(cube)
                    })
                    .collect();
                // Only a mesh with a corner to change is touched, so no other is sent to
                // the GPU again.
                if cubes.iter().all(Option::is_none) {
                    continue;
                }
                let Some(mut mesh) = meshes.get_mut(handle) else {
                    continue;
                };
                let Some(bevy::mesh::VertexAttributeValues::Float32x2(ids)) =
                    mesh.attribute_mut(Mesh::ATTRIBUTE_UV_1)
                else {
                    continue;
                };
                for (id, cube) in ids.iter_mut().zip(&cubes) {
                    let Some(cube) = cube else {
                        continue;
                    };
                    // In the order they were made, so a cube changed twice ends as the last.
                    for (_, to, map) in &ready {
                        if let Some(kind) = to.get(cube)
                            && let Some(&new) = map.get(&(*kind, id[0] as u16))
                        {
                            // Its char (D377) kept, riding on the layer.
                            id[0] = f32::from(new) + (id[0] - id[0].floor());
                            rewritten += 1;
                        }
                    }
                }
                touched += 1;
            }
        }
        for (repaint, ..) in &ready {
            info!(
                "Build: repaint of {} cubes looked up in {:.2} ms, {:.1} ms after the change: {rewritten} corners in {touched} meshes take their new layers, on screen this frame; the sections laid again behind it.",
                repaint.cubes.len(),
                started.elapsed().as_secs_f32() * 1000.0,
                repaint.since.elapsed().as_secs_f32() * 1000.0,
            );
        }
    }
}

/// Every material a kit's surface can be drawn in, for looking up a repaint's layers.
const MATERIALS: [Material; 13] = [
    Material::Log,
    Material::LogEnd,
    Material::Daub,
    Material::Board,
    Material::Covering,
    Material::Stone,
    Material::Floor,
    Material::Masonry,
    Material::Flashing,
    Material::Dressed,
    Material::Hewn,
    Material::Wattle,
    Material::Plain,
];

/// How many corners are put on the GPU a frame, at most, beyond the first section's
/// meshes: a section near is a few hundred thousand, far a few thousand. A storey's fill
/// a few frames apart is a near section's 200,000 or so, spread over two frames.
const UPLOAD_CORNERS: usize = 120_000;

/// How many frames a build's laid sections are held back for the rest sent out with them at
/// most, before they are shown as they are, so a build always shows its changes: two
/// seconds, longer than a castle's tower takes to come down (D308).
const HOLD_FRAMES: u32 = 120;

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
    (mut arrays, assets, time, mut last_kept, builders, frame): (
        Option<ResMut<Arrays>>,
        Res<AssetServer>,
        Res<Time<Real>>,
        Local<u64>,
        Query<&crate::play::Builder>,
        Res<bevy::diagnostic::FrameCount>,
    ),
) {
    let all = options.scripted() && !options.bench;
    let now = time.elapsed_secs();
    // A build whose sections wear a texture set not yet resident waits for it, so nothing is
    // drawn in a slot not yet written (D290).
    let mut waiting_for_textures: BTreeSet<Entity> = BTreeSet::new();
    if let Some(arrays) = arrays.as_mut() {
        for done in &jobs.ready {
            if let Outcome::Drawn { parts, .. } = &done.outcome
                && !arrays.ready(&parts.layers)
            {
                arrays.want(&parts.layers, &assets, now);
                waiting_for_textures.insert(done.root);
            }
        }
        // What is drawn keeps its texture sets resident, looked at every half second.
        let tick = (now * 2.0) as u64;
        for build in each(&mut site, &mut builds).filter(|_| tick != *last_kept) {
            for drawn in build.sections.drawn.values() {
                for part in &drawn.parts {
                    arrays.want(&part.layers, &assets, now);
                }
            }
        }
        // And the material in each hammer, so a change to it is a lookup at once (D298).
        if tick != *last_kept {
            for builder in &builders {
                if let Some(kind) = builder.material {
                    arrays.want(&look.layers.of_kind(kind), &assets, now);
                }
            }
        }
        *last_kept = tick;
    }
    // The builds whose changes are all laid, and those held back too long.
    let roots: BTreeSet<Entity> = jobs.ready.iter().map(|done| done.root).collect();
    let (mut complete, mut partial) = (BTreeSet::new(), BTreeSet::new());
    let standing: BTreeSet<Entity> = each(&mut site, &mut builds).map(|b| b.root).collect();
    // What was laid for a build since taken away is dropped.
    jobs.ready.retain(|done| standing.contains(&done.root));
    jobs.held.retain(|root, _| standing.contains(root));
    // Every section sent out has come back: what came back is the latest of each, laid from
    // one base, to show together. Sections still waiting to be sent, for a newer change, are
    // shown when theirs come back. And a change older than the newest is shown once nothing
    // sent out for it, or for one before it, is still on a worker, though sections for a newer
    // change are out: a build edited every few frames shows each change whole rather than
    // waiting for a quiet moment that does not come, and then torn.
    let mut back: BTreeMap<Entity, (Option<u64>, u64)> = BTreeMap::new();
    let mut streaming: BTreeMap<Entity, u64> = BTreeMap::new();
    for build in each(&mut site, &mut builds) {
        if !roots.contains(&build.root) {
            continue;
        }
        if let Some(generation) = build.sections.streaming {
            streaming.insert(build.root, generation);
        }
        let out = jobs.oldest_out(build.root);
        back.insert(build.root, (out, build.sections.generation));
        let held = jobs.held.entry(build.root).or_default();
        *held += 1;
        if out.is_none() {
            complete.insert(build.root);
            build.sections.streaming = None;
        } else if *held > HOLD_FRAMES {
            partial.insert(build.root);
        }
    }
    let all_back = |done: &Done| {
        back.get(&done.root)
            .is_some_and(|&(out, newest)| change_back(done.generation, out, newest))
    };
    // What is to be shown is first put on the GPU, oldest first, within a budget of corners
    // a frame but always something; a build's change is swapped in once all of it was put
    // in an earlier frame, so Bevy has prepared it, and the frame it is shown on only swaps
    // entities. A scripted run puts and shows at once.
    let frame = frame.0;
    let cpu: BTreeMap<Entity, bool> = each(&mut site, &mut builds)
        .map(|build| (build.root, build.by_cubes))
        .collect();
    let mut budget = UPLOAD_CORNERS;
    let mut unput: BTreeSet<Entity> = BTreeSet::new();
    for done in jobs.ready.iter_mut() {
        let streams = streaming
            .get(&done.root)
            .is_some_and(|&generation| done.generation == generation);
        let showing = complete.contains(&done.root)
            || all_back(done)
            || partial.contains(&done.root)
            || streams;
        if !showing || done.put_at.is_some() {
            continue;
        }
        let put = match &mut done.outcome {
            Outcome::Drawn { parts, .. } => {
                let mut spend = if all { usize::MAX } else { budget.max(1) };
                let put = parts.put(
                    &mut meshes,
                    cpu.get(&done.root).copied().unwrap_or(false),
                    &mut spend,
                );
                budget = if all { budget } else { spend.min(budget) };
                put
            }
            Outcome::Same => true,
        };
        if put {
            done.put_at = Some(if all { frame.wrapping_sub(1) } else { frame });
        }
        if !streams && done.put_at.is_none_or(|at| at == frame) {
            unput.insert(done.root);
        }
    }
    let mut rest = std::collections::VecDeque::new();
    let mut shown: BTreeSet<Entity> = BTreeSet::new();
    let (mut whole_shown, mut torn): (BTreeSet<Entity>, BTreeSet<Entity>) = Default::default();
    while let Some(done) = jobs.ready.pop_front() {
        let whole = complete.contains(&done.root) || all_back(&done);
        // A base laid whole anew streams in, a section as it comes.
        let streams = streaming
            .get(&done.root)
            .is_some_and(|&generation| done.generation == generation);
        if !whole && !streams && !partial.contains(&done.root) {
            rest.push_back(done);
            continue;
        }
        // Shown once all of it was put on the GPU in an earlier frame, with the rest of its
        // change; a base streaming in, a section as it is put.
        let put = done.put_at.is_some_and(|at| at != frame);
        if !put || (!streams && unput.contains(&done.root)) {
            rest.push_back(done);
            continue;
        }
        shown.insert(done.root);
        if whole {
            whole_shown.insert(done.root);
        } else if !streams && torn.insert(done.root) {
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
        let material = arrays.as_ref().map(|arrays| &arrays.material);
        show(
            &mut commands,
            &mut meshes,
            &look,
            material,
            build,
            done,
            *quality,
        );
    }
    jobs.ready = rest;
    // Held is how long since a build last showed a change whole: one edited every few frames
    // shows each as it comes back and never waits long enough to be shown torn.
    for root in shown {
        if whole_shown.contains(&root) || !jobs.ready.iter().any(|done| done.root == root) {
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
                    "Build: change on screen in {:.1} ms: {} sections set waiting, {} merged anew, {} laid the same; the base laid again by {:.1} ms, the slowest section {:.1} ms on its worker, the last back at {:.1} ms; {} corners put on the GPU meanwhile.",
                    change.since.elapsed().as_secs_f32() * 1000.0,
                    change.waiting,
                    change.merged,
                    change.same,
                    change.laid_ms,
                    change.slowest_ms,
                    change.back_ms,
                    PUT[1]
                        .load(std::sync::atomic::Ordering::Relaxed)
                        .saturating_sub(change.put_from),
                );
            }
        }
    }
    jobs.settle(waiting);
}

/// Whether what came back for a change, `generation`, is all back while sections are still
/// out: a newer change has been noticed since, and nothing of this change or an older one
/// is still on a worker (`out`, the oldest change still out). The newest change waits for
/// every section out to come back.
fn change_back(generation: u64, out: Option<u64>, newest: u64) -> bool {
    generation < newest && out.is_none_or(|out| generation < out)
}

impl Parts {
    /// Whether nothing of it is drawn.
    pub(super) fn is_empty(&self) -> bool {
        self.corners() == 0
    }

    /// How many corners its meshes have.
    fn corners(&self) -> usize {
        self.sum(Staged::corners)
    }

    /// Something of each of its meshes, summed.
    fn sum(&self, count: impl Fn(&Staged) -> usize) -> usize {
        self.parts
            .iter()
            .flatten()
            .flat_map(|part| part.near.iter().chain(&part.mid))
            .map(|(_, _, mesh)| count(mesh))
            .sum::<usize>()
            + self
                .parts
                .iter()
                .flatten()
                .flat_map(|part| part.arrays.iter().flatten())
                .map(&count)
                .sum::<usize>()
            + self.rest(&count)
    }

    /// Something of each of its meshes but its parts', summed.
    fn rest(&self, count: &dyn Fn(&Staged) -> usize) -> usize {
        self.next.iter().map(count).sum::<usize>()
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

    /// Puts its meshes among the meshes, its arrays' kept on the CPU too if `cpu`, until
    /// `budget` corners are spent; whether all of them are put.
    fn put(&mut self, meshes: &mut Assets<Mesh>, cpu: bool, budget: &mut usize) -> bool {
        let mut all = true;
        let mut put = |staged: &mut Staged, cpu: bool| {
            if matches!(staged, Staged::Made(_)) {
                if *budget == 0 {
                    all = false;
                } else {
                    *budget = budget.saturating_sub(staged.put(meshes, cpu));
                }
            }
        };
        for part in self.parts.iter_mut().flatten() {
            for staged in part.arrays.iter_mut().flatten() {
                put(staged, cpu);
            }
            for (_, _, staged) in part.near.iter_mut().chain(part.mid.iter_mut()) {
                put(staged, false);
            }
        }
        for staged in self.next.iter_mut().chain(
            [
                &mut self.plain,
                &mut self.shell,
                &mut self.far,
                &mut self.proxy,
            ]
            .into_iter()
            .flatten(),
        ) {
            put(staged, false);
        }
        for staged in self.ghosts.iter_mut().flatten() {
            put(staged, false);
        }
        all
    }
}

/// How many bytes a mesh takes on the GPU: its corners and its indices.
pub(super) fn gpu_bytes(mesh: &Mesh) -> usize {
    mesh.get_vertex_buffer_size() + mesh.get_index_buffer_bytes().map_or(0, <[u8]>::len)
}

/// What a section's mesh is drawn in: a kit's own material, or the texture arrays (D290).
enum Shade {
    Standard(Handle<StandardMaterial>),
    Arrays(Handle<BuildMaterial>),
}

/// Puts a section laid on screen, in place of what was drawn of it: its parts whose key
/// is as it was left as they are, as a voxel game remeshes only the chunks an edit reaches.
fn show(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    look: &BuildLook,
    arrays: Option<&Handle<BuildMaterial>>,
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
    let parts = *parts;
    let mut old = build.sections.drawn.remove(&done.section);
    // How it was drawn before is let go on a worker: a near section's faces and pieces are
    // hundreds of thousands of small allocations, whose freeing took up to 15 ms of the
    // frame an edit came back on (D308).
    if let Some(drawing) = old.as_mut().and_then(|old| old.drawing.take()) {
        store::let_go(drawing);
    }
    // What is on screen is kept, its rest and its parts drawn again taken away, where every
    // part not drawn again is there to keep; else all of it goes.
    let keeps = old.as_ref().is_some_and(|old| {
        old.entity.is_some()
            && old.parts.len() == parts.parts.len()
            && parts
                .parts
                .iter()
                .zip(&old.parts)
                .all(|(new, old)| new.is_some() || old.key.is_some())
    });
    let mut kept_parts: Vec<Option<store::OnScreen>> = Vec::new();
    let mut entity = None;
    // A group's meshes hang off its base, placed at its middle: no entity of its own, so a
    // town's entities are its meshes.
    let grouped = build.sections.unit > 1;
    if let Some(old) = old {
        if grouped {
            for id in old
                .rest
                .iter()
                .chain(old.parts.iter().flat_map(|part| &part.entities))
            {
                commands.entity(*id).try_despawn();
            }
        } else if keeps {
            for id in old.rest {
                commands.entity(id).try_despawn();
            }
            for (new, shown) in parts.parts.iter().zip(old.parts) {
                if new.is_some() {
                    for id in shown.entities {
                        commands.entity(id).try_despawn();
                    }
                    kept_parts.push(None);
                } else {
                    kept_parts.push(Some(shown));
                }
            }
            entity = old.entity;
        } else if let Some(entity) = old.entity {
            commands.entity(entity).despawn();
        }
    }
    let at = IVec3::from_array(done.section);
    let rest_bytes = parts.rest(&Staged::bytes);
    let mut drawn = Drawn {
        entity: None,
        ticket: done.ticket,
        key,
        reach,
        drawing,
        gpu_bytes: 0,
        bounds: parts.bounds,
        parts: Vec::new(),
        rest: Vec::new(),
    };
    // Nothing in it: nothing drawn, but what it was laid as kept.
    if parts.is_empty() && kept_parts.iter().all(Option::is_none) {
        if let Some(entity) = entity {
            commands.entity(entity).despawn();
        }
        build.sections.drawn.insert(done.section, drawn);
        return;
    }
    let looks = &look.looks;
    let placed = if grouped {
        Transform::from_translation(parts.centre)
    } else {
        Transform::IDENTITY
    };
    let entity = if grouped {
        build.root
    } else {
        entity.unwrap_or_else(|| {
            commands
                .spawn((
                    Name::new(format!("Section {at}")),
                    Section { at },
                    Transform::from_translation(parts.centre),
                    Visibility::default(),
                    ChildOf(build.root),
                ))
                .id()
        })
    };
    drawn.entity = (!grouped).then_some(entity);
    // Each detail is seen from where the finer one drawn ends.
    let near = reach >= Reach::Near;
    let mid_from = near.then_some(NEAR_M);
    let far_from = (reach >= Reach::Mid).then_some(MID_M);
    // A group is seen from its middle as far further as its sections reach, less half a
    // section, as its laying measures it (`store::distance_from`): stood beside a house, all
    // of it is near.
    let further = parts
        .bounds
        .map_or(0.0, |bounds| store::sphere(bounds).1 - store::GROUP_SHIFT_M);
    let ranged = move |mut range: VisibilityRange| {
        if grouped {
            let shift = |r: std::ops::Range<f32>| {
                // Seen from the camera on stays so.
                if r.end <= 0.0 {
                    return r;
                }
                // On the detail ranges' grid, so a town's houses share a few of them.
                let at = |m: f32| ((m + further) * 4.0).round() / 4.0;
                at(r.start).max(0.0)..at(r.end).max(0.0)
            };
            range.start_margin = shift(range.start_margin.clone());
            range.end_margin = shift(range.end_margin.clone());
        }
        range
    };
    let mut add = |mesh: Staged, material: Shade, range: VisibilityRange, cpu: bool| {
        let range = ranged(range);
        let mesh = mesh.handle(meshes, cpu);
        let mut spawned = commands.spawn((Mesh3d(mesh.clone()), range, placed, ChildOf(entity)));
        match material {
            Shade::Standard(material) => spawned.insert(MeshMaterial3d(material)),
            Shade::Arrays(material) => spawned.insert(MeshMaterial3d(material)),
        };
        (spawned.id(), mesh)
    };
    let material = |kind: Kind, material: Material| {
        look.materials
            .get(kind, material)
            .cloned()
            .unwrap_or_else(|| looks.structure.clone())
    };
    // Each part drawn again; those not, as they were.
    let mut quiet: Vec<Entity> = Vec::new();
    let mut casters: Vec<(usize, Handle<Mesh>)> = Vec::new();
    for (c, new) in parts.parts.into_iter().enumerate() {
        let Some(new) = new else {
            drawn.parts.push(
                kept_parts
                    .get_mut(c)
                    .and_then(Option::take)
                    .unwrap_or_default(),
            );
            continue;
        };
        let mut shown = store::OnScreen {
            key: parts.keys.get(c).copied(),
            layers: new.layers.clone(),
            ..Default::default()
        };
        // Its surfaces in the texture arrays, a mesh a detail (D290); a base built by its
        // cubes keeps them on the CPU too, to repaint them in place (D298).
        if let Some(arrays) = arrays {
            let keep = build.by_cubes;
            let [near_arrays, mid_arrays] = new.arrays;
            for (mesh, mid) in [(near_arrays, false), (mid_arrays, true)] {
                let Some(mesh) = mesh else {
                    continue;
                };
                shown.gpu_bytes += mesh.bytes();
                let visible = if mid {
                    range(mid_from, MID_M)
                } else {
                    range(None, NEAR_M)
                };
                let (id, handle) = add(mesh, Shade::Arrays(arrays.clone()), visible, keep);
                shown.entities.push(id);
                if mid {
                    casters.push((drawn.parts.len(), handle.clone()));
                }
                if keep {
                    shown.arrays.push(handle);
                }
            }
        }
        for (kind, which, mesh) in new.near {
            shown.gpu_bytes += mesh.bytes();
            let (id, _) = add(
                mesh,
                Shade::Standard(material(kind, which)),
                range(None, NEAR_M),
                false,
            );
            shown.entities.push(id);
        }
        for (kind, which, mesh) in new.mid {
            shown.gpu_bytes += mesh.bytes();
            let (id, mesh) = add(
                mesh,
                Shade::Standard(material(kind, which)),
                range(mid_from, MID_M),
                false,
            );
            shown.entities.push(id);
            casters.push((drawn.parts.len(), mesh));
        }
        quiet.extend(shown.entities.iter().copied());
        drawn.parts.push(shown);
    }
    let mut rest: Vec<Entity> = Vec::new();
    // What goes in next, as long as the finest detail drawn is seen.
    let next_to = if near { NEAR_M } else { MID_M };
    for mesh in parts.next {
        rest.push(
            add(
                mesh,
                Shade::Standard(looks.next.clone()),
                range(None, next_to),
                false,
            )
            .0,
        );
    }
    if let Some(mesh) = parts.plain {
        rest.push(
            add(
                mesh,
                Shade::Standard(looks.structure.clone()),
                range(None, FAR_M),
                false,
            )
            .0,
        );
    }
    if let Some(mesh) = parts.far {
        rest.push(
            add(
                mesh,
                Shade::Standard(looks.far.clone()),
                range(far_from, FAR_M),
                false,
            )
            .0,
        );
    }
    if let Some(mesh) = parts.proxy {
        rest.push(
            add(
                mesh,
                Shade::Standard(looks.far.clone()),
                proxy_range(FAR_M),
                false,
            )
            .0,
        );
    }
    if let Some(mesh) = parts.shell {
        rest.push(
            commands
                .spawn((
                    Mesh3d(mesh.handle(meshes, false)),
                    MeshMaterial3d(looks.shell.clone()),
                    PlanGhost,
                    ghost_range(),
                    NotShadowCaster,
                    placed,
                    ChildOf(entity),
                ))
                .id(),
        );
    }
    // A plan's pieces, a mesh for each shade.
    for (shade, mesh) in parts.ghosts.into_iter().enumerate() {
        if let Some(mesh) = mesh {
            rest.push(
                commands
                    .spawn((
                        Mesh3d(mesh.handle(meshes, false)),
                        MeshMaterial3d(looks.ghost[shade].clone()),
                        PlanGhost,
                        placed,
                        ghost_range(),
                        NotShadowCaster,
                        ChildOf(entity),
                    ))
                    .id(),
            );
        }
    }
    // Below the high preset the sun sees each section at its middle detail from the camera
    // out, as shipped games cast shadows from a coarser level of detail: the finest detail
    // casts none, and the middle one's shadow is cast by a copy of it only the sun sees, in
    // a plain material.
    if coarse_shadows {
        for id in quiet {
            commands.entity(id).insert(NotShadowCaster);
        }
        for (part, mesh) in casters {
            let id = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(looks.far.clone()),
                    ranged(range(None, MID_M)),
                    RenderLayers::layer(SUN_ONLY),
                    placed,
                    ChildOf(entity),
                ))
                .id();
            drawn.parts[part].entities.push(id);
        }
    }
    drawn.rest = rest;
    drawn.gpu_bytes = rest_bytes + drawn.parts.iter().map(|part| part.gpu_bytes).sum::<usize>();
    build.sections.drawn.insert(done.section, drawn);
}

/// Keeps each section drawn as finely as the camera's distance asks: one coming nearer
/// is laid again with finer detail, one gone further off with coarser, a little after it
/// crosses, so one on the line is not laid over and over.
fn stream(
    scale: Res<DetailScale>,
    quality: Res<Quality>,
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
        let unit = sections.unit;
        for (&at, drawn) in &sections.drawn {
            let distance = store::distance_from(&frame, eye, at, unit, drawn.bounds);
            let laying = sections.waiting.contains_key(&at)
                || sections.tickets.get(&at) != Some(&drawn.ticket);
            let wanted = reach(distance / scale.0, Some(drawn.reach), close(*quality));
            if !suffices(drawn.reach, wanted) && !laying {
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
        let unit = build.sections.unit;
        let bounds: BTreeMap<[i32; 3], (Vec3, Vec3)> = build
            .sections
            .drawn
            .iter()
            .filter_map(|(&at, drawn)| Some((at, drawn.bounds?)))
            .collect();
        let distance = |at: [i32; 3]| {
            store::distance_from(&frame, eye, at, unit, bounds.get(&at).copied()) / scale.0
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
            if let Some(drawn) = sections.drawn.remove(&at) {
                // A section's entity, with its meshes; a group's meshes one by one.
                let meshes = drawn
                    .rest
                    .iter()
                    .chain(drawn.parts.iter().flat_map(|part| &part.entities));
                for id in drawn.entity.iter().chain(meshes) {
                    commands.entity(*id).try_despawn();
                }
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

#[cfg(test)]
mod tests {
    use super::change_back;

    #[test]
    fn a_change_is_shown_once_it_and_older_ones_are_back_though_newer_ones_are_out() {
        // Change 3 back, 4 and 5 out, 5 the newest: 3 is whole, shown.
        assert!(change_back(3, Some(4), 5));
        // A section of change 3 still out: 3 waits, and so does 4 behind it.
        assert!(!change_back(3, Some(3), 5));
        assert!(!change_back(4, Some(3), 5));
        // The newest change waits for everything out, as before.
        assert!(!change_back(5, Some(6), 5));
        assert!(!change_back(5, None, 5));
        // An older change with nothing out at all is back.
        assert!(change_back(2, None, 5));
    }

    #[test]
    fn a_channel_made_linear_by_the_table_is_bevys_to_well_under_a_bytes_step() {
        use bevy::color::{Color, ColorToComponents};
        let mut checked = 0;
        // Across the table, between its steps, and past either end as masks can push it.
        for i in -100..=1300 {
            let c = i as f32 / 1000.0 + 0.000_37;
            let bevy = Color::srgb(c, c, c).to_linear().to_f32_array()[0];
            let ours = super::linear(c);
            assert!((ours - bevy).abs() < 1e-4, "{c}: {ours} against {bevy}");
            checked += 1;
        }
        assert_eq!(checked, 1401);
    }
}
