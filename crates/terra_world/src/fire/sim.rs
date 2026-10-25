//! A baked kind's flame (D493, docs/fire-module.md's last section): its marched field reads its
//! shape and motion from a looping 3D volume, an offline fluid simulation's flame at its own
//! cells (`tools/fire/bake`, `campfire-sim.dds` and its sidecar), in place of its lobes and noise
//! (`fire.wesl`'s `baked_gas`). As shipped games play fire back from offline sims (Houdini or
//! EmberGen flipbooks, Unreal's sparse volume textures; research/2026-10-25-fire-fluid-sim.md).
//!
//! The loop is shared by every fire that plays it, read once the first such fire is kindled, so
//! a world without one carries none of it. Each fire plays it from its own offset, mirrored or
//! not across the bake's wind, turned so the bake's wind blows the game's way, and scaled about
//! its height, at the root of one over its scale, as a fire's puffing goes. Its frames are tiled
//! across and down one 3D texture, BC5 where the adapter samples BC slices of a 3D texture and
//! decoded to Rg8 at load where it does not, so the repository carries the bake once.
//!
//! The shader reads the loop itself; the frame, tile and placing arithmetic here (`frames_at`,
//! `tile`, `texel`, `to_bake`) is its copy on the CPU, which the tests hold it to.

use super::{Blazes, Body, Drawn, FireMaterial, HEARTHS};
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat, WgpuFeatures},
        renderer::RenderDevice,
    },
};

/// The bake every baked kind plays today: the campfire's (fire round g23's sim F).
const SET: &str = "campfire-sim";
/// How tall a fire plays the bake at its own size (`Kindled::tall`): the staged campfire's, whose
/// view and size g23's sim was matched to (`burns` 1.5 m, g21's film).
const BAKED_TALL: f32 = 1.5;
/// How far a fire's scale varies about its height's, either way (D493).
const SCALE_SPREAD: f32 = 0.15;
/// How quickly the wind a loop is turned by follows the climate's, seconds: the climate's is
/// looked up every 3 s and comes in steps (`blow`), and a loop turned by a step would jump.
const TURN_EASE_S: f32 = 2.0;
/// Below this wind, metres a second, a fire's loop keeps a bearing of its own.
const CALM_MPS: f32 = 0.1;
/// DXGI's BC5 UNORM, the format the baker writes (`tools/fire/bake/bc5.ts`).
const DXGI_BC5_UNORM: u32 = 83;
/// A DDS file's header with its DX10 extension, in bytes.
const DDS_HEAD: usize = 148;

/// A bake's sidecar, what the shader and the placing need of it.
#[derive(serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Sim {
    pub frames: u32,
    pub fps: f32,
    /// A frame's cells: along the bake's wind, across it, up.
    pub cells: [u32; 3],
    pub columns: u32,
    pub rows: u32,
    /// The whole volume's texels.
    pub size: [u32; 3],
    /// The box its cells fill, metres from the fire's foot (x downwind, z up).
    pub box_min: [f32; 3],
    pub box_size: [f32; 3],
}

impl Sim {
    /// The bake's sidecar. A mistake in one is a mistake in the game, so it stops it.
    pub fn read() -> Sim {
        let path = file(&format!("{SET}.json"));
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|problem| panic!("Could not read {}: {problem}", path.display()));
        let sim: Sim = serde_json::from_str(&text)
            .unwrap_or_else(|problem| panic!("{}: {problem}", path.display()));
        assert!(
            sim.columns * sim.cells[0] == sim.size[0]
                && sim.rows * sim.cells[1] == sim.size[1]
                && sim.cells[2] == sim.size[2]
                && sim.columns * sim.rows >= sim.frames
                && sim.frames > 0
                && sim.fps > 0.0,
            "{}: its frames' tiles do not fill its volume",
            path.display()
        );
        sim
    }

    /// How long the loop plays at a fire's own size, seconds.
    pub fn seconds(&self) -> f32 {
        self.frames as f32 / self.fps
    }

    /// What `fire.wesl`'s `Fire::sim` reads: the box's corner and the frames; its size and the
    /// rate; a frame's cells and how many frames a row.
    pub fn packed(&self) -> [Vec4; 3] {
        let [x, y, z] = self.cells.map(|c| c as f32);
        [
            Vec3::from(self.box_min).extend(self.frames as f32),
            Vec3::from(self.box_size).extend(self.fps),
            Vec4::new(x, y, z, self.columns as f32),
        ]
    }

    /// The two frames a fire placed so shows at `time` (seconds on the cards' clock), and how far
    /// from the first to the second it is (`fire.wesl`'s `baked_frames`).
    #[cfg(test)]
    pub fn frames_at(&self, time: f32, placed: Placement) -> (u32, u32, f32) {
        let frames = self.frames as f32;
        let at =
            ((time * placed.rate() + placed.offset) * self.fps / frames).rem_euclid(1.0) * frames;
        let a = at.floor().min(frames - 1.0);
        (a as u32, (a as u32 + 1) % self.frames, at - a)
    }

    /// A frame's tile's corner in the volume, in texels: across and down, `columns` a row
    /// (`fire.wesl`'s `baked_tile`, `tools/fire/bake/volume.ts`).
    #[cfg(test)]
    pub fn tile(&self, frame: u32) -> UVec2 {
        UVec2::new(
            frame % self.columns * self.cells[0],
            frame / self.columns * self.cells[1],
        )
    }

    /// Where in the volume, in texels, a point of the bake's box (metres, as `Placement::to_bake`
    /// gives it) is read in `frame`: half a cell inside its tile at most, so its filtering never
    /// reaches the next frame's; none outside the box (`fire.wesl`'s `baked_gas`).
    #[cfg(test)]
    pub fn texel(&self, bake: Vec3, frame: u32) -> Option<Vec3> {
        let u = (bake - Vec3::from(self.box_min)) / Vec3::from(self.box_size);
        if u.min_element() < 0.0 || u.max_element() > 1.0 {
            return None;
        }
        let cells = UVec3::from(self.cells).as_vec3();
        let t = (u * cells).clamp(Vec3::splat(0.5), cells - 0.5);
        Some(self.tile(frame).as_vec2().extend(0.0) + t)
    }
}

