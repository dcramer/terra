#!/usr/bin/env node
// A fight clip's keys found and judged before Kimodo makes takes from them (D565 step 3, D450):
// each key solved as a person's arm reaches for it (kimodo_keys.py --reach, on Kimodo's skeleton
// from the clip's guard; Python only there, D121), the rest here.
//
//   node tools/characters/keys.ts check Keyed_Backhand             # every key of the clip's spec
//   node tools/characters/keys.ts search Keyed_Backhand 0.85 --edge-toward 0.6,0,-0.8 --swing 150
//   node tools/characters/keys.ts between Keyed_Backhand 1.4 guard  # recovery keys toward the guard
//
// check CLIP: each key's wrist (flexed + toward the palm, deviated + toward the thumb), the
// forearm's twist (+ palm up), the elbow's swing about the shoulder-to-wrist line (0 where the
// guard's elbow is) and how far each moved from the key before, and how far past the keys' bar
// (a person's wrist, wrist.ts, less kimodo_keys.py's margin for Kimodo's slack) it is.
// ✗ (exit 1): a key past the bar (kimodo_keys.py refuses it); a key sooner than FIRST_KEY_S after
// the clip's start (Kimodo snaps from the full-body guard to it). · (a caution): the last key's
// forearm twist more than GUARD_TWIST_DEG from the guard's. Each is drawn from takes; see them.
//
// search CLIP AT: one key (the spec's at AT, or a new one there from --grip) tried over blades in a
// cone about its own (the guard's for a new key) and edges about the way its tip moves from the
// key before (any edge with --free), each grip and yaw given; ranked by how far past the bar, the
// bend and the targets given. --grip "x,y,z;x,y,z", --yaw "a,b", --cone DEG (60), --edge-toward
// x,y,z with --edge-within COS (0.6), --swing, --twist, --flex, --dev DEG (targets), --twist-most
// DEG (160), --free, --top N (6). Prints each as a key to paste into the spec.
//
// between CLIP A B: keys a share of the way between two (each a key's seconds or `guard`, the
// clip's first or last pose), grip and yaw lerped, blade and edge nlerped, at --at (0.25,0.5,0.75).
//
// --list is the set (art/animation/sets/fight.json); --side left judges the shield hand's keys.
// The guard's own hand is said first: its take holds the wrist past the bar (kimodo_keys.py's
// margin, and a few degrees past a person's radial, which wrist_limit.ts cleans at keep time), so
// a key near the guard reads past as the guard does. A line a stage (D388). Kimodo is set up as
// tools/characters/README.md, "Made clips" says. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { readSpec, type Key, type Spec, type V3 } from "./clip_spec.ts";
import { KIMODO } from "./motion.ts";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const HERE = join(ROOT, "tools", "characters");
const PYTHON = join(KIMODO, "venv", "bin", "python");

/** The soonest a key may come after the clip's start, seconds: Kimodo snaps the arm from the
 * full-body guard at frame 0 to a hand key sooner (d565 jobs 5 and 7: a 0.2 s key put 6 of 8
 * backhand takes 27-35° past the wrist at 0.08-0.21 s; the slash's 0.25 s key passes). */
export const FIRST_KEY_S = 0.25;
/** The most the sword hand's last key's forearm twist may differ from the guard's, a caution
 * (d565 job 7: the backhand's
 * 1.4 s key, 48° from the guard's, left the take radial 29 at 1.67 s; a 1.9 s key between, 10°
 * from it, passed; the slash's and heavy's last keys are 33° and 21° from it). */
export const GUARD_TWIST_DEG = 45;
/** How far ahead of the grip a key's tip is taken, metres, for the way it moves. */
const TIP_M = 0.8;

export type Side = "Right" | "Left";
type Wrist = { flex: number; dev: number; twist: number; swing: number; past: number; elbow: V3 };
export type Reach = Wrist & { side: Side; at: number; grip: V3; blade: V3; short: number };
export type Bar = { flex: number; ext: number; radial: number; ulnar: number; twist: [number, number] };
export type GuardHand = { grip: V3; blade: V3; edge?: V3; back?: V3; reach: Reach; bones: Wrist };
export type Reached = { bar: Bar; guard: Record<Side, GuardHand>; keys: Reach[] };
export type Judged = { reach: Reach; flags: string[]; cautions: string[]; moved: string };

