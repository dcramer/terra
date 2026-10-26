//! Construction materials drawn from texture arrays (D290, docs/proposals/construction-002.md):
//! every texture set a material wears (its walls' stone, its floor's flags, its timber's
//! bark) is a layer of four arrays, colour, normal, occlusion-roughness-metal and height,
//! its colour and normal at the preset's texture side (`Quality::texture_side`, 2048 at high),
//! its occlusion-roughness-metal and height, which carry no fine detail, at [`SIDE`], each
//! block compressed where the GPU can (`bc`: BC7, BC5, BC1, BC4); each
//! corner of a section's mesh carries the layer it wears, so a section's surfaces are one
//! draw whatever the number of materials in it, as 7 Days to Die's paints and Teardown's
//! palettes are read from one texture by an index. A pack's own modelled pieces, read by
//! their own UVs from a large atlas, stay a material of their own (`draw::KitMaterials`).
//!
//! **Residency**: a texture set is on the GPU only while a section laid near uses it, as
//! texture streaming keeps only what is near. When a section is laid with a set that is not
//! resident, its textures are read and compressed on a worker and written into a free slot
//! of the arrays, layer by layer (`write_layers`, in the render world), and the section is
//! put on screen once they are there; a set no laid section has used for a few seconds
//! gives its slot back. The arrays grow by doubling, their slots copied, to 256 at most.
//! A table in the material maps each layer to its slot, its parallax depth and reflectance.
//!
//! The same packs are to serve the terrain (D252): a layer is a texture set's folder under
//! `assets/materials`, whatever draws it.

pub mod bc;

mod cache;

pub mod damage;

use bc::Codec;
use bevy::{
    asset::RenderAssetUsages,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        render_asset::RenderAssets,
        render_resource::{
            AsBindGroup, Extent3d, Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfo,
            TextureAspect, TextureDimension, TextureFormat, TextureViewDescriptor,
            TextureViewDimension,
        },
        renderer::{RenderDevice, RenderQueue},
        storage::ShaderBuffer,
        texture::GpuImage,
    },
    shader::ShaderRef,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use terra_build::kit::Kits;
use terra_build::{Kind, Material};
use terra_build_mesh::cells::CELLS;
use terra_world::quality::Quality;

/// The side of a texture set's occlusion-roughness-metal and height in the arrays, and its
/// colour's and normal's at the least, in texels, with their mipmaps down to 4 by 4.
pub const SIDE: u32 = 1024;
/// How many slots the arrays grow by at most at once: at 2048, 12 MiB a set, doubling past
/// this would hold far more than is laid.
const GROW_SLOTS: u32 = 8;

/// Each array's side for a preset's texture side: colour and normal at it, the rest at
/// [`SIDE`].
fn sides(top: u32) -> [u32; 4] {
    [top.max(SIDE), top.max(SIDE), SIDE, SIDE]
}

/// How each array's smaller copies are averaged: colour in linear light, normals kept unit.
const MIPS: [bc::Mips; 4] = [
    bc::Mips::Srgb,
    bc::Mips::Normal,
    bc::Mips::Linear,
    bc::Mips::Linear,
];
/// How many layers the table holds, and how many slots the arrays grow to at most: 256 is
/// the array layers every GPU wgpu runs on can hold.
pub const MAX_LAYERS: usize = 512;
const MAX_SLOTS: u32 = 256;
const FIRST_SLOTS: u32 = 16;
/// How long a texture set no laid section uses stays resident, in seconds.
const KEEP_S: f32 = 5.0;

/// The material every section's surfaces are drawn in.
pub type BuildMaterial = ExtendedMaterial<StandardMaterial, BuildArrays>;

/// What `shaders/build.wesl` is given. Field order must match the struct there.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bind_group_data(ViewKey)]
pub struct BuildArrays {
    /// For each layer: its slot, its parallax depth, its reflectance and how many layers
    /// relief mapping steps through at the preset (none: flat but for its normal map).
    #[uniform(100)]
    pub table: [Vec4; MAX_LAYERS],
    /// The shared damage mask (D290, D305): x its slot, y whether it is drawn (not at the
    /// low preset, nor before it is made: then a damaged surface is darkened by what it
    /// has lost, as far off), z how much darker a cube all but gone is.
    #[uniform(100)]
    pub damage: Vec4,
    /// For each layer, four to an entry: the slot of the texture set it wears down to
    /// (`Texture::worn`) and one, or none.
    #[uniform(100)]
    pub worn: [Vec4; MAX_LAYERS / 4],
    /// For each layer, how it breaks (`terra_build::kit::Breaks`, D305): x which way (0 the
    /// shared mask, 1 at its joints, 2 along its grain, 3 a sawn end, 4 a shingle at a
    /// time), y its splits along the grain or its end's pictures across.
    #[uniform(100)]
    pub breaks: [Vec4; MAX_LAYERS],
    /// For each layer, its detail texture (D362, `kit::Detail`): x the slot of the set and
    /// one, or none while it is not resident; y and z how many copies of it one copy of the
    /// layer's covers across and down; w how strongly it shows.
    #[uniform(100)]
    pub detail: [Vec4; MAX_LAYERS],
    /// Where detail fades: x from how far, y by how far, in metres; z 1 where the preset draws
    /// it at all.
    #[uniform(100)]
    pub near: Vec4,
    /// The debug view on (D385, `views`): x which, 0 none; y texels across a copy of a
    /// layer's colour; z the screen pixels a texel should cover a metre off. While x is 0
    /// the view's code is not compiled in at all (`ViewKey`).
    #[uniform(100)]
    pub view: Vec4,
    /// One build weathered (`crate::weathering`, research/2026-10-25-wfc-weathering.md):
    /// rows 0-2 the world-to-build affine in cubes, 3 (its least cube, 1 if on), 4 (its box
    /// in cubes, maps a row of the atlas), 5 (which way, its age, metres a cube).
    #[uniform(100)]
    pub weather: [Vec4; 6],
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    pub colour: Handle<Image>,
    #[texture(103, dimension = "2d_array")]
    pub normal: Handle<Image>,
    #[texture(104, dimension = "2d_array")]
    pub arm: Handle<Image>,
    #[texture(105, dimension = "2d_array")]
    pub height: Handle<Image>,
    /// Each texture set's cells (`cells`): its stones or shingles, numbered, its joints 0.
    #[texture(106, dimension = "2d_array")]
    pub cells: Handle<Image>,
    /// The fire lights' places and flickers (`terra_world::fire::FireGlows`), that burning
    /// char's cracks glow by: one flicker per fire, its light's (D415).
    #[storage(107, read_only)]
    pub fires: Handle<ShaderBuffer>,
    /// For each of its cubes' faces, its map's slot and one, or 0 (`terra_weathering`).
    #[texture(108, sample_type = "u_int")]
    pub weather_lookup: Handle<Image>,
    /// The motif maps, a one-channel layer each (BC4, `terra_weathering::compress::MOTIF`):
    /// crack, loss, moss, streak, lichen, soot.
    #[texture(109, dimension = "2d_array")]
    pub weather_motif: Handle<Image>,
    /// The mask maps at half size, likewise (`MASKS`): grime, wear, damp, cavity, edge.
    #[texture(110, dimension = "2d_array")]
    pub weather_masks: Handle<Image>,
}

