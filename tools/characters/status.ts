#!/usr/bin/env node
// Where character work stands, in 40 lines or fewer, from recorded results (D625): each
// criterion of tools/characters/criteria.json against its bar, how stale each check's last run
// is and the command that refreshes it, the tasks still to do in order with the files that own
// them, and the next thing to run. The first thing a character job runs (the character skill),
// as combat's (D452) and the controller's (D457) scorecards are; it runs no check itself, so it
// takes a second.
//
//   cargo xtask characters            (node tools/characters/status.ts)
//   … --json                          the same as JSON
//
// A check records what it said when it runs (record.ts): `node tools/characters/wardrobe.ts`
// and `tools/characters/build_bodies.sh`. A record is stale when a commit since it touched the
// files its check reads (criteria.json's `owns`), or when it ran on uncommitted changes to them,
// or when they are changed in this worktree; ? marks a criterion so, or never measured.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { type Wardrobe, wardrobeData } from "./build_steps.ts";
import { figures, type Pairing } from "./pairings.ts";
import { type Record, records, recordsDir } from "./record.ts";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));

type Check = { command: string; takes: string; owns: string[] };
type Criterion = { id: string; bar: string; check?: string; stage?: string; live?: string; known?: { match: string; why: string }[]; within?: [number, number] };
type Task = { what: string; files: string[]; check: string; criterion?: string };
export type Criteria = { checks: { [name: string]: Check }; criteria: Criterion[]; tasks: Task[] };

/** How fresh a check's last record is. */
export type Fresh = { record: Record | null; stale: string | null; age: string };
export type Git = (args: string[]) => string;

