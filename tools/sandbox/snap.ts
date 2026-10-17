// State snapshots and what changed between two (D399): `r snap NAME` keeps what the run
// says is true (`play.state`, and `build.state` in play) in shots/snaps/NAME.json, and
// `r diff A [B|now]` prints only what changed, a line a change: what moved more than a
// small epsilon, appeared, disappeared, or changed in number. So a check, or an agent
// finding out what an action did, reads a few lines instead of two digests side by side.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { ROOT, call, status } from "./client.ts";

export type Snap = { name: string; taken: string; frame: number | null; seconds: number | null; state?: unknown; build?: unknown };
export type Options = { epsilon?: number; most?: number };

/// A list's things are matched by the first of these they have, rather than by place in
/// the list, so one going away does not read as every one after it changing.
const IDENTITY = ["id", "name", "who", "what", "kind"];

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
const isObject = (v: unknown): v is Record<string, Json> => typeof v === "object" && v !== null && !Array.isArray(v);
const isVector = (v: unknown): v is number[] => Array.isArray(v) && v.length > 0 && v.every((n) => typeof n === "number");
const round = (n: number) => Number(n.toFixed(3));
const short = (v: unknown, width = 90) => {
  const text = JSON.stringify(isVector(v) ? v.map(round) : v) ?? "undefined";
  return text.length > width ? `${text.slice(0, width - 1)}…` : text;
};

/// What a thing in a list is: the first identity field it has, or its place in the list.
const identity = (item: Json, at: number) => {
  const field = isObject(item) ? IDENTITY.find((f) => typeof item[f] === "string" || typeof item[f] === "number") : undefined;
  return field && isObject(item) ? String(item[field]) : `${at}`;
};

/// Two lists' things paired: by identity, and among things of the same identity (three
/// deer), nearest to nearest by where they are (`at`), else in order. Each is labelled
/// `[deer]`, or `[deer#2]` where there are several: numbered in the newer list, and what
/// went numbered on after them.
function paired(was: Json[], now: Json[]): [string, Json | undefined, Json | undefined][] {
  const group = (list: Json[]) => {
    const groups = new Map<string, Json[]>();
    list.forEach((item, at) => groups.set(identity(item, at), [...(groups.get(identity(item, at)) ?? []), item]));
    return groups;
  };
  const before = group(was);
  const after = group(now);
  const out: [string, Json | undefined, Json | undefined][] = [];
  for (const name of new Set([...after.keys(), ...before.keys()])) {
    const a = before.get(name) ?? [];
    const b = after.get(name) ?? [];
    const label = (n: number) => (a.length <= 1 && b.length <= 1 ? `[${name}]` : `[${name}#${n + 1}]`);
    const where = (item: Json) => (isObject(item) && isVector(item.at) ? item.at : undefined);
    const match = new Map<number, number>(); // b's index to a's
    if ([...a, ...b].every((item) => where(item))) {
      const pairs = a.flatMap((x, i) => b.map((y, j) => ({ i, j, d: Math.hypot(...where(x)!.map((n, k) => n - (where(y)![k] ?? 0))) })));
      const usedA = new Set<number>();
      for (const { i, j } of pairs.sort((p, q) => p.d - q.d)) {
        if (usedA.has(i) || match.has(j)) continue;
        usedA.add(i);
        match.set(j, i);
      }
    } else for (let j = 0; j < Math.min(a.length, b.length); j++) match.set(j, j);
    b.forEach((item, j) => out.push([label(j), match.has(j) ? a[match.get(j)!] : undefined, item]));
    // What went is numbered on from what is there now, so no two lines share a label.
    const kept = new Set(match.values());
    a.filter((_, i) => !kept.has(i)).forEach((item, n) => out.push([label(b.length + n), item, undefined]));
  }
  return out;
}

