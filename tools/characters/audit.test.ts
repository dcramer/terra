import assert from "node:assert/strict";
import { test } from "node:test";
import { alongBone, chainOff, offAxis, type Skeleton, skinMatrix, strapOff } from "./audit.ts";
import { type Pose, type Q, type V3, len, matrix, posed } from "./glb.ts";

const RAD = Math.PI / 180;

/** A made-up skeleton: nodes by name, each with its parent and rest translation. */
function skeleton(nodes: [name: string, parent: string | null, t: V3][]): Skeleton {
  const gltfNodes = nodes.map(([name, , t]) => ({ name, translation: t, children: [] as number[] }));
  const parentOf: number[] = [];
  nodes.forEach(([, parent], i) => {
    if (parent === null) return;
    const p = nodes.findIndex(([n]) => n === parent);
    parentOf[i] = p;
    gltfNodes[p].children.push(i);
  });
  return {
    gltf: { nodes: gltfNodes } as unknown as Skeleton["gltf"],
    parentOf,
    named: (n: string) => nodes.findIndex(([m]) => m === n),
    rest: (i: number): Pose => ({ t: nodes[i][2], r: [0, 0, 0, 1], s: [1, 1, 1] }),
  };
}

// An arm whose wrist sits 15 degrees off its forearm bone's +Y, as a body whose joints were
// moved into its skin with its bones' directions kept.
const BENT = 15;
const arm = skeleton([
  ["upperarm_l", null, [0, 0, 0]],
  ["lowerarm_l", "upperarm_l", [0, 0.3, 0]],
  ["hand_l", "lowerarm_l", [0, 0.25 * Math.cos(BENT * RAD), 0.25 * Math.sin(BENT * RAD)]],
]);

test("a segment off its bone reads its angle, one on it none", () => {
  const off = Object.fromEntries(offAxis(arm).map((s) => [s.bone, s.deg]));
  assert.ok(Math.abs(off.upperarm_l) < 1e-9);
  assert.ok(Math.abs(off.lowerarm_l - BENT) < 1e-9);
});

test("the hand lands off where a clip copied bone by bone points it by the chord of the angle", () => {
  const pose = posed(arm as never, undefined, 0);
  const cm = chainOff(arm, pose, alongBone(arm, pose)).hand_l;
  // The forearm's 0.25 m turned 15 degrees: a chord of 2 x 0.25 x sin(7.5 degrees).
  assert.ok(Math.abs(cm - 2 * 25 * Math.sin((BENT / 2) * RAD)) < 1e-6, `${cm}`);
});

test("a strap along the bone's +Y reads the forearm's turn off it", () => {
  const pose = posed(arm as never, undefined, 0);
  const off = strapOff(pose, arm.named("lowerarm_l"), arm.named("hand_l"), { at: [0, 0.1, -0.04], face: [0, 0, -1], grip: [0, 1, 0] });
  assert.ok(Math.abs(off.deg - BENT) < 1e-6);
  // The wrist rises 0.25 sin(15 degrees) off the board's plane past the elbow.
  assert.ok(Math.abs(off.cm - 25 * Math.sin(BENT * RAD)) < 1e-6);
});

const turnY = (deg: number): Q => [0, Math.sin((deg * RAD) / 2), 0, Math.cos((deg * RAD) / 2)];
const at = (m: number[], p: V3): V3 => [0, 1, 2].map((j) => m[j] * p[0] + m[4 + j] * p[1] + m[8 + j] * p[2] + m[12 + j]) as V3;

test("one joint skins as its own matrix, whatever the share", () => {
  const m = matrix({ t: [0.1, 0.2, 0.3], r: turnY(40), s: [1, 1, 1] });
  for (const total of [1, 1.5, 2]) {
    const got = skinMatrix([m], [total]);
    got.forEach((x, i) => assert.ok(Math.abs(x - m[i]) < 1e-9));
  }
});

test("dual quaternions keep a twisted limb's girth where the linear blend narrows it", () => {
  // Two joints on the limb's axis (y) twisted 90 degrees apart, a skin point 1 m out between.
  const a = matrix({ t: [0, 0, 0], r: turnY(-45), s: [1, 1, 1] });
  const b = matrix({ t: [0, 0, 0], r: turnY(45), s: [1, 1, 1] });
  const p: V3 = [1, 0, 0];
  const linear = at(skinMatrix([a, b], [0.5, 0.5]), p);
  const dual = at(skinMatrix([a, b], [1, 1]), p);
  assert.ok(Math.abs(len(linear) - Math.cos(45 * RAD)) < 1e-9);
  assert.ok(Math.abs(len(dual) - 1) < 1e-9);
  // Half-way round: the point on +x stays on +x.
  assert.ok(Math.abs(dual[0] - 1) < 1e-9);
});

test("dual quaternions carry a joint's translation", () => {
  const a = matrix({ t: [0, 1, 0], r: turnY(30), s: [1, 1, 1] });
  const b = matrix({ t: [0, 1, 0], r: turnY(30), s: [1, 1, 1] });
  const got = at(skinMatrix([a, b], [1, 1]), [0, 0, 0]);
  assert.ok(Math.abs(got[1] - 1) < 1e-9 && Math.abs(got[0]) < 1e-9 && Math.abs(got[2]) < 1e-9);
});