impl MaterialExtension for BuildArrays {
    fn fragment_shader() -> ShaderRef {
        "shaders/build.wesl".into()
    }

    /// So a covering's shingles that damage took are not in depth or shadow either (D305).
    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/build_prepass.wesl".into()
    }

    /// With a debug view on, its code compiled in (`BUILD_VIEW`), so with none the shader is
    /// as it is without them, costing nothing.
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        if key.bind_group_data.view
            && let Some(fragment) = descriptor.fragment.as_mut()
        {
            fragment.shader_defs.push("BUILD_VIEW".into());
        }
        Ok(())
    }
}

/// Whether a debug view is on, which the arrays' shader is specialised by.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ViewKey {
    view: bool,
}

impl From<&BuildArrays> for ViewKey {
    fn from(arrays: &BuildArrays) -> Self {
        ViewKey {
            view: arrays.view.x > 0.5,
        }
    }
}

/// A texture set as a layer: its folder, and how its material draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct Set {
    pub path: String,
    pub depth: f32,
    pub reflectance: f32,
    /// How it breaks (D305), and the share of its height map that is joints, from which
    /// its cells are found; none, no cells.
    pub breaks: Vec4,
    pub share: f32,
    /// Its detail texture (D362): the set's folder, how many copies of it one copy of this
    /// covers across and down, and how strongly it shows.
    pub detail: Option<(String, [f32; 2], f32)>,
}

/// The kits' texture sets numbered as layers, from 1 (0 is none): each kit's materials but
/// its pieces' atlas, kits naming the same set sharing its layer.
#[derive(Debug, Default)]
pub struct Layers {
    pub sets: Vec<Set>,
    by_kind: BTreeMap<(Kind, Material), u16>,
    first: BTreeMap<Material, u16>,
    /// Each layer that wears down to another, and that other's layer.
    worn: BTreeMap<u16, u16>,
    /// Each layer with a detail texture, and the detail's layer.
    details: BTreeMap<u16, u16>,
    /// The layers of a kit that burns (D377), whose char glows while it burns.
    burns: BTreeSet<u16>,
}

impl Layers {
    pub fn of(kits: &Kits) -> Layers {
        let mut out = Layers::default();
        for kit in &kits.0 {
            for (material, texture) in &kit.textures {
                if matches!(material, Material::Piece | Material::Trim) {
                    continue;
                }
                let (breaks, share) = breaks(kit.damage.get(material), texture.cells);
                let detail = kit.details.get(material).map(|detail| {
                    let metres = detail.metres.max(0.01);
                    (
                        detail.path.clone(),
                        texture.metres.map(|m| m / metres),
                        detail.strength,
                    )
                });
                let set = Set {
                    path: texture.path.clone(),
                    depth: kit.depth(*material),
                    reflectance: texture.reflectance,
                    breaks,
                    share,
                    detail: detail.clone(),
                };
                let id = out.layer(set);
                if kit.burns.is_some() && id < MAX_LAYERS {
                    out.burns.insert(id as u16);
                }
                if id >= MAX_LAYERS {
                    warn!("Build: more than {MAX_LAYERS} texture sets; the rest drawn plain");
                    continue;
                }
                if let Some(worn) = &texture.worn {
                    let under = out.layer(Set {
                        path: worn.clone(),
                        depth: 0.0,
                        reflectance: texture.reflectance,
                        breaks,
                        share: 0.0,
                        detail: None,
                    });
                    if under < MAX_LAYERS {
                        out.worn.insert(id as u16, under as u16);
                    }
                }
                if let Some((path, _, _)) = detail {
                    let fine = out.layer(Set {
                        path,
                        depth: 0.0,
                        reflectance: texture.reflectance,
                        breaks: Vec4::ZERO,
                        share: 0.0,
                        detail: None,
                    });
                    if fine < MAX_LAYERS {
                        out.details.insert(id as u16, fine as u16);
                    }
                }
                out.by_kind.insert((kit.kind(), *material), id as u16);
                out.first.entry(*material).or_insert(id as u16);
            }
        }
        out
    }

    /// A set's layer, a new one if no kit has named it yet.
    fn layer(&mut self, set: Set) -> usize {
        match self.sets.iter().position(|s| *s == set) {
            Some(at) => at + 1,
            None => {
                self.sets.push(set);
                self.sets.len()
            }
        }
    }

    /// These layers and those they wear down to, which are made resident with them.
    pub fn with_worn(&self, ids: &[u16]) -> Vec<u16> {
        let mut out = ids.to_vec();
        out.extend(ids.iter().filter_map(|id| self.worn.get(id)));
        out
    }

    /// These layers' detail textures' layers (D362): made resident with them where the preset
    /// draws detail, but never waited for, as a face is whole without its detail.
    pub fn details_of(&self, ids: &[u16]) -> Vec<u16> {
        ids.iter()
            .filter_map(|id| self.details.get(id))
            .copied()
            .collect()
    }

