//! Running on GPUs: sharing this machine's between agents (D319, the wrapper in tools/gpu) and
//! the render box's (D445, tools/render-worker).

use crate::{Task, root};

pub(crate) const TASKS: &[Task] = &[
    Task {
        name: "gpu",
        takes: "<still|sheet|film|bake|bench> [--stall MIN] [--watch FILE] -- <command…> | --status",
        does: "run a command on the GPU at once, beside other runs; a bench waits for the card alone and holds back new runs meanwhile; stopped after so long without progress; --status lists running and waiting runs (D319, tools/gpu)",
        group: "This machine",
        run: |args| node_exit("tools/gpu/gpu.ts", args),
    },
    Task {
        name: "remote-render",
        takes: "[--rev <sha>] [--collect <path>]… [--minutes N] [--no-cook] [--gpu|--cpu] [--no-wait] [--worker HOST] -- <command…> | --wait <job> | --logs <job|daemon> | --cancel <job> | --status | --update | --doctor",
        does: "run a command (a film, sheet, shot, test or bench) on a render box over ssh at a commit of this checkout, wait for it and rsync what it wrote to ~/.cache/terra-review/remote/<job>/; exits with the command's code; jobs run side by side in TERRA_RENDER_SLOTS slots, GPU ones up to TERRA_RENDER_GPU_SLOTS, a bench alone; `--update` brings the boxes in TERRA_WORKERS (.env) to origin/main, `--doctor` checks them (D445, docs/render-worker.md)",
        group: "This machine",
        run: |args| node_exit("tools/render-worker/client.ts", args),
    },
];

/// Runs a tool in the caller's folder, not the root, since the command and the commit are the
/// caller's, and exits with the command's own code (124 when it stalled), which callers read.
fn node_exit(script: &str, args: &[String]) -> bool {
    let status = std::process::Command::new("node")
        .arg(root().join(script))
        .args(args)
        .status();
    match status.ok().and_then(|status| status.code()) {
        Some(0) => true,
        Some(code) => std::process::exit(code),
        None => false,
    }
}