function walk(path: string, a: Json | undefined, b: Json | undefined, epsilon: number, out: string[]) {
  const at = path || "(all)";
  if (a === undefined && b === undefined) return;
  if (a === undefined) return void out.push(`+ ${at}: ${short(b)}`);
  if (b === undefined) return void out.push(`- ${at}: ${short(a)}`);
  if (typeof a === "number" && typeof b === "number") {
    if (Math.abs(a - b) > epsilon) out.push(`~ ${at}: ${round(a)} -> ${round(b)}`);
    return;
  }
  if (isVector(a) && isVector(b)) {
    if (a.length !== b.length || a.some((n, i) => Math.abs(n - b[i]) > epsilon)) {
      const moved = a.length === b.length && a.length <= 3 ? ` (moved ${round(Math.hypot(...a.map((n, i) => n - b[i])))})` : "";
      out.push(`~ ${at}: ${short(a)} -> ${short(b)}${moved}`);
    }
    return;
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.some(isObject) || b.some(isObject)) {
      if (a.length !== b.length) out.push(`# ${at}: ${a.length} -> ${b.length}`);
      for (const [label, was, now] of paired(a, b)) walk(`${path}${label}`, was, now, epsilon, out);
      return;
    }
    // Plain values: what came and went, counted, so order does not matter.
    const count = (list: Json[]) => list.reduce((m, v) => m.set(short(v), (m.get(short(v)) ?? 0) + 1), new Map<string, number>());
    const was = count(a);
    const now = count(b);
    for (const [v, n] of now) for (let i = was.get(v) ?? 0; i < n; i++) out.push(`+ ${at}: ${v}`);
    for (const [v, n] of was) for (let i = now.get(v) ?? 0; i < n; i++) out.push(`- ${at}: ${v}`);
    return;
  }
  if (isObject(a) && isObject(b)) {
    for (const key of Object.keys(b)) walk(path ? `${path}.${key}` : key, a[key], b[key], epsilon, out);
    for (const key of Object.keys(a)) if (!(key in b)) walk(path ? `${path}.${key}` : key, a[key], undefined, epsilon, out);
    return;
  }
  if (JSON.stringify(a) !== JSON.stringify(b)) out.push(`~ ${at}: ${short(a, 60)} -> ${short(b, 60)}`);
}

/// Every change from `a` to `b`, a line each: `~` changed, `+` appeared, `-` went, `#` how
/// many there are changed. Numbers within `epsilon` of each other (0.01, a centimetre, as
/// `state` rounds to) are the same.
export function changes(a: unknown, b: unknown, epsilon = 0.01): string[] {
  const out: string[] = [];
  walk("", a as Json, b as Json, epsilon, out);
  return out;
}

/// What changed between two snaps, headed by which they are, at most `most` changes and a
/// count of the rest.
export function diff(a: Snap, b: Snap, { epsilon = 0.01, most = 40 }: Options = {}): string[] {
  const found = changes({ state: a.state, build: a.build }, { state: b.state, build: b.build }, epsilon);
  const frame = (s: Snap) => (s.frame === null ? "" : ` (frame ${s.frame})`);
  const head = `${a.name}${frame(a)} -> ${b.name}${frame(b)}: ${found.length ? `${found.length} change${found.length === 1 ? "" : "s"}` : "nothing changed"}`;
  const shown = found.slice(0, most);
  if (found.length > most) shown.push(`… and ${found.length - most} more (--most to see them)`);
  return [head, ...shown];
}

const optional = async (method: string) => {
  try {
    return await call(method);
  } catch {
    return undefined;
  }
};

/// What the running sandbox says is true now.
export async function take(name: string): Promise<Snap> {
  const now = await status().catch(() => undefined);
  const [state, build] = [await optional("play.state"), await optional("build.state")];
  if (state === undefined && build === undefined) throw new Error("the run answers neither play.state nor build.state; snaps are of the play experiment");
  return { name, taken: new Date().toISOString(), frame: now?.frame ?? null, seconds: now?.seconds ?? null, state, build };
}

const SNAPS = resolve(ROOT, "shots/snaps");
const file = (name: string) => resolve(SNAPS, `${name}.json`);

export async function save(name: string) {
  if (!/^[\w.-]+$/.test(name) || name === "now") throw new Error(`a snap's name is letters, digits, - _ and . (and not "now"), not ${name}`);
  const snap = await take(name);
  mkdirSync(SNAPS, { recursive: true });
  writeFileSync(file(name), JSON.stringify(snap, null, 1));
  return `${file(name)} (frame ${snap.frame})`;
}

export async function load(name: string): Promise<Snap> {
  if (name === "now") return take("now");
  if (!existsSync(file(name))) throw new Error(`no snap called ${name} in ${SNAPS}`);
  return JSON.parse(readFileSync(file(name), "utf8")) as Snap;
}
