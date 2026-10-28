#!/usr/bin/env node
// The swing foot's clearance: a made clip's foot swinging between its plants lifted, by leg IK,
// to at least a bar over its ground at mid-swing, as an animator cleans a motion-capture take
// whose toe drags (a sliding foot is the commonest mocap fault; Unreal's Foot Placement and
// Leg IK fix it at run time, our bake fixes it once). Kimodo's side-steps glide their swinging
// toe a centimetre over the ground for a third of a second (Fight_Strafe_R: 4/3 cm at
// mid-swing on the mannequin itself), which the game shows as a foot sliding where the lock
// has let it go (the duel's partner, 2026-10-28).
//
// A rule every made clip and every fight clip gets, not a number typed per clip:
//   - the swing is where the foot's lock curve is 0 (markers.ts, the same bake the game reads);
//   - the bar for the swing's highest point is the walk's, WALK_BAR_M, scaled by how far the
//     ball goes across under the body in that swing against the walk's, WALK_SWING_M, and never
//     over it: a short shuffle lifts less than a stride; a clip's entry may set its own ("clear", m);
//   - a swing whose highest point is under its bar is raised by what it lacks along half a sine,
//     0 where it leaves and lands, so its shape is kept (the walk's foot is lowest mid-swing, as
//     real gait's is); its ankle lifted straight up by two-bone IK (the knee in the plane it bent
//     in, the foot keeping its turn in the model), so the planted frames, and every swing already
//     reaching its bar (the walk, jog and sprint), are as they were.
//
//   node tools/characters/clear.ts FILE.glb... [--dry]   (before mirror.ts and markers.ts)
//
// Node 22.18 or later; no dependencies.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { dot, Glb, len, posed, type Pose, type Q, qinv, qmul, rot, sub, type V3 } from "./glb.ts";
import { compact, glbBytes } from "./glb_clips.ts";
import { bake } from "./markers.ts";
import { MIRRORS } from "./mirror.ts";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const LISTS = ["art/animation/made.json", "art/animation/sets/fight.json"].map((p) => resolve(ROOT, p));

/** The bar of a full stride: the walk's swinging foot at its highest, its heel or ball whichever
 * is lower, over where they stand (Walk_Loop, 2026-10-28: 11.8/12.4 cm on the mannequin, 10.4/11.3
 * on the body, whose foot is shorter and flatter, D542), taken under both so neither walk moves. */
export const WALK_BAR_M = 0.1;
/** How far the walk's ball goes across under the body from the plant before a swing to the one
 * after (Walk_Loop, 2026-10-28: 0.88/0.80 m on the mannequin, 0.81/0.80 on the body, the shortest
 * taken): a swing this long or longer wants the whole bar, a shorter one its share. */
export const WALK_SWING_M = 0.8;
/** A raise under this changes nothing: the clip is already clear. */
export const LEAST_M = 0.002;

/** A sorted run of frames `[first, last]` where the foot swings (its lock 0), and whether it
 * wraps round a loop's end. */
export type Swing = [number, number];

/** The runs of frames where `lock` is 0 (a loop's last frame is its first again, so a run over
 * the end continues at the start). A clip whose foot never locks has none: it never steps. */
export function swings(lock: number[], loops: boolean): Swing[] {
  const n = loops ? lock.length - 1 : lock.length;
  if (n < 2 || lock.slice(0, n).every((x) => x <= 0)) return [];
  const free = (i: number) => lock[((i % n) + n) % n] <= 0;
  const start = loops ? lock.findIndex((x, i) => i < n && x > 0) : 0;
  const out: Swing[] = [];
  for (let k = 0; k < n; k++) {
    const i = start + k;
    if (!free(i)) continue;
    if (out.length && out[out.length - 1][1] === i - 1) out[out.length - 1][1] = i;
    else out.push([i, i]);
  }
  // A clip played once: a swing touching its first or last frame has no plant on that side;
  // keep it (a step out of a still start), its sine reaching 0 a frame beyond.
  return out;
}

/** Half a sine over a swing `[a, b]` at frame `i`: 1 at its middle, 0 at the planted frames
 * either side. */
export function arc(i: number, [a, b]: Swing): number {
  return Math.sin((Math.PI * (i - a + 1)) / (b - a + 2));
}

