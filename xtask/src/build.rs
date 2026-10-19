//! The building tasks: the kit and contract checkers, the rules, the sections' and the
//! drawing's measures, and the texel density check (docs/building.md).

use crate::{Task, node, root, run};
use std::process::Command;

/// The tasks this module runs (D410), and the material bench beside them (tools/kits).
pub(crate) const TASKS: &[Task] = &[
    Task {
        name: "kits",
        takes: "",
        does: "check every building kit file: the materials it needs, sizes and the style guide's budgets",
        group: "Play and check",
        run: |_| kits(),
    },
    Task {
        name: "contract",
        takes: "",
        does: "check every model the game places against the model contract: size, anchor, axes, sockets and, for doors and shutters, fit in their hole",
        group: "Play and check",
        run: |_| contract(),
    },
    Task {
        name: "rules",
        takes: "[--pure] [--here] [--debug]",
        does: "audit building: its rules by test and live, the village's drawing, edits' and frames' timings (research/building-audit.md)",
        group: "Play and check",
        run: rules,
    },
    Task {
        name: "sections",
        takes: "[building]",
        does: "time laying and merging the village's sections, as the game's workers do, against the 2 ms bar, and the block kinds' near triangles and MiB against their budget (D284)",
        group: "Play and check",
        run: sections,
    },
    Task {
        name: "artifacts",
        takes: "[building] [--all] [--quick] [--ratchet|--raise WHY|--lower] | --frames [--quality high,low] [--checks still,sun,glide,going-up,undamaged,damaged]",
        does: "find what would draw as junk in building's meshes: z-fighting, sky through walls, gaps under roofs, pieces poking out; --frames, what only frames show, on the GPU: shimmer, acne, flashes during an edit, TAA trails, light probes popping",
        group: "Play and check",
        run: artifacts,
    },
    Task {
        name: "fuzz",
        takes: "[--seeds N] [--from S] [--ratchet|--raise WHY|--lower]",
        does: "build small bases at random from seeds, as a player builds, and find junk in their meshes as artifacts does; each base with a fault shrunk to its fewest elements and printed as a test to paste. The full check holds seeds 0 to 200 to a baseline (--ratchet)",
        group: "Play and check",
        run: fuzz,
    },
    Task {
        name: "texels",
        takes: "[building] [--worst N]",
        does: "find where building's textures are stretched or smeared, from the meshes: each kit's share of area drawn past 1.5:1 and 2.5:1, and where the worst is",
        group: "Play and check",
        run: texels,
    },
    Task {
        name: "material",
        takes: "<material>… | --all",
        does: "the material bench (D386): a material shot from fixed vantages at noon, in shade and in the evening beside its reference photo and an 18% grey card, its texel density, sharpness, grey-card balance, value range and stretch against its pattern's targets; fails on a regression; shots/material/ (tools/kits)",
        group: "Play and check",
        run: |args| node("tools/kits/material.ts", args),
    },
];

/// The model contract's checker (docs/art-style.md, D303): every model the game places,
/// each on a line with what is wrong with it. The same test runs in `check`.
pub(crate) fn contract() -> bool {
    run(
        "cargo",
        &[
            "test",
            "-q",
            "-p",
            "terra_game",
            "--lib",
            "every_model_keeps_the_contract",
            "--",
            "--nocapture",
        ],
    )
}

/// The kit checker (docs/building-plan.md): every kit file in `assets/data/kits/` has the
/// materials its pattern needs, life-size numbers, and keeps to the style guide's budgets
/// (`terra_build_mesh::checker`).
pub(crate) fn kits() -> bool {
    run("cargo", KITS)
}

/// The kit checker's command, after `cargo`.
pub(crate) const KITS: &[&str] = &["run", "-q", "-p", "terra_build_mesh", "--example", "kits"];

/// How long the village's buildings take to lay whole and their sections to cut and merge,
/// on one thread, as the game's workers do (docs/building-plan.md, checking it); a name
/// times only the buildings whose names hold it.
pub(crate) fn sections(args: &[String]) -> bool {
    let mut cargo = vec![
        "run",
        "-q",
        "--release",
        "-p",
        "terra_build_mesh",
        "--example",
        "sections",
    ];
    cargo.extend(args.iter().map(String::as_str));
    run("cargo", &cargo)
}

