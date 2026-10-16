//! A market town's plan (`terra_build::town`), drawn as an SVG to look at: the square, the
//! streets, every building's ground, the yards, the things a day is done at, and the path
//! graph its people walk, with a legend and the counts. CPU only, in a moment.
//!
//!   cargo run -p terra_build --example town -- [--seed N] [--ground flat|hillside|rolling]
//!       [--out shots/town.svg]
//!
//! With `--seeds A-B` and `--ground all` it lays every one and prints a line for each, and
//! whatever on any is cut off from its paths, without drawing.

use glam::Vec2;
use terra_build::Kind;
use terra_build::town::{self, ThingKind, Town};
use terra_build::village::Kinds;

const KINDS: Kinds = Kinds {
    timber: Kind(1),
    masonry: Kind(2),
    beams: Kind(3),
    cob: Kind(4),
    framed: Kind(5),
    timber_roof: Kind(0x800),
    masonry_roof: Kind(0x801),
    beams_roof: Kind(0x802),
};

fn main() {
    let mut seeds = (1u32, 1u32);
    let mut ground = "hillside".to_string();
    let mut out = "shots/town.svg".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seed" => {
                let seed = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
                seeds = (seed, seed);
            }
            "--seeds" => {
                let range = args.next().unwrap_or_default();
                let (a, b) = range.split_once('-').unwrap_or((&range, &range));
                seeds = (a.parse().unwrap_or(0), b.parse().unwrap_or(0));
            }
            "--ground" => ground = args.next().unwrap_or(ground),
            "--out" => out = args.next().unwrap_or(out),
            other => {
                eprintln!(
                    "town: what is {other}? --seed N | --seeds A-B, --ground flat|hillside|rolling|all, --out file.svg"
                );
                std::process::exit(2);
            }
        }
    }
    let grounds: Vec<&str> = if ground == "all" {
        vec!["flat", "hillside", "rolling"]
    } else {
        vec![ground.as_str()]
    };
    let one = grounds.len() == 1 && seeds.0 == seeds.1;
    for &ground in &grounds {
        let heights = heights(ground);
        for seed in seeds.0..=seeds.1 {
            let town = town::town(seed, KINDS, &*heights);
            if let Some(folder) = std::path::Path::new(&out).parent() {
                let _ = std::fs::create_dir_all(folder);
            }
            if one && let Err(why) = std::fs::write(&out, town::svg(&town, &*heights, &[])) {
                eprintln!("town: cannot write {out}: {why}");
                std::process::exit(1);
            }
            report(seed, ground, &town, one.then_some(out.as_str()));
        }
    }
}

/// The grounds a town is tried on: level, a hillside rising 6 in 100 to the north-east, and
/// rolling swells on a gentle slope.
fn heights(ground: &str) -> Box<dyn Fn(Vec2) -> f32> {
    match ground {
        "flat" => Box::new(|_| 0.0),
        "rolling" => {
            Box::new(|p: Vec2| 4.0 * libm::sinf(p.x / 45.0) * libm::cosf(p.y / 60.0) + p.y * 0.03)
        }
        _ => Box::new(|p: Vec2| p.dot(Vec2::new(1.0, -1.0).normalize()) * 0.06),
    }
}

fn report(seed: u32, ground: &str, town: &Town, drawn: Option<&str>) {
    let parts = town.paths.parts();
    for thing in &town.things {
        if parts[thing.node as usize] != parts[0] {
            println!(
                "  cut off: the {:?} at {:.1}, {:.1}",
                thing.kind, thing.at.x, thing.at.y
            );
        }
    }
    let count = |kind| town.things.iter().filter(|t| t.kind == kind).count();
    println!(
        "TOWN seed {seed} on {ground}: a {:?} square, {} streets, {} homes, {} buildings in all, {} yards, {} stalls, {} pens, {} hen yards, {} chopping blocks, {} path nodes and {} edges{}",
        town.square.shape,
        town.streets.len(),
        town.homes.len(),
        town.buildings.len(),
        town.yards.len(),
        count(ThingKind::Stall),
        count(ThingKind::Pen),
        count(ThingKind::Yard),
        count(ThingKind::Block),
        town.paths.nodes.len(),
        town.paths.edges.len(),
        drawn.map_or(String::new(), |out| format!("; drawn to {out}")),
    );
    let e = town::enclosure(town);
    println!(
        "  enclosed: {:.1} m front to front across its streets (a tenth under {:.1}, a tenth over {:.1}); {:.0}% of their built-up sides fronted, {} gaps of {:.1} m at the middle; {:.0}% of the square's edge fronted; {} houses of two storeys",
        e.between_fronts[0],
        e.between_fronts[1],
        e.between_fronts[2],
        e.built_up * 100.0,
        e.gaps,
        e.gap,
        e.square_fronted * 100.0,
        town.buildings
            .iter()
            .filter(|b| b.name.contains("two storeys"))
            .count(),
    );
}
