#!/usr/bin/env node
// Bakes each clip's foot locks into its file (D455): for every clip, each foot's lock curve,
// 0 to 1 through the clip (1 while the foot stands on the ground the clip strides on, falling
// to 0 over RAMP_S as it leaves), and its plants as
// spans, named as Unreal's sync markers are (`foot_down_l`, `foot_down_r`). The game keeps a
// foot where it lands by the clip's own curve, as ALS's FootLock curves and motion matching's
// contact labels do, rather than one height for every clip.
//
// Contact is the foot's speed over the clip's ground, as motion-matching databases label it:
// the heel (the foot bone) or the toe tip (`ball_leaf`) moving slower than STILL_MPS over the
// ground the clip passes, which for a stride played in place goes back under the body at the
// clip's ONE pace (the game moves the ground at a single pace: `paces`, along the way its low
// feet go back), and for a clip that stands is the body. Height is only a loose gate, LOOSE_M
// over that point's own lowest in the clip: a toe-off rolls the ball up 2.5-5 cm while it is
// nearly still. A foot whose toe tip's lowest is over HIGHEST_M (a jump's tuck) never stands;
// plants shorter than SHORTEST_S are dropped; a loop's plant over its end runs on into its
// start (`[down, up]` with `up < down`).
//
// Each plant's skate (how far its standing point goes over the clip's ground from where it
// landed) is reported. A plant that skates past SKATE_M is split there (it lands again), as a
// lock held on would pull the foot across the ground from the clip's; a stride (a clip with a
// pace) that skates so is a fault, and so is a walk (a pace under WALK_MPS) with a moment
// neither foot stands: a walk always has one down. Faults are printed (`✗`) and fail --check.
//
// Written into the glTF extras beside the paces (`{"markers": {"Jog_Fwd_Loop": {"length_s":
// 0.75, "rate": 30, "lock_l": [1, 1, ...], "lock_r": [...], "foot_down_l": [[0.68, 0.12]],
// "foot_down_r": [[0.3, 0.5]]}}}`), the game reads them as it reads the paces
// (`terra_walker::feet::markers`). Run after a body file is exported (build_bodies.sh, clip.ts).
//
// A clip that keeps its root motion (a start, a stop: made.json's `travels`; its pelvis goes
// over TRAVEL_M along the ground) also gets its distance curve, as Lyra's distance-matched
// starts and stops carry one (UE5's Distance Curve modifier, its documentation): `distance`,
// the pelvis's travel along the way it goes from its first frame at RATE (m, the file's units,
// not made monotone: the game looks it up as it does), and `distance_way`, that way in the
// pelvis's parent's space, so the game takes the travel off the pelvis (Unreal's root lock)
// and moves the body instead.
//
// A blow played in place whose root motion is kept apart (`--roots=RM.glb`, a pack's _RM clips
// of the same names, as the move's `drive` is made from them by root_drive.ts) is judged on the
// ground that root motion passes under it, frame by frame, so a lunge's planted foot stands while
// the body goes ahead; it loses its pace, as it travels by its drive and not at the body's speed.
//
//   node tools/characters/markers.ts assets/models/character/base.glb assets/models/character/fight.glb
//   node tools/characters/markers.ts FILE.glb... [--roots=RM.glb] [--print] [--check]
//
// --print lists each clip's plants and skate; --check writes nothing and fails if a file's
// markers are not what it bakes now. Either fails a clip that skates. Node 22.18 or later; no
// dependencies.

import { readFileSync, writeFileSync } from "node:fs";
import { isAdditive } from "./additive.ts";
import { scaleAgainst } from "./root_drive.ts";
import { dot, Glb, type Pose, posed, qinv, rot, type V3 } from "./glb.ts";