    /// The layer a kind's material is drawn in, or the first kit's for no kind; none for a
    /// pieces' atlas or a material with no texture.
    pub fn get(&self, kind: Kind, material: Material) -> Option<u16> {
        if matches!(material, Material::Piece | Material::Trim) {
            return None;
        }
        self.by_kind
            .get(&(kind, material))
            .or_else(|| self.first.get(&material))
            .copied()
    }

    /// The layer a kind's own material is drawn in, if its kit names one.
    pub fn own(&self, kind: Kind, material: Material) -> Option<u16> {
        self.by_kind.get(&(kind, material)).copied()
    }

    /// Every layer a kind's materials are drawn in.
    pub fn of_kind(&self, kind: Kind) -> Vec<u16> {
        self.by_kind
            .iter()
            .filter(|((k, _), _)| *k == kind)
            .map(|(_, &id)| id)
            .collect()
    }

    fn set(&self, id: u16) -> Option<&Set> {
        self.sets.get(usize::from(id).checked_sub(1)?)
    }
}

/// How a material breaks, as the shader reads it, and the share of its height that is
/// joints where it has cells.
fn breaks(breaks: Option<&terra_build::kit::Breaks>, cells: u32) -> (Vec4, f32) {
    use terra_build::kit::Breaks;
    match breaks {
        None => (Vec4::ZERO, 0.0),
        Some(Breaks::Joints { share, .. }) => (Vec4::new(1.0, 0.0, 0.0, 0.0), *share),
        Some(Breaks::Grain { splits, .. }) => (Vec4::new(2.0, *splits, 0.0, 0.0), 0.0),
        Some(Breaks::Ends) => (Vec4::new(3.0, cells.max(1) as f32, 0.0, 0.0), 0.0),
        Some(Breaks::Shingles { share }) => (Vec4::new(4.0, 0.0, 0.0, 0.0), *share),
        Some(Breaks::Crumbles) => (Vec4::new(5.0, 0.0, 0.0, 0.0), 0.0),
    }
}

/// A level of a layer to write into the arrays, in the render world.
struct Upload {
    image: AssetId<Image>,
    /// How many texels a side its first level is.
    side: u32,
    slot: u32,
    codec: Codec,
    levels: Vec<Vec<u8>>,
}

/// The uploads waiting for the render world, and the arrays as it draws them, shared by both
/// worlds.
#[derive(Resource, Clone, Default)]
struct Uploads {
    pending: Arc<Mutex<Vec<Upload>>>,
    drawn: Drawn,
}

/// Each array's copy on the GPU as the render world last drew it, by its image: its side and
/// layers, or none while it has none. The main world names the arrays, the render world fills
/// them in (`write_layers`, `Arrays::follow_drawn`).
type Drawn = Arc<Mutex<BTreeMap<AssetId<Image>, Option<[u32; 2]>>>>;

/// A texture set's four maps, read and compressed, and its cells.
struct Encoded {
    maps: [Vec<Vec<u8>>; 4],
    cells: Vec<u8>,
    /// Whether it was read from the cache, compressed before.
    kept: bool,
}

enum Loading {
    /// Its pictures being read: colour, normal, ARM and height (none if it has no height).
    Reading([Option<Handle<Image>>; 4], f32),
    Encoding(Task<Option<Encoded>>),
}

/// A layer's slot in the table where its set is resident and written; else [`MISSING`],
/// never another set's, nor the damage mask's in `DAMAGE_SLOT`: a set still being read in
/// or written is drawn plain, in its corners' colours, and never in another's pictures.
fn slot_of(slot: Option<&Slot>) -> f32 {
    slot.filter(|slot| slot.ready_in == 0)
        .map_or(MISSING, |slot| slot.slot as f32)
}

/// A table entry's slot for a layer whose set is not resident yet (`build.wesl` reads it).
const MISSING: f32 = -1.0;

/// A texture set resident in a slot: ready once the frame its layers were written in has
/// passed.
struct Slot {
    slot: u32,
    ready_in: u8,
    last_used: f32,
}

/// The arrays and what is resident in them.
#[derive(Resource)]
pub struct Arrays {
    pub material: Handle<BuildMaterial>,
    images: [Handle<Image>; 4],
    codecs: [Codec; 4],
    /// Whether the GPU samples BC, and each array's side, by the preset (`sides`).
    compressed: bool,
    sides: [u32; 4],
    /// Where detail fades, by the preset (`Quality::detail_fade_m`).
    near: Vec4,
    /// Each set's cells, a byte a texel, unfiltered, one level.
    cells: Handle<Image>,
    capacity: u32,
    layers: Arc<Layers>,
    slots: BTreeMap<String, Slot>,
    free: Vec<u32>,
    loading: BTreeMap<String, Loading>,
    uploads: Uploads,
    table_dirty: bool,
    /// Whether an array was drawn last frame at another size than asked (`follow_drawn`).
    behind: bool,
    /// How many layers relief mapping steps through at the preset (`Quality::relief_layers`):
    /// none at the low preset, which draws every material flat but for its normal map.
    relief: f32,
    /// The shared damage mask, being made and then written, in `DAMAGE_SLOT`.
    damage: DamageMask,
    /// Whether the mask is drawn at the preset (not at the low).
    damage_detail: bool,
    /// Whether a check has hidden the damage drawing, to compare a frame with it and
    /// without on what is not damaged (`build.damage_drawn`, `cargo xtask artifacts --frames`).
    damage_hidden: bool,
    /// Whether a file is there, by its path under `assets/`.
    exists: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    /// Texture sets made resident and let go, and how long the slowest took from asked to
    /// ready, for the log.
    pub made: usize,
    pub dropped: usize,
    /// Of those made resident, how many were read compressed from the cache (`cache`).
    pub kept: usize,
}

/// The slot of the arrays the shared damage mask is kept in.
const DAMAGE_SLOT: u32 = 0;
/// How much darker a damaged cube all but gone is where the mask is not drawn, as
/// `terra_build_mesh::mesh::wear` darkens one far off.
const DAMAGE_DARK: f32 = 0.34;

/// The shared damage mask's way to its slot: its maps being made and compressed on a
/// worker, then written, a frame or two from ready, then there.
enum DamageMask {
    Making(Task<[Vec<Vec<u8>>; 4]>),
    Written(u8),
    Ready,
}

