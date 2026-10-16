//! The building tasks: the kit and contract checkers, the rules, the sections' and the
//! drawing's measures, and the texel density check (docs/building.md).

use crate::{root, run};
use std::process::Command;

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
/// (`terra_build::checker`).
pub(crate) fn kits() -> bool {
    run(
        "cargo",
        &["run", "-q", "-p", "terra_build", "--example", "kits"],
    )
}

/// How long the village's buildings take to lay whole and their sections to cut and merge,
/// on one thread, as the game's workers do (docs/building-plan.md, checking it); a name
/// times only the buildings whose names hold it.
pub(crate) fn sections(args: &[String]) -> bool {
    let mut cargo = vec![
        "run",
        "-q",
        "--release",
        "-p",
        "terra_build",
        "--example",
        "sections",
    ];
    cargo.extend(args.iter().map(String::as_str));
    run("cargo", &cargo)
}

/// Where building's textures are stretched (`crates/terra_build/examples/texels.rs`): a name
/// keeps only the bases whose names hold it.
pub(crate) fn texels(args: &[String]) -> bool {
    let mut cargo = vec![
        "run",
        "-q",
        "--release",
        "-p",
        "terra_build",
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
                    "terra_build",
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
/// plan in each kind and every two kinds mixed, at every detail (`terra_build::inspect`); a
/// name keeps only the bases whose names hold it, and `--all` lists every fault found. With
/// `--frames`, what only frames show, from scripted runs on the GPU (`tools/sandbox/artifacts.ts`).
pub(crate) fn artifacts(args: &[String]) -> bool {
    if args.iter().any(|a| a == "--frames") {
        let mut all = vec!["tools/sandbox/artifacts.ts"];
        all.extend(args.iter().filter(|a| *a != "--frames").map(String::as_str));
        return run("node", &all);
    }
    let mut cargo = vec![
        "run",
        "-q",
        "--release",
        "-p",
        "terra_build",
        "--example",
        "artifacts",
        "--",
    ];
    cargo.extend(args.iter().map(String::as_str));
    run("cargo", &cargo)
}
