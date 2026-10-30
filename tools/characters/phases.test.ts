import assert from "node:assert/strict";
import { test } from "node:test";
import { contactOf, paced, strike } from "./phases.ts";
import type { V3 } from "./glb.ts";

const STEP = 1 / 60;
// A tip a metre ahead, going `at(t)` metres to the left at time t, for `seconds`.
const path = (seconds: number, at: (t: number) => number) =>
  [...Array(Math.round(seconds / STEP) + 1).keys()].map((i) => ({ t: i * STEP, tip: [at(i * STEP), 0, 1] as V3 }));
const bar = { speed: 4, ahead: 0.5 };

test("a sweep that lasts is a strike, its span the frames at pace, its fastest moment in it", () => {
  // 1.5 m across between 0.5 and 0.8 s, 5 m/s; still before and after.
  const p = strike(paced(path(1.2, (t) => Math.min(1.5, Math.max(0, (t - 0.5) * 5))), STEP), bar);
  assert.ok(p.active, "a sweep strikes");
  assert.ok(p.active[0] >= 0.45 && p.active[0] <= 0.55 && p.active[1] >= 0.75 && p.active[1] <= 0.85, `span ${p.active}`);
  assert.ok(p.fastest!.t >= p.active[0] && p.fastest!.t <= p.active[1]);
  assert.equal(p.jumps.length, 0);
});

test("a tip that jumps for a frame, out and back, at 48 m/s is a jump, not a strike", () => {
  const pop = strike(paced(path(1.2, (t) => (Math.abs(t - 0.6) < STEP / 2 ? 0.8 : 0)), STEP), bar);
  assert.equal(pop.active, undefined);
  assert.ok(pop.jumps.some((f) => Math.abs(f.t - 0.6) < STEP && f.speed > 40), "the pop is listed as a jump");
});

test("a two-frame jump to a new place is no strike either", () => {
  const step = strike(paced(path(1.2, (t) => (t > 0.6 ? 1.6 : t > 0.58 ? 0.8 : 0)), STEP), bar);
  assert.equal(step.active, undefined);
  assert.ok(step.jumps.length > 0);
});

test("a pop beside a real sweep leaves the sweep the strike", () => {
  // The pop, at 0.2 s, is faster than the sweep; the sweep, 0.6 to 0.9 s, is still the strike.
  const p = strike(paced(path(1.2, (t) => (Math.abs(t - 0.2) < STEP / 2 ? 0.8 : Math.min(1.5, Math.max(0, (t - 0.6) * 5)))), STEP), bar);
  assert.ok(p.active && p.active[0] > 0.5, `span ${p.active}`);
  assert.equal(p.jumps.length > 0, true);
});

test("a fast sweep round the body strikes over the few frames its tip passes the front", () => {
  // A bought take's blow: the tip swung round the hips on a 1.2 m circle, a half turn from the
  // right side through the front to the left in 0.1 s, eased in and out (57 m/s at its fastest),
  // ahead of 0.5 m for about 50 ms, under STRIKE_S, of a sweep at pace for longer.
  const ease = (u: number) => u * u * (3 - 2 * u);
  const turn = (t: number) => Math.PI * ease(Math.min(1, Math.max(0, (t - 0.5) / 0.1)));
  const frames = [...Array(73).keys()].map((i) => ({ t: i * STEP, tip: [1.2 * Math.cos(turn(i * STEP)), 0, 1.2 * Math.sin(turn(i * STEP))] as V3 }));
  const p = strike(paced(frames, STEP), bar);
  assert.ok(p.active, "the sweep strikes");
  assert.ok(p.active[0] > 0.505 && p.active[1] < 0.595, `span ${p.active}: the frames ahead, inside the sweep`);
  assert.ok(p.fastest!.tip[2] > 0.5 && p.fastest!.pace > 25, `fastest ${p.fastest!.pace} m/s, ${p.fastest!.tip[2]} m ahead`);
  assert.equal(p.jumps.length, 0, "a sweep, not a jump");
});

test("a blow lands where its tip is furthest ahead: a sweep at the front, a thrust at full reach", () => {
  // A sweep round the body, 1.3 m from the hips, right to left; fastest past the front, 60 deg
  // round the left, as a bought take's often is.
  const sweep = [-90, -45, 0, 30, 60, 90].map((deg, i) => {
    const r = (deg * Math.PI) / 180;
    return { t: i / 60, tip: [1.3 * Math.sin(r), 0, 1.3 * Math.cos(r)] as V3, speed: deg === 60 ? 80 : 30 };
  });
  const fastest = sweep.reduce((a, b) => (b.speed > a.speed ? b : a));
  assert.ok(Math.abs((Math.atan2(fastest.tip[0], fastest.tip[2]) * 180) / Math.PI) > 35, "its fastest frame is off to the side");
  const hit = contactOf(sweep)!;
  assert.ok(Math.abs(hit.tip[0]) < 1e-6 && Math.abs(hit.tip[2] - 1.3) < 1e-6, `lands ahead: ${hit.tip}`);
  // A thrust out to the right of ahead and in to 1.75 m, then drawn back.
  const thrust = [0.2, 0.9, 1.5, 1.75, 1.6, 1.2].map((z, i) => ({ t: i / 60, tip: [-0.3, 0.3, z] as V3 }));
  assert.equal(contactOf(thrust)!.tip[2], 1.75);
  assert.equal(contactOf([]), undefined);
});
