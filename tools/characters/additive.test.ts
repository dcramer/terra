// The carried arm's additive swing: deltas against a loop's mean, their share, and the game's file.
// node --test tools/characters/additive.test.ts

import { test } from "node:test";
import assert from "node:assert/strict";
import { additiveName, degrees, isAdditive, lessMean, meanOf, partOf, sharesIn, swung } from "./additive.ts";
import { Glb, type Q, qmul } from "./glb.ts";

const about = (axis: [number, number, number], deg: number): Q => {
  const h = (deg * Math.PI) / 360;
  return [axis[0] * Math.sin(h), axis[1] * Math.sin(h), axis[2] * Math.sin(h), Math.cos(h)];
};
const close = (a: Q, b: Q) => Math.abs(a.reduce((s, x, k) => s + x * b[k], 0)) > 1 - 1e-9;

test("a swing about a pose is that swing less the pose, and its mean is the pose", () => {
  const pose = qmul(about([0, 1, 0], 40), about([0, 0, 1], -70));
  const swing = [-20, -10, 0, 10, 20, 10, 0, -10].map((d) => about([1, 0, 0], d));
  const qs = swing.map((s) => qmul(s, pose));
  const mean = meanOf(qs);
  assert.ok(close(mean, pose), `mean ${mean} against ${pose}`);
  const deltas = lessMean(qs, mean);
  deltas.forEach((d, i) => assert.ok(close(d, swing[i])));
  // Laid back over any pose on the left, the delta gives the swing about that pose.
  const other = about([0, 0, 1], 30);
  assert.ok(close(qmul(deltas[0], other), qmul(swing[0], other)));
});

test("a share of a turn turns that share of it, the short way", () => {
  const q = about([0, 1, 0], 60);
  assert.ok(Math.abs(degrees(partOf(q, 0.5)) - 30) < 1e-9);
  assert.ok(close(partOf(q, 0), [0, 0, 0, 1]) && close(partOf(q, 1), q));
  assert.ok(Math.abs(degrees(partOf(q.map((x) => -x) as Q, 0.25)) - 15) < 1e-9);
});

test("a swing clip is told by its name", () => {
  assert.equal(additiveName("Jog_Fwd_Loop", "R"), "Jog_Fwd_Loop.Arm_R_Add");
  assert.ok(isAdditive("Jog_Fwd_Loop.Arm_L_Add") && !isAdditive("Jog_Fwd_Loop") && !isAdditive("Jog_Stop_R"));
});

test("the game's clips file swings a carried arm through the jog at its pose's share, and the torch's less", () => {
  const g = new Glb("assets/models/character/fight.glb");
  const shares = sharesIn(g);
  assert.ok(shares.Carry_Tool > shares.Carry_Torch && shares.Carry_Torch > 0, JSON.stringify(shares));
  const jog = g.clip("Jog_Fwd_Loop")!;
  const held = { ...g.layered(jog, g.clip("Carry_Tool")!, ["clavicle_r"]), name: jog.name };
  const arm = (c: typeof jog, t: number) => c.tracks.get("upperarm_r")!.r!(t) as Q;
  const turns = (share: number) => {
    const c = swung(g, held, "R", share);
    let most = 0;
    for (let i = 0; i <= 20; i++) {
      const [a, b] = [arm(c, (i / 20) * jog.end), arm(held, (i / 20) * jog.end)];
      most = Math.max(most, degrees(qmul(a, [-b[0], -b[1], -b[2], b[3]])));
    }
    return most;
  };
  const [tool, torch] = [turns(shares.Carry_Tool), turns(shares.Carry_Torch)];
  assert.ok(tool > 10 && torch < tool, `the jog swings the tool's arm ${tool.toFixed(1)} deg, the torch's ${torch.toFixed(1)}`);
  assert.equal(turns(0), 0);
});
