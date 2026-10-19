//! A model (`.glb`) cooked (D188): each of its pictures cooked by what its materials use it
//! for and put back in the file as a KTX2 (`image/ktx2`, which Bevy's glTF loader reads by
//! its MIME type), every other byte as it was. The binary chunk is laid out again because
//! the pictures change size; accessors point into buffer views, so only the views move.

use serde_json::Value;
use terra_core::cooked::Kind;

/// A glTF binary's JSON and its one buffer.
pub struct Glb {
    pub json: Value,
    pub bin: Vec<u8>,
}

const MAGIC: u32 = 0x4654_6C67; // "glTF"
const JSON: u32 = 0x4E4F_534A;
const BIN: u32 = 0x004E_4942;

fn word(bytes: &[u8], at: usize) -> Result<u32, String> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| "cut short".to_string())
}

pub fn read(bytes: &[u8]) -> Result<Glb, String> {
    if word(bytes, 0)? != MAGIC || word(bytes, 4)? != 2 {
        return Err("not a glTF 2 binary".into());
    }
    let (mut at, mut json, mut bin) = (12usize, None, Vec::new());
    while at + 8 <= bytes.len() {
        let (len, kind) = (word(bytes, at)? as usize, word(bytes, at + 4)?);
        let chunk = bytes
            .get(at + 8..at + 8 + len)
            .ok_or("a chunk runs past the file")?;
        match kind {
            JSON => json = Some(serde_json::from_slice(chunk).map_err(|p| p.to_string())?),
            BIN => bin = chunk.to_vec(),
            _ => {}
        }
        at += 8 + len;
    }
    Ok(Glb {
        json: json.ok_or("no JSON chunk")?,
        bin,
    })
}

pub fn write(glb: &Glb) -> Vec<u8> {
    let mut json = serde_json::to_vec(&glb.json).expect("JSON writes");
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut bin = glb.bin.clone();
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let total = 12 + 8 + json.len() + if bin.is_empty() { 0 } else { 8 + bin.len() };
    let mut out = Vec::with_capacity(total);
    for w in [MAGIC, 2, total as u32, json.len() as u32, JSON] {
        out.extend_from_slice(&w.to_le_bytes());
    }
    out.extend_from_slice(&json);
    if !bin.is_empty() {
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&BIN.to_le_bytes());
        out.extend_from_slice(&bin);
    }
    out
}

/// What each image is used as, by its materials: colour where any material takes it for
/// its base colour or glow, a normal where it is only ever a normal map, data otherwise;
/// and whether it is only ever occlusion, whose red alone is read.
pub fn uses(json: &Value) -> Vec<(Kind, bool)> {
    let images = json["images"].as_array().map_or(0, Vec::len);
    // Each image's uses: base colour or glow, normal, metal-roughness, occlusion.
    let mut used = vec![[false; 4]; images];
    let source = |texture: &Value| -> Option<usize> {
        let t = texture["index"].as_u64()? as usize;
        json["textures"][t]["source"].as_u64().map(|s| s as usize)
    };
    for material in json["materials"].as_array().into_iter().flatten() {
        let pbr = &material["pbrMetallicRoughness"];
        for (texture, slot) in [
            (&pbr["baseColorTexture"], 0),
            (&material["emissiveTexture"], 0),
            (&material["normalTexture"], 1),
            (&pbr["metallicRoughnessTexture"], 2),
            (&material["occlusionTexture"], 3),
        ] {
            if let Some(i) = source(texture).filter(|&i| i < images) {
                used[i][slot] = true;
            }
        }
    }
    (0..images)
        .map(|i| match used[i] {
            [true, ..] => (Kind::Colour, false),
            [false, true, false, false] => (Kind::Normal, false),
            [false, false, false, true] => (Kind::Data, true),
            _ => (Kind::Data, false),
        })
        .collect()
}

/// The bytes of image `i`, if it is a picture in the buffer.
pub fn image_bytes(glb: &Glb, i: usize) -> Option<&[u8]> {
    let view = glb.json["images"][i]["bufferView"].as_u64()? as usize;
    let (offset, len) = view_range(&glb.json["bufferViews"][view]);
    glb.bin.get(offset..offset + len)
}

fn view_range(view: &Value) -> (usize, usize) {
    (
        view["byteOffset"].as_u64().unwrap_or(0) as usize,
        view["byteLength"].as_u64().unwrap_or(0) as usize,
    )
}

/// The model with `cooked` (an image's index, its KTX2) in place of those pictures.
pub fn replace(glb: &mut Glb, cooked: Vec<(usize, Vec<u8>)>) {
    let mut by_view = std::collections::HashMap::new();
    for (i, bytes) in cooked {
        if let Some(view) = glb.json["images"][i]["bufferView"].as_u64() {
            by_view.insert(view as usize, bytes);
            glb.json["images"][i]["mimeType"] = Value::from("image/ktx2");
        }
    }
    let mut bin = Vec::with_capacity(glb.bin.len());
    if let Some(views) = glb.json["bufferViews"].as_array_mut() {
        for (v, view) in views.iter_mut().enumerate() {
            let (offset, len) = view_range(view);
            // Sixteen-byte alignment, enough for any accessor's component.
            while bin.len() % 16 != 0 {
                bin.push(0);
            }
            let start = bin.len();
            match by_view.get(&v) {
                Some(bytes) => bin.extend_from_slice(bytes),
                None => bin.extend_from_slice(&glb.bin[offset..offset + len]),
            }
            view["byteOffset"] = Value::from(start);
            view["byteLength"] = Value::from(bin.len() - start);
        }
    }
    glb.json["buffers"][0]["byteLength"] = Value::from(bin.len());
    glb.bin = bin;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_written_reads_back_with_its_pictures_replaced() {
        let json = serde_json::json!({
            "asset": {"version": "2.0"},
            "buffers": [{"byteLength": 12}],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": 4},
                {"buffer": 0, "byteOffset": 4, "byteLength": 8},
            ],
            "images": [{"bufferView": 1, "mimeType": "image/png"}],
            "textures": [{"source": 0}],
            "materials": [{"normalTexture": {"index": 0}}],
        });
        let mut glb = Glb {
            json,
            bin: (0u8..12).collect(),
        };
        assert_eq!(uses(&glb.json), vec![(Kind::Normal, false)]);
        replace(&mut glb, vec![(0, vec![9; 20])]);
        let back = read(&write(&glb)).unwrap();
        assert_eq!(back.json["images"][0]["mimeType"], "image/ktx2");
        assert_eq!(image_bytes(&back, 0).unwrap(), &[9; 20]);
        let (offset, len) = view_range(&back.json["bufferViews"][0]);
        assert_eq!(&back.bin[offset..offset + len], &[0, 1, 2, 3]);
    }
}