const gitIn =
  (root: string): Git =>
  (args) => {
    try {
      return execFileSync("git", args, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
    } catch {
      return "\u0000";
    }
  };

const owned = (path: string, owns: string[]) => owns.some((o) => (o.includes("*") ? new RegExp(`^${o.replace(/[.]/g, "\\.").replace(/\*/g, ".*")}$`).test(path) : path === o || path.startsWith(o.endsWith("/") ? o : `${o}/`) || path === o.replace(/\/$/, "")));

function ago(time: string, now: number): string {
  const h = (now - Date.parse(time)) / 3600e3;
  return h < 1 ? `${Math.max(1, Math.round(h * 60))} min ago` : h < 48 ? `${Math.round(h)} h ago` : `${Math.round(h / 24)} days ago`;
}

/** The record a check's status stands on, and why it is stale if it is: the newest run at a
 *  commit main holds (this worktree's HEAD or before), else the newest at all. */
export function freshness(check: Check, runs: Record[], git: Git, here: string[], now = Date.now()): Fresh {
  const landed = runs.find((r) => r.commit && git(["merge-base", "--is-ancestor", r.commit, "HEAD"]) !== "\u0000");
  const r = landed ?? runs[0] ?? null;
  if (!r) return { record: null, stale: "never recorded here", age: "" };
  const age = ago(r.time, now);
  const reasons: string[] = [];
  if (!landed) reasons.push(`ran at ${r.commit.slice(0, 8)}, which this worktree does not hold`);
  else {
    const n = Number(git(["rev-list", "--count", `${r.commit}..HEAD`, "--", ...check.owns]));
    if (n > 0) reasons.push(`${n} commit${n > 1 ? "s" : ""} to its files since`);
  }
  const ranDirty = r.dirty.filter((p) => owned(p, check.owns));
  if (ranDirty.length) reasons.push(`ran on uncommitted ${ranDirty[0]}${ranDirty.length > 1 ? ` and ${ranDirty.length - 1} more` : ""}`);
  const dirtyHere = here.filter((p) => owned(p, check.owns));
  if (dirtyHere.length) reasons.push(`${dirtyHere[0]}${dirtyHere.length > 1 ? ` and ${dirtyHere.length - 1} more` : ""} changed here`);
  return { record: r, stale: reasons.length ? reasons.join("; ") : null, age };
}

export type Row = { id: string; mark: "✓" | "✗" | "?"; said: string; bar: string; check?: string };

/** The bodies a player is offered: those the creator has roles for (`Creator::offered`). */
export const offered = (root = ROOT): string[] => Object.keys((JSON.parse(readFileSync(join(root, "assets/models/character/creator.json"), "utf8")) as { roles: object }).roles);

/** What a live criterion reads from the catalogue: of the bodies players are offered, which
 * the bars hold; a body in the catalogue not yet offered is said beside them, never judged. */
export function live(kind: string, w: Wardrobe, on: string[] = offered()): { ok: boolean; said: string; n?: number } {
  if (kind === "hair") {
    // A look's hair is its piece of the hair slot, as the game composes it: a piece of the
    // catalogue made for that slot (a beard or a name the catalogue lacks is no hair).
    const looks = Object.values(w.looks) as { body: string; pieces: Record<string, string> }[];
    const haired = (ls: typeof looks) => ls.filter((l) => w.pieces[l.pieces.hair]?.slot === "hair").length;
    const [judged, rest] = [looks.filter((l) => on.includes(l.body)), looks.filter((l) => !on.includes(l.body))];
    const more = rest.length ? `; ${haired(rest)} of ${rest.length} on bodies not yet offered` : "";
    return { ok: haired(judged) === judged.length, said: `${haired(judged)} of ${judged.length} looks wear hair${more}` };
  }
  if (kind === "figures") {
    const rows = ((w as unknown as { pairings?: Pairing[] }).pairings ?? []) as Pairing[];
    const per = Object.keys(w.bodies).map((b) => [b, figures(w, rows, b)] as const);
    const least = Math.min(...per.filter(([b]) => on.includes(b)).map(([, f]) => f.count));
    return { ok: true, n: least, said: per.map(([b, f]) => `${b}${on.includes(b) ? "" : " (not yet offered)"} ${f.count} figures (${f.coloured} in colourways)`).join(", ") };
  }
  return { ok: false, said: `no live reading "${kind}"` };
}

export function rows(c: Criteria, fresh: { [check: string]: Fresh }, w: Wardrobe | null): Row[] {
  return c.criteria.map((k): Row => {
    if (k.live) {
      if (!w) return { id: k.id, mark: "?", said: "the catalogue does not read", bar: k.bar };
      const l = live(k.live, w);
      const ok = l.ok && (!k.within || (l.n !== undefined && l.n >= k.within[0] && l.n <= k.within[1]));
      return { id: k.id, mark: ok ? "✓" : "✗", said: l.said, bar: k.bar };
    }
    const f = fresh[k.check ?? ""];
    const r = f?.record;
    if (!r) return { id: k.id, mark: "?", said: "not measured", bar: k.bar, check: k.check };
    let ok: boolean;
    let said: string;
    if (k.stage) {
      const s = r.stages.find((x) => x.name === k.stage);
      if (!s) return { id: k.id, mark: "?", said: "not reached (an earlier stage stopped it)", bar: k.bar, check: k.check };
      ok = s.ok;
      said = s.said;
    } else {
      const unknown = r.failures.filter((x) => !(k.known ?? []).some((kn) => x.includes(kn.match)));
      const known = r.failures.length - unknown.length;
      ok = unknown.length === 0;
      said = `${r.pass ? "BUILD pass" : `${r.failures.length} failure${r.failures.length > 1 ? "s" : ""}`}${known ? `, ${known} known` : ""}${unknown.length ? `: ${unknown[0]}` : ""}${r.args ? ` (${r.args})` : ""}`;
    }
    return { id: k.id, mark: f.stale ? "?" : ok ? "✓" : "✗", said, bar: k.bar, check: k.check };
  });
}

/** `path#symbol` as `path:line` where the symbol is defined, or the path as it is. */
export function where(file: string, root = ROOT): string {
  const [path, symbol] = file.split("#");
  if (!symbol) return path;
  const full = join(root, path);
  if (!existsSync(full)) return `${path} (missing)`;
  const lines = readFileSync(full, "utf8").split("\n");
  const re = new RegExp(`\\b(function|fn|const|let|struct|enum|type|class|def)\\s+${symbol}\\b`);
  const i = lines.findIndex((l) => re.test(l));
  return i < 0 ? `${path} (no ${symbol})` : `${path}:${i + 1} ${symbol}`;
}

const clip = (s: string, n: number) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);