/** A failure that says what to do (D556), printed as one line by this tool and clip.ts. */
export class Failed extends Error {}

const unit = (v: V3): V3 => {
  const l = Math.hypot(...v) || 1;
  return [v[0] / l, v[1] / l, v[2] / l];
};
const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const mix = (a: V3, b: V3, f: number): V3 => [a[0] + f * (b[0] - a[0]), a[1] + f * (b[1] - a[1]), a[2] + f * (b[2] - a[2])];
const plus = (a: V3, b: V3, s = 1): V3 => [a[0] + s * b[0], a[1] + s * b[1], a[2] + s * b[2]];
const along = (a: V3, sa: number, b: V3, sb: number): V3 => plus(plus([0, 0, 0], a, sa), b, sb);
const square = (v: V3, n: V3): V3 => plus(v, n, -dot(v, n));
const r2 = (v: V3): V3 => v.map((x) => Math.round(x * 100) / 100 + 0) as V3;
const deg = (r: number) => (r * 180) / Math.PI;
const rad = (d: number) => (d * Math.PI) / 180;
const angle = (a: V3, b: V3) => deg(Math.acos(Math.max(-1, Math.min(1, dot(unit(a), unit(b))))));
/** The difference of two angles in degrees, -180..180. */
export const angleStep = (a: number, b: number) => ((((b - a) % 360) + 540) % 360) - 180;
const tipOf = (grip: V3, blade: V3) => plus(grip, unit(blade), TIP_M);

/** The guard take of a clip set as Kimodo's NPZ (tools/characters/out/clip/<set>/<guard>.npz),
 * converted again when the take is newer; clip.ts's guard stage and this tool share it. */
export function guardNpz(list: string, guard: string, run: (command: string, args: string[]) => void): string {
  const bvh = join(list.replace(/\.json$/, ""), `${guard}.bvh`);
  const npz = join(HERE, "out", "clip", basename(list, ".json"), `${guard}.npz`);
  if (!existsSync(bvh)) throw new Failed(`no ${relative(ROOT, bvh)}: keep a take of ${guard} first (motion.ts --keep)`);
  mkdirSync(dirname(npz), { recursive: true });
  if (!existsSync(npz) || statSync(npz).mtimeMs < statSync(bvh).mtimeMs) run(join(KIMODO, "venv", "bin", "kimodo_convert"), [bvh, npz, "--bvh_standard_tpose"]);
  return npz;
}