/** A point moving slower than this over the clip's ground stands on it. */
export const STILL_MPS = 0.5;
/** ...or than this share of the clip's pace, if more. */
export const STILL_SHARE = 0.2;
/** A point this near its own lowest in the clip may stand: the loose height gate. */
export const LOOSE_M = 0.07;
/** A foot whose toe tip's lowest is higher than this over the body's ground never stands. */
export const HIGHEST_M = 0.12;
/** A plant starts this far back from where the foot goes still while it is this near its lowest. */
export const TOUCH_S = 0.15;
export const TOUCH_M = 0.04;
/** A plant shorter than this is a foot passing low, not a plant. */
export const SHORTEST_S = 0.05;
/** The lock curve falls from 1 to 0 over this long once a plant ends. */
export const RAMP_S = 0.1;
/** A plant whose standing point goes further than this over the clip's ground fails the clip. */
export const SKATE_M = 0.06;
/** A stride slower than this is a walk: one foot or the other is always down. */
export const WALK_MPS = 2.0;
/** Lock curve samples a second. */
export const RATE = 60;
/** Low feet going back slower than this stand still: the clip does not travel. */
const TRAVELS_MPS = 0.3;
/** A clip whose pelvis goes this far along the ground keeps its root motion: it has a distance
 * curve. A stride played in place goes nowhere; a one-shot's drift is under it. */
export const TRAVEL_M = 0.5;
/** A clip loops when each ball is this near where it started at its end. */
const LOOP_M = 0.01;
const FPS = 60;
/** A frame's velocity is over this many frames either side. */
const SPREAD = 2;
/** Each foot's heel and toe tip, left then right. */
export const FEET = [
  ["foot_l", "ball_leaf_l"],
  ["foot_r", "ball_leaf_r"],
] as const;
export const NAMES = ["foot_down_l", "foot_down_r"] as const;

export type Span = [number, number];
export type Markers = {
  length_s: number;
  rate: number;
  lock_l: number[];
  lock_r: number[];
  foot_down_l: Span[];
  foot_down_r: Span[];
  distance?: number[];
  distance_way?: V3;
};
/** One foot's bake: its curve at FPS, its plants as frame runs, its worst plant's skate before
 * a plant was split, and how many it was split. */
export type Foot = { lock: number[]; runs: [number, number][]; skate_m: number; splits: number };

const round = (x: number, by = 1000) => Math.round(x * by) / by;

/** A point's velocity (x, z) at every frame over SPREAD frames either side: a loop's first and
 * last frames are one pose, so it wraps past them; a clip played once stops at its ends. */
function velocities(path: V3[], fps: number, loops: boolean): [number, number][] {
  const n = path.length;
  const at = (i: number) => (loops ? path[(((i % (n - 1)) + n - 1) % (n - 1))] : path[Math.min(n - 1, Math.max(0, i))]);
  return path.map((_, i) => {
    const [was, now] = [at(i - SPREAD), at(i + SPREAD)];
    const span = loops ? 2 * SPREAD : Math.min(n - 1, i + SPREAD) - Math.max(0, i - SPREAD);
    return [((now[0] - was[0]) * fps) / span, ((now[2] - was[2]) * fps) / span];
  });
}

/** One foot from its heel's and toe tip's paths sampled at `fps` (y up, the body's ground at
 * 0), the clip's `ground` velocity (x, z), one for the clip or one a frame (a clip driven by its
 * root motion), and whether the clip loops. */
