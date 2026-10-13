//! The game's Tripo models (a log, a door, a window), read from their glTF files and placed
//! as the game places them (`terra_game`'s `build/pieces.rs`), so the reference and every
//! candidate's openings are the game's own. Each model's textures are written out beside
//! the spike's models, under `tex/<name>/`, for the written models to refer to.

use glam::{Mat3, Quat, Vec3};
use serde_json::Value;
use std::f32::consts::{PI, TAU};
use terra_build::Surface;
use terra_build::logs::Split;

pub struct Model {
    /// Its material's key: `tex/<name>`.
    pub key: String,
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    uvs: Vec<[f32; 2]>,
    centre: Vec3,
    size: Vec3,
}

fn view_start(json: &Value, accessor: &Value) -> usize {
    let view = &json["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
    view["byteOffset"].as_u64().unwrap_or(0) as usize
        + accessor["byteOffset"].as_u64().unwrap_or(0) as usize
}

fn floats(bin: &[u8], json: &Value, accessor: usize, width: usize) -> Vec<f32> {
    let a = &json["accessors"][accessor];
    let view = &json["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
    let start = view_start(json, a);
    let count = a["count"].as_u64().unwrap() as usize;
    let stride = view["byteStride"].as_u64().unwrap_or(width as u64 * 4) as usize;
    let mut out = Vec::with_capacity(count * width);
    for i in 0..count {
        for k in 0..width {
            let at = start + i * stride + k * 4;
            out.push(f32::from_le_bytes(bin[at..at + 4].try_into().unwrap()));
        }
    }
    out
}

fn indices(bin: &[u8], json: &Value, accessor: usize) -> Vec<usize> {
    let a = &json["accessors"][accessor];
    let start = view_start(json, a);
    let count = a["count"].as_u64().unwrap() as usize;
    let kind = a["componentType"].as_u64().unwrap();
    (0..count)
        .map(|i| match kind {
            5125 => u32::from_le_bytes(bin[start + i * 4..start + i * 4 + 4].try_into().unwrap())
                as usize,
            5123 => u16::from_le_bytes(bin[start + i * 2..start + i * 2 + 2].try_into().unwrap())
                as usize,
            _ => bin[start + i] as usize,
        })
        .collect()
}

impl Model {
    /// Reads a model of one mesh and one material, writing its textures to `out/tex/<name>`.
    pub fn load(path: &str, out: &str) -> Model {
        let name = std::path::Path::new(path)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let bytes = std::fs::read(path).unwrap();
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json: Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
        let bin = &bytes[20 + length + 8..];
        let primitive = &json["meshes"][0]["primitives"][0];
        let attribute = |name: &str| primitive["attributes"][name].as_u64().unwrap() as usize;
        let p = floats(bin, &json, attribute("POSITION"), 3);
        let n = floats(bin, &json, attribute("NORMAL"), 3);
        let t = floats(bin, &json, attribute("TEXCOORD_0"), 2);
        let order = indices(bin, &json, primitive["indices"].as_u64().unwrap() as usize);
        let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let mut model = Model {
            key: format!("tex/{name}"),
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            centre: Vec3::ZERO,
            size: Vec3::ONE,
        };
        for &i in &order {
            let at = Vec3::new(p[i * 3], p[i * 3 + 1], p[i * 3 + 2]);
            low = low.min(at);
            high = high.max(at);
            model.positions.push(at);
            model
                .normals
                .push(Vec3::new(n[i * 3], n[i * 3 + 1], n[i * 3 + 2]));
            model.uvs.push([t[i * 2], t[i * 2 + 1]]);
        }
        model.centre = (low + high) * 0.5;
        model.size = (high - low).max(Vec3::splat(1e-3));
        // Its textures, as files: colour, normal, and metal and roughness.
        let material = &json["materials"][primitive["material"].as_u64().unwrap() as usize];
        let dir = format!("{out}/tex/{name}");
        std::fs::create_dir_all(&dir).unwrap();
        for (slot, file) in [
            (
                &material["pbrMetallicRoughness"]["baseColorTexture"],
                "color",
            ),
            (&material["normalTexture"], "normal"),
            (
                &material["pbrMetallicRoughness"]["metallicRoughnessTexture"],
                "mr",
            ),
        ] {
            let texture = slot["index"].as_u64().unwrap() as usize;
            let source = json["textures"][texture]["source"].as_u64().unwrap() as usize;
            let image = &json["images"][source];
            let view = &json["bufferViews"][image["bufferView"].as_u64().unwrap() as usize];
            let start = view["byteOffset"].as_u64().unwrap_or(0) as usize;
            let end = start + view["byteLength"].as_u64().unwrap() as usize;
            let ext = if image["mimeType"] == "image/png" {
                "png"
            } else {
                "jpg"
            };
            std::fs::write(format!("{dir}/{file}.{ext}"), &bin[start..end]).unwrap();
        }
        model
    }

    /// A copy of it, moved, turned and scaled, into a surface.
    pub fn place(&self, at: (Vec3, Quat, Vec3), out: &mut Surface) {
        let (translation, rotation, scale) = at;
        let flip = scale.x * scale.y * scale.z < 0.0;
        let mut add = |i: usize| {
            out.positions
                .push(translation + rotation * (self.positions[i] * scale));
            out.normals
                .push((rotation * (self.normals[i] / scale)).normalize_or_zero());
            out.uvs.push(self.uvs[i]);
            out.colours.push([1.0; 3]);
        };
        for tri in 0..self.positions.len() / 3 {
            let t = [tri * 3, tri * 3 + 1, tri * 3 + 2];
            if flip {
                add(t[0]);
                add(t[2]);
                add(t[1]);
            } else {
                t.into_iter().for_each(&mut add);
            }
        }
    }

    /// As `pieces.rs` fits a model to a box: its axes along `x`, `y`, scaled to `extents`.
    pub fn fill(&self, centre: Vec3, x: Vec3, y: Vec3, extents: Vec3) -> (Vec3, Quat, Vec3) {
        let x = x.normalize_or(Vec3::X);
        let y = (y - x * y.dot(x)).normalize_or(Vec3::Y);
        let z = x.cross(y);
        let rotation = Quat::from_mat3(&Mat3::from_cols(x, y, z));
        let scale = extents / self.size;
        (centre - rotation * (self.centre * scale), rotation, scale)
    }

    /// As `pieces.rs` fits a log: along it, as thick as the kit's logs, turned by its seed.
    pub fn log(
        &self,
        from: Vec3,
        to: Vec3,
        split: Split,
        seed: u32,
        diameter: f32,
    ) -> (Vec3, Quat, Vec3) {
        let length = from.distance(to).max(1e-3);
        let axis = (to - from) / length;
        let thick = self.size.y.max(self.size.z);
        let mut scale = Vec3::new(length / self.size.x, diameter / thick, diameter / thick);
        let flip = seed & 0x10000 != 0;
        let mut spin = (seed & 0xffff) as f32 / 65536.0 * TAU;
        let mut middle = (from + to) * 0.5;
        if split == Split::Bottom {
            scale.y *= 0.5;
            middle.y -= diameter * 0.25;
            spin = 0.0;
        }
        let turn = Quat::from_rotation_arc(Vec3::X, axis)
            * Quat::from_rotation_x(spin)
            * Quat::from_rotation_y(if flip { PI } else { 0.0 });
        (middle - turn * (self.centre * scale), turn, scale)
    }
}
