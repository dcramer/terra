//! What the model files under `assets/` carry, read without Bevy, for the tests that check
//! the data naming their joints, sockets and morphs names things that are there.

use serde_json::Value;
use std::path::PathBuf;

/// Where a file under `assets/` is.
pub fn asset(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(path)
}

/// The glTF JSON of a binary glTF file under `assets/`: its first chunk.
pub fn json(path: &str) -> Value {
    let bytes = std::fs::read(asset(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!(&bytes[0..4], b"glTF", "{path} is not a binary glTF file");
    let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    assert_eq!(
        &bytes[16..20],
        b"JSON",
        "{path}: its first chunk is not JSON"
    );
    serde_json::from_slice(&bytes[20..20 + length]).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn names(json: &Value, indices: impl Iterator<Item = usize>) -> Vec<String> {
    indices
        .filter_map(|i| json["nodes"][i]["name"].as_str().map(str::to_string))
        .collect()
}

/// Every node's name.
pub fn nodes(json: &Value) -> Vec<String> {
    let count = json["nodes"].as_array().map_or(0, Vec::len);
    names(json, 0..count)
}

/// The names of the joints its skins bind to.
pub fn joints(json: &Value) -> Vec<String> {
    let skins = json["skins"].as_array().cloned().unwrap_or_default();
    let indices = skins
        .iter()
        .flat_map(|skin| skin["joints"].as_array().cloned().unwrap_or_default())
        .filter_map(|i| i.as_u64().map(|i| i as usize));
    names(json, indices)
}

/// The names of its meshes' morph targets.
pub fn morphs(json: &Value) -> Vec<String> {
    json["meshes"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|mesh| {
            mesh["extras"]["targetNames"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|name| name.as_str().map(str::to_string))
        .collect()
}

/// The names of its animations: the clips it carries.
pub fn animations(json: &Value) -> Vec<String> {
    json["animations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|clip| clip["name"].as_str().map(str::to_string))
        .collect()
}

/// Each node's glTF extras, by its name.
pub fn extras(json: &Value) -> Vec<(String, Value)> {
    json["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|node| node["extras"].is_object())
        .map(|node| {
            (
                node["name"].as_str().unwrap_or_default().to_string(),
                node["extras"].clone(),
            )
        })
        .collect()
}

/// The value under `key` in the first node's extras that has one.
pub fn extra(json: &Value, key: &str) -> Option<Value> {
    extras(json)
        .into_iter()
        .find_map(|(_, extras)| extras.get(key).cloned())
}

/// The `.glb` files in a folder under `assets/`, as paths under `assets/`.
pub fn glbs(folder: &str) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(asset(folder))
        .unwrap_or_else(|e| panic!("{folder}: {e}"))
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            name.ends_with(".glb").then(|| format!("{folder}/{name}"))
        })
        .collect();
    found.sort();
    found
}