export function format(c: Criteria, fresh: { [check: string]: Fresh }, rs: Row[], head: string, root = ROOT): string {
  const out: string[] = [];
  const failing = rs.filter((r) => r.mark !== "✓");
  out.push(`CHARACTERS ${failing.length ? `${rs.length - failing.length} of ${rs.length} at their bar` : `all ${rs.length} at their bar`} (at ${head.slice(0, 8)}; records in ${recordsDir().replace(process.env.HOME ?? "\u0000", "~")})`);
  const w = Math.max(...rs.map((r) => r.id.length));
  for (const r of rs) out.push(`${r.mark} ${r.id.padEnd(w)}  ${clip(r.said, 72)}${r.mark === "✓" ? "" : `  [bar: ${clip(r.bar, 60)}]`}`);
  out.push("", "Checks (recorded when they run):");
  for (const [name, ch] of Object.entries(c.checks)) {
    const f = fresh[name];
    const state = !f?.record ? "never recorded" : f.stale ? `stale: ${f.stale} (${f.age})` : `fresh (${f.age}, at ${f.record.commit.slice(0, 8)})`;
    out.push(`  ${name}: ${clip(state, 90)}`, `    refresh: ${ch.command}  (${ch.takes})`);
  }
  // Tasks whose criterion fails come first, then the rest in their order.
  const bad = new Set(failing.map((r) => r.id));
  const tasks = [...c.tasks.filter((t) => t.criterion && bad.has(t.criterion)), ...c.tasks.filter((t) => !(t.criterion && bad.has(t.criterion)))];
  out.push("", "Tasks, in order (tools/characters/criteria.json):");
  for (const [i, t] of tasks.entries()) {
    out.push(`${i + 1}. ${clip(t.what, 220)}`);
    out.push(`   ${clip(`${t.files.map((f) => where(f, root)).join(", ")} · check: ${t.check}`, 220)}`);
  }
  // The next thing to run: a check whose record is stale or missing, before any task.
  const stale = Object.entries(c.checks).find(([n]) => !fresh[n]?.record || fresh[n].stale);
  const broken = rs.find((r) => r.mark === "✗");
  const next = stale
    ? `${stale[1].command}  (its record is ${fresh[stale[0]]?.record ? "stale" : "missing"})`
    : broken
      ? `the task for ${broken.id}, or read the log ${fresh[broken.check ?? ""]?.record?.log ?? "of its check"}`
      : tasks[0]
        ? tasks[0].check
        : "nothing: every criterion at its bar and no task left";
  out.push("", `Next: ${next}`);
  return out.slice(0, 40).join("\n");
}

export function criteria(root = ROOT): Criteria {
  return JSON.parse(readFileSync(join(root, "tools", "characters", "criteria.json"), "utf8"));
}

if (import.meta.main) {
  const c = criteria();
  const git = gitIn(ROOT);
  const here = git(["status", "--porcelain", "--untracked-files=no"])
    .split("\n")
    .filter((l) => l && l !== "\u0000")
    .map((l) => l.slice(3));
  const fresh = Object.fromEntries(Object.entries(c.checks).map(([n, ch]) => [n, freshness(ch, records(n), git, here)]));
  let w: Wardrobe | null = null;
  try {
    w = wardrobeData(ROOT);
  } catch {
    w = null;
  }
  const rs = rows(c, fresh, w);
  if (process.argv.includes("--json")) console.log(JSON.stringify({ rows: rs, checks: fresh }, null, 1));
  else console.log(format(c, fresh, rs, git(["rev-parse", "HEAD"])));
}