/** Each frame's raise of a foot whose lowest point is `sole` over its ground (m, per frame),
 * its lock curve `lock`, and the bar of each of its swings `bars`: a swing whose foot never
 * rises to its bar is lifted along half a sine just enough that its highest comes to the bar,
 * its shape kept; a swing that reaches its bar and every planted frame are not
 * moved. */
export function raises(sole: number[], lock: number[], loops: boolean, bars: (s: Swing) => number): number[] {
  const n = loops ? sole.length - 1 : sole.length;
  const out = new Array(sole.length).fill(0);
  const at = (i: number) => ((i % n) + n) % n;
  for (const s of swings(lock, loops)) {
    // The least lift along the sine that brings some frame to the bar: frame i reaches it at
    // (bar - sole_i) / arc_i, so the swing's new highest is the bar exactly.
    const bar = bars(s);
    let lift = Infinity;
    for (let i = s[0]; i <= s[1]; i++) lift = Math.min(lift, (bar - sole[at(i)]) / arc(i, s));
    lift = Math.max(0, lift);
    for (let i = s[0]; i <= s[1]; i++) out[at(i)] = lift * arc(i, s);
  }
  if (loops) out[n] = out[0];
  return out;
}

const norm = (v: V3): V3 => {
  const l = len(v) || 1;
  return [v[0] / l, v[1] / l, v[2] / l];
};
const add = (a: V3, b: V3, k = 1): V3 => [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];

/** The least turn taking direction `a` to `b`. */
export function between(a: V3, b: V3): Q {
  const [u, v] = [norm(a), norm(b)];
  const c = cross(u, v);
  const w = 1 + dot(u, v);
  if (w < 1e-9) {
    const side = norm(Math.abs(u[0]) < 0.9 ? cross(u, [1, 0, 0]) : cross(u, [0, 1, 0]));
    return [side[0], side[1], side[2], 0];
  }
  const l = Math.hypot(c[0], c[1], c[2], w);
  return [c[0] / l, c[1] / l, c[2] / l, w / l];
}

/** Two-bone IK (Unreal's Two Bone IK, stand.py's `two_bone`): the knee for a thigh from `hip`
 * and a shin reaching `ankle`, in the plane of the hip, the ankle and the knee it had (`knee`),
 * the segments their lengths now; and the ankle reached (short of it, on its line, past reach). */
export function twoBone(hip: V3, knee: V3, ankle: V3, goal: V3): { knee: V3; ankle: V3 } {
  const [l1, l2] = [len(sub(knee, hip)), len(sub(ankle, knee))];
  const to = sub(goal, hip);
  const axis = norm(to);
  const d = Math.max(Math.abs(l1 - l2) + 1e-6, Math.min(len(to), l1 + l2 - 1e-6));
  const cosA = Math.max(-1, Math.min(1, (l1 * l1 + d * d - l2 * l2) / (2 * l1 * d)));
  let side = sub(knee, hip);
  side = norm(add(side, axis, -dot(side, axis)));
  const k = add(hip, add(axis.map((x) => x * cosA) as V3, side, Math.sqrt(1 - cosA * cosA)), l1);
  return { knee: k, ankle: add(hip, axis, d) };
}

type Entry = { clear?: number };
type Lists = Record<string, Entry>;

/** Which clips the rule is for, each with its entry: every made and fight clip, and the
 * mirrors made from them (mirror.ts makes them again from the cleared ones). */
export function listed(): Lists {
  const out: Lists = {};
  for (const file of LISTS) Object.assign(out, JSON.parse(readFileSync(file, "utf8")));
  const mirrors: Record<string, string> = JSON.parse(readFileSync(MIRRORS, "utf8"));
  for (const [made, from] of Object.entries(mirrors)) if (out[from]) out[made] = out[from];
  return out;
}

/** A foot of a clip: its swings, its lowest point over its ground at mid-swing before and after
 * (the least of its swings), the most it was raised, and the shortest swing's way over the ground. */
export type Cleared = { clip: string; side: "l" | "r"; swings: number; before: number; after: number; most: number; over: number };

const SIDES = ["l", "r"] as const;

/** `file`'s bytes with the rule applied to `clips` (name -> entry), and what it did to each
 * foot it raised; undefined when it raised none. */
