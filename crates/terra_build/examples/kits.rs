//! The kit checker, `cargo xtask kits` (docs/building-plan.md): reads every kit file in
//! `assets/data/kits/` and says, for each, whether it passes `terra_build::checker` and its
//! pack's checks (`terra_build::packs`, D349: its maps' height, tiling and real size, its
//! relief, the roles its pack gives), and what it measured against the style guide's
//! budgets. Fails when any kit does.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::ExitCode;
use terra_build::Material;
use terra_build::checker::{self, MEMBER_PER_METRE, ROOF_PER_SQUARE_METRE};
use terra_build::kit::Kit;
use terra_build::packs::{self, Measured, Picture};
use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;

#[derive(serde::Deserialize)]
struct Items {
    kinds: Vec<Item>,
}

#[derive(serde::Deserialize)]
struct Item {
    name: String,
}

/// A JPEG map, decoded to its red, green and blue; none if it is not there or not a JPEG.
fn picture(path: &Path) -> Option<Picture> {
    let bytes = std::fs::read(path).ok()?;
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(&bytes[..]), options);
    let rgb = decoder.decode().ok()?;
    let (width, height) = decoder.dimensions()?;
    Some(Picture { width, height, rgb })
}

fn main() -> ExitCode {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let read = |path: &Path| {
        std::fs::read_to_string(path).map_err(|why| format!("{}: {why}", path.display()))
    };
    let items: Vec<String> = match read(&assets.join("data/items.ron"))
        .and_then(|text| ron::from_str::<Items>(&text).map_err(|why| why.to_string()))
    {
        Ok(items) => items.kinds.into_iter().map(|item| item.name).collect(),
        Err(why) => {
            println!("kits: the items could not be read: {why}");
            return ExitCode::FAILURE;
        }
    };
    let mut kits = Vec::new();
    // The roles each material's own lines and packs give (D349).
    let mut given: BTreeMap<String, BTreeSet<Material>> = BTreeMap::new();
    let mut failed = false;
    let mut paths: Vec<_> = std::fs::read_dir(assets.join("data/kits"))
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    paths.sort();
    for path in paths
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
    {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        // A material over its pattern and pack, with its dressed pieces (D215, D290).
        let read = |file: &str| read(&assets.join(file));
        let file = format!("{}/{name}", terra_build::kit::MATERIALS);
        match Kit::read(&file, &read) {
            Ok(kit) => kits.push((name.clone(), kit)),
            Err(why) => {
                println!("{name}: does not read: {why}");
                failed = true;
                continue;
            }
        }
        let own = read(&file)
            .and_then(|text| terra_build::layers::own_textures(&text, &read))
            .unwrap_or_default();
        given.insert(
            name,
            own.iter()
                .filter_map(|role| ron::from_str::<Material>(role).ok())
                .collect(),
        );
    }
    if kits.is_empty() {
        println!("kits: no kit files");
        return ExitCode::FAILURE;
    }
    // Each texture set's maps measured once, however many materials wear it.
    let measures: RefCell<BTreeMap<String, Option<Measured>>> = RefCell::default();
    let measured = |folder: &str| -> Option<Measured> {
        if let Some(m) = measures.borrow().get(folder) {
            return m.clone();
        }
        let dir = assets.join(folder);
        let m = picture(&dir.join("color.jpg")).map(|colour| {
            packs::measure(
                &colour,
                picture(&dir.join("arm.jpg")).as_ref(),
                picture(&dir.join("height.jpg")).as_ref(),
            )
        });
        measures.borrow_mut().insert(folder.to_string(), m.clone());
        m
    };
    let exists = |path: &str| assets.join(path).is_file();
    for (name, mut report) in checker::check_all(&kits, &exists, &items) {
        if let Some((_, kit)) = kits.iter().find(|(n, _)| *n == name) {
            let own = given.get(&name).cloned().unwrap_or_default();
            report.problems.extend(packs::check(kit, &own, &measured));
        }
        let m = &report.measured;
        let per_metre = m.per_metre.map_or("no logs".into(), |n| {
            format!("{n}/{MEMBER_PER_METRE} a metre of log")
        });
        let dress = m
            .dress_per_cube
            .map_or(String::new(), |n| format!(", its dress {n} a cube"));
        println!(
            "{name}: {} ({per_metre}, {}/{ROOF_PER_SQUARE_METRE} a square metre of roof, {} on a cabin{dress})",
            if report.problems.is_empty() {
                "ok"
            } else {
                "FAILS"
            },
            m.per_square_metre,
            m.triangles
        );
        for problem in &report.problems {
            println!("  {problem}");
        }
        failed |= !report.problems.is_empty();
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
