//! Construction materials drawn from texture arrays (D290, docs/proposals/construction-002.md):
//! every texture set a material wears (its walls' stone, its floor's flags, its timber's
//! bark) is a layer of four arrays, colour, normal, occlusion-roughness-metal and height,
//! each at the texture budget ([`SIDE`]) and block compressed where the GPU can (`bc`); each
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
        texture::GpuImage,
    },
    shader::ShaderRef,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use terra_build::cells::CELLS;
use terra_build::kit::Kits;
use terra_build::{Kind, Material};
use terra_world::quality::Quality;

/// The texture budget: every texture set is this many texels square in the arrays, with its
/// mipmaps down to 4 by 4.
pub const SIDE: u32 = 1024;
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
}

impl MaterialExtension for BuildArrays {
    fn fragment_shader() -> ShaderRef {
        "shaders/build.wesl".into()
    }

    /// So a covering's shingles that damage took are not in depth or shadow either (D305).
    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/build_prepass.wesl".into()
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
                let set = Set {
                    path: texture.path.clone(),
                    depth: kit.depth(*material),
                    reflectance: texture.reflectance,
                    breaks,
                    share,
                };
                let id = out.layer(set);
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
                    });
                    if under < MAX_LAYERS {
                        out.worn.insert(id as u16, under as u16);
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

/// The uploads waiting for the render world, shared by both worlds.
#[derive(Resource, Clone, Default)]
struct Uploads(Arc<Mutex<Vec<Upload>>>);

/// A texture set's four maps, read and compressed, and its cells.
struct Encoded {
    maps: [Vec<Vec<u8>>; 4],
    cells: Vec<u8>,
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
    /// Each set's cells, a byte a texel, unfiltered, one level.
    cells: Handle<Image>,
    capacity: u32,
    layers: Arc<Layers>,
    slots: BTreeMap<String, Slot>,
    free: Vec<u32>,
    loading: BTreeMap<String, Loading>,
    uploads: Uploads,
    table_dirty: bool,
    /// How many layers relief mapping steps through at the preset (`Quality::relief_layers`):
    /// none at the low preset, which draws every material flat but for its normal map.
    relief: f32,
    /// The shared damage mask, being made and then written, in `DAMAGE_SLOT`.
    damage: DamageMask,
    /// Whether the mask is drawn at the preset (not at the low).
    damage_detail: bool,
    /// Whether a file is there, by its path under `assets/`.
    exists: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    /// Texture sets made resident and let go, and how long the slowest took from asked to
    /// ready, for the log.
    pub made: usize,
    pub dropped: usize,
}

/// The slot of the arrays the shared damage mask is kept in.
const DAMAGE_SLOT: u32 = 0;
/// How much darker a damaged cube all but gone is where the mask is not drawn, as
/// `terra_build::loss` darkens one far off.
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
                TextureFormat::Bc1RgbaUnormSrgb,
                TextureFormat::Bc5RgUnorm,
                TextureFormat::Bc1RgbaUnorm,
                TextureFormat::Bc4RUnorm,
            ],
            [Codec::Bc1, Codec::Bc5, Codec::Bc1, Codec::Bc4],
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

