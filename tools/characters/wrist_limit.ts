#!/usr/bin/env node
// The sword hand's wrist held inside a person's (D565 step 3): every fight clip's hand_r turned,
// frame by frame, the least that brings its bend within wrist.ts's limits, as a joint limit
// clamps a bone after the solve (Unreal's Control Rig limits, Havok's ragdoll constraints): a
// take's own small overshoot (the guard's wrist 4-5° past radial, Kimodo's slack about a key) is
// cleaned once, at keep time, so every blow's first frame still meets the guard.
//
// It only cleans: a frame wanting more than MOST_DEG is refused, since that hand needs other keys
// (kimodo_keys.py refuses a key no wrist reaches; change the clip's spec, never this limit).
//
//   node tools/characters/wrist_limit.ts FILE.glb [--dry]   (after the export, before mirror.ts and markers.ts)
//
// Node 22.18 or later; no dependencies.

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { dot, Glb, posed, type Q, qinv, qmul, rot, sub, type V3 } from "./glb.ts";
import { compact, glbBytes } from "./glb_clips.ts";
import { bendOf, handAxes, type HandAxes, turnBetween, WRIST } from "./wrist.ts";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const MOVES = resolve(ROOT, "assets/data/moves");
/** The most a frame's hand is turned to meet the limits, degrees. */
export const MOST_DEG = 15;
/** How far inside each of WRIST's bars a limited frame is held, degrees, so the move check's
 * measure of it reads it within the bar on every body template, not on it: the limit is solved
 * on the base body, and the templates' hands read up to 1.2° further (Keyed_Heavy take 7's ulnar,
 * 2026-10-28). */
export const INSIDE_DEG = 2;

const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const about = (axis: V3, deg: number): Q => {
  const l = Math.hypot(...axis) || 1;
  const s = Math.sin((deg * Math.PI) / 360) / l;
  return [axis[0] * s, axis[1] * s, axis[2] * s, Math.cos((deg * Math.PI) / 360)];
};

/** A hand's rotation `handR` off a forearm from `elbow` to `wrist`, turned the least that brings
 * its bend (relative to `rest`) within WRIST's flexion, extension and deviation; and by how much. */
export function limited(elbow: V3, wrist: V3, handR: Q, axes: HandAxes, rest: { flex: number; dev: number }): { r: Q; turned: number } {
  let r = handR;
  const [maxFlex, maxExt, maxRadial, maxUlnar] = [WRIST.flex, WRIST.ext, WRIST.radial, WRIST.ulnar].map((bar) => bar - INSIDE_DEG);
  for (let i = 0; i < 6; i++) {
    const b = bendOf(elbow, wrist, r, axes);
    const [flex, dev] = [b.flex - rest.flex, b.dev - rest.dev];
    const flexBy = flex > maxFlex ? flex - maxFlex : -flex > maxExt ? flex + maxExt : 0;
    const devBy = dev > maxRadial ? dev - maxRadial : -dev > maxUlnar ? dev + maxUlnar : 0;
    if (Math.abs(flexBy) < 1e-3 && Math.abs(devBy) < 1e-3) break;
    // The hand's line turned back toward the forearm: out of the palm's side by its flexion, out
    // of the thumb's by its deviation.
    const h = rot(r, axes.along);
    if (flexBy) r = qmul(about(cross(h, rot(r, axes.palm)), -flexBy), r);
    if (devBy) r = qmul(about(cross(rot(r, axes.along), rot(r, axes.thumb)), -devBy), r);
  }
  return { r, turned: turnBetween(handR, r) };
}

/** A clip's frames whose hand was turned, the most it was and when. */
export type Limited = { clip: string; frames: number; most: number; at: number };

/** `bytes` with the sword wrist of each of `clips` limited, what it did to each, and the clips
 * refused (a frame wanting more than MOST_DEG), left as they were. */
