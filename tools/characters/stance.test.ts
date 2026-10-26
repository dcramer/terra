import assert from "node:assert/strict";
import { join } from "node:path";
import { test } from "node:test";
import { type Clip, Glb, posed, qinv, qmul, rot, sub, type Q } from "./glb.ts";
import { median, stance } from "./stance.ts";

const body = new Glb(join(import.meta.dirname, "..", "..", "assets", "models", "character", "base.glb"));
const rest = posed(body, undefined, 0);

/** A second-long clip holding bone `name` turned by `q` about the model's axes at its joint. */
function turned(name: string, q: Q, t?: number[]): Clip {
  const i = body.named(name);
  const parent = rest[body.parentOf[i]].r;
  const r = qmul(qmul(qmul(qinv(parent), q), parent), body.rest(i).r);
  return { name, end: 1, tracks: new Map([[name, { r: () => r, ...(t ? { t: () => t } : {}) }]]) };
}

const about = (axis: [number, number, number], deg: number): Q => {
  const h = (deg * Math.PI) / 360;
  return [axis[0] * Math.sin(h), axis[1] * Math.sin(h), axis[2] * Math.sin(h), Math.cos(h)];
};

test("a body held at rest stands as at rest", () => {
  const s = stance(body, { name: "rest", end: 1, tracks: new Map() });
  assert.ok(Math.abs(s.knee - s.kneeRest) < 1e-9);
  assert.equal(s.kneeStep, 0);
  assert.deepEqual(s.pelvis.map((x) => Math.abs(x) < 1e-9), [true, true]);
  assert.ok(Math.abs(s.head) < 1e-9);
});

test("a body turned on the ground and lowered 2 cm measures its hips 2 cm low and nothing else", () => {
  // The skeleton's root turned a quarter about up, and moved down 2 cm in the model (its parent
  // scales by the body's size, so the step is divided by it).
  const root = body.named("root");
  const scale = rest[body.parentOf[root]].s[1];
  const t = sub(body.rest(root).t, [0, 0.02 / scale, 0]);
  const s = stance(body, turned("root", about([0, 1, 0], 90), t));
  assert.ok(Math.abs(s.knee - s.kneeRest) < 1e-6);
  assert.ok(Math.abs(s.pelvis[0] + 2) < 1e-6 && Math.abs(s.pelvis[1] + 2) < 1e-6);
  assert.ok(Math.abs(s.head) < 1e-6, `head ${s.head}`);
});

test("a trunk bowed forward at the waist puts the head forward by the bow, the knees untouched", () => {
  // 10 degrees about the body's left (+x; forward is +z): the head swings about the waist.
  const q = about([1, 0, 0], 10);
  const waist = rest[body.named("spine_01")].t;
  const fromWaist = sub(rest[body.named("Head")].t, waist);
  const expected = (rot(q, fromWaist)[2] - fromWaist[2]) * 100;
  const s = stance(body, turned("spine_01", q));
  assert.ok(expected > 5, `a 10 degree bow moves the head ${expected} cm`);
  assert.ok(Math.abs(s.head - expected) < 0.05, `head ${s.head} cm, expected ${expected}`);
  assert.ok(Math.abs(s.knee - s.kneeRest) < 1e-6);
});

test("a shin bent back at the knee bends the knee by as much", () => {
  // Turned about the line across the knee, square to the thigh and shin, so the bend adds.
  const [h, k, f] = ["thigh_l", "calf_l", "foot_l"].map((b) => rest[body.named(b)].t);
  const [a, b] = [sub(k, h), sub(f, k)];
  const across = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const l = Math.hypot(...across);
  const s = stance(body, turned("calf_l", about([across[0] / l, across[1] / l, across[2] / l] as [number, number, number], 20)));
  // The right knee stays at rest, the left bends 20 more: the median of both sides' samples
  // is their middle.
  assert.ok(Math.abs(s.kneeMost - (s.kneeRest + 20)) < 0.5, `most ${s.kneeMost}, rest ${s.kneeRest}`);
  assert.ok(Math.abs(s.knee - (s.kneeRest + 10)) < 0.5);
});

test("the median is the middle value, or the mean of the middle two", () => {
  assert.equal(median([3, 1, 2]), 2);
  assert.equal(median([4, 1, 3, 2]), 2.5);
});
