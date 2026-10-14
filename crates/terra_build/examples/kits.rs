//! The kit checker, `cargo xtask kits` (docs/building-plan.md): reads every kit file in
//! `assets/data/kits/` and says, for each, whether it passes `terra_build::checker` and what
//! it measured against the style guide's budgets. Fails when any kit does.

use std::path::Path;
use std::process::ExitCode;
use terra_build::checker::{self, MEMBER_PER_METRE, ROOF_PER_SQUARE_METRE};
use terra_build::kit::Kit;

#[derive(serde::Deserialize)]
struct Items {
    kinds: Vec<Item>,
}

#[derive(serde::Deserialize)]
struct Item {
    name: String,
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
        match Kit::read(&format!("{}/{name}", terra_build::kit::MATERIALS), &read) {
            Ok(kit) => kits.push((name, kit)),
            Err(why) => {
                println!("{name}: does not read: {why}");
                failed = true;
            }
        }
    }
    if kits.is_empty() {
        println!("kits: no kit files");
        return ExitCode::FAILURE;
    }
    let exists = |path: &str| assets.join(path).is_file();
    for (name, report) in checker::check_all(&kits, &exists, &items) {
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
