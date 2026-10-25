//! The dungeon sheet (docs/dungeons.md): every theme over many seeds, a line each with its
//! solve rates, times, connectivity and variety, and a map sheet a theme of the first seeds.
//!
//! `cargo run --release -p terra_dungeon --bin dungeons -- [--seeds 1000] [--maps DIR]
//! [--panels 8] [--theme NAME]`

use std::path::PathBuf;
use terra_dungeon::sheet::{self, Run};
use terra_dungeon::{ModuleSet, Theme, map, theme};

fn main() {
    let mut seeds = 1000u64;
    let mut panels = 8usize;
    let mut maps: Option<PathBuf> = None;
    let mut only: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .unwrap_or_else(|| fail(&format!("{arg} needs a value")))
        };
        match arg.as_str() {
            "--seeds" => {
                seeds = value()
                    .parse()
                    .unwrap_or_else(|_| fail("--seeds takes a number"))
            }
            "--panels" => {
                panels = value()
                    .parse()
                    .unwrap_or_else(|_| fail("--panels takes a number"))
            }
            "--maps" => maps = Some(PathBuf::from(value())),
            "--theme" => only = Some(value()),
            other => fail(&format!("unknown option {other}")),
        }
    }
    let themes = Theme::all(&theme::assets()).unwrap_or_else(|e| fail(&e));
    let mut failed = false;
    for theme in themes
        .iter()
        .filter(|t| only.as_ref().is_none_or(|n| *n == t.name))
    {
        let set = ModuleSet::new(theme).unwrap_or_else(|e| fail(&e));
        let list: Vec<u64> = (0..seeds).collect();
        let runs: Vec<Run> = terra_gen::par::map(&list, |&s| sheet::run(theme, &set, s).0);
        let ok = runs.iter().all(|r| r.solvable)
            && runs
                .iter()
                .filter(|r| r.placed)
                .all(|r| r.connected && r.verified);
        failed |= !ok;
        println!(
            "{} {}",
            if ok { "✓" } else { "✗" },
            sheet::line(theme, &runs)
        );
        if let Some(dir) = &maps {
            std::fs::create_dir_all(dir)
                .unwrap_or_else(|e| fail(&format!("{}: {e}", dir.display())));
            let drawn: Vec<(u64, terra_dungeon::Dungeon)> = (0..seeds)
                .filter_map(|s| sheet::run(theme, &set, s).1.map(|d| (s, d)))
                .take(panels)
                .collect();
            let path = dir.join(format!("{}.png", theme.name));
            map::sheet(theme, &set, &drawn)
                .save(&path)
                .unwrap_or_else(|e| fail(&e));
            println!("  map: {}", path.display());
        }
    }
    if failed {
        std::process::exit(1);
    }
}

fn fail(why: &str) -> ! {
    eprintln!("✗ {why}");
    std::process::exit(2);
}
