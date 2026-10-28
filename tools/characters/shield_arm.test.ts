// The shield arm fitted to each body after the clips (shield_arm.ts, D554): on main's assets the
// guard's board lands on its target on every template, and a strap moved 2 cm leaves it there.
// The solve on a synthetic arm is terra_walker::shield_arm's tests'.

import { test } from "node:test";
import assert from "node:assert/strict";
import { Glb, type Pose, type V3, len, posed, sub } from "./glb.ts";
import { frameOf, hang } from "./held.ts";
import { FAR, NEAR, fitter, weightOf } from "./shield_arm.ts";
import { boardPlace, placeOff, readBoard, readTargets } from "./shield_place.ts";
import { shaped, skinOf } from "./skin.ts";

const body = new Glb("assets/models/character/base.glb");
const clips = new Glb("assets/models/character/fight.glb");
const rest = posed(body, undefined, 0);
const shield = hang(body, rest, "socket.hand.L", readBoard()!.shield);
const { shapes } = skinOf(body, "assets/models/character/templates.json");
const fit = fitter(body, rest, shield);
const [spine, lower, hand, fist] = ["spine_03", "lowerarm_l", "hand_l", "socket.hand.L"].map((n) => body.named(n));
const placeOf = (pose: Pose[]) => boardPlace(pose, rest, spine, frameOf(shield, pose, (n) => body.named(n), (i) => body.parentOf[i]), shield);
const target = readTargets().Low_Guard;
const guard = clips.clip("Low_Guard")!;

test("a board is pulled wholly onto a target near it and not at all from far", () => {
  assert.equal(weightOf(0), 1);
  assert.equal(weightOf(NEAR), 1);
  assert.equal(weightOf(FAR), 0);
  const mid = weightOf((NEAR + FAR) / 2);
  assert.ok(mid > 0.4 && mid < 0.6, `${mid}`);
});

test("the guard's board lands on its target on every template, the female's included", () => {
  assert.ok(shapes.some((s) => s.name === "female"), "the templates include the female");
  for (const shape of shapes) {
    const off = placeOff(placeOf(fit(shaped(body, shape, guard, 0), shape.moved)), target);
    assert.ok(off.cm <= 1 && off.deg <= 3, `${shape.name}: ${off.cm.toFixed(2)} cm, ${off.deg.toFixed(2)}° off`);
  }
});

// The strap's fist moved `cm` along the forearm toward the elbow (less than 0: toward the wrist),
// the board riding with it: the guard's board off its target before the fit and after.
const strapMoved = (shape: (typeof shapes)[number], cm: number) => {
  const pose = shaped(body, shape, guard, 0);
  const way = sub(pose[lower].t, pose[hand].t);
  pose[fist] = { ...pose[fist], t: pose[fist].t.map((x, j) => x + (cm / 100) * (way[j] / len(way))) as V3 };
  return [placeOff(placeOf(pose), target), placeOff(placeOf(fit(pose, shape.moved)), target)];
};

test("a strap moved 2 cm along the forearm leaves the guard's board on its target", () => {
  // On the base body (the male template), either way: the bake's bar (D554 round 2).
  for (const cm of [2, -2]) {
    const [moved, off] = strapMoved(shapes.find((s) => s.name === "male")!, cm);
    assert.ok(moved.cm > 1.4, `the case moves the board (${moved.cm.toFixed(2)} cm)`);
    assert.ok(off.cm <= 0.5, `male, strap ${cm} cm: ${off.cm.toFixed(2)} cm off`);
  }
  // On every template toward the wrist. Toward the elbow the female's arm, 5.4 cm in at the
  // shoulder and 2 cm shorter in the forearm, does not reach (about 1.7 cm short).
  for (const shape of shapes) {
    const [, off] = strapMoved(shape, -2);
    assert.ok(off.cm <= 0.5, `${shape.name}, strap toward the wrist: ${off.cm.toFixed(2)} cm off`);
  }
});