/// The formats of the four arrays, compressed or not.
fn formats(compressed: bool) -> ([TextureFormat; 4], [Codec; 4]) {
    if compressed {
        (
            [
                TextureFormat::Bc7RgbaUnormSrgb,
                TextureFormat::Bc5RgUnorm,
                TextureFormat::Bc1RgbaUnorm,
                TextureFormat::Bc4RUnorm,
            ],
            [Codec::Bc7, Codec::Bc5, Codec::Bc1, Codec::Bc4],
        )
    } else {
        (
            [
                TextureFormat::Rgba8UnormSrgb,
                TextureFormat::Rg8Unorm,
                TextureFormat::Rgba8Unorm,
                TextureFormat::R8Unorm,
            ],
            [Codec::Raw(4), Codec::Raw(2), Codec::Raw(4), Codec::Raw(1)],
        )
    }
}

/// An array of `slots` layers in a format, `side` square with mipmaps, to be written layer
/// by layer.
fn array(format: TextureFormat, slots: u32, filtered: bool, side: u32) -> Image {
    sized(format, slots, filtered, side, bc::levels(side))
}

/// The shared damage mask's four maps at each array's side, each level compressed.
fn damage_maps(sides: [u32; 4], codecs: [Codec; 4]) -> [Vec<Vec<u8>>; 4] {
    let maps = damage::maps();
    std::array::from_fn(|i| {
        let side = sides[i];
        let base = if side == SIDE {
            maps[i].clone()
        } else {
            to_side(&maps[i], SIDE, SIDE, 4, side)
        };
        bc::chain(&base, side, MIPS[i])
            .iter()
            .enumerate()
            .map(|(level, pixels)| codecs[i].encode(pixels, side >> level))
            .collect()
    })
}

/// As [`array`], `side` square with so many levels.
fn sized(format: TextureFormat, slots: u32, filtered: bool, side: u32, levels: u32) -> Image {
    let mut image = Image::new_uninit(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: slots,
        },
        TextureDimension::D2,
        format,
        // Kept in the main world too, though it holds no data there, so it can be grown.
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    image.texture_descriptor.mip_level_count = levels;
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    // Grown, its layers are copied at every level by `write_layers`.
    image.copy_on_resize = false;
    if filtered {
        let mut sampler = bevy::image::ImageSamplerDescriptor::linear();
        sampler.set_anisotropic_filter(16);
        sampler.set_address_mode(bevy::image::ImageAddressMode::Repeat);
        image.sampler = bevy::image::ImageSampler::Descriptor(sampler);
    }
    image
}

impl Arrays {
    pub fn new(
        images: &mut Assets<Image>,
        materials: &mut Assets<BuildMaterial>,
        compressed: bool,
        quality: Quality,
        layers: Arc<Layers>,
        exists: Arc<dyn Fn(&str) -> bool + Send + Sync>,
        fires: Handle<ShaderBuffer>,
    ) -> Arrays {
        let (formats, codecs) = formats(compressed);
        let sides = sides(quality.texture_side());
        let images_of: [Handle<Image>; 4] =
            std::array::from_fn(|i| images.add(array(formats[i], FIRST_SLOTS, i == 0, sides[i])));
        let cells = images.add(sized(
            TextureFormat::Rgba8Unorm,
            FIRST_SLOTS,
            false,
            CELLS,
            1,
        ));
        let [weather_lookup, weather_motif, weather_masks] =
            crate::weathering::placeholders(images);
        let images = images_of;
        let material = materials.add(BuildMaterial {
            base: StandardMaterial {
                perceptual_roughness: 1.0,
                metallic: 0.0,
                // May discard: a covering's lost shingles (D305), in every pass.
                alpha_mode: AlphaMode::Mask(0.5),
                ..default()
            },
            extension: BuildArrays {
                table: [Vec4::ZERO; MAX_LAYERS],
                damage: Vec4::new(DAMAGE_SLOT as f32, 0.0, DAMAGE_DARK, 0.0),
                worn: [Vec4::ZERO; MAX_LAYERS / 4],
                breaks: [Vec4::ZERO; MAX_LAYERS],
                detail: [Vec4::ZERO; MAX_LAYERS],
                near: near(quality),
                view: Vec4::ZERO,
                colour: images[0].clone(),
                normal: images[1].clone(),
                arm: images[2].clone(),
                height: images[3].clone(),
                cells: cells.clone(),
                fires,
                weather: [Vec4::ZERO; 6],
                weather_lookup,
                weather_motif,
                weather_masks,
            },
        });
        Arrays {
            material,
            images,
            codecs,
            compressed,
            sides,
            near: near(quality),
            cells,
            capacity: FIRST_SLOTS,
            layers,
            slots: BTreeMap::new(),
            // The first slot is the damage mask's, made once and kept.
            free: (DAMAGE_SLOT + 1..FIRST_SLOTS).rev().collect(),
            damage: DamageMask::Making(
                AsyncComputeTaskPool::get().spawn(async move { damage_maps(sides, codecs) }),
            ),
            loading: BTreeMap::new(),
            uploads: Uploads::default(),
            table_dirty: true,
            behind: true,
            relief: quality.relief_layers(),
            damage_detail: true,
            damage_hidden: false,
            exists,
            made: 0,
            dropped: 0,
            kept: 0,
        }
    }

    /// The kits' layers changed, as when a kit's file is saved.
    pub fn relayer(&mut self, layers: Arc<Layers>) {
        self.layers = layers;
        self.table_dirty = true;
    }

    /// How many texels across a copy of a layer's colour is.
    pub fn colour_side(&self) -> u32 {
        self.sides[0]
    }

    /// Hides the damage drawing, or shows it again: a check's, to compare what is not
    /// damaged with it and without.
    pub fn hide_damage(&mut self, hidden: bool) {
        if self.damage_hidden != hidden {
            self.damage_hidden = hidden;
            self.table_dirty = true;
        }
    }

    /// Whether every layer of these is resident and written.
    pub fn ready(&self, ids: &[u16]) -> bool {
        self.layers.with_worn(ids).iter().all(|&id| {
            self.layers.set(id).is_none_or(|set| {
                self.slots
                    .get(&set.path)
                    .is_some_and(|slot| slot.ready_in == 0)
            })
        })
    }