export function foot(heel: V3[], toe: V3[], fps: number, ground: [number, number] | [number, number][], loops: boolean): Foot {
  const n = toe.length;
  const under = (i: number): [number, number] => (typeof ground[0] === "number" ? (ground as [number, number]) : (ground as [number, number][])[i]);
  const none: Foot = { lock: new Array(n).fill(0), runs: [], skate_m: 0, splits: 0 };
  if (n < 2) return none;
  const lowest = (path: V3[]) => Math.min(...path.map((p) => p[1]));
  if (lowest(toe) > HIGHEST_M) return none;
  // Each frame, the velocity over the clip's ground of the slower of the points that may stand
  // (none where neither may).
  const points = [heel, toe].map((path) => ({ path, low: lowest(path) + LOOSE_M, v: velocities(path, fps, loops) }));
  const slipping = toe.map((_, i) => {
    const each = points.filter(({ path, low }) => path[i][1] <= low).map(({ v }): [number, number] => [v[i][0] - under(i)[0], v[i][1] - under(i)[1]]);
    return each.sort((a, b) => Math.hypot(...a) - Math.hypot(...b))[0];
  });
  const slip = slipping.map((v) => (v ? Math.hypot(...v) : Infinity));
  // How far a plant's standing point goes over the ground at most from where it landed.
  const slid = (frames: number[]) => {
    let [x, z, most] = [0, 0, 0];
    for (const i of frames) {
      x += slipping[i]![0] / fps;
      z += slipping[i]![1] / fps;
      most = Math.max(most, Math.hypot(x, z));
    }
    return most;
  };
  // Faster strides land and roll off with more give: the threshold grows with the pace.
  const down = slip.map((s, i) => s < Math.max(STILL_MPS, STILL_SHARE * Math.hypot(...under(i))));
  // Runs of down frames, as frame ranges [first, last] (last may pass n - 1 over a loop's end).
  let runs: [number, number][] = [];
  for (let i = 0; i < n; i++) {
    if (!down[i]) continue;
    if (runs.length && runs.at(-1)![1] === i - 1) runs.at(-1)![1] = i;
    else runs.push([i, i]);
  }
  // A loop's last frame is its first: a run to its end goes on into its start.
  if (loops && runs.length === 1 && runs[0][0] === 0 && runs[0][1] >= n - 2) {
    return { lock: new Array(n).fill(1), runs: [[0, n - 1]], skate_m: round(slid([...Array(n - 1).keys()])), splits: 0 };
  }
  if (loops && runs.length > 1 && runs[0][0] === 0 && runs.at(-1)![1] >= n - 2) {
    const last = runs.pop()!;
    runs[0] = [last[0], runs[0][1] + n - 1];
  }
  runs = runs.filter(([a, b]) => (b - a) / fps >= SHORTEST_S);
  const wrap = (i: number) => (loops ? (((i % (n - 1)) + n - 1) % (n - 1)) : i);
  // A plant whose standing point has gone SKATE_M from where it landed ends there and lands
  // again: a lock held on would pull the foot across the ground from the clip's.
  let skate = 0;
  let splits = 0;
  const plants: [number, number][] = [];
  for (const [a, b] of runs) {
    skate = Math.max(skate, slid([...Array(b - a + 1).keys()].map((k) => wrap(a + k))));
    let [x, z, from] = [0, 0, a];
    for (let i = a; i <= b; i++) {
      x += slipping[wrap(i)]![0] / fps;
      z += slipping[wrap(i)]![1] / fps;
      if (Math.hypot(x, z) > SKATE_M && i > from) {
        plants.push([from, i - 1]);
        [x, z, from] = [0, 0, i];
        splits++;
      }
    }
    plants.push([from, b]);
  }
  // A plant starts at touchdown: back from where the foot goes still, while its heel or toe
  // tip is within TOUCH_M of its lowest (a heel strike skims in before it stops), TOUCH_S at
  // most and never into the plant before, so the game locks it where it lands.
  const touching = (i: number) => heel[wrap(i)][1] <= lowest(heel) + TOUCH_M || toe[wrap(i)][1] <= lowest(toe) + TOUCH_M;
  for (let k = 0; k < plants.length; k++) {
    const [a, b] = plants[k];
    const before = k > 0 ? plants[k - 1][1] : loops ? plants.at(-1)![1] - (n - 1) : -1;
    let from = a;
    while (from - 1 > before + 1 && a - (from - 1) <= TOUCH_S * fps && (loops || from - 1 >= 0) && touching(from - 1)) from--;
    plants[k] = from < 0 ? [from + n - 1, b + n - 1] : [from, b];
  }
  const lock = new Array(n).fill(0);
  for (const [a, b] of plants) {
    for (let i = a; i <= b; i++) lock[wrap(i)] = 1;
    // Falling to 0 over RAMP_S as the foot leaves, so the game eases it from the lock to the
    // clip while it lifts rather than snapping it there (or sliding it while on the ground).
    const ramp = RAMP_S * fps;
    for (let k = 1; k < ramp && (loops || b + k < n); k++) lock[wrap(b + k)] = Math.max(lock[wrap(b + k)], 1 - k / ramp);
  }
  if (loops) lock[n - 1] = lock[0];
  return { lock, runs: plants, skate_m: round(skate), splits };
}

/** The ground a stride played in place stands its feet on (x, z): the middle of its low toe
 * tips' velocities, both feet's together, scaled to the clip's one pace where it has one, so
 * the feet are judged on the ground the game passes under them; zero for a clip that does not
 * travel. */
