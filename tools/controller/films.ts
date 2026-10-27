// The filmed course's films kept by commit (D457, D414): a film made on a clean checkout keeps
// its checks under ~/.cache/terra-review/controller-films/<commit>/, so nightly's scorecard
// (`scorecard.ts --filmed`) banks the film's rows from main's own film, the newest one that
// nothing the film reads (xtask/src/scope.rs `CONTROLLER`) changed since, without filming. A
// film of a dirty checkout or of a commit main never had is never banked.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { scopePaths } from "../continuous/continuous.ts";

const ROOT = resolve(import.meta.dirname, "../..");
export const FILMS = join(homedir(), ".cache/terra-review/controller-films");
/// The most films kept; the oldest go first.
const KEPT = 12;
/// What a film keeps: the checks the scorecard reads and their lines.
const KEPT_FILES = ["checks.json", "checks.txt"];

/// Where a film was made: the commit checked out, and whether the tracked files were as committed.
export type Filmed = { commit: string; clean: boolean; when: string };

function git(args: string[], cwd = ROOT): { ok: boolean; out: string } {
  const r = spawnSync("git", args, { cwd, encoding: "utf8", maxBuffer: 1 << 26 });
  return { ok: r.status === 0, out: (r.stdout ?? "").trim() };
}

/// The checkout's commit and whether it is clean (untracked files aside: what is built is tracked).
export function stamp(cwd = ROOT): Filmed {
  const commit = git(["rev-parse", "HEAD"], cwd).out;
  const dirty = git(["status", "--porcelain", "--untracked-files=no"], cwd).out;
  return { commit, clean: commit.length > 0 && dirty === "", when: new Date().toISOString() };
}

/// Keeps a clean film's checks from `out` under its commit; a dirty one is not kept. What it did.
export function keep(out: string, filmed: Filmed, films = FILMS): string {
  writeFileSync(join(out, "filmed.json"), JSON.stringify(filmed, null, 1) + "\n");
  if (!filmed.clean) return `not kept for nightly: the checkout differs from ${filmed.commit.slice(0, 10)}`;
  const dir = join(films, filmed.commit);
  mkdirSync(dir, { recursive: true });
  for (const file of [...KEPT_FILES, "filmed.json"]) if (existsSync(join(out, file))) copyFileSync(join(out, file), join(dir, file));
  const all = readdirSync(films).map((name) => ({ name, at: statSync(join(films, name)).mtimeMs }));
  for (const old of all.sort((a, b) => b.at - a.at).slice(KEPT)) rmSync(join(films, old.name), { recursive: true, force: true });
  return `kept for nightly as ${dir}`;
}

/// Whether a film's rows still hold after these changed files: none under what the film reads.
export function holds(changed: string[], reads: string[]): boolean {
  return !changed.some((file) => reads.some((prefix) => file.startsWith(prefix)));
}

export type Found = { dir: string; commit: string; behind: number } | { why: string };

/// The film of `head`'s own history nearest to it whose rows still hold at `head`: kept, clean,
/// an ancestor, and nothing the film reads changed since.
export function mainFilm(head = "HEAD", films = FILMS, cwd = ROOT): Found {
  const reads = scopePaths(readFileSync(join(cwd, "xtask/src/scope.rs"), "utf8"), "CONTROLLER");
  if (!reads.length) return { why: "xtask/src/scope.rs has no CONTROLLER list" };
  const names = existsSync(films) ? readdirSync(films) : [];
  let best: { dir: string; commit: string; behind: number } | undefined;
  let nearestStale: { commit: string; behind: number } | undefined;
  for (const commit of names) {
    const dir = join(films, commit);
    if (!existsSync(join(dir, "checks.json"))) continue;
    if (!git(["merge-base", "--is-ancestor", commit, head], cwd).ok) continue;
    const behind = Number(git(["rev-list", "--count", `${commit}..${head}`], cwd).out);
    const changed = git(["diff", "--name-only", "--no-renames", commit, head], cwd).out.split("\n").filter(Boolean);
    if (!holds(changed, reads)) {
      if (!nearestStale || behind < nearestStale.behind) nearestStale = { commit, behind };
      continue;
    }
    if (!best || behind < best.behind) best = { dir, commit, behind };
  }
  if (best) return best;
  const stale = nearestStale ? `; the nearest, ${nearestStale.commit.slice(0, 10)} (${nearestStale.behind} commits back), is before a change to what the film reads` : "";
  return { why: `no clean film of this commit's history in ${films}${stale}: film main with \`cargo xtask gpu film -- cargo xtask controller --film\` on a clean checkout` };
}
