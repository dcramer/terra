import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { type Arm, CLEAR, back, clear, ease, held, part, radius, reach } from "./carry_clear.ts";
import { type V3 } from "./glb.ts";
import { rustConst } from "./rust_const.ts";
import { WRIST } from "./wrist.ts";

// terra_walker::carry_clear's own test body: a crouching body's right arm, its hand at knee
// height, and a look 60 cm long in it pointing down and ahead, 17 cm under the ground.
const arm: Arm = { shoulder: [-0.2, 0.9, 0], elbow: [-0.2, 0.62, -0.03], hand: [-0.2, 0.35, 0.04] };
const look: V3[] = [0, 0.25, 0.5, 0.75, 1].map((k) => [arm.hand[0], arm.hand[1] - 0.52 * k, arm.hand[2] + 0.3 * k]);
const lowest = (by: V3, points: V3[]) => {
  const laid = reach(arm, by);
  return Math.min(...points.map((p) => held(arm, laid, back(arm, laid, undefined), p)[1]));
};

test("the wrist's limits and the margins are the game's, read from its source", () => {
  const text = readFileSync("crates/terra_walker/src/carry_clear.rs", "utf8");
  const of = (name: string) => Number(text.split(`${name}: f32 = `)[1].split(";")[0]);
  assert.deepEqual([WRIST.flex, WRIST.ext, WRIST.radial, WRIST.ulnar], [of("WRIST_FLEX_DEG"), of("WRIST_EXT_DEG"), of("WRIST_RADIAL_DEG"), of("WRIST_ULNAR_DEG")]);
  assert.deepEqual([CLEAR.ground, CLEAR.body], [of("CLEAR_GROUND_M"), of("CLEAR_BODY_M")]);
  assert.equal(CLEAR.on, text.length && /pub const CLEAR_ON: bool = true;/.test(readFileSync("crates/terra_walker/src/carry_arm.rs", "utf8")) ? 1 : 0);
  // The planted fault: a constant the source has not is an error, not a default.
  assert.throws(() => rustConst("crates/terra_walker/src/carry_clear.rs", "WRIST_TWIST_DEG"), /no const WRIST_TWIST_DEG/);
});

test("a look under the ground is lifted to its margin by the least move, a clear one not at all", () => {
  assert.ok(lowest([0, 0, 0], look) < -0.16, "the planted look no longer goes under the ground");
  const by = clear(arm, undefined, look, 0, [], [-1, 0, 0]);
  const low = lowest(by, look);
  assert.ok(low >= CLEAR.ground - 1e-4 && low < CLEAR.ground + 0.01, `its lowest point at ${low} m, the margin ${CLEAR.ground}`);
  assert.deepEqual(clear(arm, undefined, look, -1, [], [-1, 0, 0]), [0, 0, 0]);
  assert.deepEqual(reach(arm, [0, 0, 0]), [[0, 0, 0, 1], [0, 0, 0, 1]]);
});

test("a point inside a shin goes out on the hand's side to the capsule's margin", () => {
  const shin = { a: [-0.1, 0.45, 0.1] as V3, b: [-0.1, 0.05, 0.1] as V3, radius: 0.05 };
  // 2 cm inside the capsule, on the side away from the hand's.
  const point: V3 = [-0.07, 0.2, 0.1];
  const by = clear(arm, undefined, [point], -1, [shin], [-1, 0, 0]);
  const laid = reach(arm, by);
  const p = held(arm, laid, [0, 0, 0, 1], point);
  const off = Math.hypot(p[0] - shin.a[0], p[2] - shin.a[2]);
  assert.ok(off >= shin.radius + CLEAR.body - 1e-3 && off < shin.radius + CLEAR.body + 0.01, `${off} m off the shin's axis`);
  assert.ok(p[0] < shin.a[0], `it went out at x ${p[0]}, on the body's side of the shin, not the hand's`);
});

test("the capsules' parts, their radius and the ease are the game's", () => {
  assert.deepEqual(["thigh_l", "calf_twist_01_l", "thigh_twist_01_r", "calf_r", "pelvis", "spine_03", "neck_01", "foot_l"].map(part), [0, 1, 2, 3, 4, 4, undefined, undefined]);
  // Nine in ten of eleven distances lie within the tenth.
  assert.equal(radius([11, 3, 2, 1, 4, 5, 6, 7, 8, 9, 10]), 10);
  assert.equal(radius([]), 0);
  // A step's first frame at 60 a second: 1 - (1 + rate t) e^(-rate t) of the way.
  const e = ease({ at: [0, 0, 0], speed: [0, 0, 0] }, [0, 1, 0], 36, 1 / 60);
  assert.ok(Math.abs(e.at[1] - (1 - (1 + 0.6) * Math.exp(-0.6))) < 1e-12);
});
