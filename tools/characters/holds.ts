#!/usr/bin/env node
// The hold report: how every held look (assets/models/held) is held, in numbers, with no GPU, so
// a hold is judged without rendering and reading stills. Every look on every body template and
// the unmorphed base body; a line a look that passes, and for one that fails only the measures
// over their bars, each with body, clip, time, value, bar and what to change (D388, D556).
//
//   size   its length (or longest extent) against its kind's life-size range (attach_kinds.json);
//          read, not barred: its head's span in fists (no source gives a bar)
//   grip   the handle's radius against the thickest a power grip closes on (NIOSH 2004-164, "A
//          Guide to Selecting Non-Powered Hand Tools": 2 in, 5.1 cm across); the grip's distance
//          from the butt against its kind's range; the grip off the handle's line
//          (attach_contract.ts GRIP_M); the closed fingers off the handle (grip.ts fistOff, 1 cm,
//          moves.ts fist_cm); read: how far the handle stands out past the little
//          finger and, a blade's, past the thumb's side before it widens
//   carry  through every clip the game carries it in (terra_walker's CARRIED_IN), 15 poses a
//          second from 0: the wrist's flexion, extension, radial and ulnar deviation against a
//          person's (wrist.ts WRIST, AAOS); the look's lowest point against the ground under the
//          feet; how deep it goes into the trunk and into the calves and feet (moves.ts's own
//          numbers and its 1 cm bar); how far above level its working end rises (moves.ts
//          carried_rise_deg, 10 degrees, held.carry.axe_down's bar), of a pose that hangs: one
//          marked upright (a torch, a spear, a bow) is meant to rise; read: of an upright pose,
//          how far below level its working end falls at worst (carried_fall_deg; negative, it
//          stays that far above); the handle below and above level and out from the body, the
//          elbow's bend
//   stand  in the standing idle (Idle_Loop), how both arms hang (hold_pose.ts stancePose): the
//          upper arm's degrees out from the trunk (ARM_OUT_DEG) and the gap between forearm or
//          palm and the trunk (ARM_GAP_CM), of the arm carrying a handle that hangs and of a
//          free arm; read: the elbow's bend, and the arm of an upright pose or a hung look
//   use    the same bars through the walker's use clips, or its class's set (clearances only: a
//          set's wrists are moves.ts's own rows)
//
// Same inputs, same bytes: looks, bodies and rows sorted, fixed sample times, one decimal, no
// clock. A look's run on a body is kept by the hash of all it reads (fit_cache.ts), so an
// unchanged look costs nothing. A still is needed only for what no number here covers: the
// silhouette (whether it reads as the thing it is), its materials, and the fingers' look on it.
//
//   node tools/characters/holds.ts [ITEM...] [--json FILE] [--body male,base] [--fresh]
//
// Node 22.18 or later; no dependencies.

import { appendFileSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { availableParallelism } from "node:os";
import { basename, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { GRIP_M, HEAD_SHARE, KINDS, type Look, ROOT, dataOf, fittedAs, handleAt, headReach, kindOf, readLook } from "./attach_contract.ts";
import { HELD_CM, PARTNER, movesReads, runMoves, setOf } from "./fit.ts";
import { Cache, closure, key } from "./fit_cache.ts";
import { Glb, type V3, posed, qinv, rot, sub } from "./glb.ts";
import { FINGER_M, FIST_CM, cut, handleIn } from "./grip.ts";
import { carriedClips, carryHold, carryUpright } from "./held.ts";
import { BASE } from "./skin.ts";
import { WRIST } from "./wrist.ts";

const CHARS = join(ROOT, "tools", "characters");
const OUT = join(CHARS, "out");
const HELD = join(ROOT, "assets", "models", "held");
/** The thickest handle a power grip closes on, its radius in cm (NIOSH 2004-164: 2 in across). */
export const HANDLE_R_CM = 2.55;
/** How far above level a look carried in the fist may tip its working end, degrees: the bar of
 * held.carry.axe_down (tools/combat/criteria.json, D564; a jump's arms lift it a little). */
export const RISE_DEG = 10;
/** The clip a body stands idle in, where the stance is judged (`stand.*`). */
export const STANDING = "Idle_Loop";
/** How far an arm hanging at the side may stand out from the trunk while standing idle: the
 * upper arm's degrees out (hold_pose.ts `stancePose`). A person's hangs 4.5 degrees out (median;
 * IQR 0.9 to 7.8) with the elbow bent 15.5 (13.2 to 18.1): "Three-dimensional alignment of the
 * upper extremity in the standing neutral position in healthy subjects", J Orthop Surg Res 2022,
 * doi 10.1186/s13018-022-03113-5 (upright CT; read from its abstract, not the full text). The
 * library's Idle_Loop holds ours 12 out, and a tool in the fist needs a little more to pass the
 * thigh; 20 is where it starts to read as held away (the developers' still of 2026-11-09, the
 * carried arm 26 to 28.5 out: "our stance with the hammer is still stupid"). */
export const ARM_OUT_DEG = 20;
/** And the gap between the forearm's or palm's skin and the trunk's, cm: a hand's breadth (the
 * same still: "a gap of a hand or more", 12.8 to 18.7 measured). */
export const ARM_GAP_CM = 10;
/** How far under the ground a look may reach, cm. */
export const GROUND_CM = 0;

/** One measure of one look: a `bar` it is judged against (ok true or false), or a reading (ok
 * null: no proven bar). `value` is at most `bar`, or within it when the bar is a range. */
export type Measure = { look: string; measure: string; body: string; clip: string; time: number | null; value: number; bar: number | [number, number] | null; unit: string; ok: boolean | null; where: string };

const r1 = (x: number) => Math.round(x * 10) / 10;
const within = (v: number, bar: number | [number, number]) => (typeof bar === "number" ? v <= bar : v >= bar[0] && v <= bar[1]);

/** The fist on the handle's line, from the base body at rest: where the little finger's and the
 * index finger's sides lie along the hand socket's +Y (m from its origin), and its breadth. */
export function fistOf(body: Glb): { lo: number; hi: number; breadth: number } {
  const rest = posed(body, undefined, 0);
  const s = rest[body.named("socket.hand.R")];
  const y = (n: string) => rot(qinv(s.r), sub(rest[body.named(n)].t, s.t))[1] / s.s[1];
  const [a, b] = [y("pinky_01_r"), y("index_01_r")];
  const [lo, hi] = [Math.min(a, b) - FINGER_M, Math.max(a, b) + FINGER_M];
  return { lo, hi, breadth: hi - lo };
}

/** Where a handle of radius `r` on the line `at` stops being a handle going up +Y from the grip:
 * the first 5 mm slab with a vertex further than twice its radius from its line. */
function handleTop(look: Look, at: [number, number], r: number): number {
  let top = Infinity;
  for (const p of look.points) if (p[1] > 0 && Math.hypot(p[0] - at[0], p[2] - at[1]) > 2 * r + 0.005) top = Math.min(top, p[1]);
  return Math.floor(top / 0.005) * 0.005;
}

/** The measures a look's own shape gives: its size against its kind, its grip against a fist. */
export function shapeOf(look: Look, kind: string | undefined, mount: string | undefined, fist: { lo: number; hi: number; breadth: number }): Measure[] {
  const name = look.name.split("/").at(-1)!;
  const out: Measure[] = [];
  const add = (measure: string, value: number, unit: string, bar: Measure["bar"], where: string) =>
    out.push({ look: name, measure, body: "-", clip: "-", time: null, value, bar, unit, ok: bar === null ? null : within(value, bar), where });
  const rules = kind ? KINDS[kind] : undefined;
  const lo: V3 = [Infinity, Infinity, Infinity];
  const hi: V3 = [-Infinity, -Infinity, -Infinity];
  for (const p of look.points) for (let k = 0; k < 3; k++) (lo[k] = Math.min(lo[k], p[k])), (hi[k] = Math.max(hi[k], p[k]));
  const size = hi.map((x, k) => x - lo[k]);
  if (rules?.length_m) add("size.length", size[1], "m", rules.length_m, `its ${kind}'s length along +Y`);
  else if (rules?.size_m) add("size.extent", Math.max(...size), "m", rules.size_m, `its ${kind}'s longest extent`);
  else add("size.extent", Math.max(...size), "m", null, kind ? `a ${kind} has no range` : "its data names no kind");
  if (mount !== "in hand") return out;
  const at = handleAt(look);
  if (!at) return out;
  const head = headReach(look, at);
  add("size.head_fists", Math.max(head.x, head.z) / fist.breadth, "fists", null, `its far ${HEAD_SHARE * 100}% across, a fist ${r1(fist.breadth * 100)} cm`);
  add("grip.offset", Math.hypot(...at) * 100, "cm", GRIP_M * 100, "the grip off the handle's line");
  const butt = -lo[1];
  if (rules?.grip_from_end_m) add("grip.butt", butt * 100, "cm", rules.grip_from_end_m.map((x) => x * 100) as [number, number], "the grip from the butt");
  else if (rules?.grip_share) add("grip.share", butt / size[1], "of its length", rules.grip_share, "the grip from the butt");
  add("grip.past_little", (butt + fist.lo) * 100, "cm", null, "the handle past the little finger");
  const handle = handleIn(cut(look.points, look.tris));
  if (!handle) return out;
  add("grip.radius", handle.r * 100, "cm", HANDLE_R_CM, "the handle's radius at the grip");
  const top = handleTop(look, at, handle.r);
  if (kind === "blade" && Number.isFinite(top)) add("grip.past_thumb", (top - fist.hi) * 100, "cm", null, "the handle past the thumb's side before it widens");
  return out;
}

/** A row of moves.ts --json, cut to one look's numbers. */
export type Row = { move?: string; clip: string; measured: Record<string, number>; where?: Record<string, string> };

// moves.ts's keys for a look, the measure each is, its bar (null: a reading) and unit.
const KEYS: [string, string, number | null, string, boolean][] = [
  ["held_cm", "trunk", HELD_CM, "cm", false],
  ["held_legs_cm", "legs", HELD_CM, "cm", false],
  ["hold_ground_cm", "ground", GROUND_CM, "cm", false],
  ["hold_wrist_flex_deg", "wrist_flex", WRIST.flex, "deg", false],
  ["hold_wrist_ext_deg", "wrist_ext", WRIST.ext, "deg", false],
  ["hold_wrist_radial_deg", "wrist_radial", WRIST.radial, "deg", false],
  ["hold_wrist_ulnar_deg", "wrist_ulnar", WRIST.ulnar, "deg", false],
  ["hold_below_deg", "below", null, "deg", true],
  ["hold_above_deg", "above", null, "deg", true],
  ["hold_out_deg", "out", null, "deg", true],
  ["hold_elbow_deg", "elbow", null, "deg", true],
];

// moves.ts's row "fist" (the worst of every clip it played, with where): the fingers off the
// handle (grip.ts fistOff, the same in every pose: the fist is closed once a body), and how far
// above level the carried look's working end rises (carried_rise_deg, the carried clips alone;
// moves.ts writes it only for a pose not marked upright) or, of an upright pose, how far below
// level it falls (carried_fall_deg: a reading, no bar proven by a planted fault yet, D632).
const FIST: [string, string, number | null, string][] = [
  ["carried_rise_deg", "carry.rise", RISE_DEG, "deg"],
  ["carried_fall_deg", "carry.fall", null, "deg"],
  ["fist_cm", "grip.fist_off", FIST_CM, "cm"],
];

// moves.ts's stance keys (hold_pose.ts `stancePose`) in the standing idle, `arm` the arm that
// carries the look and `free` the other: the upper arm out from the trunk and the gap between
// the forearm or palm and the trunk. An upright pose's own arm (a torch held up) is read, not
// barred: it is meant to be away from the side.
const STAND: [string, string, number | null, string, boolean][] = [
  ["stand_arm_out_deg", "arm_out", ARM_OUT_DEG, "deg", true],
  ["stand_arm_gap_cm", "arm_gap", ARM_GAP_CM, "cm", true],
  ["stand_arm_bend_deg", "arm_bend", null, "deg", true],
  ["stand_free_out_deg", "free_out", ARM_OUT_DEG, "deg", false],
  ["stand_free_gap_cm", "free_gap", ARM_GAP_CM, "cm", false],
  ["stand_free_bend_deg", "free_bend", null, "deg", false],
];

/** A body's rows judged for one look: each of its numbers a measure, `carry.` in a clip the
 * game carries it in, `use.` in any other clip or move, `stand.` in the standing idle; `hangs`
 * whether its carry pose hangs at the side (not one marked upright). */
export function judged(look: string, body: string, rows: Row[], carried: string[], hangs = true): Measure[] {
  const out: Measure[] = [];
  for (const row of rows) {
    const clip = row.move ?? row.clip;
    if (!row.move && row.clip === STANDING && carried.includes(STANDING))
      for (const [k, measure, bar, unit, own] of STAND) {
        const value = row.measured[`${k}.${look}`];
        if (value === undefined) continue;
        const where = row.where?.[`${k}.${look}`] ?? "";
        const t = / at ([\d.]+) s/.exec(where);
        const held = own && !hangs ? null : bar;
        out.push({ look, measure: `stand.${measure}`, body, clip, time: t ? Number(t[1]) : null, value, bar: held, unit, ok: held === null ? null : value <= held, where });
      }
    if (!row.move && row.clip === "fist") {
      for (const [k, measure, bar, unit] of FIST) {
        const value = row.measured[`${k}.${look}`];
        if (value === undefined) continue;
        const where = row.where?.[`${k}.${look}`] ?? "";
        const at = /(\w+) at ([\d.]+) s/.exec(where);
        out.push({ look, measure, body, clip: at?.[1] ?? "-", time: at ? Number(at[2]) : null, value, bar, unit, ok: bar === null ? null : value <= bar, where });
      }
      continue;
    }
    const carry = !row.move && carried.includes(row.clip);
    for (const [k, measure, bar, unit, carriedOnly] of KEYS) {
      const value = row.measured[`${k}.${look}`];
      if (value === undefined || (carriedOnly && !carry)) continue;
      const where = row.where?.[`${k}.${look}`] ?? "";
      const t = / at ([\d.]+) s/.exec(where);
      // A use clip may strike the ground (a pickaxe's work): there the ground is read, not barred.
      const held = measure === "ground" && !carry ? null : bar;
      out.push({ look, measure: `${carry ? "carry" : "use"}.${measure}`, body, clip, time: t ? Number(t[1]) : null, value, bar: held, unit, ok: held === null ? null : value <= held, where });
    }
  }
  return out;
}

/** What to change for a measure over its bar (D556). */
export function fixFor(measure: string): string {
  const [group, what] = measure.split(".");
  const refit = "then node tools/characters/fit.ts ITEM";
  if (group === "stand") return "that arm stands out from the side as the body idles: bring its carry pose in (the pose named beside the look, art/animation/sets/fight.json: the upper arm and forearm aims nearer straight down, then build_bodies.sh --step fight), or, a free arm, see what turns it out (terra_walker::arms keep_off_hips); never raise ARM_OUT_DEG or ARM_GAP_CM";
  if (group === "size") return `make it life size in art/held/ITEM.json (its length), ${refit}; never widen attach_kinds.json's range for one look`;
  if (what === "radius") return `thin the handle in its model (the source art/held/ITEM.json names), ${refit}`;
  if (what === "fist_off") return `its handle is thinner or thicker than the closed fist lies on (grip.radius; the knuckle bone named is that far off its surface): seat it nearer the fingers (terra_walker::grip::seat and grip.ts seatOf, both) or change the handle's section in its model, ${refit}; never raise grip.ts's FIST_CM`;
  if (what === "rise") return "its working end tips above level in that clip: tip its carry pose down (hold.py's aim), or have the arm's give lift the hand without turning the forearm up (terra_walker::carry_arm and carry_arm.ts, both); never raise RISE_DEG";
  if (group === "grip") return `move its grip in art/held/ITEM.json (grip_from_end), ${refit}`;
  if (what.startsWith("wrist")) return group === "carry" ? "re-aim the arm of its carry pose (named beside the look; hold.py's aim) so the forearm, not the wrist, points the look; never widen wrist.ts's limits" : "the clip bends the wrist whatever is held: remake its keys (clip.ts); never widen wrist.ts's limits";
  if (what === "ground") return "raise or level its carry pose in that clip, or let the arm give as the body crouches";
  return `the point named (cm from its grip, +Y its handle) is in that part: change its carry pose or the clip, or refit the look (art/held/ITEM.json), ${refit}`;
}

const f = (m: Measure) => `${r1(m.value)} ${m.unit}`;
const barOf = (m: Measure) => (Array.isArray(m.bar) ? `${m.bar[0]} to ${m.bar[1]}` : `${m.bar}`);
const worst = (ms: Measure[]) => ms.reduce((a, b) => (b.value > a.value ? b : a));

/** What a passing look's line reads: its size and grip, then the worst of each carry measure. */
const READ = ["size.length", "size.extent", "size.head_fists", "grip.radius", "grip.past_little", "grip.past_thumb", "grip.fist_off", "carry.rise", "carry.fall", "carry.below", "carry.above", "carry.wrist_ulnar", "carry.wrist_ext", "carry.trunk", "carry.legs", "carry.ground", "stand.arm_out", "stand.arm_gap", "stand.free_out", "stand.free_gap", "use.trunk", "use.legs"];

/** The report's line for one look, `pose` its carry pose: what it reads if every bar holds, else
 * only the measures over their bars, each one's worst case (value, bar, body, clip, time, and
 * for a clearance the look's point and the part it is in) with how many rows and bodies fail
 * it; and which measures failed, for `fixFor`. */
export function lineFor(look: string, ms: Measure[], pose?: string): { ok: boolean; line: string; failed: string[] } {
  const by = new Map<string, Measure[]>();
  for (const m of ms) by.set(m.measure, [...(by.get(m.measure) ?? []), m]);
  const failing = [...by].filter(([, v]) => v.some((m) => m.ok === false)).sort(([a], [b]) => (a < b ? -1 : 1));
  const name = `${look}${pose ? ` (${pose})` : ""}`;
  if (!failing.length) {
    const parts = READ.filter((k) => by.has(k)).map((k) => `${/^(size|grip)/.test(k) ? k.split(".")[1] : k} ${f(worst(by.get(k)!))}`);
    return { ok: true, line: `✓ ${name}: ${parts.join(", ")}; ${new Set(ms.map((m) => m.body).filter((b) => b !== "-")).size} bodies`, failed: [] };
  }
  const parts = failing.map(([measure, all]) => {
    const bad = all.filter((m) => m.ok === false);
    const w = Array.isArray(all[0].bar) ? bad[0] : worst(bad);
    if (w.clip === "-") return `${measure} ${f(w)} (bar ${barOf(w)})`;
    const point = /^its point (\([^)]*\)) cm in (\w+)/.exec(w.where);
    return `${measure} ${f(w)} (bar ${barOf(w)}, ${w.body}, ${w.clip}${w.time === null ? "" : ` ${w.time.toFixed(2)} s`}${point ? `, its point ${point[1]} in ${point[2]}` : ""}, ${bad.length} rows on ${new Set(bad.map((m) => m.body)).size} bodies)`;
  });
  return { ok: false, line: `✗ ${name}: ${parts.join("; ")}`, failed: failing.map(([m]) => m) };
}

/** The bodies a look is measured on: every template, then the unmorphed base. */
export const bodies = (): string[] => [...Object.keys(JSON.parse(readFileSync(join(ROOT, "assets", "models", "character", "templates.json"), "utf8"))), BASE];

/** `tune` and `clips` are solve.ts's: hold parameters tried for the run (tune.ts) and, in place
 * of a look's set and every clip, these clips alone with the look alone in its hand. */
export type Ctx = { cache?: Cache; log: string; out: string; tune?: Record<string, number>; clips?: string[] };

/** moves.ts's rows for a look's file on one body, cut to the look's own numbers: through its
 * class's set with the set's partner, or the walker's use clips, and every carried clip. */
export async function rowsOf(file: string, body: string, ctx: Ctx): Promise<Row[]> {
  const name = basename(file, ".glb");
  const of = setOf(name);
  const { set, hand, clips } = ctx.clips ? { set: "none", hand: of.hand, clips: ctx.clips.join(",") } : of;
  const hold = set === "none" ? [`socket.hand.${hand}=${file}`] : (["R", "L"] as const).map((h) => `socket.hand.${h}=${h === hand ? file : PARTNER[h]}`);
  const tuned = Object.keys(ctx.tune ?? {}).sort();
  const tune = tuned.length ? ["--tune", JSON.stringify(Object.fromEntries(tuned.map((k) => [k, ctx.tune![k]])))] : [];
  const args = ["--set", set, "--carry", "--arms", ...hold.flatMap((h) => ["--hold", h]), "--also", clips || carriedClips().join(","), ...tune];
  const scripts = closure(join(CHARS, "moves.ts"), ROOT);
  const reads = [...scripts, ...movesReads(scripts), ...hold.map((h) => join(ROOT, h.split("=")[1])), ...(set === "none" ? [] : [join(ROOT, set)])];
  // The look is in the key: a set's two partners run the same command, and each keeps only its own rows.
  const k = key("holds", ROOT, reads, [args, body, name]);
  const kept = ctx.cache?.get(k, ROOT);
  if (kept) return kept.data as Row[];
  const json = join(ctx.out, `holds-${name}-${body}${tune.length || ctx.clips ? `-${k.slice(0, 16)}` : ""}.json`);
  const rows = await runMoves([join(CHARS, "moves.ts"), ...args, "--template", body, "--json", json], json, ctx.log);
  if (typeof rows === "string") throw new Error(`${name} on ${body}: ${rows}`);
  const own = (o: Record<string, unknown> = {}) => Object.fromEntries(Object.entries(o).filter(([key]) => key.endsWith(`.${name}`)));
  const cut = (rows as Row[]).map((r) => ({ move: r.move, clip: r.clip, measured: own(r.measured), where: own(r.where) }) as Row).filter((r) => Object.keys(r.measured).length);
  // A look no row measured would read as a pass (D632).
  if (!cut.length) throw new Error(`${name} on ${body}: moves.ts measured nothing of it (no held_cm.${name} in its rows); see ${ctx.log}, and check the look hangs from a hand (art/held/${name}.json)`);
  ctx.cache?.put(k, ROOT, { ok: true, line: "", data: cut });
  return cut;
}

/** Every measure of one look's file on these bodies, sorted. */
export async function measured(file: string, on: string[], ctx: Ctx, limit: <T>(job: () => Promise<T>) => Promise<T> = (job) => job()): Promise<Measure[]> {
  const name = basename(file, ".glb");
  const look = readLook(file, `held/${name}`);
  const data = dataOf(look.name);
  const shape = shapeOf(look, kindOf(look, data), fittedAs(look, data), fistOf(new Glb(join(ROOT, "assets", "models", "character", "base.glb"))));
  const carried = carriedClips();
  // The arm's own stance is barred of a handle in the fist carried hanging; a basket's own
  // width holds its arm out, and that is read.
  const hangs = !carryUpright(resolve(ROOT, file)) && fittedAs(look, data) === "in hand";
  const runs = await Promise.all(on.map((body) => limit(() => rowsOf(file, body, ctx)).then((rows) => judged(name, body, rows, carried, hangs))));
  const order = (m: Measure) => `${m.measure} ${m.body} ${m.clip}`;
  return [...shape, ...runs.flat().sort((a, b) => (order(a) < order(b) ? -1 : order(a) > order(b) ? 1 : 0))];
}

/** At most `n` jobs at once. */
export function pool(n: number) {
  let free = n;
  const waiting: (() => void)[] = [];
  return async <T>(job: () => Promise<T>): Promise<T> => {
    if (free > 0) free--;
    else await new Promise<void>((go) => waiting.push(go));
    try {
      return await job();
    } finally {
      const next = waiting.shift();
      if (next) next();
      else free++;
    }
  };
}

if (import.meta.main) {
  const { values: opt, positionals } = parseArgs({ allowPositionals: true, options: { json: { type: "string" }, body: { type: "string" }, fresh: { type: "boolean", default: false }, tune: { type: "string" } } });
  // `--tune JSON`: tune.ts's parameters tried for this run, as solve.ts tries them.
  let tune: Record<string, number> | undefined;
  try {
    tune = opt.tune ? JSON.parse(opt.tune) : undefined;
  } catch (e) {
    console.error(`✗ holds: --tune is not JSON (${(e as Error).message}) (D556): give tune.ts's parameters as an object, e.g. --tune '{"clear.on":1}'`);
    process.exit(2);
  }
  const all = readdirSync(HELD).filter((f) => f.endsWith(".glb")).map((f) => f.slice(0, -4)).sort();
  const unknown = positionals.filter((p) => !all.includes(p));
  if (unknown.length) {
    console.error(`✗ holds: no held look ${unknown.join(", ")} in assets/models/held (D556): name any of ${all.join(", ")}, or none for all of them`);
    process.exit(2);
  }
  const names = positionals.length ? [...new Set(positionals)].sort() : all;
  mkdirSync(OUT, { recursive: true });
  const log = join(OUT, "holds.log");
  writeFileSync(log, "");
  const ctx: Ctx = { cache: opt.fresh ? undefined : new Cache(join(OUT, "holds-cache")), log, out: OUT, tune };
  const limit = pool(availableParallelism());
  const on = opt.body ? opt.body.split(",") : bodies();
  try {
    const table = await Promise.all(names.map((n) => measured(join("assets", "models", "held", `${n}.glb`), on, ctx, limit)));
    let failed = 0;
    const fixes = new Set<string>();
    names.forEach((n, i) => {
      const { ok, line, failed: which } = lineFor(n, table[i], carryHold(join(HELD, `${n}.glb`)));
      if (!ok) failed++;
      console.log(line);
      for (const m of which) fixes.add(m.replace(/wrist_\w+/, "wrist"));
      for (const m of table[i]) appendFileSync(log, `${JSON.stringify(m)}\n`);
    });
    for (const m of [...fixes].sort()) console.log(`  to fix ${m}: ${fixFor(m)}`);
    if (opt.json) writeFileSync(opt.json, JSON.stringify(table.flat(), null, 1) + "\n");
    console.log(`${failed ? "✗" : "✓"} holds: ${names.length} looks on ${on.length} bodies, ${failed} over a bar; every measure in tools/characters/out/holds.log`);
    process.exit(failed ? 1 : 0);
  } catch (e) {
    console.error(`✗ holds: ${(e as Error).message}`);
    process.exit(2);
  }
}
