#!/usr/bin/env node
// Does a loop's start blend with what the game plays from it? For each of the loop's moments, the
// nearest pose of each other clip, by the mean distance of the head, shoulders, elbows, hands and
// knees from the pelvis (the hips' turn taken out, a lean kept), cm. Compare a new start against
// the clip it replaces: a crossfade from a nearer pose moves the body less.
//
//   node spike/pose-sheet/blend.ts LOOP.glb LOOP_CLIP OTHER.glb CLIP...

import { Glb, len, posed, qinv, rot, sub, type Q, type V3 } from "../../tools/characters/glb.ts";

const [loopFile, loopName, otherFile, ...others] = process.argv.slice(2);
if (!others.length) throw new Error("blend: give LOOP.glb LOOP_CLIP OTHER.glb and the clips the game plays from the loop");
const a = new Glb(loopFile);
const b = new Glb(otherFile);
const JOINTS = ["Head", "upperarm_r", "upperarm_l", "lowerarm_r", "lowerarm_l", "hand_r", "hand_l", "calf_r", "calf_l"];
function shape(g: Glb, name: string, t: number): V3[] {
  const clip = g.clip(name);
  if (!clip) throw new Error(`blend: no clip ${name}; give one the file holds`);
  const p = posed(g, clip, t);
  const pel = p[g.named("pelvis")];
  const f = rot(pel.r, [0, 0, 1]);
  const yaw = Math.atan2(f[0], f[2]);
  const q: Q = [0, Math.sin(yaw / 2), 0, Math.cos(yaw / 2)];
  return JOINTS.map((j) => rot(qinv(q), sub(p[g.named(j)].t, pel.t)));
}
const samples = (g: Glb, name: string, n: number) => Array.from({ length: n }, (_, k) => shape(g, name, (k / n) * g.clip(name)!.end));
const dist = (x: V3[], y: V3[]) => (x.reduce((s, v, i) => s + len(sub(v, y[i])), 0) / x.length) * 100;
const loop = samples(a, loopName, 24);
for (const name of others) {
  const to = samples(b, name, 48);
  const near = loop.map((s) => Math.min(...to.map((o) => dist(s, o))));
  console.log(`${loopName} -> ${name}: from its first pose ${near[0].toFixed(1)} cm, mean ${(near.reduce((x, y) => x + y, 0) / near.length).toFixed(1)}, worst ${Math.max(...near).toFixed(1)}`);
}
