// SessionStart and SubagentStart hook, the start of every session's routine (D183, D259):
// - in a worktree with no build yet, seed it: bring the shared seed build up to date with
//   origin/main and link its compiled dependencies in, so the first build compiles only our own
//   crates (`cargo xtask seed`, which also fast-forwards the main checkout when it is clean);
// - at most once every 15 minutes, in the background from the main checkout: the same seed
//   routine (log in the main checkout's target/seed.log), so the main checkout and the shared
//   seed keep up with origin/main, and clearing away worktrees agents are done with
//   (`cargo xtask worktrees --prune`, log in target/worktrees.log).
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
const interval = 15 * 60 * 1000;
/** Runs a task from the main checkout in the background, unless it ran in the last 15 minutes. */
function now(task: string[], logName: string) {
  const log = join(main, "target", logName);
  if (main === "" || (existsSync(log) && Date.now() - statSync(log).mtimeMs <= interval)) return;
  mkdirSync(join(main, "target"), { recursive: true });
  const out = openSync(log, "w");
  spawn("cargo", ["xtask", ...task], {
    cwd: main,
    detached: true,
    stdio: ["ignore", out, out],
  })
    .on("error", () => {})
    .unref();
}
now(["seed"], "seed.log");
now(["worktrees", "--prune"], "worktrees.log");