export function groundOf(toes: V3[][], fps: number, pace?: number): [number, number] {
  const vs: [number, number][] = [];
  for (const path of toes) {
    const lowest = Math.min(...path.map((p) => p[1]));
    for (let i = 1; i < path.length; i++)
      if (path[i][1] <= lowest + 0.05) vs.push([(path[i][0] - path[i - 1][0]) * fps, (path[i][2] - path[i - 1][2]) * fps]);
  }
  if (!vs.length) return [0, 0];
  const mid = (xs: number[]) => xs.sort((a, b) => a - b)[Math.floor(xs.length / 2)];
  const g: [number, number] = [mid(vs.map((v) => v[0])), mid(vs.map((v) => v[1]))];
  const speed = Math.hypot(g[0], g[1]);
  if (speed < TRAVELS_MPS) return [0, 0];
  const by = pace === undefined ? 1 : pace / speed;
  return [g[0] * by, g[1] * by];
}

/** The ground under a clip played in place whose root motion is kept apart (a pack's _RM file,
 * `roots`, its clip of the same name with the travel on the node "root"), a velocity (x, z) at
 * each of `n + 1` frames over `end` seconds: the root's own, backward, as a planted foot of the
 * in-place clip goes back under the body while the game drives the body ahead by that motion
 * (Unreal's root motion from animation; the move's `drive`, root_drive.ts). In the body's
 * metres: the root's travel scaled as the body's skeleton is against the pack's. */
export function rootGround(body: Glb, roots: Glb, clip: string, n: number, end: number): [number, number][] {
  const root = roots.named("root");
  const c = roots.clip(clip)!;
  const by = scaleAgainst(body, roots);
  const path = Array.from({ length: n + 1 }, (_, i) => posed(roots, c, Math.min(c.end, (i / n) * end))[root].t.map((x) => x * by) as V3);
  return velocities(path, n / Math.max(end, 1e-6), false).map(([x, z]) => [-x, -z]);
}

/** The pelvis's travel along the ground (`path`, its places at `fps`, y up) for a clip that
 * goes over TRAVEL_M from its first frame to its last: along that way at RATE to `end`, and the
 * way in the pelvis's parent's space (`parent`, its pose at the first frame). */
export function distanceOf(path: V3[], fps: number, end: number, parent?: Pose): { distance: number[]; way: V3 } | undefined {
  const flat = (p: V3): V3 => [p[0] - path[0][0], 0, p[2] - path[0][2]];
  const went = flat(path.at(-1)!);
  const length = Math.hypot(went[0], went[2]);
  if (length < TRAVEL_M) return undefined;
  const ahead: V3 = [went[0] / length, 0, went[2] / length];
  const distance = Array.from({ length: Math.round(end * RATE) + 1 }, (_, i) => round(dot(flat(path[Math.min(path.length - 1, Math.round((i / RATE) * fps))]), ahead)));
  // A model metre in the parent's units: not normalised, so a scaled parent (the Rig's 1.08)
  // takes off as much travel as the clip put on.
  const local = parent ? rot(qinv(parent.r), ahead).map((x, k) => x / parent.s[k]) : ahead;
  return { distance, way: local.map((x) => round(x, 1e5)) as V3 };
}

/** A foot's curve resampled from FPS to RATE. */
const resampled = (lock: number[], fps: number, end: number) =>
  Array.from({ length: Math.round(end * RATE) + 1 }, (_, i) => round(lock[Math.min(lock.length - 1, Math.round((i / RATE) * fps))], 100));

export type Baked = { markers: Markers; skate_m: [number, number]; splits: number; neither_s: number; pace?: number; rooted?: boolean };

/** Every clip's bake in a body file: its markers, each foot's worst skate, its splits, how long
 * neither foot stands, and its pace. A clip `roots` has (its root motion kept apart, `rootGround`)
 * is judged on the ground that motion passes under it, and has no pace: it travels by its root
 * motion, not at the body's speed. */