/// Where a fire stands in its loop: its offset into it (seconds), its turn (radians from its
/// frame's east toward its north, where the bake's wind blows), whether it is mirrored across
/// the bake's wind (-1) or not (1), and its scale (the bake's metres to its own).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub offset: f32,
    pub turn: f32,
    pub mirror: f32,
    pub scale: f32,
}

impl Placement {
    /// A fire's, from its seed, its height and the wind across its frame (east, north): any
    /// offset of a loop `seconds` long, either mirror, its scale its height's over the bake's
    /// within `SCALE_SPREAD`, turned downwind or, in a calm, to a bearing of its own.
    pub fn of(seed: f32, tall: f32, downwind: Vec2, seconds: f32) -> Placement {
        let luck = |k: u32| terra_gen::hash::unit(terra_gen::hash::of(&[seed.to_bits(), k]));
        let turn = if downwind.length() > CALM_MPS {
            libm::atan2f(downwind.y, downwind.x)
        } else {
            luck(3) * core::f32::consts::TAU
        };
        Placement {
            offset: luck(0) * seconds,
            turn,
            mirror: if luck(1) < 0.5 { 1.0 } else { -1.0 },
            scale: tall.max(0.05) / BAKED_TALL * (1.0 + SCALE_SPREAD * (2.0 * luck(2) - 1.0)),
        }
    }

    /// How fast it plays its loop: as the root of one over its scale (puffing at about 1.5/√D Hz).
    #[cfg(test)]
    pub fn rate(self) -> f32 {
        1.0 / libm::sqrtf(self.scale)
    }

    /// A point of its fire's frame (east, up, north, metres from its foot) in the bake's box
    /// (downwind, across, up).
    #[cfg(test)]
    pub fn to_bake(self, q: Vec3) -> Vec3 {
        let (s, c) = (libm::sinf(self.turn), libm::cosf(self.turn));
        Vec3::new(c * q.x + s * q.z, self.mirror * (c * q.z - s * q.x), q.y) / self.scale
    }

    /// What `fire.wesl`'s `Fire::baked` reads.
    pub fn packed(self) -> Vec4 {
        Vec4::new(self.offset, self.turn, self.mirror, self.scale)
    }
}

/// The wind across a fire's frame at `at`: east, north (`hearth::frame`).
fn downwind(at: Vec3, wind: Vec3) -> Vec2 {
    let (east, north) = super::hearth::frame(at.normalize_or(Vec3::Y));
    Vec2::new(wind.dot(east), wind.dot(north))
}

fn file(path: &str) -> std::path::PathBuf {
    crate::data::assets().join("textures/fire").join(path)
}

