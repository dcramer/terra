//! A glTF binary of shapes, each a node of its own named for the camera to find, each
//! drawn in materials whose textures are the game's own (`assets/materials/<name>/`),
//! referred to from `assets/models/spike/`, so the game loads them as it loads any model.

use glam::Vec3;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use terra_build::Surface;

/// A shape: its name, where it stands, and its surfaces by material folder.
pub struct Shape {
    pub name: String,
    pub at: Vec3,
    pub surfaces: BTreeMap<String, Surface>,
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        libm::powf((c + 0.055) / 1.055, 2.4)
    }
}

/// Writes the shapes to `path`; returns the triangles in each.
pub fn write(path: &str, root: &str, shapes: &[Shape]) -> Vec<usize> {
    let mut bin: Vec<u8> = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut materials: Vec<String> = Vec::new();
    let mut meshes = Vec::new();
    let mut nodes = vec![json!({"name": root, "children": (1..=shapes.len()).collect::<Vec<_>>()})];
    let mut counts = Vec::new();
    let mut push = |bytes: &[u8], target: u32| -> usize {
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let offset = bin.len();
        bin.extend_from_slice(bytes);
        views.push(
            json!({"buffer": 0, "byteOffset": offset, "byteLength": bytes.len(), "target": target}),
        );
        views.len() - 1
    };
    for shape in shapes {
        let mut primitives = Vec::new();
        let mut triangles = 0;
        for (folder, surface) in &shape.surfaces {
            if surface.positions.is_empty() {
                continue;
            }
            triangles += surface.positions.len() / 3;
            let material = match materials.iter().position(|m| m == folder) {
                Some(i) => i,
                None => {
                    materials.push(folder.clone());
                    materials.len() - 1
                }
            };
            let floats = |v: &mut Vec<u8>, f: f32| v.extend_from_slice(&f.to_le_bytes());
            let (mut pos, mut nor, mut uv, mut col, mut idx) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
            let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for (i, p) in surface.positions.iter().enumerate() {
                low = low.min(*p);
                high = high.max(*p);
                for f in p.to_array() {
                    floats(&mut pos, f);
                }
                for f in surface.normals[i].normalize_or(Vec3::Y).to_array() {
                    floats(&mut nor, f);
                }
                let t = surface.uvs.get(i).copied().unwrap_or([0.0; 2]);
                floats(&mut uv, t[0]);
                floats(&mut uv, t[1]);
                let c = surface.colours.get(i).copied().unwrap_or([1.0; 3]);
                for f in c {
                    floats(&mut col, srgb_to_linear(f));
                }
                floats(&mut col, 1.0);
                idx.extend_from_slice(&(i as u32).to_le_bytes());
            }
            let n = surface.positions.len();
            let mut accessor = |bytes: &[u8], kind: &str, target: u32, extra: Value| {
                let view = push(bytes, target);
                let mut a = json!({"bufferView": view, "componentType": if target == 34963 {5125} else {5126}, "count": n, "type": kind});
                if let (Value::Object(a), Value::Object(e)) = (&mut a, extra) {
                    a.extend(e);
                }
                accessors.push(a);
                accessors.len() - 1
            };
            let p = accessor(
                &pos,
                "VEC3",
                34962,
                json!({"min": low.to_array(), "max": high.to_array()}),
            );
            let nn = accessor(&nor, "VEC3", 34962, json!({}));
            let t = accessor(&uv, "VEC2", 34962, json!({}));
            let c = accessor(&col, "VEC4", 34962, json!({}));
            let i = accessor(&idx, "SCALAR", 34963, json!({}));
            primitives.push(json!({
                "attributes": {"POSITION": p, "NORMAL": nn, "TEXCOORD_0": t, "COLOR_0": c},
                "indices": i,
                "material": material,
            }));
        }
        counts.push(triangles);
        meshes.push(json!({"name": shape.name, "primitives": primitives}));
        nodes.push(json!({"name": shape.name, "mesh": meshes.len() - 1, "translation": shape.at.to_array()}));
    }
    // Each material's four textures, from the game's materials folder.
    let mut images = Vec::new();
    let mut textures = Vec::new();
    let mut texture = |uri: String| {
        images.push(json!({"uri": uri}));
        textures.push(json!({"source": images.len() - 1, "sampler": 0}));
        textures.len() - 1
    };
    let materials: Vec<Value> = materials
        .iter()
        .map(|folder| {
            // A Tripo model's own textures, written beside the spike's models.
            if folder.starts_with("tex/") {
                let file = |name: &str| {
                    let png = format!("{}/models/spike/{folder}/{name}.png", crate::ASSETS);
                    let ext = if std::path::Path::new(&png).exists() {
                        "png"
                    } else {
                        "jpg"
                    };
                    format!("{folder}/{name}.{ext}")
                };
                let colour = texture(file("color"));
                let normal = texture(file("normal"));
                let mr = texture(file("mr"));
                return json!({
                    "name": folder,
                    "pbrMetallicRoughness": {
                        "baseColorTexture": {"index": colour},
                        "metallicRoughnessTexture": {"index": mr},
                    },
                    "normalTexture": {"index": normal},
                });
            }
            let base = format!("../../materials/{folder}");
            let colour = texture(format!("{base}/color.jpg"));
            let normal = texture(format!("{base}/normal.jpg"));
            let arm = texture(format!("{base}/arm.jpg"));
            json!({
                "name": folder,
                "pbrMetallicRoughness": {
                    "baseColorTexture": {"index": colour},
                    "metallicRoughnessTexture": {"index": arm},
                    "metallicFactor": 1.0,
                    "roughnessFactor": 1.0,
                },
                "normalTexture": {"index": normal},
                "occlusionTexture": {"index": arm},
            })
        })
        .collect();
    let document = json!({
        "asset": {"version": "2.0", "generator": "spike/timber-walls"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": nodes,
        "meshes": meshes,
        "materials": materials,
        "textures": textures,
        "images": images,
        "samplers": [{"magFilter": 9729, "minFilter": 9987, "wrapS": 10497, "wrapT": 10497}],
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}],
    });
    let mut text = serde_json::to_vec(&document).unwrap();
    while text.len() % 4 != 0 {
        text.push(b' ');
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let total = 12 + 8 + text.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&text);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    std::fs::write(path, out).unwrap();
    counts
}
