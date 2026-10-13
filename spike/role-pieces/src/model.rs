//! Models read from glTF files: the game's door and window (Tripo, their textures written
//! out beside the spike's models under `tex/<name>/` for the written models to refer to),
//! and the pieces the spike's Blender scripts author, which take the kit's own materials.

use glam::{Mat3, Quat, Vec3};
use serde_json::Value;
use terra_build::Surface;

#[derive(Clone)]
pub struct Model {
    /// Its material's key: `tex/<name>`.
    pub key: String,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<[f32; 2]>,
    /// Each corner's colour, as authored (a stone's shade), or white.
    pub colours: Vec<[f32; 3]>,
    pub centre: Vec3,
    pub size: Vec3,
    pub low: Vec3,
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

/// A colour accessor's values, as linear RGB from 0 to 1, whatever their type.
fn colours(bin: &[u8], json: &Value, accessor: usize) -> Vec<[f32; 3]> {
    let a = &json["accessors"][accessor];
    let view = &json["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
    let start = view_start(json, a);
    let count = a["count"].as_u64().unwrap() as usize;
    let width = if a["type"] == "VEC4" { 4 } else { 3 };
    let kind = a["componentType"].as_u64().unwrap();
    let size = match kind {
        5121 => 1,
        5123 => 2,
        _ => 4,
    };
    let stride = view["byteStride"].as_u64().unwrap_or((width * size) as u64) as usize;
    (0..count)
        .map(|i| {
            let mut c = [1.0f32; 3];
            for (k, v) in c.iter_mut().enumerate() {
                let at = start + i * stride + k * size;
                *v = match kind {
                    5121 => bin[at] as f32 / 255.0,
                    5123 => u16::from_le_bytes([bin[at], bin[at + 1]]) as f32 / 65535.0,
                    _ => f32::from_le_bytes(bin[at..at + 4].try_into().unwrap()),
                };
            }
            c
        })
        .collect()
}

/// The glTF document and its binary chunk.
fn read(path: &str) -> (Value, Vec<u8>) {
    let bytes = std::fs::read(path).unwrap_or_else(|why| panic!("{path}: {why}"));
    let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json: Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
    let bin = bytes[20 + length + 8..].to_vec();
    (json, bin)
}

/// A node's transform, as a matrix applied to its mesh's points.
fn node_matrix(node: &Value) -> glam::Mat4 {
    let v = |key: &str, n: usize, default: &[f32]| -> Vec<f32> {
        node[key].as_array().map_or(default.to_vec(), |a| {
            a.iter().take(n).map(|x| x.as_f64().unwrap() as f32).collect()
        })
    };
    let t = v("translation", 3, &[0.0, 0.0, 0.0]);
    let r = v("rotation", 4, &[0.0, 0.0, 0.0, 1.0]);
    let s = v("scale", 3, &[1.0, 1.0, 1.0]);
    glam::Mat4::from_scale_rotation_translation(
        Vec3::new(s[0], s[1], s[2]),
        Quat::from_xyzw(r[0], r[1], r[2], r[3]),
        Vec3::new(t[0], t[1], t[2]),
    )
}

impl Model {
    /// Reads every mesh of a model, as one, writing its first material's textures to
    /// `out/tex/<name>` if it has any.
    pub fn load(path: &str, out: &str) -> Model {
        Self::load_nodes(path, out, None).remove(0).1
    }

    /// Reads a model's named nodes, each as a model of its own, in its node's own space
    /// (its translation taken off), or with `None` all of them as one.
    pub fn load_nodes(path: &str, out: &str, each: Option<()>) -> Vec<(String, Model)> {
        let name = std::path::Path::new(path)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let (json, bin) = read(path);
        let empty = |key: String| Model {
            key,
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colours: Vec::new(),
            centre: Vec3::ZERO,
            size: Vec3::ONE,
            low: Vec3::ZERO,
        };
        let mut found: Vec<(String, Model)> = Vec::new();
        let nodes = json["nodes"].as_array().cloned().unwrap_or_default();
        for node in &nodes {
            let Some(mesh) = node["mesh"].as_u64() else {
                continue;
            };
            let node_name = node["name"].as_str().unwrap_or("").to_string();
            let matrix = if each.is_some() {
                // Turned and scaled as authored, but about its own origin.
                let mut m = node_matrix(node);
                m.w_axis = glam::Vec4::new(0.0, 0.0, 0.0, 1.0);
                m
            } else {
                node_matrix(node)
            };
            if each.is_some() || found.is_empty() {
                found.push((node_name.clone(), empty(format!("tex/{name}"))));
            }
            let model = &mut found.last_mut().unwrap().1;
            let normal_matrix = glam::Mat3::from_mat4(matrix).inverse().transpose();
            for primitive in json["meshes"][mesh as usize]["primitives"]
                .as_array()
                .cloned()
                .unwrap_or_default()
            {
                let attribute =
                    |name: &str| primitive["attributes"][name].as_u64().map(|a| a as usize);
                let p = floats(&bin, &json, attribute("POSITION").unwrap(), 3);
                let n = floats(&bin, &json, attribute("NORMAL").unwrap(), 3);
                let t = attribute("TEXCOORD_0").map(|a| floats(&bin, &json, a, 2));
                let c = attribute("COLOR_0").map(|a| colours(&bin, &json, a));
                let order = indices(&bin, &json, primitive["indices"].as_u64().unwrap() as usize);
                for &i in &order {
                    let at = matrix.transform_point3(Vec3::new(p[i * 3], p[i * 3 + 1], p[i * 3 + 2]));
                    model.positions.push(at);
                    model.normals.push(
                        (normal_matrix * Vec3::new(n[i * 3], n[i * 3 + 1], n[i * 3 + 2]))
                            .normalize_or_zero(),
                    );
                    model
                        .uvs
                        .push(t.as_ref().map_or([0.0; 2], |t| [t[i * 2], t[i * 2 + 1]]));
                    model
                        .colours
                        .push(c.as_ref().map_or([1.0; 3], |c| c[i].map(|v| libm::powf(v, 1.0 / 2.2))));
                }
            }
        }
        for (_, model) in &mut found {
            let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for p in &model.positions {
                low = low.min(*p);
                high = high.max(*p);
            }
            model.centre = (low + high) * 0.5;
            model.size = (high - low).max(Vec3::splat(1e-3));
            model.low = low;
        }
        // The first material's textures, as files: colour, normal, and metal and roughness.
        let material = &json["materials"][0];
        if !material.is_null() && !json["textures"].is_null() {
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
                let Some(texture) = slot["index"].as_u64() else {
                    continue;
                };
                let source = json["textures"][texture as usize]["source"]
                    .as_u64()
                    .unwrap() as usize;
                let image = &json["images"][source];
                let Some(view) = image["bufferView"].as_u64() else {
                    continue;
                };
                let view = &json["bufferViews"][view as usize];
                let start = view["byteOffset"].as_u64().unwrap_or(0) as usize;
                let end = start + view["byteLength"].as_u64().unwrap() as usize;
                let ext = if image["mimeType"] == "image/png" {
                    "png"
                } else {
                    "jpg"
                };
                std::fs::write(format!("{dir}/{file}.{ext}"), &bin[start..end]).unwrap();
            }
        }
        found
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
            out.colours.push(self.colours[i]);
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

    /// As the game fits a model to a box: its axes along `x`, `y`, scaled to `extents`.
    pub fn fill(&self, centre: Vec3, x: Vec3, y: Vec3, extents: Vec3) -> (Vec3, Quat, Vec3) {
        let x = x.normalize_or(Vec3::X);
        let y = (y - x * y.dot(x)).normalize_or(Vec3::Y);
        let z = x.cross(y);
        let rotation = Quat::from_mat3(&Mat3::from_cols(x, y, z));
        let scale = extents / self.size;
        (centre - rotation * (self.centre * scale), rotation, scale)
    }
}