    /// Asks for these layers to be made resident, and marks them used now.
    pub fn want(&mut self, ids: &[u16], assets: &AssetServer, now: f32) {
        let layers = self.layers.clone();
        let mut wanted = layers.with_worn(ids);
        if self.near.z > 0.0 {
            wanted.extend(layers.details_of(ids));
        }
        for id in wanted {
            let Some(set) = layers.set(id) else {
                continue;
            };
            if let Some(slot) = self.slots.get_mut(set.path.as_str()) {
                slot.last_used = now;
                continue;
            }
            if self.loading.contains_key(set.path.as_str()) {
                continue;
            }
            let path = set.path.clone();
            let read = |file: &str, srgb: bool| -> Handle<Image> {
                assets
                    .load_builder()
                    .with_settings(move |settings: &mut bevy::image::ImageLoaderSettings| {
                        settings.is_srgb = srgb;
                        settings.asset_usage = RenderAssetUsages::MAIN_WORLD;
                    })
                    .load(format!("{path}/{file}"))
            };
            let height =
                (self.exists)(&format!("{path}/height.jpg")).then(|| read("height.jpg", false));
            let share = set.share;
            self.loading.insert(
                path.clone(),
                Loading::Reading(
                    [
                        Some(read("color.jpg", true)),
                        Some(read("normal.jpg", false)),
                        Some(read("arm.jpg", false)),
                        height,
                    ],
                    share,
                ),
            );
        }
    }

    /// How many bytes the arrays take on the GPU, and how many texture sets are resident.
    pub fn bytes(&self, images: &Assets<Image>) -> (usize, usize) {
        let bytes = self
            .images
            .iter()
            .chain([&self.cells])
            .filter_map(|handle| images.get(handle))
            .map(super::memory::texture_bytes)
            .sum();
        (bytes, self.slots.len())
    }

    /// The bytes one texture set takes in the arrays: its four maps with their mipmaps.
    pub fn set_bytes(&self) -> usize {
        self.codecs
            .iter()
            .zip(self.sides)
            .map(|(codec, side)| {
                (0..bc::levels(side))
                    .map(|level| codec.bytes(side >> level))
                    .sum::<usize>()
            })
            .sum()
    }

    /// The colour and normal maps' side, in texels.
    pub fn side(&self) -> u32 {
        self.sides[0]
    }

    /// Each layer's detail, where its detail's set is resident.
    fn detail(&self) -> [Vec4; MAX_LAYERS] {
        let mut out = [Vec4::ZERO; MAX_LAYERS];
        for (&layer, &fine) in &self.layers.details {
            let (Some(set), Some(slot)) = (
                self.layers.set(layer),
                self.layers
                    .set(fine)
                    .and_then(|set| self.slots.get(&set.path))
                    .filter(|slot| slot.ready_in == 0),
            ) else {
                continue;
            };
            let Some((_, scale, strength)) = &set.detail else {
                continue;
            };
            if let Some(entry) = out.get_mut(usize::from(layer)) {
                *entry = Vec4::new(slot.slot as f32 + 1.0, scale[0], scale[1], *strength);
            }
        }
        out
    }

    /// The arrays made again at another side, when the preset's texture side changes: what
    /// was resident is let go and read again at the new side as sections ask for it, as a
    /// game's texture quality setting reloads its textures.
    fn resize(
        &mut self,
        top: u32,
        images: &mut Assets<Image>,
        materials: &mut Assets<BuildMaterial>,
    ) {
        let (formats, codecs) = formats(self.compressed);
        self.sides = sides(top);
        self.images = std::array::from_fn(|i| {
            images.add(array(formats[i], self.capacity, i == 0, self.sides[i]))
        });
        if let Some(mut material) = materials.get_mut(&self.material) {
            material.extension.colour = self.images[0].clone();
            material.extension.normal = self.images[1].clone();
            material.extension.arm = self.images[2].clone();
            material.extension.height = self.images[3].clone();
        }
        self.dropped += self.slots.len();
        self.slots.clear();
        self.loading.clear();
        self.free = (DAMAGE_SLOT + 1..self.capacity).rev().collect();
        if let Ok(mut uploads) = self.uploads.pending.lock() {
            uploads.clear();
        }
        let sides = self.sides;
        self.damage = DamageMask::Making(
            AsyncComputeTaskPool::get().spawn(async move { damage_maps(sides, codecs) }),
        );
        self.table_dirty = true;
        info!("Build: texture arrays made again at {top} texels a side");
    }

    /// The arrays grown, doubled up to so many more slots at once, their new slots free; false
    /// when they are full.
    fn grow(&mut self, images: &mut Assets<Image>) -> bool {
        let grown = (self.capacity + self.capacity.min(GROW_SLOTS)).min(MAX_SLOTS);
        if grown == self.capacity {
            return false;
        }
        for handle in self.images.iter().chain([&self.cells]) {
            if let Some(mut image) = images.get_mut(handle) {
                image.texture_descriptor.size.depth_or_array_layers = grown;
            }
        }
        // So its bind group is made again with the grown arrays.
        self.table_dirty = true;
        self.free.extend((self.capacity..grown).rev());
        self.capacity = grown;
        info!("Build: the texture arrays grown to {grown} slots");
        true
    }

    /// Each array as asked of the GPU: its image, side and layers.
    fn asked(&self) -> [(AssetId<Image>, [u32; 2]); 5] {
        let mut asked = [(self.cells.id(), [CELLS, self.capacity]); 5];
        for (i, image) in self.images.iter().enumerate() {
            asked[i] = (image.id(), [self.sides[i], self.capacity]);
        }
        asked
    }

    /// How many slots every array's drawn copy holds, at the side asked of it: none while any
    /// has no copy yet or one at another side.
    fn drawn_slots(&self) -> u32 {
        let Ok(mut drawn) = self.uploads.drawn.lock() else {
            return 0;
        };
        let asked = self.asked();
        drawn.retain(|id, _| asked.iter().any(|(named, _)| named == id));
        asked
            .iter()
            .map(|(id, [side, _])| match *drawn.entry(*id).or_insert(None) {
                Some([drawn_side, layers]) if drawn_side == *side => layers,
                _ => 0,
            })
            .min()
            .unwrap_or(0)
    }