export function bake(body: Glb, roots?: Glb): Record<string, Baked> {
  const points = FEET.map((pair) => pair.map((name) => body.named(name)));
  if (points.flat().some((b) => b < 0)) return {};
  const paces: Record<string, number> = (body.gltf.nodes.find((n: { extras?: { paces?: unknown } }) => n.extras?.paces) as { extras: { paces: Record<string, number> } } | undefined)?.extras.paces ?? {};
  const pelvis = body.named("pelvis");
  const out: ReturnType<typeof bake> = {};
  // An arm's additive swing (additive.ts) stands no foot: it rides on its loop.
  for (const name of body.clipNames().filter((name) => !isAdditive(name))) {
    const clip = body.clip(name)!;
    const n = Math.max(1, Math.round(clip.end * FPS));
    const paths: V3[][][] = points.map((pair) => pair.map(() => []));
    const hips: V3[] = [];
    let parent: Pose | undefined;
    for (let i = 0; i <= n; i++) {
      const pose = posed(body, clip, (i / n) * clip.end);
      points.forEach((pair, side) => pair.forEach((p, k) => paths[side][k].push(pose[p].t)));
      if (pelvis >= 0) hips.push(pose[pelvis].t);
      if (i === 0 && pelvis >= 0 && body.parentOf[pelvis] !== undefined) parent = pose[body.parentOf[pelvis]];
    }
    const fps = n / Math.max(clip.end, 1e-6);
    // A clip with its root motion apart is a one-shot that goes somewhere, never a loop.
    const loops = !roots?.clip(name) && paths.every(([, toe]) => Math.hypot(...toe[0].map((x, k) => x - toe.at(-1)![k])) < LOOP_M);
    const rooted = roots?.clip(name) !== undefined;
    const ground = rooted
      ? rootGround(body, roots!, name, n, clip.end)
      : groundOf(
          paths.map(([, toe]) => toe),
          fps,
          paces[name],
        );
    const [l, r] = paths.map(([heel, toe]) => foot(heel, toe, fps, ground, loops));
    const span = (runs: [number, number][]): Span[] => runs.map(([a, b]) => [round(a / fps), round((b > n - 1 ? b - (n - 1) : b) / fps)]);
    // How long neither foot stands: a walk always has one down.
    const neither = l.lock.filter((x, i) => x <= 0 && r.lock[i] <= 0).length - (loops && l.lock[0] <= 0 && r.lock[0] <= 0 ? 1 : 0);
    const travel = hips.length ? distanceOf(hips, fps, clip.end, parent) : undefined;
    out[name] = {
      markers: {
        length_s: round(clip.end),
        rate: RATE,
        lock_l: resampled(l.lock, fps, clip.end),
        lock_r: resampled(r.lock, fps, clip.end),
        foot_down_l: span(l.runs),
        foot_down_r: span(r.runs),
        ...(travel ? { distance: travel.distance, distance_way: travel.way } : {}),
      },
      skate_m: [l.skate_m, r.skate_m],
      splits: l.splits + r.splits,
      pace: rooted ? undefined : paces[name],
      rooted,
      neither_s: round(Math.max(0, neither) / fps),
    };
  }
  return out;
}

/** `file` with `markers` written into the extras of the node that carries its paces (else its
 * scene's first node), the paces of the clips `rooted` dropped (they travel by their root motion):
 * its JSON chunk rewritten, its binary chunk as it was. */
export function written(file: string, markers: Record<string, Markers>, rooted: string[] = []): Buffer {
  const bytes = readFileSync(file);
  const jsonLen = bytes.readUInt32LE(12);
  const gltf = JSON.parse(bytes.subarray(20, 20 + jsonLen).toString());
  const carrier =
    gltf.nodes.find((n: { extras?: { paces?: unknown } }) => n.extras?.paces) ?? gltf.nodes[gltf.scenes?.[gltf.scene ?? 0]?.nodes?.[0] ?? 0];
  carrier.extras = { ...(carrier.extras ?? {}), markers };
  for (const clip of rooted) delete carrier.extras.paces?.[clip];
  let json = Buffer.from(JSON.stringify(gltf));
  if (json.length % 4) json = Buffer.concat([json, Buffer.alloc(4 - (json.length % 4), 0x20)]);
  const rest = bytes.subarray(20 + jsonLen);
  const head = Buffer.alloc(20);
  head.write("glTF", 0);
  head.writeUInt32LE(2, 4);
  head.writeUInt32LE(20 + json.length + rest.length, 8);
  head.writeUInt32LE(json.length, 12);
  head.write("JSON", 16);
  return Buffer.concat([head, json, rest]);
}

