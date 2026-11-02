// A passed push check, reused after a clean rebase (D445): with TERRA_REMOTE_CHECKS=1 the push
// hook's check (tools/git/remote-check.ts) waits on the render worker, main moves meanwhile, the
// push is rejected and the rebased push would check again. As a merge queue reasons about
// batches, a commit series checked once need not be checked again on a newer main when its
// patches are the same (`git patch-id --stable`, commit by commit) and nothing main gained since
// can change what the check said: `cargo xtask check` reads the files a push changed (format,
// sizes, one definition, each a file at a time) and builds the crates they are in with clippy, so
// main's new commits must touch none of the pushed files, no crate the pushed crates depend on
// or that depends on them, no file every check reads (the workspace's Cargo files, the
// toolchain, lint settings, xtask, Bevy's patches) and no file a crate's code includes
// (`include_str!` and its kin). Markdown outside the crates and xtask, which the check never
// reads, is left out of both the patches and the overlap (`checkReads`). When in doubt it checks
// again: a merge in the series, a main that was rewritten, a record it cannot read.
//
// A pass is kept as `<git common dir>/terra-checks/<sha>.json` (`save`), shared by the
// worktrees of one clone, a week. Node 22.18+, no dependencies.

import { spawnSync } from "node:child_process";
import { mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join, posix, resolve } from "node:path";

/// A passed check: the commit, the merge-base with origin/main it was checked over, and its
/// commits' patch-ids from there, oldest first.
export type Passed = { sha: string; base: string; patches: string[]; at: string };

/// What main gained and what the push changes, with the tree's crates and their dependencies
/// (by folder name), and the gained files a crate's code includes.
export type Change = { gained: string[]; pushed: string[]; deps: Map<string, string[]>; included: Set<string> };

/// Files and folders every check reads: a change there can change any check's result
/// (xtask/src/scope.rs's EVERYTHING, the check's own code in xtask/, the patched Bevy crates).
export const EVERY_CHECK = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "rust-toolchain", "clippy.toml", "rustfmt.toml", ".cargo/", "xtask/", "patches/"];

const KEEP_DAYS = 7;
const everyCheck = (f: string) => EVERY_CHECK.some((p) => f === p || (p.endsWith("/") && f.startsWith(p)));

/// The crate a file is in, by its folder under crates/, or undefined.
export function crateOf(file: string, crates: Map<string, string[]>): string | undefined {
  const m = /^crates\/([^/]+)\//.exec(file);
  return m && crates.has(m[1]) ? m[1] : undefined;
}

/// The workspace crates a Cargo.toml depends on (normal, dev, build, any target), of `names`.
export function manifestDeps(text: string, names: Set<string>): string[] {
  const out = new Set<string>();
  let inDeps = false;
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    const header = /^\[([^\]]+)\]$/.exec(line);
    if (header) {
      const parts = header[1].split(".");
      const at = parts.findIndex((p) => /dependencies$/.test(p));
      inDeps = at >= 0 && at === parts.length - 1;
      // `[dependencies.terra_gen]`
      if (at >= 0 && at === parts.length - 2 && names.has(parts.at(-1)!)) out.add(parts.at(-1)!);
      continue;
    }
    if (!inDeps) continue;
    const m = /^([A-Za-z0-9_-]+)\s*(\.|=)/.exec(line);
    if (m && names.has(m[1])) out.add(m[1]);
  }
  return [...out];
}

/// Whether crate `a` reaches crate `b` through dependencies.
function reaches(deps: Map<string, string[]>, a: string, b: string): boolean {
  const seen = new Set<string>();
  const stack = [a];
  while (stack.length) {
    const c = stack.pop()!;
    if (c === b) return true;
    if (seen.has(c)) continue;
    seen.add(c);
    stack.push(...(deps.get(c) ?? []));
  }
  return false;
}

