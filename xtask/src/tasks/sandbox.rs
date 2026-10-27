//! The sandbox, the test environment (crates/terra_sandbox): run, as the workbench, or driven.

use crate::{Task, node, run};

pub(crate) const TASKS: &[Task] = &[
    Task {
        name: "sandbox",
        takes: "[args]",
        does: "the sandbox, the test environment; --list for its experiments (crates/terra_sandbox)",
        group: "Look at things",
        run: sandbox,
    },
    Task {
        name: "workbench",
        takes: "[args]",
        does: "models in the game's renderer: --model, --prop Forge|all, --hold, --sheet",
        group: "Look at things",
        run: |args| cargo_run("terra_sandbox", &["--experiment", "workbench"], args),
    },
    Task {
        name: "remote",
        takes: "<command> [args]",
        does: "drive a running sandbox: start, warp, give, look, shot… (tools/sandbox)",
        group: "Look at things",
        run: |args| node("tools/sandbox/remote.ts", args),
    },
];

/// The sandbox with what it was given; with nothing, its experiments, since the planet alone has
/// no sky, player or game and is only what the experiments build on.
fn sandbox(args: &[String]) -> bool {
    if !args.is_empty() {
        return cargo_run("terra_sandbox", &[], args);
    }
    if !cargo_run("terra_sandbox", &["--list"], &[]) {
        return false;
    }
    println!();
    println!("Start one with: cargo xtask sandbox --experiment <name>");
    println!("Play the game on the GPU with: cargo xtask play");
    true
}

/// A package's program, with some arguments first.
pub(crate) fn cargo_run(package: &str, first: &[&str], args: &[String]) -> bool {
    let mut all = vec!["run", "-p", package, "--"];
    all.extend(first);
    all.extend(args.iter().map(String::as_str));
    run("cargo", &all)
}