function show(clip: string, b: Baked) {
  const m = b.markers;
  const each = NAMES.map((k, side) => `${k.slice(-1)} ${m[k].map(([a, z]) => `${a.toFixed(2)}-${z.toFixed(2)}`).join(" ") || "-"} skate ${(b.skate_m[side] * 100).toFixed(1)} cm`);
  const went = m.distance ? `  travels ${Math.max(...m.distance).toFixed(2)} m` : "";
  console.log(`  ${clip.padEnd(22)} ${m.length_s.toFixed(2)} s  ${each.join("  ")}  neither ${b.neither_s.toFixed(2)} s${b.splits ? `  split ${b.splits}` : ""}${went}`);
}

/** What fails a clip's bake: a stride that skates, a walk with neither foot down. */
export function faults(clip: string, b: Baked): string[] {
  if (b.pace === undefined) return [];
  const out: string[] = [];
  if (b.splits) out.push(`${clip} skates ${b.skate_m.map((s) => (s * 100).toFixed(1)).join("/")} cm a plant (over ${SKATE_M * 100})`);
  if (b.pace < WALK_MPS && b.neither_s > 0) out.push(`${clip} walks with neither foot down ${b.neither_s.toFixed(2)} s`);
  return out;
}

/** Bakes `file`'s markers into it (`write`) or only compares: its clips' count, whether the
 * markers it carries were already what it bakes, the clips split, and the faults. */
export function bakeFile(file: string, write = true, print = false, roots?: string): { count: number; same: boolean; split: string[]; faults: string[] } {
  const body = new Glb(file);
  const baked = bake(body, roots ? new Glb(roots) : undefined);
  const markers = Object.fromEntries(Object.entries(baked).map(([clip, b]) => [clip, b.markers]));
  const rooted = Object.keys(baked).filter((clip) => baked[clip].rooted);
  const paced = (body.gltf.nodes.find((n: { extras?: { paces?: unknown } }) => n.extras?.paces) as { extras: { paces: Record<string, number> } } | undefined)?.extras.paces ?? {};
  if (print) for (const [clip, b] of Object.entries(baked)) show(clip, b);
  const split = Object.entries(baked)
    .filter(([, b]) => b.splits)
    .map(([clip, b]) => `${clip} (${b.skate_m.map((s) => (s * 100).toFixed(1)).join("/")} cm)`);
  const had = body.gltf.nodes.map((n) => (n as { extras?: { markers?: unknown } }).extras?.markers).find(Boolean);
  const same = JSON.stringify(had) === JSON.stringify(markers) && !rooted.some((clip) => clip in paced);
  if (write && !same) writeFileSync(file, written(file, markers, rooted));
  return { count: Object.keys(markers).length, same, split, faults: Object.entries(baked).flatMap(([clip, b]) => faults(clip, b)) };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const args = process.argv.slice(2);
  const files = args.filter((a) => !a.startsWith("--"));
  if (!files.length || args.includes("--help")) {
    console.log("markers.ts FILE.glb... [--roots=RM.glb] [--print] [--check]");
    process.exit(files.length ? 0 : 1);
  }
  const check = args.includes("--check");
  const roots = args.find((a) => a.startsWith("--roots="))?.slice("--roots=".length);
  let failed = 0;
  for (const file of files) {
    const { count, same, split, faults } = bakeFile(file, !check, args.includes("--print"), roots);
    if (split.length) console.log(`  ${file}: ${split.length} clips' plants split where they skate over ${SKATE_M * 100} cm: ${split.join(", ")}`);
    for (const fault of faults) console.log(`✗ ${file}: ${fault}`);
    if (check) {
      console.log(`${same ? "✓" : "✗"} ${file}: ${count} clips${same ? "" : ", markers stale: run markers.ts on it"}`);
      failed += same ? 0 : 1;
    } else console.log(`markers: ${file}, ${count} clips ${same ? "as baked" : "baked"}`);
    if (check) failed += faults.length ? 1 : 0;
  }
  process.exit(failed ? 1 : 0);
}