/// Where building's textures are stretched (`crates/terra_build_scenes/examples/texels.rs`): a name
/// keeps only the bases whose names hold it.
pub(crate) fn texels(args: &[String]) -> bool {
    let mut cargo = vec![
        "run",
        "-q",
        "--release",
        "-p",
        "terra_build_scenes",
        "--example",
        "texels",
        "--",
    ];
    cargo.extend(args.iter().map(String::as_str));
    run("cargo", &cargo)
}

/// The building audit (research/building-audit.md): terra_build's rules tests with the
/// known failures too, the random operations over many seeds, the drawing audit over the
/// village, and the rules checked live (tools/sandbox/rules.ts) unless `--pure`. Every part
/// runs whatever an earlier one says, and each says pass or fail.
pub(crate) fn rules(args: &[String]) -> bool {
    let pure = args.iter().any(|arg| arg == "--pure");
    let mut results = vec![
        (
            "rules tests, known failures included",
            run(
                "cargo",
                &[
                    "test",
                    "-q",
                    "-p",
                    "terra_build",
                    "--test",
                    "rules",
                    "--",
                    "--include-ignored",
                ],
            ),
        ),
        ("random operations over 64 seeds", {
            println!("\n> TERRA_SEEDS=64 cargo test -p terra_build random_operations");
            Command::new("cargo")
                .args([
                    "test",
                    "-q",
                    "--release",
                    "-p",
                    "terra_build",
                    "random_operations",
                ])
                .env("TERRA_SEEDS", "64")
                .current_dir(root())
                .status()
                .is_ok_and(|status| status.success())
        }),
        (
            "drawing audit",
            run(
                "cargo",
                &[
                    "run",
                    "-q",
                    "--release",
                    "-p",
                    "terra_build_scenes",
                    "--example",
                    "drawing",
                ],
            ),
        ),
    ];
    if !pure {
        let mut all = vec!["tools/sandbox/rules.ts"];
        all.extend(args.iter().map(String::as_str));
        results.push(("live rules (tools/sandbox/rules.ts)", run("node", &all)));
    }
    println!();
    for (what, passed) in &results {
        println!("{}  {what}", if *passed { "ok  " } else { "FAIL" });
    }
    results.iter().all(|(_, passed)| *passed)
}

/// What a renderer would show as junk in the meshes building draws, every roof over every
/// plan in each kind and every two kinds mixed, at every detail (`terra_build_mesh::inspect`); a
/// name keeps only the bases whose names hold it, and `--all` lists every fault found. With
/// `--frames`, what only frames show, from scripted runs on the GPU (`tools/sandbox/artifacts.ts`).
pub(crate) fn artifacts(args: &[String]) -> bool {
    let (program, all) = artifacts_command(args);
    run(program, &all.iter().map(String::as_str).collect::<Vec<_>>())
}

/// The program `artifacts` runs and its arguments.
pub(crate) fn artifacts_command(args: &[String]) -> (&'static str, Vec<String>) {
    if args.iter().any(|a| a == "--frames") {
        let mut all = vec!["tools/sandbox/artifacts.ts".to_string()];
        all.extend(args.iter().filter(|a| *a != "--frames").cloned());
        return ("node", all);
    }
    let mut cargo: Vec<String> = [
        "run",
        "-q",
        "--release",
        "-p",
        "terra_build_scenes",
        "--example",
        "artifacts",
        "--",
    ]
    .map(String::from)
    .to_vec();
    cargo.extend(args.iter().cloned());
    ("cargo", cargo)
}

/// The fuzz pass (`terra_build_scenes::fuzz`): small bases built at random from seeds, as
/// a player builds, each through the mesh checker; a base with a fault shrunk to its fewest
/// elements and printed as a test. `--seeds N` and `--from S` say which; `--ratchet` holds
/// the check's seeds to the baseline, `--raise "<why>"` writes what rose (D414).
pub(crate) fn fuzz(args: &[String]) -> bool {
    artifacts(&fuzz_args(args))
}

/// What `fuzz` asks of `artifacts`.
pub(crate) fn fuzz_args(args: &[String]) -> Vec<String> {
    let mut all = vec!["--fuzz".to_string()];
    all.extend(args.iter().cloned());
    all
}
