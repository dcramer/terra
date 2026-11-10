import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { type Arm, CLEAR, type Spread, back, clear, ease, held, offAxis, part, radii, radius, reach } from "./carry_clear.ts";
import { Glb, type V3, dot, posed, sub } from "./glb.ts";
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

test("a limb whose joints run off its middle is as thick as it is, the body's calf as its section", () => {
  // The fault planted: a calf 5 cm in radius whose joints run 4 cm in front of its middle, as
  // a shin's do, is 5 cm, not the 9 its far side stands off them.
  const ring = Array.from({ length: 16 }, (_, k): V3 => offAxis([0.05 * Math.cos((k * Math.PI) / 8), 0.3, -0.04 + 0.05 * Math.sin((k * Math.PI) / 8)], [0, 0, 0], [0, 1, 0])!);
  assert.ok(Math.abs(Math.max(...ring.map((o) => Math.hypot(...o))) - 0.09) < 1e-9);
  assert.ok(Math.abs(radius(ring) - 0.05) < 1e-9, `${radius(ring)}`);
  // The body's own left calf, known another way: the half-width of the whole mesh's section
  // across the middle of the bone (every vertex within 3% of its length of mid-length, on the
  // left of the body), the mean of its halves across and fore and aft.
  const body = new Glb("assets/models/character/base.glb");
  const rest = posed(body, undefined, 0);
  const [a, b] = [rest[body.named("calf_l")].t, rest[body.named("foot_l")].t];
  const along = sub(b, a);
  const [lo, hi] = [[9, 9], [-9, -9]];
  for (const node of body.gltf.nodes as { mesh?: number; skin?: number }[]) {
    if (node.mesh === undefined || node.skin === undefined) continue;
    for (const prim of body.gltf.meshes![node.mesh].primitives as { attributes: Record<string, number> }[]) {
      const at = body.read(prim.attributes.POSITION);
      for (let v = 0; v < at.length / 3; v++) {
        const p: V3 = [at[v * 3], at[v * 3 + 1], at[v * 3 + 2]];
        if (p[0] < 0.01 || Math.abs(dot(sub(p, a), along) / dot(along, along) - 0.5) > 0.03) continue;
        [lo[0], lo[1], hi[0], hi[1]] = [Math.min(lo[0], p[0]), Math.min(lo[1], p[2]), Math.max(hi[0], p[0]), Math.max(hi[1], p[2])];
      }
    }
  }
  const half = (hi[0] - lo[0] + hi[1] - lo[1]) / 4;
  assert.ok(half > 0.04 && half < 0.1, `the section's half-width reads ${half} m: the test's own measure is off`);
  const spread: Spread[] = [];
  const calf = radii(body, spread)[1];
  assert.ok(Math.abs(calf - half) < 0.02, `the calf's radius ${calf} m, its section's half-width at mid-length ${half} m`);
  // Measured from the joints' line, as it was, it reads the far side's.
  assert.ok(spread[1].axis[1] - half > 0.03, `from the axis ${spread[1].axis[1]} m`);
});

test("the capsules' parts, their radius and the ease are the game's", () => {
  assert.deepEqual(["thigh_l", "calf_twist_01_l", "thigh_twist_01_r", "calf_r", "pelvis", "spine_03", "neck_01", "foot_l"].map(part), [0, 1, 2, 3, 4, 4, undefined, undefined]);
  // Ten pairs of vertices 1 to 10 either side of the axis, their middle on it: nine within 9.
  assert.equal(radius([10, 3, 2, 1, 4, 5, 6, 7, 8, 9].flatMap((k): V3[] => [[k, 0, 0], [-k, 0, 0]])), 9);
  assert.equal(radius([]), 0);
  // A vertex stands off the line square to it; past a bone's end it is not the part's.
  assert.deepEqual(offAxis([0.03, 0.4, 0], [0, 0, 0], [0, 1, 0]), [0.03, 0, 0]);
  assert.equal(offAxis([0.03, 1.04, 0], [0, 0, 0], [0, 1, 0]), undefined);
  assert.equal(offAxis([1, 0, 0], [0, 1, 0], [0, 1, 0]), undefined);
  // A step's first frame at 60 a second: 1 - (1 + rate t) e^(-rate t) of the way.
  const e = ease({ at: [0, 0, 0], speed: [0, 0, 0] }, [0, 1, 0], 36, 1 / 60);
  assert.ok(Math.abs(e.at[1] - (1 - (1 + 0.6) * Math.exp(-0.6))) < 1e-12);
});