/** Each key's reach from the guard pose (frame `frame` of `npz`), the bar and the guard's hands. */
export function reachAll(npz: string, frame: number, keys: (Key & { side?: Side })[]): Reached {
  const dir = mkdtempSync(join(tmpdir(), "terra-keys-"));
  try {
    writeFileSync(join(dir, "keys.json"), JSON.stringify({ frame, keys }));
    const r = spawnSync(PYTHON, ["-I", join(HERE, "kimodo_keys.py"), "--reach", npz, join(dir, "keys.json")], { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
    const line = (r.stdout ?? "").split("\n").find((l) => l.startsWith("REACH "));
    if (r.status !== 0 || !line) throw new Failed(`kimodo_keys.py --reach exited ${r.status}: ${(r.stderr ?? "").trim().split("\n").slice(-3).join(" | ")}`);
    return JSON.parse(line.slice(6));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** Which of the bar's limits a wrist is past, in plain words. */
export function pastWhat(r: Wrist, bar: Bar): string {
  const over = [
    [r.flex - bar.flex, `flexed ${r.flex.toFixed(0)} (bar ${bar.flex})`],
    [-r.flex - bar.ext, `extended ${(-r.flex).toFixed(0)} (bar ${bar.ext})`],
    [r.dev - bar.radial, `radial ${r.dev.toFixed(0)} (bar ${bar.radial})`],
    [-r.dev - bar.ulnar, `ulnar ${(-r.dev).toFixed(0)} (bar ${bar.ulnar})`],
    [bar.twist[0] - r.twist, `twisted ${r.twist.toFixed(0)} (bar ${bar.twist[0]})`],
    [r.twist - bar.twist[1], `twisted ${r.twist.toFixed(0)} (bar ${bar.twist[1]})`],
  ] as const;
  return over.filter(([d]) => d > 0).map(([, s]) => s).join(", ");
}

/** A wrist in plain words. */
const wrist = (r: Wrist, bar: Bar) =>
  `flexed ${r.flex.toFixed(0)}, deviated ${r.dev.toFixed(0)}, twisted ${r.twist.toFixed(0)}, elbow ${r.swing.toFixed(0)}; ` +
  (r.past > 0 ? `${r.past.toFixed(1)}° past the bar: ${pastWhat(r, bar)}` : "within the bar");

/** One hand's keys judged in order, the guard's own hand at the clip's start and end: each key's
 * flags (✗) and cautions (·) and how far its arm moved from the one before. Pure. */
export function judge(guard: GuardHand, keys: Reach[], bar: Bar): Judged[] {
  return keys.map((r, i) => {
    const before = i ? keys[i - 1] : { ...guard.bones, at: 0 };
    const flags: string[] = [];
    const cautions: string[] = [];
    if (r.past > 0) flags.push(`past the bar (${pastWhat(r, bar)}): kimodo_keys.py refuses it; change its edge, blade or grip (keys.ts search)`);
    if (r.at < FIRST_KEY_S) flags.push(`${r.at.toFixed(2)} s is sooner than ${FIRST_KEY_S} s after the guard: Kimodo snaps the arm to it; key it later`);
    const toGuard = Math.abs(r.twist - guard.bones.twist);
    if (r.side === "Right" && i === keys.length - 1 && toGuard > GUARD_TWIST_DEG)
      cautions.push(`twisted ${toGuard.toFixed(0)}° from the guard's ${guard.bones.twist.toFixed(0)} (more than ${GUARD_TWIST_DEG}): a take's recovery may leave the wrist; add a key between (keys.ts between ${r.at} guard)`);
    const moved = `from ${i ? `the ${before.at.toFixed(2)} s key` : "the guard"} in ${(r.at - before.at).toFixed(2)} s: ` +
      `elbow ${angleStep(before.swing, r.swing) >= 0 ? "+" : ""}${angleStep(before.swing, r.swing).toFixed(0)}, upper arm ${angle(before.elbow, r.elbow).toFixed(0)}°, ` +
      `twist ${r.twist - before.twist >= 0 ? "+" : ""}${(r.twist - before.twist).toFixed(0)}`;
    return { reach: r, flags, cautions, moved };
  });
}

/** The guard's lines: its hand on its own bones and as a key reaches it. */
function sayGuard(spec: Spec, side: Side, got: Reached) {
  const g = got.guard[side];
  console.log(`${g.bones.past > 0 ? "·" : "✓"} guard: ${spec.guard.clip} frame ${spec.guard.frame}, ${side.toLowerCase()} hand at ${JSON.stringify(r2(g.grip))}: ` +
    `its own bones ${wrist(g.bones, got.bar)}; as a key ${wrist(g.reach, got.bar)}`);
  if (g.bones.past > 0)
    console.log(`  the guard take holds its wrist past the keys' bar, so a key near it reads past as it does, and Kimodo meets such a key as it meets the guard; ` +
      `wrist_limit.ts cleans the few degrees past a person's at keep time. A key is still refused past the bar: keep keys a little further from the guard`);
}

/** The keys of one hand of a spec, sorted. */
const handKeys = (spec: Spec, side: Side) => [...(side === "Right" ? spec.hand : spec.left)].sort((a, b) => a.at - b.at);

/** A key printed as the spec takes it. */
export const keyJson = (k: Key) =>
  JSON.stringify({ at: Math.round(k.at * 100) / 100, grip: r2(k.grip), ...(k.blade && { blade: r2(k.blade) }), ...(k.edge && { edge: r2(k.edge) }), ...(k.back && { back: r2(k.back) }), ...(k.yaw !== undefined && { yaw: Math.round(k.yaw) }) });

/** A spec's keys of one hand judged: the guard, the bar and each key's verdict. */
export function checkSpec(spec: Spec, side: Side, npz: string) {
  const keys = handKeys(spec, side);
  if (!keys.length) throw new Failed(`${spec.name} has no ${side.toLowerCase()} keys: --side ${side === "Right" ? "left" : "right"}?`);
  const got = reachAll(npz, spec.guard.frame, keys.map((k) => ({ ...k, side })));
  return { got, judged: judge(got.guard[side], got.keys, got.bar) };
}

function check(spec: Spec, side: Side, npz: string): number {
  const { got, judged } = checkSpec(spec, side, npz);
  sayGuard(spec, side, got);
  for (const j of judged) {
    const mark = j.flags.length ? "✗" : j.cautions.length ? "·" : "✓";
    const short = j.reach.short > 0.005 ? `, ${(j.reach.short * 100).toFixed(1)} cm short of its grip` : "";
    console.log(`${mark} ${j.reach.at.toFixed(2)} s: ${wrist(j.reach, got.bar)}${short}; ${j.moved}${[...j.flags, ...j.cautions].map((f) => `\n    ${f}`).join("")}`);
  }
  const bad = judged.filter((j) => j.flags.length).length;
  const warn = judged.filter((j) => j.cautions.length).length;
  const n = `${judged.length} ${side.toLowerCase()} keys`;
  console.log(bad ? `✗ check: ${spec.name}, ${bad} of ${n} refused: change them before a take, never the bar` : `✓ check: ${spec.name}, ${n} within the bar${warn ? `, ${warn} with a caution` : ""}`);
  return bad ? 1 : 0;
}

/** Directions within `degrees` of `b`, rings `step` apart. */
export function cone(b: V3, degrees: number, step: number): V3[] {
  b = unit(b);
  const p = unit(cross(b, Math.abs(b[2]) < 0.9 ? [0, 0, 1] : [1, 0, 0]));
  const q = cross(b, p);
  const out: V3[] = [b];
  for (let t = step; t <= degrees + 1e-6; t += step) {
    const n = Math.max(6, Math.round((2 * Math.PI * Math.sin(rad(t))) / rad(step)));
    for (let j = 0; j < n; j++) out.push(along(b, Math.cos(rad(t)), along(p, Math.cos((2 * Math.PI * j) / n), q, Math.sin((2 * Math.PI * j) / n)), Math.sin(rad(t))));
  }
  return out;
}

/** A key a share `f` of the way from `a` to `b`: grip and yaw lerped, blade and edge (or back)
 * nlerped, the edge made square to the blade. */
export function between(a: Key, b: Key, f: number): Key {
  const blade = unit(mix(a.blade!, b.blade!, f));
  const turn = a.edge && b.edge ? "edge" : "back";
  return { at: a.at + f * (b.at - a.at), grip: mix(a.grip, b.grip, f), blade, [turn]: unit(square(mix(a[turn]!, b[turn]!, f), blade)), yaw: (1 - f) * (a.yaw ?? 0) + f * (b.yaw ?? 0) };
}

/** The guard's own hand as a key at `at`. */
const guardKey = (g: GuardHand, at: number): Key => ({ at, grip: g.grip, blade: g.blade, ...(g.edge ? { edge: g.edge } : { back: g.back }), yaw: 0 });

const v3 = (s: string, what: string): V3 => {
  const v = s.split(",").map(Number);
  if (v.length !== 3 || v.some((x) => !Number.isFinite(x))) throw new Failed(`${what} "${s}": three numbers, right,ahead,up (e.g. 0.6,0,-0.8)`);
  return v as V3;
};
const num = (s: unknown, what: string) => {
  if (s === undefined) return undefined;
  const n = Number(s);
  if (!Number.isFinite(n)) throw new Failed(`${what} "${s}": a number`);
  return n;
};

export type Want = { swing?: number; twist?: number; flex?: number; dev?: number; twistMost: number; free: boolean };

/** A candidate's cost: past the bar above all, then the bend (or its distance from a target), how
 * far its edge is tilted off the tip's way, and the targets. */
export function cost(r: Wrist, tilt: number, want: Want) {
  let c = r.past * 100 + (want.free ? 0 : Math.abs(tilt) / 2) + Math.max(0, r.twist - want.twistMost) * 2;
  c += want.flex === undefined ? Math.abs(r.flex) / 4 : Math.abs(r.flex - want.flex) / 2;
  c += want.dev === undefined ? Math.abs(r.dev) / 2 : Math.abs(r.dev - want.dev) / 2;
  if (want.twist !== undefined) c += Math.abs(r.twist - want.twist) / 2;
  if (want.swing !== undefined) c += Math.abs(angleStep(r.swing, want.swing)) / 4;
  return c;
}

function search(spec: Spec, side: Side, npz: string, at: number, v: Record<string, string | boolean | undefined>) {
  if (!(at > 0 && at < spec.seconds)) throw new Failed(`AT ${at}: a key's seconds inside the clip (0 to ${spec.seconds})`);
  const keys = handKeys(spec, side);
  const own = keys.find((k) => Math.abs(k.at - at) < 1e-6);
  const grips = v.grip ? (v.grip as string).split(";").map((g) => v3(g, "--grip")) : own ? [own.grip] : [];
  if (!grips.length) throw new Failed(`${spec.name} has no key at ${at} s: give a --grip "right,ahead,up" (metres from the hips) to search a new one`);
  const yaws = v.yaw ? (v.yaw as string).split(",").map((y) => num(y, "--yaw")!) : [own?.yaw ?? 0];
  const want: Want = { swing: num(v.swing, "--swing"), twist: num(v.twist, "--twist"), flex: num(v.flex, "--flex"), dev: num(v.dev, "--dev"), twistMost: num(v["twist-most"], "--twist-most") ?? 160, free: !!v.free || side === "Left" };
  const toward = v["edge-toward"] ? unit(v3(v["edge-toward"] as string, "--edge-toward")) : undefined;
  const within = num(v["edge-within"], "--edge-within") ?? 0.6;
  const before = keys.filter((k) => k.at < at).at(-1);
  const g = reachAll(npz, spec.guard.frame, before ? [{ ...before, side }] : []);
  const guard = g.guard[side];
  const prev = before ? g.keys[0] : { ...guard.bones, at: 0 };
  const prevTip = tipOf(before?.grip ?? guard.grip, before?.blade ?? guard.blade);
  const turn = side === "Right" ? "edge" : "back";
  const tried: { key: Key; tilt: number }[] = [];
  for (const grip of grips)
    for (const yaw of yaws)
      for (const blade of cone(own?.blade ?? guard.blade, num(v.cone, "--cone") ?? 60, 10)) {
        const lead = unit(square(plus(tipOf(grip, blade), prevTip, -1), unit(blade)));
        const across = cross(unit(blade), lead);
        for (const tilt of want.free ? [...Array(12).keys()].map((i) => i * 30 - 180) : [0, -20, 20, -35, 35]) {
          const edge = along(lead, Math.cos(rad(tilt)), across, Math.sin(rad(tilt)));
          if (toward && dot(edge, toward) < within) continue;
          tried.push({ key: { at, grip, blade: unit(blade), [turn]: edge, yaw }, tilt });
        }
      }
  if (!tried.length) throw new Failed(`no edge within ${within} of --edge-toward ${v["edge-toward"]}: widen --cone or lower --edge-within`);
  const got = reachAll(npz, spec.guard.frame, tried.map((t) => ({ ...t.key, side })));
  const ranked = got.keys.map((r, i) => ({ r, key: tried[i].key, c: cost(r, tried[i].tilt, want) })).sort((a, b) => a.c - b.c);
  sayGuard(spec, side, got);
  const clean = ranked.filter((x) => x.r.past <= 0).length;
  console.log(`${clean ? "✓" : "✗"} search: ${spec.name} at ${at.toFixed(2)} s, ${tried.length} keys tried, ${clean} within the bar` +
    (clean ? `; the best (the key before: elbow ${prev.swing.toFixed(0)}, twisted ${prev.twist.toFixed(0)})` : `: move the grip (--grip), widen --cone or drop --edge-toward`));
  for (const x of ranked.slice(0, num(v.top, "--top") ?? 6)) console.log(`  cost ${x.c.toFixed(1)}: ${wrist(x.r, got.bar)}; ${keyJson(x.key)}`);
  return clean ? 0 : 1;
}

function interpolate(spec: Spec, side: Side, npz: string, a: string, b: string, at: string) {
  const keys = handKeys(spec, side);
  const g = reachAll(npz, spec.guard.frame, []).guard[side];
  const pick = (s: string, end: number): Key => {
    if (s === "guard") return guardKey(g, end);
    const k = keys.find((k) => Math.abs(k.at - Number(s)) < 1e-6);
    if (!k) throw new Failed(`${spec.name} has no ${side.toLowerCase()} key at ${s}: one of ${keys.map((k) => k.at).join(", ")}, or guard`);
    return k;
  };
  const shares = at.split(",").map((f) => num(f, "--at")!);
  const made = shares.map((f) => between(pick(a, 0), pick(b, spec.seconds), f));
  const got = reachAll(npz, spec.guard.frame, made.map((k) => ({ ...k, side })));
  sayGuard(spec, side, got);
  got.keys.forEach((r, i) => console.log(`${r.past > 0 ? "✗" : "✓"} ${shares[i]} of the way: ${wrist(r, got.bar)}; ${keyJson(made[i])}`));
  const bad = got.keys.filter((r) => r.past > 0).length;
  console.log(`${bad ? "✗" : "✓"} between: ${spec.name} ${a} to ${b}, ${made.length - bad} of ${made.length} keys within the bar${bad ? ": try other shares (--at), or search one (keys.ts search)" : ""}`);
  return bad ? 1 : 0;
}

function main() {
  const { values: v, positionals: [mode, clip, ...rest] } = parseArgs({
    allowPositionals: true,
    options: {
      list: { type: "string", default: "art/animation/sets/fight.json" }, side: { type: "string", default: "right" },
      grip: { type: "string" }, yaw: { type: "string" }, cone: { type: "string" }, "edge-toward": { type: "string" }, "edge-within": { type: "string" },
      swing: { type: "string" }, twist: { type: "string" }, flex: { type: "string" }, dev: { type: "string" }, "twist-most": { type: "string" },
      free: { type: "boolean" }, top: { type: "string" }, at: { type: "string", default: "0.25,0.5,0.75" },
    },
  });
  const usage = "usage: keys.ts check CLIP | search CLIP AT [--grip, --yaw, --cone, --edge-toward, --swing, --twist, --flex, --dev, --free] | between CLIP A|guard B|guard [--at 0.25,0.5] (--list SET.json, --side left)";
  if (!["check", "search", "between"].includes(mode) || !clip || (mode !== "check" && !rest.length)) throw new Failed(usage);
  if (!existsSync(PYTHON)) throw new Failed(`no Kimodo at ${KIMODO}: see tools/characters/README.md, "Made clips"`);
  const list = resolve(ROOT, v.list!);
  const entries: Record<string, Record<string, unknown>> = JSON.parse(readFileSync(list, "utf8"));
  if (!entries[clip]?.guard) throw new Failed(`${clip} has no spec in ${relative(ROOT, list)}: one of ${Object.keys(entries).filter((k) => entries[k].guard).join(", ")}`);
  const spec = readSpec(clip, entries[clip]);
  const side: Side = v.side === "left" ? "Left" : "Right";
  const npz = guardNpz(list, spec.guard.clip, (command, args) => {
    const r = spawnSync(command, args, { cwd: ROOT, encoding: "utf8" });
    if (r.status !== 0) throw new Failed(`${basename(command)} exited ${r.status}: ${(r.stderr ?? "").trim().split("\n").slice(-2).join(" | ")}`);
  });
  if (mode === "check") return check(spec, side, npz);
  if (mode === "search") return search(spec, side, npz, num(rest[0], "AT")!, v);
  if (rest.length < 2) throw new Failed(usage);
  return interpolate(spec, side, npz, rest[0], rest[1], v.at!);
}

if (import.meta.main) {
  try {
    process.exitCode = main();
  } catch (e) {
    const parse = String((e as { code?: string }).code ?? "").startsWith("ERR_PARSE_ARGS");
    if (!(e instanceof Failed) && !parse) throw e;
    console.log(`✗ keys: ${(e as Error).message.split("\n")[0]}${parse ? " (a value starting with a minus is written --yaw=-15)" : ""}`);
    process.exitCode = 1;
  }
}
