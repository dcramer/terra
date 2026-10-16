//! The material bench's data (`cargo xtask look`, tools/kits/look.ts, D386): for each
//! material named, or every one, as its file resolves over its pattern and pack, the texture
//! its bench measures (its path, its metres and its colour map's texels a metre at the high
//! preset), what its pattern holds it to, every texture set it draws and the photograph it
//! is judged beside, as JSON on one line.

use serde_json::json;
use std::path::Path;
use std::process::ExitCode;
use terra_build::kit::Kit;
use zune_core::bytestream::ZCursor;

/// A texture set's colour map's side at the high preset (`Quality::texture_side`, D362): a
/// larger map is drawn at this, a smaller one at its own.
const HIGH_SIDE: f32 = 2048.0;

/// A JPEG's width and height, from its header.
fn size(path: &Path) -> Option<(usize, usize)> {
    let bytes = std::fs::read(path).ok()?;
    let mut decoder = zune_jpeg::JpegDecoder::new(ZCursor::new(&bytes[..]));
    decoder.decode_headers().ok()?;
    decoder.dimensions()
}

fn main() -> ExitCode {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let wanted: Vec<String> = std::env::args().skip(1).collect();
    let kits = match Kit::read_folder(&assets) {
        Ok(kits) => kits,
        Err(why) => {
            eprintln!("look: the materials do not read: {why}");
            return ExitCode::FAILURE;
        }
    };
    let mut out = Vec::new();
    for (name, kit) in &kits {
        if !wanted.is_empty() && !wanted.contains(name) {
            continue;
        }
        let surface = kit.textures.get(&kit.look.surface).map(|texture| {
            let side = size(&assets.join(&texture.path).join("color.jpg"));
            let density = side.map(|(w, h)| {
                [
                    (w as f32).min(HIGH_SIDE) / texture.metres[0],
                    (h as f32).min(HIGH_SIDE) / texture.metres[1],
                ]
            });
            json!({
                "role": kit.look.surface,
                "path": texture.path,
                "metres": texture.metres,
                "pixels": side.map(|(w, h)| [w, h]),
                "density": density,
            })
        });
        out.push(json!({
            "name": name,
            "id": kit.id,
            "pattern": format!("{:?}", kit.pattern),
            "surface": surface,
            "targets": kit.look,
            "roles": kit.textures.keys().collect::<Vec<_>>(),
            "reference": kit.reference,
        }));
    }
    let missing: Vec<&String> = wanted
        .iter()
        .filter(|name| !kits.iter().any(|(each, _)| each == *name))
        .collect();
    if !missing.is_empty() {
        let names: Vec<&String> = kits.iter().map(|(name, _)| name).collect();
        eprintln!("look: no material {missing:?}; the materials are {names:?}");
        return ExitCode::FAILURE;
    }
    println!("{}", serde_json::Value::Array(out));
    ExitCode::SUCCESS
}
