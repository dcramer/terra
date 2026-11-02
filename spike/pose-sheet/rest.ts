#!/usr/bin/env node
// Do empty hands hang as a person's do? The gate the idle lacked: a resting palm faces the thigh or
// a little behind it, and the arm hangs near the side. `Idle_Loop` passed every check with its
// palms out until the developer saw it. The bars are from `Walk_Loop`, sampled over the clip.
//
//   node spike/pose-sheet/rest.ts FILE.glb CLIP

import { dot, Glb, len, posed, sub, type V3 } from "../../tools/characters/glb.ts";

const FORWARD_MOST = 0.05;
const WIDE_MOST_M = 0.18;
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a: V3): V3 => {
  const l = len(a);
  return [a[0] / l, a[1] / l, a[2] / l];
};
const [file, name] = process.argv.slice(2);
const g = new Glb(file);
const clip = g.clip(name);
if (!clip) throw new Error(`rest: no clip ${name} in ${file}; give a clip the file holds`);
let failed = false;
for (const side of ["r", "l"] as const) {
  let forward = -1;
  let wide = 0;
  for (let k = 0; k <= 12; k++) {
    const p = posed(g, clip, (k / 12) * clip.end);
    const at = (bone: string) => p[g.named(`${bone}_${side}`)].t;
    const fingers = norm(sub(at("middle_01"), at("hand")));
    const thumbward = norm(sub(at("index_01"), at("pinky_01")));
    // The palm's way, mirrored for the left hand; he faces +Z.
    const palm = side === "r" ? cross(thumbward, fingers) : cross(fingers, thumbward);
    forward = Math.max(forward, dot(norm(palm), [0, 0, 1]));
    wide = Math.max(wide, Math.abs(at("hand")[0] - at("thigh")[0]));
  }
  const fault = forward > FORWARD_MOST || wide > WIDE_MOST_M;
  failed ||= fault;
  console.log(`${fault ? "✗" : "✓"} ${name} ${side}: palm forward at most ${(forward * 100).toFixed(0)}% (bar ${FORWARD_MOST * 100}%), wrist ${(wide * 100).toFixed(0)} cm out from the hip (bar ${WIDE_MOST_M * 100} cm)`);
}
if (failed) console.log("  roll the forearm in with `palms` or bring the arm in with `arms_in` in the keys; don't widen the bars (Walk_Loop sets them)");
process.exit(failed ? 1 : 0);
