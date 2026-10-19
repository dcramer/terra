//! Sharing this machine's GPU between agents (D319): the scheduler in tools/gpu.

use crate::{Task, root};

pub(crate) const TASKS: &[Task] = &[Task {
    name: "gpu",
    takes: "<still|sheet|film|bake|bench> [--stall MIN] [--watch FILE] [--vram MB] -- <command…> | --status",
    does: "run a command on the GPU when its memory fits, beside other runs, short ones first, a bench alone; stopped after so long silent, retried once on a lost device or out of memory; --status lists running and waiting jobs (D319, tools/gpu)",
    group: "This machine",
    run: gpu,
}];

/// Runs the scheduler in the caller's folder, not the root, since the command is the caller's,
/// and exits with the command's own code (124 when it stalled), which callers read.
fn gpu(args: &[String]) -> bool {
    let status = std::process::Command::new("node")
        .arg(root().join("tools/gpu/gpu.ts"))
        .args(args)
        .status();
    match status.ok().and_then(|status| status.code()) {
        Some(0) => true,
        Some(code) => std::process::exit(code),
        None => false,
    }
}
