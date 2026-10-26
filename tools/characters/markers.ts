#!/usr/bin/env node
// Bakes each clip's foot-down markers into its file (D455): for every clip, each foot's
// planted phases, the spans its ball of the foot stands on the ground the clip strides on,
// named as Unreal's sync markers are (`foot_down_l`, `foot_down_r`), so the game keeps a foot
// where it lands by the clip's own data rather than one height for every clip (a fighter's
// stance stands its ball 3-4 cm up, a jog's rolling foot 5.6-6.5 cm). ALS- and Lyra-era rigs
// bake the same per clip (foot-lock curves, foot-down markers); the markers will serve sync
// groups too.
//
// A foot is down where its ball is within LOW_M of its own lowest in the clip and moves over
// the clip's ground slower than STILL_MPS: the ground the clip's feet stand on, which for a
// stride played in place passes under the body as its planted feet go back (the middle of the
// foot's low ball's velocities, as pace.py measures a pace), and for a clip that stands is the
// body.
// A foot whose lowest is over HIGHEST_M (a jump's tuck) is never down. Spans shorter than
// SHORTEST_S are dropped; a loop's span over its end runs on into its start, `[down, up]`
// with `up < down`.
//
// Written into the glTF extras beside the paces (`{"markers": {"Jog_Fwd_Loop": {"length_s":
// 0.75, "foot_down_l": [[0.68, 0.12]], "foot_down_r": [[0.3, 0.5]]}}}`), the game reads them
// as it reads the paces (`terra_walker::feet::markers`). Run after a body file is exported
// (build_bodies.sh, clip.ts).
//
//   node tools/characters/markers.ts assets/models/character/base.glb assets/models/character/fight.glb
//   node tools/characters/markers.ts FILE.glb... [--print] [--check]
//
// --print lists each clip's spans; --check writes nothing and fails if a file's markers are
// not what it bakes now. Node 22.18 or later; no dependencies.

import { readFileSync, writeFileSync } from "node:fs";
import { Glb, posed, type V3 } from "./glb.ts";

/** A ball this near its lowest in the clip may be down. */
export const LOW_M = 0.025;
/** A ball moving slower than this over the clip's ground is down. */
export const STILL_MPS = 0.8;
/** A foot whose lowest is higher than this over the body's ground is never down. */
export const HIGHEST_M = 0.12;
/** A down span shorter than this is a foot passing low, not a plant. */
export const SHORTEST_S = 0.05;
/** Low balls moving slower than this together stand still: the clip does not travel. */
const TRAVELS_MPS = 0.3;
/** A clip loops when each ball is this near where it started at its end. */
const LOOP_M = 0.01;
const FPS = 60;
/** A frame's velocity is over this many frames either side. */
const SPREAD = 2;
export const FEET = ["ball_l", "ball_r"] as const;
export const NAMES = ["foot_down_l", "foot_down_r"] as const;

export type Span = [number, number];
export type Markers = { length_s: number; foot_down_l: Span[]; foot_down_r: Span[] };

const round = (x: number) => Math.round(x * 1000) / 1000;

/** One foot's down spans from its ball's path sampled at `fps` (y up, the body's ground at 0),
 * the clip's `ground` velocity (x, z), and whether the clip loops. */
export function spans(path: V3[], fps: number, ground: [number, number], loops: boolean): Span[] {
  const n = path.length;
  if (n < 2) return [];
  const lowest = Math.min(...path.map((p) => p[1]));
  if (lowest > HIGHEST_M) return [];
  // A frame's velocity over SPREAD frames either side (a clip keyed at 24 or 30 a second and
  // sampled at 60 jerks frame to frame); a loop's first and last frames are one pose, so it
  // wraps past them, and a clip played once stops at its ends.
  const at = (i: number) => (loops ? path[(((i % (n - 1)) + n - 1) % (n - 1))] : path[Math.min(n - 1, Math.max(0, i))]);
  const down = path.map((p, i) => {
    const [was, now] = [at(i - SPREAD), at(i + SPREAD)];
    const span = loops ? 2 * SPREAD : Math.min(n - 1, i + SPREAD) - Math.max(0, i - SPREAD);
    const v = [((now[0] - was[0]) * fps) / span - ground[0], ((now[2] - was[2]) * fps) / span - ground[1]];
    return p[1] <= lowest + LOW_M && Math.hypot(v[0], v[1]) < STILL_MPS;
  });
  // Runs of down frames, as frame ranges [first, last].
  const runs: [number, number][] = [];
  for (let i = 0; i < n; i++) {
    if (!down[i]) continue;
    if (runs.length && runs.at(-1)![1] === i - 1) runs.at(-1)![1] = i;
    else runs.push([i, i]);
  }
  // A loop's last frame is its first: a run to its end goes on into its start.
  if (loops && runs.length > 1 && runs[0][0] === 0 && runs.at(-1)![1] >= n - 2) {
    const last = runs.pop()!;
    runs[0] = [last[0], runs[0][1] + n - 1];
  } else if (loops && runs.length === 1 && runs[0][0] === 0 && runs[0][1] >= n - 2) {
    return [[0, round((n - 1) / fps)]];
  }
  const length = (n - 1) / fps;
  return runs
    .filter(([a, b]) => (b - a) / fps >= SHORTEST_S)
    .map(([a, b]): Span => [round(a / fps), round((b / fps) % (length + 1e-9))]);
}