export function cleared(bytes: Buffer, clips: Lists): { out?: Buffer; done: Cleared[]; seen: Cleared[] } {
  const body = new Glb(bytes);
  const gltf = body.gltf as Glb["gltf"] & { buffers: { byteLength: number }[]; bufferViews: { buffer: number; byteOffset?: number; byteLength: number }[]; animations: NonNullable<Glb["gltf"]["animations"]> };
  const baked = bake(body);
  const node = (n: string) => body.named(n);
  const legs = SIDES.map((s) => ({ hip: node(`thigh_${s}`), knee: node(`calf_${s}`), ankle: node(`foot_${s}`) }));
  // Heel (the ankle) and ball, as stride.ts and the films' checks read a foot: the toe's leaf
  // tip dips under where it stands when the foot points down mid-swing, so it is no ground.
  const points = SIDES.map((s) => [node(`foot_${s}`), node(`ball_${s}`)]);
  const jsonLen = bytes.readUInt32LE(12);
  const binLen = bytes.readUInt32LE(20 + jsonLen);
  const extra: Buffer[] = [];
  let offset = binLen;
  const push = (values: number[], type: string, count: number, minMax = false): number => {
    const buf = Buffer.alloc(values.length * 4);
    values.forEach((v, i) => buf.writeFloatLE(v, i * 4));
    gltf.bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: buf.length });
    extra.push(buf);
    offset += buf.length;
    const acc: Glb["gltf"]["accessors"][number] & { min?: number[] } = { bufferView: gltf.bufferViews.length - 1, componentType: 5126, count, type };
    if (minMax) Object.assign(acc, { min: [Math.min(...values)], max: [Math.max(...values)] });
    gltf.accessors.push(acc);
    return gltf.accessors.length - 1;
  };
  const done: Cleared[] = [];
  const seen: Cleared[] = [];
  for (const anim of gltf.animations) {
    const entry = clips[anim.name];
    const b = baked[anim.name];
    if (!entry || !b) continue;
    const clip = body.clip(anim.name)!;
    // The legs keyed again on the markers' frames, so a swing is exactly the frames whose lock is
    // 0 and no planted frame is interpolated from a raised key (the takes' keys are often sparser:
    // Fight_Strafe_R has 32 against 79 frames).
    const end = Math.max(...anim.samplers.map((s) => (gltf.accessors[s.input] as { max?: number[] }).max?.[0] ?? 0));
    const times = b.markers.lock_l.map((_, k) => Math.min(end, k / b.markers.rate));
    const poses = times.map((t) => posed(body, clip, t));
    const loops = points.every(([, toe]) => len(sub(poses[0][toe].t, poses[poses.length - 1][toe].t)) < 0.01);
    let changed = false;
    const newPoses = poses.map((p) => p.map((x) => ({ ...x })));
    const n = loops ? times.length - 1 : times.length;
    const wrap = (i: number) => ((i % n) + n) % n;
    SIDES.forEach((s, side) => {
      const [heel, ball] = points[side];
      const lock = side === 0 ? b.markers.lock_l : b.markers.lock_r;
      const low = [heel, ball].map((p) => Math.min(...poses.map((q) => q[p].t[1])));
      const sole = poses.map((q) => Math.min(q[heel].t[1] - low[0], q[ball].t[1] - low[1]));
      // How far the ball goes across under the body (the clip is played in place) from the
      // planted frame before the swing to the one after.
      const across = (sw: Swing) => {
        const went = sub(poses[wrap(sw[1] + 1)][ball].t, poses[wrap(sw[0] - 1)][ball].t);
        return Math.hypot(went[0], went[2]);
      };
      const bars = (sw: Swing) => entry.clear ?? WALK_BAR_M * Math.min(1, across(sw) / WALK_SWING_M);
      const up = raises(sole, lock, loops, bars);
      const most = Math.max(0, ...up);
      const all = swings(lock, loops);
      if (!all.length) return; // a foot that never steps
      const peak = (sw: Swing, by: number[]) => Math.max(...Array.from({ length: sw[1] - sw[0] + 1 }, (_, k) => sole[wrap(sw[0] + k)] + by[wrap(sw[0] + k)]));
      const none = up.map(() => 0);
      const foot: Cleared = {
        clip: anim.name,
        side: s,
        swings: all.length,
        before: Math.min(...all.map((sw) => peak(sw, none))),
        after: Math.min(...all.map((sw) => peak(sw, up))),
        most,
        over: Math.min(...all.map(across)),
      };
      seen.push(foot);
      if (most < LEAST_M) return;
      changed = true;
      const leg = legs[side];
      done.push(foot);
      up.forEach((r, i) => {
        if (r <= 0) return;
        const p = poses[i];
        const [hip, knee, ankle] = [p[leg.hip].t, p[leg.knee].t, p[leg.ankle].t];
        const goal: V3 = [ankle[0], ankle[1] + r, ankle[2]];
        const to = twoBone(hip, knee, ankle, goal);
        const turnThigh = between(sub(knee, hip), sub(to.knee, hip));
        const thigh = qmul(turnThigh, p[leg.hip].r);
        const shin = rot(turnThigh, sub(ankle, knee));
        const calf = qmul(qmul(between(shin, sub(to.ankle, to.knee)), turnThigh), p[leg.knee].r);
        const q = newPoses[i];
        q[leg.hip] = { ...q[leg.hip], r: thigh };
        q[leg.knee] = { ...q[leg.knee], r: calf };
      });
    });
    if (!changed) continue;
    // Each leg's three bones keyed again in their parents' frames: the thigh and calf turned,
    // the foot keeping its turn in the model.
    for (const ch of anim.channels) {
      if (ch.target.path !== "rotation") continue;
      const n = ch.target.node;
      const leg = legs.find((l) => l.hip === n || l.knee === n || l.ankle === n);
      if (!leg) continue;
      const parent = body.parentOf[n];
      let prev: Q | undefined;
      const values = times.flatMap((_, i) => {
        const was = poses[i];
        const now = newPoses[i];
        const world = n === leg.ankle ? was[n].r : now[n].r;
        const v = [...qmul(qinv(now[parent].r), world)] as Q;
        if (prev && v.reduce((acc, x, k) => acc + x * prev![k], 0) < 0) v.forEach((x, k) => (v[k] = -x));
        prev = v;
        return v;
      });
      const input = push(times, "SCALAR", times.length, true);
      anim.samplers.push({ input, output: push(values, "VEC4", times.length), interpolation: "LINEAR" } as (typeof anim.samplers)[number]);
      ch.sampler = anim.samplers.length - 1;
    }
  }
  if (!done.length) return { done, seen };
  gltf.buffers[0].byteLength = offset;
  const bin = Buffer.concat([bytes.subarray(28 + jsonLen, 28 + jsonLen + binLen), ...extra]);
  return { out: compact(glbBytes(gltf, bin)), done, seen };
}

