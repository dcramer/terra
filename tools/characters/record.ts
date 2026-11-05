// A character check's result, recorded for `cargo xtask characters` (status.ts) to read instead of
// running it again: what it said, a line a stage, with the commit it ran at and the files that
// were uncommitted then, so the status can say how stale it is. Kept per machine, not per
// worktree, so a fresh job's worktree sees the last results: a line a run in
// ~/.cache/terra-characters/<check>.jsonl (TERRA_CHARACTER_RECORDS to put them elsewhere), the
// last 20 runs kept.

import { execFileSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const KEEP = 20;

export type StageResult = { name: string; ok: boolean; said: string };
export type Record = {
  check: string;
  time: string;
  commit: string;
  /** Paths uncommitted in the worktree it ran in, at most 50. */
  dirty: string[];
  worktree: string;
  pass: boolean;
  stages: StageResult[];
  /** What failed, a line each, at most 20. */
  failures: string[];
  log?: string;
  /** How it was run, when that differs from the check's command (`--step wardrobe:male`). */
  args?: string;
};

export function recordsDir(): string {
  return process.env.TERRA_CHARACTER_RECORDS ?? join(process.env.XDG_CACHE_HOME ?? join(homedir(), ".cache"), "terra-characters");
}

function git(args: string[], cwd: string): string {
  try {
    return execFileSync("git", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
  } catch {
    return "";
  }
}

/** Records a run of `check`; never fails the check it is called from. */
export function record(check: string, r: Pick<Record, "pass" | "stages" | "failures" | "log" | "args">, root = ROOT): void {
  try {
    const dirty = git(["status", "--porcelain", "--untracked-files=no"], root)
      .split("\n")
      .filter(Boolean)
      .map((l) => l.slice(3))
      .slice(0, 50);
    const line: Record = { check, time: new Date().toISOString(), commit: git(["rev-parse", "HEAD"], root), dirty, worktree: root, ...r, failures: r.failures.slice(0, 20) };
    const dir = recordsDir();
    mkdirSync(dir, { recursive: true });
    const file = join(dir, `${check}.jsonl`);
    appendFileSync(file, JSON.stringify(line) + "\n");
    const lines = readFileSync(file, "utf8").split("\n").filter(Boolean);
    if (lines.length > KEEP) writeFileSync(file, lines.slice(-KEEP).join("\n") + "\n");
  } catch {
    // A record lost only makes the status say "not recorded"; the check's own verdict stands.
  }
}

/** Every recorded run of `check`, newest first. */
export function records(check: string): Record[] {
  const file = join(recordsDir(), `${check}.jsonl`);
  if (!existsSync(file)) return [];
  const out: Record[] = [];
  for (const l of readFileSync(file, "utf8").split("\n")) {
    if (!l) continue;
    try {
      out.push(JSON.parse(l));
    } catch {
      // A line cut short by a crash is skipped.
    }
  }
  return out.reverse();
}