/// Whether what main gained leaves the pushed commits' check as it was: undefined when it does,
/// else why not, naming the first file that could change it.
export function changesCheck(c: Change): string | undefined {
  const pushed = new Set(c.pushed);
  const pushedCrates = new Set(c.pushed.map((f) => crateOf(f, c.deps)).filter((n) => n !== undefined));
  const pushedAll = c.pushed.find(everyCheck);
  for (const g of c.gained) {
    if (pushed.has(g)) return `main changed ${g} too`;
    if (everyCheck(g)) return `main changed ${g}, which every check reads`;
    if (c.included.has(g)) return `main changed ${g}, which a crate's code includes`;
    const crate = crateOf(g, c.deps);
    if (crate && pushedAll) return `the push changes ${pushedAll}, so its check lints every crate, and main changed ${g}`;
    if (!crate) continue;
    for (const p of pushedCrates) {
      if (crate === p) return `main changed ${g}, in ${p}, which the push changes`;
      if (reaches(c.deps, p, crate)) return `main changed ${g}, in ${crate}, which ${p} depends on`;
      if (reaches(c.deps, crate, p)) return `main changed ${g}, in ${crate}, which depends on ${p}`;
    }
  }
  return undefined;
}

// ---- Git ----

function git(cwd: string, args: string[], input?: string): { ok: boolean; out: string } {
  const r = spawnSync("git", args, { cwd, encoding: "utf8", input, maxBuffer: 256 * 1024 ** 2 });
  return { ok: r.status === 0, out: (r.stdout ?? "").trim() };
}
const lines = (s: string) => s.split("\n").filter(Boolean);

/// Whether the check reads a file at all: every file but Markdown outside the crates and xtask
/// (docs, research, skills, READMEs), which no stage reads and no crate includes. Leaving those
/// out matters: nearly every push appends to docs/decisions-log.md, whose patch then changes its
/// context on every rebase, and would overlap every other push's.
export const checkReads = (f: string) => !f.endsWith(".md") || f.startsWith("crates/") || f.startsWith("xtask/");

/// The patch-ids of the commits base..sha, oldest first, over the files the check reads
/// (`checkReads`; a commit of none is "none"); undefined for a merge among them, where it cannot tell.
export function patchIds(cwd: string, base: string, sha: string): string[] | undefined {
  const revs = git(cwd, ["rev-list", "--reverse", "--parents", `${base}..${sha}`]);
  if (!revs.ok) return undefined;
  const commits = lines(revs.out).map((l) => l.split(" "));
  if (!commits.length || commits.some((c) => c.length !== 2)) return undefined;
  const ids: string[] = [];
  for (const [c] of commits) {
    const files = git(cwd, ["diff-tree", "--no-commit-id", "--name-only", "--no-renames", "-r", c]);
    if (!files.ok) return undefined;
    const read = lines(files.out).filter(checkReads);
    if (!read.length) {
      ids.push("none");
      continue;
    }
    const diff = git(cwd, ["diff-tree", "-p", "--no-renames", "--binary", c, "--", ...read.map((f) => `:(literal)${f}`)]);
    const id = git(cwd, ["patch-id", "--stable"], `${diff.out}\n`);
    const word = id.out.split(" ")[0];
    if (!diff.ok || !id.ok || !/^[0-9a-f]{40,64}$/.test(word)) return undefined;
    ids.push(word);
  }
  return ids;
}

/// Where passes are kept: the clone's common git folder, shared by its worktrees.
export function store(cwd: string): string {
  return join(resolve(cwd, git(cwd, ["rev-parse", "--git-common-dir"]).out), "terra-checks");
}

/// Keeps a passed check, and drops those older than a week.
export function save(cwd: string, sha: string, base: string, now = new Date()): void {
  const patches = patchIds(cwd, base, sha);
  if (!patches) return;
  const dir = store(cwd);
  mkdirSync(dir, { recursive: true });
  const passed: Passed = { sha, base, patches, at: now.toISOString() };
  writeFileSync(join(dir, `${sha}.json`), `${JSON.stringify(passed)}\n`);
  for (const f of readdirSync(dir)) {
    const p = join(dir, f);
    if (now.getTime() - statSync(p).mtimeMs > KEEP_DAYS * 86400_000) rmSync(p, { force: true });
  }
}

