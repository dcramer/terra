// SessionStart and SubagentStart hook: in a worktree with no build yet, start copying the
// main checkout's compiled dependencies in, so the first build compiles only our own crates
// (D183, `cargo xtask seed`). Claude Code passes the session's folder as `cwd`; for a worktree
// session CLAUDE_PROJECT_DIR is the main checkout, not the worktree.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
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
