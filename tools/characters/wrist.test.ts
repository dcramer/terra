import { test } from "node:test";
import assert from "node:assert/strict";
import { type Q, type V3, qmul, rot } from "./glb.ts";
import { bendOf, edgeLead, handAxes, turnBetween, wristWorst, type WristPose } from "./wrist.ts";

const about = (axis: V3, degrees: number): Q => {
  const h = (degrees * Math.PI) / 360;
  return [axis[0] * Math.sin(h), axis[1] * Math.sin(h), axis[2] * Math.sin(h), Math.cos(h)];
};
const id: Q = [0, 0, 0, 1];
// A right arm out along +X, palm down (-Y), the thumb ahead (+Z): the T-pose's.
const axes = handAxes(id, [0, 0, 0], [0.1, 0, 0], [0.03, -0.01, 0.04], [0, -1, 0]);
const elbow: V3 = [-0.25, 0, 0];
const near = (a: number, b: number) => assert.ok(Math.abs(a - b) < 1e-6, `${a} is not ${b}`);

test("a hand bent toward its palm is flexed, toward its thumb deviated radially", () => {
  // The fingers turned down toward the palm about the hand's across (+Z): flexion.
  near(bendOf(elbow, [0, 0, 0], about([0, 0, 1], -60), axes).flex, 60);
  near(bendOf(elbow, [0, 0, 0], about([0, 0, 1], 40), axes).flex, -40);
  // Turned toward the thumb (+Z) about the palm's normal: radial.
  near(bendOf(elbow, [0, 0, 0], about([0, 1, 0], -25), axes).dev, 25);
  near(bendOf(elbow, [0, 0, 0], about([0, 1, 0], 30), axes).dev, -30);
  // Twisted about the forearm (pronation): no bend.
  const twisted = bendOf(elbow, [0, 0, 0], about([1, 0, 0], 70), axes);
  near(twisted.flex, 0);
  near(twisted.dev, 0);
});

test("the worst of a take names its angles past the limits and its biggest turn in a key", () => {
  const key = (q: Q): WristPose => ({ elbow, wrist: [0, 0, 0], foreR: id, handR: q });
  const keys = [key(id), key(about([0, 0, 1], -50)), key(about([0, 0, 1], -100)), key(about([0, 1, 0], 10))];
  const w = wristWorst(keys, axes, { flex: 0, dev: 0 }, (i) => `key ${i}`);
  near(w.flex, 100);
  assert.equal(w.where.flex, "key 2");
  // Back from 100° of flexion to 10° radial in one key: the biggest turn.
  assert.ok(w.key > 95 && w.key < 105, `${w.key}`);
  near(w.over, w.key - 45);
  // Every key after the first: a 50° turn, 100° of flexion, the 100° turn back.
  assert.equal(w.keysOver, 3);
  // A forearm turned with its hand is no bend and no turn in the wrist.
  const turned = about([0, 1, 0], 80);
  const still = wristWorst([key(id), { elbow: rot(turned, elbow), wrist: [0, 0, 0], foreR: turned, handR: turned }], axes, { flex: 0, dev: 0 }, String);
  assert.ok(Math.abs(still.flex) < 1e-6 && still.key < 1e-3 && still.over < 0);
});

test("an edge leads when the tip moves toward its -Z, whatever the look is turned to", () => {
  const r = about([0, 1, 0], 90); // the look's -Z is the world's -X
  const toward = (v: V3) => ({ moved: v, r });
  near(edgeLead([toward([-1, 0, 0]), toward([-0.5, 0.3, 0])]).lead, 1);
  // Its flat (±X, the world's ±Z here) leading.
  const flat = edgeLead([toward([0, 0, 1]), toward([0, 0, -1])]);
  near(flat.lead, 0);
  near(flat.flat, 1);
  // A slow step (under half the top speed) does not count.
  near(edgeLead([toward([-1, 0, 0]), toward([0.4, 0, 0])]).lead, 1);
  assert.ok(turnBetween(id, qmul(r, r)) - 180 < 1e-6);
});

test("a thrust is judged by its point, not by which way its edge drifts", () => {
  const r = about([0, 1, 0], 90); // the look's -Z is the world's -X, its +X the world's -Z
  const toward = (v: V3) => ({ moved: v, r });
  // The point driven ahead (+Y, up here) with its back edge (+Z, the world's +X) drifting a little:
  // judged by the edge, it would lead with its back (-1).
  const thrust = edgeLead([toward([0.2, 1, 0]), toward([0.1, 0.9, 0])]);
  assert.ok(thrust.point);
  assert.ok(thrust.lead > 0.95 && thrust.lead <= 1, `${thrust.lead}`);
  near(thrust.flat, 0);
  // The point pulled back (-Y) is no thrust; a slash moving mostly across stays judged by its edge.
  assert.ok(!edgeLead([toward([0, -1, 0])]).point);
  const slash = edgeLead([toward([-1, 0.4, 0]), toward([-1, 0.3, 0])]);
  assert.ok(!slash.point);
  near(slash.lead, 1);
});