/** The middle of a low ball's velocities (x, z), the ground a stride played in place stands
 * it on; zero for a clip that does not travel. A foot's own, as a made stride's feet may go
 * back a little faster or slower than each other: its plant is where it goes back steadily. */
export function groundOf(path: V3[], fps: number): [number, number] {
  const vs: [number, number][] = [];
  const lowest = Math.min(...path.map((p) => p[1]));
  for (let i = 1; i < path.length; i++)
    if (path[i][1] <= lowest + 0.05) vs.push([(path[i][0] - path[i - 1][0]) * fps, (path[i][2] - path[i - 1][2]) * fps]);
  if (!vs.length) return [0, 0];
  const mid = (xs: number[]) => xs.sort((a, b) => a - b)[Math.floor(xs.length / 2)];
  const g: [number, number] = [mid(vs.map((v) => v[0])), mid(vs.map((v) => v[1]))];
  return Math.hypot(g[0], g[1]) < TRAVELS_MPS ? [0, 0] : g;
}

/** Every clip's markers in a body file. */
export function bake(body: Glb): Record<string, Markers> {
  const balls = FEET.map((name) => body.named(name));
  if (balls.some((b) => b < 0)) return {};
  const out: Record<string, Markers> = {};
  for (const name of body.clipNames()) {
    const clip = body.clip(name)!;
    const n = Math.max(1, Math.round(clip.end * FPS));
    const paths: V3[][] = [[], []];
    for (let i = 0; i <= n; i++) {
      const pose = posed(body, clip, (i / n) * clip.end);
      balls.forEach((b, side) => paths[side].push(pose[b].t));
    }
    const fps = n / Math.max(clip.end, 1e-6);
    const loops = paths.every((path) => Math.hypot(...path[0].map((x, k) => x - path.at(-1)![k])) < LOOP_M);
    const [l, r] = paths.map((path) => spans(path, fps, groundOf(path, fps), loops));
    out[name] = { length_s: round(clip.end), foot_down_l: l, foot_down_r: r };
  }
  return out;
}

/** `file` with `markers` written into the extras of the node that carries its paces (else its
 * scene's first node): its JSON chunk rewritten, its binary chunk as it was. */
export function written(file: string, markers: Record<string, Markers>): Buffer {
  const bytes = readFileSync(file);
  const jsonLen = bytes.readUInt32LE(12);
  const gltf = JSON.parse(bytes.subarray(20, 20 + jsonLen).toString());
  const carrier =
    gltf.nodes.find((n: { extras?: { paces?: unknown } }) => n.extras?.paces) ?? gltf.nodes[gltf.scenes?.[gltf.scene ?? 0]?.nodes?.[0] ?? 0];
  carrier.extras = { ...(carrier.extras ?? {}), markers };
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

function show(clip: string, m: Markers) {
  const each = NAMES.map((k) => `${k.slice(-1)} ${m[k].map(([a, b]) => `${a.toFixed(2)}-${b.toFixed(2)}`).join(" ") || "-"}`);
  console.log(`  ${clip.padEnd(22)} ${m.length_s.toFixed(2)} s  ${each.join("  ")}`);
}

/** Bakes `file`'s markers into it (`write`) or only compares: its clips' count, and whether
 * the markers it carries were already what it bakes. */
export function bakeFile(file: string, write = true, print = false): { count: number; same: boolean } {
  const body = new Glb(file);
  const markers = bake(body);
  if (print) for (const [clip, m] of Object.entries(markers)) show(clip, m);
  const had = body.gltf.nodes.map((n) => (n as { extras?: { markers?: unknown } }).extras?.markers).find(Boolean);
  const same = JSON.stringify(had) === JSON.stringify(markers);
  if (write && !same) writeFileSync(file, written(file, markers));
  return { count: Object.keys(markers).length, same };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const args = process.argv.slice(2);
  const files = args.filter((a) => !a.startsWith("--"));
  if (!files.length || args.includes("--help")) {
    console.log("markers.ts FILE.glb... [--print] [--check]");
    process.exit(files.length ? 0 : 1);
  }
  const check = args.includes("--check");
  let stale = 0;
  for (const file of files) {
    const { count, same } = bakeFile(file, !check, args.includes("--print"));
    if (check) {
      console.log(`${same ? "✓" : "✗"} ${file}: ${count} clips${same ? "" : ", markers stale: run markers.ts on it"}`);
      stale += same ? 0 : 1;
    } else console.log(`markers: ${file}, ${count} clips ${same ? "as baked" : "baked"}`);
  }
  process.exit(stale ? 1 : 0);
}
