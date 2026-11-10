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

// A creature's own body, posed whole from the way it is bound: nothing of any clip in it.
const TROLL = "assets/models/characters/troll.glb";
const troll = new Glb(TROLL);
const trollWorld = (l: Pose[]): Pose[] => {
  const w: Pose[] = [];
  const at = (i: number): Pose => (w[i] ??= troll.parentOf[i] === undefined ? l[i] : apply(at(troll.parentOf[i]), l[i]));
  l.forEach((_, i) => at(i));
  return w;
};

test("the `bind` guard starts every joint of the skin as it is bound, turned and placed", () => {
  const k = keyClip({ name: "Bound", body: TROLL, guard: "bind", fists: true, end: 1, keys: [{ t: 0 }, { t: 1 }] }, troll);
  const joints = troll.gltf.skins![0].joints;
  assert.deepEqual([...k.tracked].sort(), [...joints].sort());
  assert.equal(k.placed.size, joints.length);
  for (const i of joints) {
    // The legs are solved to their feet, which stand where the bind pose has them.
    if (/^(thigh|calf|foot|ball)_/.test(troll.gltf.nodes[i].name!)) continue;
    const [got, rest] = [k.samples[0][i], troll.rest(i)];
    assert.ok(len(sub(got.t, rest.t)) < 1e-6 && Math.abs(Math.abs(got.r[0] * rest.r[0] + got.r[1] * rest.r[1] + got.r[2] * rest.r[2] + got.r[3] * rest.r[3]) - 1) < 1e-6, `${troll.gltf.nodes[i].name} is not as bound`);
  }
  const [first, bound] = [trollWorld(k.samples[0]), trollWorld(troll.gltf.nodes.map((_, i) => troll.rest(i)))];
  // (To the centimetre: a leg bound straight is solved a little short of its full length.)
  for (const s of ["r", "l"]) assert.ok(len(sub(first[troll.named(`ball_${s}`)].t, bound[troll.named(`ball_${s}`)].t)) < 0.02, `ball_${s} stands where it is bound`);
});

test("a keyed knee bends the way its key points, swings there without a jump, and the foot stays", () => {
  // The hips dropped and carried ahead of both planted feet: unkeyed, the knees fold forward and
  // down (a kneel); the right one is keyed out to its side.
  const spec = {
    name: "Knee_Out",
    body: TROLL,
    guard: "bind",
    fists: true,
    end: 1,
    keys: [{ t: 0 }, { t: 1, pelvis: { move: [0, -0.7, 0.5] }, foot_r: { knee: [1, 0.3, 0] } }],
  };
  const frames = keyClip(spec, troll).samples.map(trollWorld);
  const at = (f: number, n: string) => frames[f][troll.named(n)].t;
  const last = frames.length - 1;
  // Each knee's offset from its hip-to-ankle line: the right one out to the right (-X) and up, the
  // left one, unkeyed, forward.
  const off = (s: string): V3 => {
    const [hip, knee, ankle] = [at(last, `thigh_${s}`), at(last, `calf_${s}`), at(last, `foot_${s}`)];
    const line = sub(ankle, hip);
    const k = dot(sub(knee, hip), line) / dot(line, line);
    return sub(knee, [hip[0] + line[0] * k, hip[1] + line[1] * k, hip[2] + line[2] * k]);
  };
  const [right, left] = [off("r"), off("l")];
  assert.ok(len(right) > 0.5, `the leg is bent, its knee ${len(right).toFixed(2)} m off the line`);
  assert.ok(right[0] < 0 && -right[0] > 0.8 * len(right), `the keyed knee points to its right, ${right.map((v) => v.toFixed(2))}`);
  assert.ok(left[2] > right[2] + 0.15, `the unkeyed knee keeps the guard's way, further forward, ${left.map((v) => v.toFixed(2))} against ${right.map((v) => v.toFixed(2))}`);
  const bound = trollWorld(troll.gltf.nodes.map((_, i) => troll.rest(i)));
  for (const s of ["r", "l"]) assert.ok(len(sub(at(last, `ball_${s}`), bound[troll.named(`ball_${s}`)].t)) < 1e-3, "a foot not told to move stays where it is bound");
  // No frame's knee jumps: the guard's way gives over to the key's by degrees.
  const jump = Math.max(...frames.slice(1).map((_, f) => len(sub(at(f + 1, "calf_r"), at(f, "calf_r")))));
  assert.ok(jump < 0.08, `the knee moves ${jump.toFixed(3)} m in one frame`);
});
