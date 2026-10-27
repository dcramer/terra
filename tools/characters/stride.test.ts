import assert from "node:assert/strict";
import { test } from "node:test";
import { measure, type Sample } from "./stride.ts";

// A made walk whose answers are known: a 2 s loop, the left foot down at 0 s and the right at
// 1 s, each on the ground 1.2 s and lifted 0.3 m (a sine) over its 0.8 s swing; planted feet
// pass back at 1 m/s; the hips lowest 0.2 s after each contact, 0.15 m under it; the crown
// lowest (against the hips) 0.3 s after that.
function walk(n: number): Sample[] {
  const out: Sample[] = [];
  for (let k = 0; k < n; k++) {
    const t = (k * 2) / n;
    const foot = (from: number) => {
      const p = (((t - from) % 2) + 2) % 2;
      // A heavy foot comes down steeply: within ON_GROUND_M of the ground only in its last frame.
      const up = p < 1.2 ? 0 : 0.3 * Math.sqrt(Math.sin((Math.PI * (p - 1.2)) / 0.8));
      const ahead = p < 1.2 ? 0.6 - p : -0.6 + (1.2 * (p - 1.2)) / 0.8;
      return { ankle: [0.2, 0.1 + up, ahead] as [number, number, number], ball: [0.2, 0.03 + up, ahead + 0.15] as [number, number, number] };
    };
    const sink = (lag: number) => -0.15 * Math.cos((2 * Math.PI * (t - lag)) / 1);
    const hips = 1 + sink(0.2) + 0.15 * Math.cos((2 * Math.PI * 0.2) / 1) - 0.15;
    out.push({ t, feet: { l: foot(0), r: foot(1) }, hips: [0, hips, 0], crown: [0, hips + 1 + sink(0.5) * 0.5, 0] });
  }
  return out;
}

test("a made walk's footfalls, clearance, sink, lag and pace are measured as made", () => {
  const m = measure(walk(240), 2);
  // A foot is down once within ON_GROUND_M of its lowest: a few hundredths before it touches.
  const near = (t: number, want: number) => Math.abs(((t - want + 3) % 2) - 1) < 0.05;
  assert.deepEqual(m.falls.map((f) => f.side).sort(), ["l", "r"]);
  for (const f of m.falls) assert.ok(near(f.at, f.side === "l" ? 0 : 1), `${f.side} down at ${f.at}`);
  for (const f of m.falls) {
    assert.ok(Math.abs(f.clearance - 0.3) < 0.01, `clearance ${f.clearance}`);
    assert.ok(Math.abs(f.lowest - 0.2) < 0.05, `lowest ${f.lowest}`);
    assert.ok(Math.abs(f.drop - 0.15 * (1 - Math.cos(2 * Math.PI * 0.2))) < 0.02, `drop ${f.drop}`);
    assert.ok(Math.abs(f.lag - 0.3) < 0.03, `lag ${f.lag}`);
    assert.ok(Math.abs(f.ahead - 0.6) < 0.05, `ahead ${f.ahead}`);
  }
  assert.ok(Math.abs(m.pace - 1) < 0.02, `pace ${m.pace}`);
});