/// An array of `slots` layers in a format, with mipmaps, to be written layer by layer.
fn array(format: TextureFormat, slots: u32, filtered: bool) -> Image {
    sized(format, slots, filtered, SIDE, bc::levels(SIDE))
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
        layers: Arc<Layers>,
        exists: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    ) -> Arrays {
        let (formats, codecs) = formats(compressed);
        let images_of: [Handle<Image>; 4] =
            std::array::from_fn(|i| images.add(array(formats[i], FIRST_SLOTS, i == 0)));
        let cells = images.add(sized(
            TextureFormat::Rgba8Unorm,
            FIRST_SLOTS,
            false,
            CELLS,
            1,
        ));
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
                colour: images[0].clone(),
                normal: images[1].clone(),
                arm: images[2].clone(),
                height: images[3].clone(),
                cells: cells.clone(),
            },
        });
        Arrays {
            material,
            images,
            codecs,
            cells,
            capacity: FIRST_SLOTS,
            layers,
            slots: BTreeMap::new(),
            // The first slot is the damage mask's, made once and kept.
            free: (DAMAGE_SLOT + 1..FIRST_SLOTS).rev().collect(),
            damage: DamageMask::Making(AsyncComputeTaskPool::get().spawn(async move {
                let maps = damage::maps();
                std::array::from_fn(|i| {
                    bc::chain(&maps[i], SIDE)
                        .iter()
                        .enumerate()
                        .map(|(level, pixels)| codecs[i].encode(pixels, SIDE >> level))
                        .collect()
                })
            })),
            loading: BTreeMap::new(),
            uploads: Uploads::default(),
            table_dirty: true,
            relief: Quality::High.relief_layers(),
            damage_detail: true,
            exists,
            made: 0,
            dropped: 0,
        }
    }

    /// The kits' layers changed, as when a kit's file is saved.
    pub fn relayer(&mut self, layers: Arc<Layers>) {
        self.layers = layers;
        self.table_dirty = true;
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
        for id in layers.with_worn(ids) {
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
            .map(|codec| {
                (0..bc::levels(SIDE))
                    .map(|level| codec.bytes(SIDE >> level))
                    .sum::<usize>()
            })
            .sum()
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

/// A picture's RGBA bytes at the budget, `SIDE` square: averaged down where it is a whole
/// multiple of it, else sampled between texels, wrapping, as it tiles.
fn to_side(data: &[u8], width: u32, height: u32, channels: usize) -> Vec<u8> {
    let (w, h, side) = (width as usize, height as usize, SIDE as usize);
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
pub(crate) fn pixels(image: &Image) -> Option<(Vec<u8>, u32, u32, usize)> {
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
    // The damage mask: written once made, drawn once written, but not at the low preset.
    let detail = *quality != Quality::Low;
    if arrays.damage_detail != detail {
        arrays.damage_detail = detail;
        arrays.table_dirty = true;
    }
    match &mut arrays.damage {
        DamageMask::Making(task) => {
            if let Some(maps) = block_on(poll_once(task)) {
                if let Ok(mut uploads) = arrays.uploads.0.lock() {
                    for (i, levels) in maps.into_iter().enumerate() {
                        uploads.push(Upload {
                            image: arrays.images[i].id(),
                            side: SIDE,
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
            } else {
                *frames -= 1;
            }
        }
        DamageMask::Ready => {}
    }
    // Written last frame: ready now.
    for slot in arrays.slots.values_mut() {
        if slot.ready_in > 0 {
            slot.ready_in -= 1;
            arrays.table_dirty |= slot.ready_in == 0;
        }
    }
    let codecs = arrays.codecs;
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
                    let mut out: [Vec<Vec<u8>>; 4] = Default::default();
                    let mut found = Vec::new();
                    for (i, map) in maps.into_iter().enumerate() {
                        // Its stones or shingles, from its height as read (D305), as the
                        // CPU finds them from the same picture; none, one cell.
                        if i == 3 {
                            found = match &map {
                                Some((data, w, h, channels)) => terra_build::cells::label(
                                    data,
                                    *w as usize,
                                    *h as usize,
                                    *channels,
                                    share,
                                ),
                                None => [128u8, 128, 128, 255].repeat((CELLS * CELLS) as usize),
                            };
                        }
                        let base = match map {
                            Some((data, w, h, channels)) => to_side(&data, w, h, channels),
                            // No height: flat.
                            None => vec![0u8; (SIDE * SIDE * 4) as usize],
                        };
                        out[i] = bc::chain(&base, SIDE)
                            .iter()
                            .enumerate()
                            .map(|(level, pixels)| codecs[i].encode(pixels, SIDE >> level))
                            .collect();
                    }
                    Some(Encoded {
                        maps: out,
                        cells: found,
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
        // A slot: a free one, else the arrays doubled.
        if arrays.free.is_empty() {
            let grown = (arrays.capacity * 2).min(MAX_SLOTS);
            if grown == arrays.capacity {
                warn!("Build: the texture arrays are full at {MAX_SLOTS} sets; {path} waits");
                continue;
            }
            for handle in arrays.images.iter().chain([&arrays.cells]) {
                if let Some(mut image) = images.get_mut(handle) {
                    image.texture_descriptor.size.depth_or_array_layers = grown;
                }
            }
            // So its bind group is made again with the grown arrays.
            arrays.table_dirty = true;
            arrays.free.extend((arrays.capacity..grown).rev());
            arrays.capacity = grown;
            info!("Build: the texture arrays grown to {grown} slots");
        }
        let Some(slot) = arrays.free.pop() else {
            continue;
        };
        if let Ok(mut uploads) = arrays.uploads.0.lock() {
            for (i, levels) in encoded.maps.into_iter().enumerate() {
                uploads.push(Upload {
                    image: arrays.images[i].id(),
                    side: SIDE,
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
        let drawn = arrays.damage_detail && matches!(arrays.damage, DamageMask::Ready);
        if let Some(mut material) = materials.get_mut(&arrays.material) {
            material.extension.table = table;
            material.extension.worn = worn;
            material.extension.breaks = breaks;
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
    let Ok(mut pending) = uploads.0.lock() else {
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
    pending.retain(|upload| {
        let Some(image) = gpu.get(upload.image) else {
            return true;
        };
        if image.texture_descriptor.size.depth_or_array_layers <= upload.slot {
            return true;
        }
        for (level, data) in upload.levels.iter().enumerate() {
            let side = upload.side >> level;
            let (row, rows) = match upload.codec {
                Codec::Raw(n) => (side * n as u32, side),
                Codec::Bc1 | Codec::Bc4 => (side.div_ceil(4) * 8, side.div_ceil(4)),
                Codec::Bc5 => (side.div_ceil(4) * 16, side.div_ceil(4)),
            };
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
    world.resource_scope(|world, mut images: Mut<Assets<Image>>| {
        let mut materials = world.get_resource_mut::<Assets<BuildMaterial>>()?;
        let mut arrays = Arrays::new(&mut images, &mut materials, compressed, layers, exists);
        arrays.uploads = uploads;
        info!(
            "Build: texture arrays of {SIDE} texels a side, {}, {:.1} MiB a texture set.",
            if compressed {
                "BC compressed"
            } else {
                "uncompressed"
            },
            arrays.set_bytes() as f32 / (1024.0 * 1024.0)
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
mod tests {
    use super::*;

    #[test]
    fn a_set_not_yet_resident_or_written_never_reads_another_slot_nor_the_damage_mask() {
        // Not resident, resident but not yet written, and ready.
        let reading = Slot {
            slot: 3,
            ready_in: 1,
            last_used: 0.0,
        };
        let ready = Slot {
            slot: 3,
            ready_in: 0,
            last_used: 0.0,
        };
        for drawn in [slot_of(None), slot_of(Some(&reading))] {
            assert!(
                drawn < 0.0,
                "drawn plain until written, not from slot {drawn}"
            );
            assert_ne!(drawn, DAMAGE_SLOT as f32);
        }
        assert_eq!(slot_of(Some(&ready)), 3.0);
    }

    #[test]
    fn kits_naming_the_same_set_share_its_layer_and_pieces_have_none() {
        let texture = |path: &str| terra_build::kit::Texture {
            path: path.into(),
            metres: [1.0, 1.0],
            depth: 0.0,
            reflectance: 0.35,
            strips: None,
            cells: 1,
            worn: None,
        };
        let kit = |id: u16, masonry: &str| terra_build::kit::Kit {
            id,
            textures: [
                (Material::Masonry, texture(masonry)),
                (Material::Dressed, texture("materials/dressed")),
                (Material::Piece, texture("materials/atlas")),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let layers = Layers::of(&Kits(vec![
            kit(2, "materials/stone"),
            kit(7, "materials/lime"),
        ]));
        let stone = layers.get(Kind(2), Material::Masonry).unwrap();
        let lime = layers.get(Kind(7), Material::Masonry).unwrap();
        assert_ne!(stone, lime);
        assert_eq!(
            layers.get(Kind(2), Material::Dressed),
            layers.get(Kind(7), Material::Dressed)
        );
        assert_eq!(layers.get(Kind(2), Material::Piece), None);
        assert!(stone > 0 && lime > 0);
        assert_eq!(layers.sets.len(), 3);
    }

    #[test]
    fn a_picture_twice_the_budget_is_averaged_down_and_tiles() {
        let side = SIDE as usize * 2;
        let data: Vec<u8> = (0..side * side)
            .flat_map(|i| [(i % 2 * 200) as u8, 50, 50, 255])
            .collect();
        let out = to_side(&data, side as u32, side as u32, 4);
        assert_eq!(out.len(), SIDE as usize * SIDE as usize * 4);
        assert!(out.chunks_exact(4).all(|p| p[0] == 100 && p[1] == 50));
    }
}