    /// Follows the arrays' copies the render world draws, and returns how many slots they all
    /// hold. The material binds the copy of each array drawn when it is made, and a grown or
    /// remade array's copy is swapped in only as a frame's uploads leave room (D420,
    /// `terra_world::uploads`), frames later: a material made meanwhile kept the old copy, and
    /// the sets in its new slots read its last layer, another set's pictures (the village's
    /// timber drawn broken and see-through). So the material is made again every frame until
    /// every array is drawn as asked, and once more the frame they are; and a set written into
    /// a slot no drawn copy holds yet is not counted written until one does.
    fn follow_drawn(&mut self) -> u32 {
        let drawn = self.drawn_slots();
        let behind = drawn < self.capacity;
        self.table_dirty |= behind || self.behind;
        self.behind = behind;
        // Written last frame: ready now.
        for slot in self.slots.values_mut() {
            if slot.ready_in > 0 && slot.slot < drawn {
                slot.ready_in -= 1;
                self.table_dirty |= slot.ready_in == 0;
            }
        }
        drawn
    }

    /// Each layer's worn set's slot and one, where it is resident, four layers an entry.
    fn worn(&self) -> [Vec4; MAX_LAYERS / 4] {
        let mut out = [Vec4::ZERO; MAX_LAYERS / 4];
        for (&layer, &under) in &self.layers.worn {
            let Some(slot) = self
                .layers
                .set(under)
                .and_then(|set| self.slots.get(&set.path))
                .filter(|slot| slot.ready_in == 0)
            else {
                continue;
            };
            let layer = usize::from(layer);
            if let Some(entry) = out.get_mut(layer / 4) {
                entry[layer % 4] = slot.slot as f32 + 1.0;
            }
        }
        out
    }

    /// How each layer breaks.
    fn breaks(&self) -> [Vec4; MAX_LAYERS] {
        let mut out = [Vec4::ZERO; MAX_LAYERS];
        for (i, set) in self.layers.sets.iter().enumerate() {
            if let Some(entry) = out.get_mut(i + 1) {
                *entry = set.breaks;
                // z: whether it burns (D377), its char glowing.
                entry.z = f32::from(u8::from(self.layers.burns.contains(&((i + 1) as u16))));
            }
        }
        out
    }

    fn table(&self) -> [Vec4; MAX_LAYERS] {
        let mut table = [Vec4::new(MISSING, 0.0, 0.0, 0.0); MAX_LAYERS];
        for (i, set) in self.layers.sets.iter().enumerate() {
            if let Some(entry) = table.get_mut(i + 1) {
                *entry = Vec4::new(
                    slot_of(self.slots.get(&set.path)),
                    set.depth,
                    set.reflectance,
                    self.relief,
                );
            }
        }
        table
    }
}

/// Where a preset fades detail, as the shader reads it (`BuildArrays::near`).
fn near(quality: Quality) -> Vec4 {
    quality
        .detail_fade_m()
        .map_or(Vec4::ZERO, |[from, by]| Vec4::new(from, by, 1.0, 0.0))
}

/// A picture's RGBA bytes `side` square: averaged down where it is a whole multiple of it,
/// else sampled between texels, wrapping, as it tiles.
fn to_side(data: &[u8], width: u32, height: u32, channels: usize, side: u32) -> Vec<u8> {
    let (w, h, side) = (width as usize, height as usize, side as usize);
    let texel = |x: usize, y: usize, c: usize| -> f32 {
        if c < channels {
            f32::from(data[((y % h) * w + x % w) * channels + c])
        } else if c == 3 {
            255.0
        } else {
            f32::from(data[((y % h) * w + x % w) * channels])
        }
    };
    let mut out = vec![0u8; side * side * 4];
    if w % side == 0 && h % side == 0 {
        let (fx, fy) = (w / side, h / side);
        for y in 0..side {
            for x in 0..side {
                for c in 0..4 {
                    let mut sum = 0.0;
                    for j in 0..fy {
                        for i in 0..fx {
                            sum += texel(x * fx + i, y * fy + j, c);
                        }
                    }
                    out[(y * side + x) * 4 + c] = (sum / (fx * fy) as f32).round() as u8;
                }
            }
        }
        return out;
    }
    for y in 0..side {
        let v = (y as f32 + 0.5) * h as f32 / side as f32 - 0.5;
        let (y0, ty) = (v.floor().rem_euclid(h as f32) as usize, v - v.floor());
        for x in 0..side {
            let u = (x as f32 + 0.5) * w as f32 / side as f32 - 0.5;
            let (x0, tx) = (u.floor().rem_euclid(w as f32) as usize, u - u.floor());
            for c in 0..4 {
                let top = texel(x0, y0, c) * (1.0 - tx) + texel(x0 + 1, y0, c) * tx;
                let bottom = texel(x0, y0 + 1, c) * (1.0 - tx) + texel(x0 + 1, y0 + 1, c) * tx;
                out[(y * side + x) * 4 + c] = (top * (1.0 - ty) + bottom * ty).round() as u8;
            }
        }
    }
    out
}

/// A loaded picture's first level's bytes, its size and channels a texel.
pub fn pixels(image: &Image) -> Option<(Vec<u8>, u32, u32, usize)> {
    let size = image.texture_descriptor.size;
    let data = image.data.as_ref()?;
    let texels = (size.width * size.height) as usize;
    let channels = image
        .texture_descriptor
        .format
        .block_copy_size(None)
        .map_or(4, |b| b as usize)
        .clamp(1, 4);
    let bytes = texels * channels;
    (data.len() >= bytes).then(|| (data[..bytes].to_vec(), size.width, size.height, channels))
}