export function wristLimited(bytes: Buffer, clips: Set<string>): { out?: Buffer; done: Limited[]; refused: Limited[] } {
  const body = new Glb(bytes);
  const gltf = body.gltf as Glb["gltf"] & { buffers: { byteLength: number }[]; bufferViews: { buffer: number; byteOffset?: number; byteLength: number }[]; animations: NonNullable<Glb["gltf"]["animations"]> };
  const hand = body.named("hand_r");
  const parent = body.parentOf[hand];
  const at = (n: string) => body.named(n);
  const restPose = posed(body, undefined, 0);
  const socket = restPose[at("socket.hand.R")];
  const axes = handAxes(restPose[hand].r, restPose[hand].t, restPose[at("middle_01_r")].t, restPose[at("thumb_01_r")].t, rot(socket.r, [-1, 0, 0]));
  const elbowOf = (p: ReturnType<typeof posed>) => p[at("lowerarm_r")].t;
  const rest = bendOf(elbowOf(restPose), restPose[hand].t, restPose[hand].r, axes);
  const jsonLen = bytes.readUInt32LE(12);
  const binLen = bytes.readUInt32LE(20 + jsonLen);
  const extra: Buffer[] = [];
  let offset = binLen;
  const done: Limited[] = [];
  const refused: Limited[] = [];
  for (const anim of gltf.animations) {
    if (!clips.has(anim.name)) continue;
    const ch = anim.channels.find((c) => c.target.node === hand && c.target.path === "rotation");
    if (!ch) continue;
    const clip = body.clip(anim.name)!;
    const times = Array.from(body.read(anim.samplers[ch.sampler].input));
    const seen: Limited = { clip: anim.name, frames: 0, most: 0, at: 0 };
    let prev: Q | undefined;
    const values = times.flatMap((t) => {
      const p = posed(body, clip, t);
      const { r, turned } = limited(elbowOf(p), p[hand].t, p[hand].r, axes, rest);
      if (turned > 0.01) seen.frames++;
      if (turned > seen.most) Object.assign(seen, { most: turned, at: t });
      const v = [...qmul(qinv(p[parent].r), r)] as Q;
      if (prev && dot(v.slice(0, 3) as V3, prev.slice(0, 3) as V3) + v[3] * prev[3] < 0) v.forEach((x, k) => (v[k] = -x));
      prev = v;
      return v;
    });
    if (seen.most > MOST_DEG) {
      refused.push(seen);
      continue;
    }
    if (!seen.frames) continue;
    done.push(seen);
    const buf = Buffer.alloc(values.length * 4);
    values.forEach((v, i) => buf.writeFloatLE(v, i * 4));
    gltf.bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: buf.length });
    extra.push(buf);
    offset += buf.length;
    gltf.accessors.push({ bufferView: gltf.bufferViews.length - 1, componentType: 5126, count: times.length, type: "VEC4" });
    anim.samplers.push({ input: anim.samplers[ch.sampler].input, output: gltf.accessors.length - 1, interpolation: "LINEAR" } as (typeof anim.samplers)[number]);
    ch.sampler = anim.samplers.length - 1;
  }
  if (!done.length) return { done, refused };
  gltf.buffers[0].byteLength = offset;
  const bin = Buffer.concat([bytes.subarray(28 + jsonLen, 28 + jsonLen + binLen), ...extra]);
  return { out: compact(glbBytes(gltf, bin)), done, refused };
}

/** The clips the move sets play (assets/data/moves: each move's clip and the guard), the ones
 * the wrist rows measure; the set's other takes (the old text-only blows the trailer plays, the
 * Keyed_* sources of the Open_* clips) are left as they are, as are Quaternius's blows
 * (Swap_*), which play as made (D450). */
export function played(): Set<string> {
  const out = new Set<string>();
  for (const f of readdirSync(MOVES).filter((f) => f.endsWith(".ron")))
    for (const m of readFileSync(resolve(MOVES, f), "utf8").matchAll(/\b(?:clip|guard): "([^"]+)"/g)) out.add(m[1]);
  for (const c of out) if (c.startsWith("Swap_")) out.delete(c);
  return out;
}

/** What is wrong with the clips refused, and what to do. */
export const refusal = (refused: Limited[]) =>
  `${refused.map((r) => `${r.clip} wants its sword hand turned ${r.most.toFixed(0)}° at ${r.at.toFixed(2)} s`).join("; ")}, ` +
  `more than the ${MOST_DEG}° the wrist limit cleans (D565 step 3): remake the take from keys a wrist reaches (the clip's spec in fight.json, its keys judged and searched with keys.ts), never widen the limit`;

/** Limits `file`'s played clips in place; throws, writing nothing, when a clip is refused. */
export function limitFile(file: string, write = true): Limited[] {
  const { out, done, refused } = wristLimited(readFileSync(file), played());
  if (refused.length) throw new Error(`wrist_limit: ${refusal(refused)}`);
  if (out && write) writeFileSync(file, out);
  return done;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const files = process.argv.slice(2).filter((a) => !a.startsWith("--"));
  const dry = process.argv.includes("--dry");
  if (!files.length) {
    console.log("wrist_limit.ts FILE.glb... [--dry]   (after the export, before mirror.ts and markers.ts)");
    process.exit(1);
  }
  for (const file of files) {
    try {
      const done = limitFile(file, !dry);
      const each = done.map((d) => `${d.clip} ${d.frames} frames, most ${d.most.toFixed(1)}° at ${d.at.toFixed(2)} s`);
      console.log(`wrist_limit: ${file}, ${done.length} clips limited${dry ? " (not written)" : ""}${each.length ? ": " + each.join(", ") : ""}`);
    } catch (e) {
      console.log(`✗ ${(e as Error).message}`);
      process.exitCode = 1;
    }
  }
}