function records(cwd: string): Passed[] {
  const dir = store(cwd);
  let names: string[] = [];
  try {
    names = readdirSync(dir).filter((f) => f.endsWith(".json"));
  } catch {
    return [];
  }
  const out: Passed[] = [];
  for (const f of names) {
    try {
      const p = JSON.parse(readFileSync(join(dir, f), "utf8")) as Passed;
      if (typeof p.sha === "string" && typeof p.base === "string" && Array.isArray(p.patches)) out.push(p);
    } catch {}
  }
  return out.sort((a, b) => b.at.localeCompare(a.at));
}

/// The tree's crates (folders under crates/ with a Cargo.toml) and their workspace dependencies.
function crateGraph(cwd: string, sha: string): Map<string, string[]> | undefined {
  const ls = git(cwd, ["ls-tree", "--name-only", `${sha}:crates`]);
  if (!ls.ok) return undefined;
  const names = new Set<string>();
  const manifests = new Map<string, string>();
  for (const n of lines(ls.out)) {
    const m = git(cwd, ["show", `${sha}:crates/${n}/Cargo.toml`]);
    if (m.ok) {
      names.add(n);
      manifests.set(n, m.out);
    }
  }
  return new Map([...manifests].map(([n, text]) => [n, manifestDeps(text, names)]));
}

/// Of `files`, those a crate's code may include: any whose name appears in a Rust file that uses
/// `include_str!`, `include_bytes!` or `include!` (a name alone, so a false match only checks again).
function includedOf(cwd: string, sha: string, files: string[]): Set<string> | undefined {
  const outside = files.filter((f) => !f.startsWith("crates/") && !f.endsWith(".md"));
  if (!outside.length) return new Set();
  const users = git(cwd, ["grep", "-l", "-E", "include_(str|bytes)!|include!\\(", sha, "--", "crates", "xtask"]);
  const paths = lines(users.out).map((l) => l.replace(`${sha}:`, ""));
  if (!paths.length) return users.ok || users.out === "" ? new Set() : undefined;
  const found = new Set<string>();
  for (const f of outside) {
    const hit = git(cwd, ["grep", "-l", "-F", posix.basename(f), sha, "--", ...paths]);
    if (hit.out) found.add(f);
  }
  return found;
}

export type Reuse = { passed: Passed; gained: number } | { why: string };

/// A passed check `sha` may stand on, over its merge-base `base` with origin/main: one of the
/// same patches, over a base main has since moved on from by commits that cannot change it.
export function reusable(cwd: string, sha: string, base: string): Reuse {
  const patches = patchIds(cwd, base, sha);
  if (!patches) return { why: "a merge among the pushed commits, or git could not give their patches" };
  const same = records(cwd).filter((p) => p.patches.length === patches.length && p.patches.every((x, i) => x === patches[i]));
  if (!same.length) return { why: "no earlier pass of these patches" };
  let why = "";
  for (const passed of same) {
    if (passed.base === base) return { passed, gained: 0 };
    if (!git(cwd, ["merge-base", "--is-ancestor", passed.base, base]).ok) {
      why ||= `main at ${base.slice(0, 9)} does not hold ${passed.base.slice(0, 9)}, which the pass was checked over`;
      continue;
    }
    const gained = git(cwd, ["diff", "--name-only", "--no-renames", passed.base, base]);
    const pushed = git(cwd, ["diff", "--name-only", "--no-renames", base, sha]);
    const deps = crateGraph(cwd, sha);
    const included = gained.ok ? includedOf(cwd, sha, lines(gained.out)) : undefined;
    if (!gained.ok || !pushed.ok || !deps || !included) {
      why ||= "git could not say what main gained";
      continue;
    }
    const changed = changesCheck({ gained: lines(gained.out).filter(checkReads), pushed: lines(pushed.out).filter(checkReads), deps, included });
    if (!changed) return { passed, gained: Number(git(cwd, ["rev-list", "--count", `${passed.base}..${base}`]).out) || 0 };
    why ||= changed;
  }
  return { why };
}
