// The push hook's checks on the render worker (D37, D445): with TERRA_REMOTE_CHECKS=1 (the
// environment over .env), the pre-push hook runs `cargo xtask check` (formatting, the size and
// one-definition lints, clippy of the crates changed) on the worker at each commit being pushed,
// over what it changed since its merge-base with origin/main, and passes or fails on the result.
// For a machine that must not build (an embargo, a laptop); never a way to skip the checks: a
// worker that cannot be reached fails the push.
//
//   node tools/git/remote-check.ts --enabled   exits 0 when TERRA_REMOTE_CHECKS=1
//   node tools/git/remote-check.ts             the commits on stdin, as git gives a pre-push hook
//                                              (`<ref> <sha> <remote ref> <remote sha>` lines); HEAD without
//
// It prints what the check printed on the worker, a line a stage (D388), and, on a failure, where
// the check's logs came back (`--collect target/check`). Node 22.18+, no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { envFile } from "../render-worker/client.ts";

const CLIENT = resolve(import.meta.dirname, "../render-worker/client.ts");
const ZERO = /^0+$/;

export function enabled(cwd: string, env = process.env): boolean {
  return (env.TERRA_REMOTE_CHECKS || envFile(cwd).TERRA_REMOTE_CHECKS || "") === "1";
}

/// The commits a push sends, from the hook's stdin: each line's local sha, deletions left out.
export function pushed(stdin: string): string[] {
  const shas = stdin
    .split("\n")
    .map((l) => l.trim().split(/\s+/)[1])
    .filter((s): s is string => !!s && /^[0-9a-f]{40}$/.test(s) && !ZERO.test(s));
  return [...new Set(shas)];
}

const git = (cwd: string, args: string[]) => spawnSync("git", args, { cwd, encoding: "utf8" }).stdout?.trim() ?? "";

/// The check at one commit on the worker; true when it passed.
function check(cwd: string, sha: string): boolean {
  const base = git(cwd, ["merge-base", sha, "origin/main"]);
  const out = join(homedir(), ".cache/terra-review/remote", `check-${sha.slice(0, 12)}`);
  rmSync(out, { recursive: true, force: true });
  const command = ["cargo", "xtask", "check", ...(base ? ["--base", base] : []), ...(process.env.TERRA_VERBOSE === "1" ? ["--verbose"] : [])];
  console.log(`remote check of ${sha.slice(0, 9)} on the render worker (TERRA_REMOTE_CHECKS=1)`);
  const args = [CLIENT, "--rev", sha, "--no-cook", "--cpu", "--minutes", "60", "--collect", "target/check", "--out", out, "--", ...command];
  const client = spawnSync(process.execPath, args, { cwd, encoding: "utf8", maxBuffer: 64 * 1024 ** 2 });
  const said = `${client.stdout ?? ""}${client.stderr ?? ""}`;
  const jobLog = join(out, "job.log");
  if (!existsSync(jobLog)) {
    // The job never ran or never came back: the worker is down or unreachable.
    console.log(said.trimEnd().split("\n").slice(-10).join("\n"));
    console.log("✗ the remote check could not run; the push waits for it (cargo xtask remote-render --doctor, or unset TERRA_REMOTE_CHECKS where cargo may run)");
    return false;
  }
  writeFileSync(join(out, "client.log"), said);
  const lines = readFileSync(jobLog, "utf8")
    .split("\n")
    .filter((l) => l && !l.startsWith("render-worker: "));
  console.log(lines.join("\n"));
  if (client.status === 0) return true;
  const logs = join(out, "target/check");
  console.log(`✗ remote check of ${sha.slice(0, 9)} failed${existsSync(logs) ? `; the stages' logs: ${logs}` : ""}; the worker's job: ${join(out, "client.log")}`);
  return false;
}

function main(argv: string[]): number {
  const cwd = process.cwd();
  if (argv.includes("--enabled")) return enabled(cwd) ? 0 : 1;
  const stdin = process.stdin.isTTY ? "" : readFileSync(0, "utf8");
  const shas = stdin.trim() ? pushed(stdin) : [git(cwd, ["rev-parse", "HEAD"])];
  for (const sha of shas) if (!check(cwd, sha)) return 1;
  return 0;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