/// Reads and compresses what is asked for, writes what is ready into the arrays, gives back
/// the slots of what has not been used for a while, and keeps the table up to date.
pub fn stream(
    mut arrays: ResMut<Arrays>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<BuildMaterial>>,
    assets: Res<AssetServer>,
    time: Res<Time<Real>>,
    quality: Res<Quality>,
) {
    let arrays = &mut *arrays;
    let now = time.elapsed_secs();
    if arrays.relief != quality.relief_layers() {
        arrays.relief = quality.relief_layers();
        arrays.table_dirty = true;
    }
    if arrays.near != near(*quality) {
        arrays.near = near(*quality);
        arrays.table_dirty = true;
    }
    if arrays.sides[0] != quality.texture_side().max(SIDE) {
        arrays.resize(quality.texture_side(), &mut images, &mut materials);
    }
    // The damage mask: written once made, drawn once written, but not at the low preset.
    let detail = *quality != Quality::Low;
    if arrays.damage_detail != detail {
        arrays.damage_detail = detail;
        arrays.table_dirty = true;
    }
    let drawn = arrays.follow_drawn();
    match &mut arrays.damage {
        DamageMask::Making(task) => {
            if let Some(maps) = block_on(poll_once(task)) {
                if let Ok(mut uploads) = arrays.uploads.pending.lock() {
                    for (i, levels) in maps.into_iter().enumerate() {
                        uploads.push(Upload {
                            image: arrays.images[i].id(),
                            side: arrays.sides[i],
                            slot: DAMAGE_SLOT,
                            codec: arrays.codecs[i],
                            levels,
                        });
                    }
                }
                arrays.damage = DamageMask::Written(2);
            }
        }
        DamageMask::Written(frames) => {
            if *frames == 0 {
                arrays.damage = DamageMask::Ready;
                arrays.table_dirty = true;
            } else if DAMAGE_SLOT < drawn {
                *frames -= 1;
            }
        }
        DamageMask::Ready => {}
    }
    let codecs = arrays.codecs;
    let sides = arrays.sides;
    let mut finished = Vec::new();
    for (path, loading) in &mut arrays.loading {
        match loading {
            Loading::Reading(handles, share) => {
                let share = *share;
                let failed = handles.iter().flatten().any(|handle| {
                    matches!(
                        assets.get_load_state(handle),
                        Some(bevy::asset::LoadState::Failed(_))
                    )
                });
                if failed {
                    error!("Build: the textures of {path} did not load");
                    finished.push((path.clone(), None));
                    continue;
                }
                if !handles
                    .iter()
                    .flatten()
                    .all(|handle| images.get(handle).is_some())
                {
                    continue;
                }
                let maps: Vec<Option<(Vec<u8>, u32, u32, usize)>> = handles
                    .iter()
                    .map(|handle| handle.as_ref().and_then(|h| images.get(h)).and_then(pixels))
                    .collect();
                let task = AsyncComputeTaskPool::get().spawn(async move {
                    // Compressed before and kept: read back (`cache`).
                    let kept = cache::file(cache::key(&maps, sides, codecs, share));
                    if let Some((maps, cells)) = kept.as_deref().and_then(cache::read) {
                        return Some(Encoded {
                            maps,
                            cells,
                            kept: true,
                        });
                    }
                    let mut out: [Vec<Vec<u8>>; 4] = Default::default();
                    let mut found = Vec::new();
                    for (i, map) in maps.into_iter().enumerate() {
                        // Its stones or shingles, from its height as read (D305), as the
                        // CPU finds them from the same picture; none, one cell.
                        if i == 3 {
                            found = match &map {
                                Some((data, w, h, channels)) => terra_build_mesh::cells::label(
                                    data,
                                    *w as usize,
                                    *h as usize,
                                    *channels,
                                    share,
                                ),
                                None => [128u8, 128, 128, 255].repeat((CELLS * CELLS) as usize),
                            };
                        }
                        let side = sides[i];
                        let base = match map {
                            Some((data, w, h, channels)) => to_side(&data, w, h, channels, side),
                            // No height: flat.
                            None => vec![0u8; (side * side * 4) as usize],
                        };
                        out[i] = bc::chain(&base, side, MIPS[i])
                            .iter()
                            .enumerate()
                            .map(|(level, pixels)| codecs[i].encode(pixels, side >> level))
                            .collect();
                    }
                    let stored = (out, found);
                    if let Some(path) = &kept {
                        cache::write(path, &stored);
                    }
                    Some(Encoded {
                        maps: stored.0,
                        cells: stored.1,
                        kept: false,
                    })
                });
                *loading = Loading::Encoding(task);
            }
            Loading::Encoding(task) => {
                if let Some(encoded) = block_on(poll_once(task)) {
                    finished.push((path.clone(), encoded));
                }
            }
        }
    }
    for (path, encoded) in finished {
        arrays.loading.remove(&path);
        let Some(encoded) = encoded else {
            continue;
        };
        // A slot: a free one, else the arrays grown, doubled up to so many more at once.
        if arrays.free.is_empty() && !arrays.grow(&mut images) {
            warn!("Build: the texture arrays are full at {MAX_SLOTS} sets; {path} waits");
            continue;
        }
        let Some(slot) = arrays.free.pop() else {
            continue;
        };
        if let Ok(mut uploads) = arrays.uploads.pending.lock() {
            for (i, levels) in encoded.maps.into_iter().enumerate() {
                uploads.push(Upload {
                    image: arrays.images[i].id(),
                    side: arrays.sides[i],
                    slot,
                    codec: codecs[i],
                    levels,
                });
            }
            uploads.push(Upload {
                image: arrays.cells.id(),
                side: CELLS,
                slot,
                codec: Codec::Raw(4),
                levels: vec![encoded.cells],
            });
        }
        arrays.slots.insert(
            path,
            Slot {
                slot,
                ready_in: 2,
                last_used: now,
            },
        );
        arrays.made += 1;
        arrays.kept += usize::from(encoded.kept);
        debug!(
            "Build: a texture set resident in slot {slot} of {}",
            arrays.capacity
        );
    }
    // What has not been used for a while gives its slot back.
    let stale: Vec<String> = arrays
        .slots
        .iter()
        .filter(|(_, slot)| slot.ready_in == 0 && now - slot.last_used > KEEP_S)
        .map(|(path, _)| path.clone())
        .collect();
    for path in stale {
        if let Some(slot) = arrays.slots.remove(&path) {
            arrays.free.push(slot.slot);
            arrays.dropped += 1;
            arrays.table_dirty = true;
        }
    }
    if arrays.table_dirty {
        arrays.table_dirty = false;
        let table = arrays.table();
        let worn = arrays.worn();
        let breaks = arrays.breaks();
        let detail = arrays.detail();
        let near = arrays.near;
        let drawn = arrays.damage_detail
            && !arrays.damage_hidden
            && matches!(arrays.damage, DamageMask::Ready);
        if let Some(mut material) = materials.get_mut(&arrays.material) {
            material.extension.table = table;
            material.extension.worn = worn;
            material.extension.breaks = breaks;
            material.extension.detail = detail;
            material.extension.near = near;
            material.extension.damage.y = if drawn { 1.0 } else { 0.0 };
        }
    }
}

