import { test } from "node:test";
import assert from "node:assert/strict";
import { carryArm } from "./carry_arm.ts";
import { Glb, type V3, posed, rot } from "./glb.ts";

const offUp = (v: V3) => (Math.acos(v[1] / Math.hypot(...v)) * 180) / Math.PI;

// The torch's carry pose on the right arm over a crouch, whose chest hunches over 80 degrees:
// laid in the chest's frame the handle goes with it; held in the body's, the handle points as
// it does with the carry pose on a body standing at rest, the answer posed another way.
test("a carried arm keeps its turn in the body however far the chest leans", () => {
  const g = new Glb("assets/models/character/fight.glb");
  const rest = posed(g, undefined, 0);
  const carry = g.clip("Carry_Torch")!;
  const handle = g.named("socket.hand.R");
  const up = (pose: typeof rest) => rot(pose[handle].r, [0, 1, 0]);
  // Every bone at rest but the arm, which takes the carry pose.
  const standing = posed(g, g.layered({ name: "rest", end: 0, tracks: new Map() }, carry, ["clavicle_r"]), 0);
  const crouch = g.layered(g.clip("Crouch_Idle_Loop")!, carry, ["clavicle_r"]);
  const hold = carryArm(g, rest, ["clavicle_r"]);
  for (const t of [0, 0.4, 0.8]) {
    const laid = posed(g, crouch, t);
    assert.ok(offUp(up(laid)) > 45, `at ${t} s the crouch's chest no longer tips the laid handle`);
    const held = up(posed(g, crouch, t, hold(laid)));
    const want = up(standing);
    for (let i = 0; i < 3; i++) assert.ok(Math.abs(held[i] - want[i]) < 1e-6, `at ${t} s: ${held} against ${want}`);
  }
});