/// What the material binds until a fire plays the loop: one empty texel.
pub(super) fn unread() -> Image {
    Image::new(
        Extent3d::default(),
        TextureDimension::D3,
        vec![0, 0],
        TextureFormat::Rg8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// The bake's volume as the adapter can sample it: its BC5 as it is where `features` hold BC
/// slices of a 3D texture, decoded to Rg8 where they do not (D493).
fn volume(sim: &Sim, features: WgpuFeatures) -> Image {
    let path = file(&format!("{SET}.dds"));
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|problem| panic!("Could not read {}: {problem}", path.display()));
    let [w, h, d] =
        dds_size(&bytes).unwrap_or_else(|| panic!("{}: not a BC5 3D DDS", path.display()));
    assert_eq!(
        [w, h, d],
        sim.size,
        "{}: not its sidecar's size",
        path.display()
    );
    let size = Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: d,
    };
    let data = &bytes[DDS_HEAD..];
    let bc = WgpuFeatures::TEXTURE_COMPRESSION_BC | WgpuFeatures::TEXTURE_COMPRESSION_BC_SLICED_3D;
    let (data, format) = if features.contains(bc) {
        (data.to_vec(), TextureFormat::Bc5RgUnorm)
    } else {
        (decode_bc5(data, [w, h, d]), TextureFormat::Rg8Unorm)
    };
    Image::new(
        size,
        TextureDimension::D3,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// The size of a DDS file of one BC5 3D texture, as the baker writes it (`bc5.ts`'s `dds3d`),
/// when it is one and holds all its blocks.
fn dds_size(bytes: &[u8]) -> Option<[u32; 3]> {
    let word = |o: usize| Some(u32::from_le_bytes(bytes.get(o..o + 4)?.try_into().ok()?));
    if bytes.get(0..4)? != b"DDS " || bytes.get(84..88)? != b"DX10" || word(128)? != DXGI_BC5_UNORM
    {
        return None;
    }
    let size = [word(16)?, word(12)?, word(24)?];
    let blocks = (size[0] / 4) as usize * (size[1] / 4) as usize * size[2] as usize;
    (bytes.len() == DDS_HEAD + blocks * 16).then_some(size)
}

/// One BC4 block's sixteen values, x fastest (Direct3D 10's BC4 UNORM): two endpoints, then
/// sixteen 3-bit indices into eight values, the six between interpolated in sevenths when the
/// first is the greater, otherwise four in fifths and the last two 0 and 255.
fn decode_bc4(block: &[u8]) -> [u8; 16] {
    let (e0, e1) = (u32::from(block[0]), u32::from(block[1]));
    let mut palette = [e0, e1, 0, 0, 0, 0, 0, 255];
    if e0 > e1 {
        for (i, p) in palette.iter_mut().enumerate().skip(2) {
            *p = ((8 - i as u32) * e0 + (i as u32 - 1) * e1 + 3) / 7;
        }
    } else {
        for (i, p) in palette.iter_mut().enumerate().take(6).skip(2) {
            *p = ((6 - i as u32) * e0 + (i as u32 - 1) * e1 + 2) / 5;
        }
        palette[6] = 0;
    }
    let bits = block[2..8]
        .iter()
        .rev()
        .fold(0u64, |bits, &b| (bits << 8) | u64::from(b));
    std::array::from_fn(|t| palette[((bits >> (3 * t)) & 7) as usize] as u8)
}

/// A BC5 3D texture decoded to Rg8: slice by slice, its blocks left to right and top to bottom,
/// each a red BC4 block then a green one.
fn decode_bc5(data: &[u8], [w, h, d]: [u32; 3]) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    let mut out = vec![0u8; w * h * d as usize * 2];
    for (b, block) in data.chunks_exact(16).enumerate() {
        let (bx, rest) = (b % (w / 4), b / (w / 4));
        let (by, z) = (rest % (h / 4), rest / (h / 4));
        let (r, g) = (decode_bc4(&block[..8]), decode_bc4(&block[8..]));
        for t in 0..16 {
            let i = (z * h + by * 4 + t / 4) * w + bx * 4 + t % 4;
            out[i * 2] = r[t];
            out[i * 2 + 1] = g[t];
        }
    }
    out
}

/// Each kept fire that plays the loop told its place in it, the loop read and bound the first
/// time one does.
pub(super) fn tell(
    time: Res<Time>,
    blazes: Res<Blazes>,
    drawn: Res<Drawn>,
    device: Option<Res<RenderDevice>>,
    mut images: ResMut<Assets<Image>>,
    mut fires: ResMut<Assets<FireMaterial>>,
    mut read: Local<Option<(Sim, Vec3)>>,
) {
    let baked = Body::Baked.told();
    if read.is_none() && !blazes.bodies.contains(&baked) {
        return;
    }
    let (sim, wind) = read.get_or_insert_with(|| {
        let sim = Sim::read();
        let features = device.map_or(WgpuFeatures::empty(), |device| device.features());
        let image = images.add(volume(&sim, features));
        for handle in &drawn.fire {
            if let Some(mut material) = fires.get_mut(handle) {
                material.extension.sim = image.clone();
                material.extension.uniform.sim = sim.packed();
            }
        }
        (sim, drawn.wind)
    });
    *wind = wind.lerp(
        drawn.wind,
        1.0 - libm::expf(-time.delta_secs() / TURN_EASE_S),
    );
    let mut placed = [Vec4::ZERO; HEARTHS];
    for (i, (blaze, _)) in blazes
        .kept
        .iter()
        .zip(&blazes.bodies)
        .take(HEARTHS)
        .enumerate()
        .filter(|(_, (_, body))| **body == baked)
    {
        let across = downwind(blaze.at, *wind);
        placed[i] = Placement::of(blaze.seed, blaze.tall, across, sim.seconds()).packed();
    }
    for handle in &drawn.fire {
        if let Some(mut material) = fires.get_mut(handle) {
            material.extension.uniform.baked = placed;
        }
    }
}

#[cfg(test)]
mod tests;
