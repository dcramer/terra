import assert from "node:assert/strict";
import { test } from "node:test";
import { apply, dot, Glb, len, sub, type Pose, type V3 } from "./glb.ts";
import { curve, keyClip, MANNEQUIN } from "./key.ts";

test("the keys' curve passes through every key and never overshoots between them", () => {
  const ts = [0, 1, 2, 3];
  const ys = [0, 10, 10, -5];
  const f = curve(ts, ys);
  ts.forEach((t, i) => assert.equal(f(t), ys[i]));
  for (let t = 0; t <= 3; t += 0.01) {
    const i = Math.min(2, Math.floor(t));
    const [lo, hi] = [Math.min(ys[i], ys[i + 1]), Math.max(ys[i], ys[i + 1])];
    assert.ok(f(t) >= lo - 1e-9 && f(t) <= hi + 1e-9, `${f(t)} at ${t} outside ${lo}..${hi}`);
  }
  // A held key (10 to 10) stays put; a loop's speed runs on through its seam.
  assert.equal(f(1.5), 10);
  const loop = curve([0, 1, 2, 3], [0, 4, 8, 0], true);
  const slope = (g: (t: number) => number, t: number) => (g(t + 1e-4) - g(t - 1e-4)) / 2e-4;
  assert.ok(Math.abs(slope(loop, 1e-4) - slope(loop, 3 - 1e-4)) < 0.01);
});

// A small idle on the mannequin, from the kept start: its outcomes are what the keys ask.
const g = new Glb(MANNEQUIN);
const spec = {
  name: "Test_Idle",
  guard: "idle-start.json",
  start: { pelvis: { roll: 0 }, chest: { roll: 0 }, head: { yaw: 0, pitch: 0, roll: 0 }, stance: 0.22, turn_out: 7, knees: 16 },
  end: 2,
  loop: true,
  hang: true,
  rest_arm: { out: 7.5, forward: 0, turn_in: 20, elbow: 18, palm: 0 },
  fingers: 90,
  keys: [
    { t: 0, guard: true },
    { t: 1, pelvis: { move: [0.03, -0.01, 0], roll: 3 }, chest: { yaw: 4 } },
    { t: 2, guard: true },
  ],
};
const k = keyClip(spec, g, "art/animation/keyed");
function world(l: Pose[]): Pose[] {
  const w: Pose[] = [];
  const at = (i: number): Pose => (w[i] ??= g.parentOf[i] === undefined ? l[i] : apply(at(g.parentOf[i]), l[i]));
  l.forEach((_, i) => at(i));
  return w;
}
const frames = k.samples.map(world);
const at = (f: number, n: string) => frames[f][g.named(n)].t;
const deg = (a: V3, b: V3) => (Math.acos(Math.max(-1, Math.min(1, dot(a, b) / (len(a) * len(b))))) * 180) / Math.PI;

test("a keyed loop ends where it starts, and feet not told to move stay to the millimetre", () => {
  const last = frames.length - 1;
  for (const n of ["pelvis", "Head", "hand_r", "hand_l", "ball_r", "ball_l"]) assert.ok(len(sub(at(0, n), at(last, n))) < 1e-4, n);
  for (const side of ["r", "l"]) {
    const drift = Math.max(...frames.map((_, f) => len(sub(at(f, `ball_${side}`), at(0, `ball_${side}`)))));
    assert.ok(drift < 1e-3, `ball_${side} moved ${(drift * 100).toFixed(2)} cm while the hips swayed`);
  }
  // The hips did sway, as keyed.
  assert.ok(len(sub(at(60, "pelvis"), at(0, "pelvis"))) > 0.02);
});

test("the start stands as `start` says: feet apart and turned out, knees bent, empty arms hanging", () => {
  // The balls 22 cm apart side to side (he faces +Z), level front to back.
  assert.ok(Math.abs(Math.abs(at(0, "ball_r")[0] - at(0, "ball_l")[0]) - 0.22) < 1e-3);
  assert.ok(Math.abs(at(0, "ball_r")[2] - at(0, "ball_l")[2]) < 1e-3);
  for (const s of ["r", "l"]) {
    const knee = deg(sub(at(0, `calf_${s}`), at(0, `thigh_${s}`)), sub(at(0, `foot_${s}`), at(0, `calf_${s}`)));
    assert.ok(Math.abs(knee - 16) < 1, `knee ${s} ${knee.toFixed(1)}`);
    // The toes 7 degrees out along the ankle-to-ball line; his right is -X.
    const d = sub(at(0, `ball_${s}`), at(0, `foot_${s}`));
    const out = (Math.atan2((s === "r" ? -1 : 1) * d[0], d[2]) * 180) / Math.PI;
    assert.ok(Math.abs(out - 7) < 0.5, `foot ${s} out ${out.toFixed(1)}`);
    // The upper arm 7.5 degrees out from straight down, the elbow bent 18.
    const upper = sub(at(0, `lowerarm_${s}`), at(0, `upperarm_${s}`));
    const away = (Math.atan2((s === "r" ? -1 : 1) * upper[0], -upper[1]) * 180) / Math.PI;
    assert.ok(Math.abs(away - 7.5) < 1, `upper arm ${s} ${away.toFixed(1)} out`);
    const elbow = deg(upper, sub(at(0, `hand_${s}`), at(0, `lowerarm_${s}`)));
    assert.ok(Math.abs(elbow - 18) < 0.5, `elbow ${s} ${elbow.toFixed(1)}`);
  }
});
