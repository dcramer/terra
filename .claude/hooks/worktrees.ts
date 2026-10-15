// SessionStart and SubagentStart hook, for worktrees (D183, D259):
// - in a worktree with no build yet, start linking the main checkout's compiled dependencies
//   in, so the first build compiles only our own crates (`cargo xtask seed`);
// - at most once every 15 minutes, clear away worktrees agents are done with, in the background
//   (`cargo xtask worktrees --prune`, log in the main checkout's target/worktrees.log).
// Claude Code passes the session's folder as `cwd`; for a worktree session CLAUDE_PROJECT_DIR
// is the main checkout, not the worktree.
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, openSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

let cwd = "";
try {
  cwd = JSON.parse(readFileSync(0, "utf8")).cwd ?? "";
} catch {
  // No input: nothing to do.
}
// A linked worktree has a `.git` file pointing at the main checkout; the main checkout has a folder.
const git = join(cwd, ".git");
const worktree = cwd !== "" && existsSync(git) && statSync(git).isFile();
if (worktree && !existsSync(join(cwd, "target"))) {
  spawnSync("cargo", ["xtask", "seed", "--background"], {
    cwd,
    stdio: ["ignore", "inherit", "inherit"],
  });
}

const main = process.env.CLAUDE_PROJECT_DIR ?? "";
const log = join(main, "target", "worktrees.log");
const interval = 15 * 60 * 1000;
if (main !== "" && (!existsSync(log) || Date.now() - statSync(log).mtimeMs > interval)) {
  mkdirSync(join(main, "target"), { recursive: true });
  const out = openSync(log, "w");
  spawn("cargo", ["xtask", "worktrees", "--prune"], {
    cwd: main,
    detached: true,
    stdio: ["ignore", out, out],
  })
    .on("error", () => {})
    .unref();
}
