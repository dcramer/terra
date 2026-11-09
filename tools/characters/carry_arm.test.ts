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

// The give's pop at a jump's start (held-carry j4): a fold stepping from nothing to whole in
// one frame moves the elbow no faster than ELBOW_MOST_DEG_S, by the spring's own answer,
// 1 - (1 + rate t) e^(-rate t); and through Jump_Start itself, which opens on a crouch and
// stands 0.07 s later, the eased give's elbow never turns faster, where followed frame by
// frame it let go of 40 degrees inside those 0.07 s (over 570 degrees a second).
test("a step in the fold moves the elbow no faster than the give's rate", async () => {
  const { ELBOW_MOST_DEG_S, GIVE, ease, easedThrough, fold } = await import("./carry_arm.ts");
  let eased = { at: 0, speed: 0 };
  for (let frame = 1; frame <= 30; frame++) {
    const before = eased.at;
    eased = ease(eased, 1, 1 / 60);
    const t = frame / 60;
    assert.ok(Math.abs(eased.at - (1 - (1 + GIVE.rate * t) * Math.exp(-GIVE.rate * t))) < 1e-9, `frame ${frame}: ${eased.at}`);
    const speed = (eased.at - before) * GIVE.elbow * 60;
    assert.ok(speed >= 0 && speed <= ELBOW_MOST_DEG_S, `frame ${frame}: ${speed} degrees a second`);
  }
  const g = new Glb("assets/models/character/fight.glb");
  const start = g.clip("Jump_Start")!;
  const legs = ["l", "r"].map((s) => [g.named(`thigh_${s}`), g.named(`calf_${s}`), g.named(`foot_${s}`)] as const);
  const raw = (t: number) => fold(posed(g, start, t), legs);
  assert.ok((raw(0) - raw(1 / 15)) * GIVE.elbow * 15 > 570, "Jump_Start no longer opens on a crouch it leaves in a frame: the planted step is gone");
  const given = easedThrough(start.end, false, raw);
  let most = 0;
  for (let i = 1; i <= start.end * 60; i++) most = Math.max(most, Math.abs(given(i / 60) - given((i - 1) / 60)) * GIVE.elbow * 60);
  assert.ok(most > 100 && most <= ELBOW_MOST_DEG_S, `through Jump_Start the elbow turns ${most} degrees a second at most, bar ${ELBOW_MOST_DEG_S}`);
});

// The pickaxe under the ground in a landing (held-carry j3): Carry_Tool, a hanging pose, over
// Jump_Land's crouch. Its fold is whole there and nothing standing; given, the hand is higher
// and further out than held in the body's frame alone, and the handle no longer points down.
test("a hanging carried arm gives as a landing folds the legs, and not standing", async () => {
  const { fold } = await import("./carry_arm.ts");
  const g = new Glb("assets/models/character/fight.glb");
  const rest = posed(g, undefined, 0);
  const carry = g.clip("Carry_Tool")!;
  const hand = g.named("socket.hand.R");
  const legs = ["l", "r"].map((s) => [g.named(`thigh_${s}`), g.named(`calf_${s}`), g.named(`foot_${s}`)] as const);
  assert.equal(fold(posed(g, g.layered(g.clip("Idle_Loop")!, carry, ["clavicle_r"]), 0), legs), 0);
  const land = g.layered(g.clip("Jump_Land")!, carry, ["clavicle_r"]);
  const laid = posed(g, land, 0.2);
  assert.equal(fold(laid, legs), 1);
  const stiff = posed(g, land, 0.2, carryArm(g, rest, ["clavicle_r"])(laid))[hand];
  const given = posed(g, land, 0.2, carryArm(g, rest, [], ["clavicle_r"])(laid))[hand];
  assert.ok(given.t[1] > stiff.t[1] + 0.05 && given.t[0] < stiff.t[0] - 0.05, `the hand at ${given.t}, held stiff ${stiff.t}`);
  assert.ok(offUp(rot(laid[hand].r, [0, 1, 0])) > 150, "the landing no longer tips the laid handle down");
  assert.ok(offUp(rot(given.r, [0, 1, 0])) < 90, `the given handle points ${offUp(rot(given.r, [0, 1, 0]))} degrees off up`);
});
