//! The world's plans and rules run on the CPU, no GPU: a town's plan, its people's day, the
//! land's field maps.

use crate::{Task, run};

pub(crate) const TASKS: &[Task] = &[
    Task {
        name: "town",
        takes: "[--seed N | --seeds A-B] [--ground flat|hillside|rolling|all] [--out shots/town.svg]",
        does: "a market town's plan from its seed, drawn as an SVG: square, streets, buildings, yards, the things its day is done at and its path graph; CPU only (terra_build::town)",
        group: "Look at things",
        run: |args| cargo_example("terra_build", "town", args, false),
    },
    Task {
        name: "life",
        takes: "[--seed N | --seeds A-B] [--ground flat|hillside|rolling|all] [--from HOUR] [--hours N]",
        does: "the life check (D326): a market town's people through a game day on the CPU, failing on anyone stuck, in a wall, inside another, sliding their feet or late for their day; counts by hour and a map in shots/life-*.svg (terra_life)",
        group: "Play and check",
        run: |args| cargo_example("terra_life", "life", args, true),
    },
    Task {
        name: "fields",
        takes: "[--radius 30000] [--at E,N] [--size 8000] [--cell 32] [--date 06-20] [--site lake|tundra|town[@E,N]] [--out shots/fields]",
        does: "the field maps: a region of the world's land on a date as maps of its rules (height, slope, facing, the growing season and the tree line, snow, shelter) and their numbers, on the CPU, no GPU (D326)",
        group: "Look at things",
        run: |args| {
            let mut cargo = vec![
                "run",
                "-q",
                "--release",
                "-p",
                "terra_land",
                "--bin",
                "fields",
                "--",
            ];
            cargo.extend(args.iter().map(String::as_str));
            run("cargo", &cargo)
        },
    },
];

/// A crate's example program, built quietly and run from the root.
fn cargo_example(package: &str, example: &str, args: &[String], release: bool) -> bool {
    let mut all = vec!["run", "-q", "-p", package, "--example", example];
    if release {
        all.insert(2, "--release");
    }
    all.push("--");
    all.extend(args.iter().map(String::as_str));
    run("cargo", &all)
}