/// The arrays' textures as last written, in the render world: when one grows, its layers
/// are copied from the old into the new at every level before anything more is written.
/// (Bevy's own copy on resize copies the first level only.)
#[derive(Resource, Default)]
struct Written(BTreeMap<AssetId<Image>, bevy::render::render_resource::Texture>);

/// Writes the layers made resident into the arrays on the GPU, each level of each map, once
/// the arrays exist at a size that holds them.
fn write_layers(
    uploads: Res<Uploads>,
    gpu: Res<RenderAssets<GpuImage>>,
    queue: Res<RenderQueue>,
    device: Res<RenderDevice>,
    mut written: ResMut<Written>,
) {
    // Each array as drawn now, for the main world to follow (`Arrays::follow_drawn`).
    if let Ok(mut drawn) = uploads.drawn.lock() {
        for (id, seen) in drawn.iter_mut() {
            *seen = gpu.get(*id).map(|image| {
                let size = image.texture_descriptor.size;
                [size.width, size.depth_or_array_layers]
            });
        }
    }
    let Ok(mut pending) = uploads.pending.lock() else {
        return;
    };
    // An array grown since it was last written: its old layers into the new one.
    let ids: std::collections::BTreeSet<AssetId<Image>> =
        pending.iter().map(|upload| upload.image).collect();
    for id in ids {
        let Some(image) = gpu.get(id) else {
            continue;
        };
        if let Some(old) = written.0.get(&id)
            && old.id() != image.texture.id()
        {
            let layers = old
                .depth_or_array_layers()
                .min(image.texture_descriptor.size.depth_or_array_layers);
            let mut encoder = device.create_command_encoder(&default());
            for level in 0..image
                .texture_descriptor
                .mip_level_count
                .min(old.mip_level_count())
            {
                let side = image.texture_descriptor.size.width >> level;
                let at = |texture| TexelCopyTextureInfo {
                    texture,
                    mip_level: level,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                };
                encoder.copy_texture_to_texture(
                    at(old),
                    at(&image.texture),
                    Extent3d {
                        width: side,
                        height: side,
                        depth_or_array_layers: layers,
                    },
                );
            }
            queue.submit([encoder.finish()]);
        }
        written.0.insert(id, image.texture.clone());
    }
    // An array made again at another side lets its old texture go.
    written.0.retain(|id, _| gpu.get(*id).is_some());
    pending.retain(|upload| {
        let Some(image) = gpu.get(upload.image) else {
            return true;
        };
        if image.texture_descriptor.size.depth_or_array_layers <= upload.slot {
            return true;
        }
        for (level, data) in upload.levels.iter().enumerate() {
            let side = upload.side >> level;
            let (row, rows) = upload.codec.rows(side);
            queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &image.texture,
                    mip_level: level as u32,
                    origin: Origin3d {
                        x: 0,
                        y: 0,
                        z: upload.slot,
                    },
                    aspect: TextureAspect::All,
                },
                data,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(rows),
                },
                Extent3d {
                    width: side,
                    height: side,
                    depth_or_array_layers: 1,
                },
            );
        }
        false
    });
}

/// Whether the GPU samples BC textures.
fn compressed(world: &World) -> bool {
    world.get_resource::<RenderDevice>().is_some_and(|device| {
        device
            .features()
            .contains(bevy::render::render_resource::WgpuFeatures::TEXTURE_COMPRESSION_BC)
    })
}

/// Makes the arrays and their material, where things are drawn.
pub fn setup(
    world: &mut World,
    layers: Arc<Layers>,
    exists: Arc<dyn Fn(&str) -> bool + Send + Sync>,
) -> Option<Arrays> {
    let compressed = compressed(world);
    let uploads = world.get_resource::<Uploads>()?.clone();
    let quality = world.get_resource::<Quality>().copied().unwrap_or_default();
    let fires = match world.get_resource::<terra_world::fire::FireGlows>() {
        Some(glows) => glows.buffer.clone(),
        None => world
            .get_resource_mut::<Assets<ShaderBuffer>>()?
            .add(ShaderBuffer::from(vec![
                [0.0f32; 4];
                terra_world::fire::GLOWS
            ])),
    };
    world.resource_scope(|world, mut images: Mut<Assets<Image>>| {
        let mut materials = world.get_resource_mut::<Assets<BuildMaterial>>()?;
        let mut arrays = Arrays::new(
            &mut images,
            &mut materials,
            compressed,
            quality,
            layers,
            exists,
            fires,
        );
        arrays.uploads = uploads;
        info!(
            "Build: texture arrays of {} texels a side, {}, {:.1} MiB a texture set; compressed sets kept in {}.",
            arrays.side(),
            if compressed {
                "BC compressed"
            } else {
                "uncompressed"
            },
            arrays.set_bytes() as f32 / (1024.0 * 1024.0),
            cache::file(0)
                .as_deref()
                .and_then(std::path::Path::parent)
                .map_or("nowhere".into(), |folder| folder.display().to_string())
        );
        Some(arrays)
    })
}

pub fn plugin(app: &mut App) {
    if !app.is_plugin_added::<bevy::pbr::PbrPlugin>() {
        return;
    }
    let uploads = Uploads::default();
    app.add_plugins(MaterialPlugin::<BuildMaterial>::default())
        .insert_resource(uploads.clone());
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .insert_resource(uploads)
            .init_resource::<Written>()
            .add_systems(Render, write_layers.in_set(RenderSystems::PrepareResources));
    }
}

#[cfg(test)]
mod tests;