/** Clears `file`'s listed clips in place (every clip with `all`); what it did and saw. */
export function clearFile(file: string, write = true, all = false): { done: Cleared[]; seen: Cleared[] } {
  const clips = listed();
  if (all) for (const name of new Glb(file).clipNames()) clips[name] ??= {};
  const { out, done, seen } = cleared(readFileSync(file), clips);
  if (out && write) writeFileSync(file, out);
  return { done, seen };
}

const cm = (x: number) => (x * 100).toFixed(1);

if (import.meta.url === `file://${process.argv[1]}`) {
  const files = process.argv.slice(2).filter((a) => !a.startsWith("--"));
  const dry = process.argv.includes("--dry");
  const print = process.argv.includes("--print");
  if (!files.length) {
    console.log("clear.ts FILE.glb... [--dry] [--print [--all]]   (before mirror.ts and markers.ts; --print: every foot, --all: every clip)");
    process.exit(1);
  }
  for (const file of files) {
    const { done, seen } = clearFile(file, !dry && !print, process.argv.includes("--all"));
    if (print)
      for (const d of seen)
        console.log(`${d.clip} ${d.side}: ${d.swings} swings over ${d.over.toFixed(2)} m, mid-swing ${cm(d.before)} cm, ${d.most >= LEAST_M ? `raised to ${cm(d.after)}, most ${cm(d.most)}` : "clear"}`);
    const each = done.map((d) => `${d.clip} ${d.side} ${cm(d.before)}->${cm(d.after)} cm`);
    console.log(`clear: ${file}, ${done.length} feet raised${dry || print ? " (not written)" : ""}${each.length ? ": " + each.join(", ") : ""}`);
  }
}
