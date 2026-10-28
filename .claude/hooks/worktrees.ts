// SessionStart and SubagentStart hook, the start of every session's routine (D183, D259):
// - in a worktree with no build yet, seed it: bring the shared seed build up to date with
//   origin/main and link its compiled dependencies in, so the first build compiles only our own
//   crates (`cargo xtask seed`, which also fast-forwards the main checkout when it is clean);
// - at most once every 15 minutes, in the background from the main checkout: the same seed
//   routine (log in the main checkout's target/seed.log), so the main checkout and the shared
//   seed keep up with origin/main, and clearing away worktrees agents are done with
//   (`cargo xtask worktrees --prune`, log in target/worktrees.log).
// TERRA_NO_LOCAL_BUILDS=1 (a machine whose builds all run on the render worker) skips both seed
// steps: nothing builds locally there, so the seed would only spend its CPU. The main checkout is
// still fast-forwarded (`git pull --ff-only` when clean), since new worktrees start from it.
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
const local = process.env.TERRA_NO_LOCAL_BUILDS !== "1";
if (local && worktree && !existsSync(join(cwd, "target"))) {
  spawnSync("cargo", ["xtask", "seed", "--background"], {
    cwd,
    stdio: ["ignore", "inherit", "inherit"],
  });
}

const main = process.env.CLAUDE_PROJECT_DIR ?? "";
/** Whether a checkout has no changes to tracked files, so a fast-forward cannot clash with any. */
function clean(dir: string) {
  const status = spawnSync("git", ["status", "--porcelain", "--untracked-files=no"], { cwd: dir });
  return status.status === 0 && status.stdout.toString() === "";
}
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
if (local) now(["seed"], "seed.log");
else if (main !== "" && clean(main)) {
  spawn("git", ["pull", "-q", "--ff-only"], { cwd: main, detached: true, stdio: "ignore" })
    .on("error", () => {})
    .unref();
}
now(["worktrees", "--prune"], "worktrees.log");
