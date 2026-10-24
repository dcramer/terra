import assert from "node:assert/strict";
import { test } from "node:test";
import { paced, strike } from "./phases.ts";
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
