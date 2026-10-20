import assert from "node:assert/strict";
import { test } from "node:test";
import { BED_BUDGET, bed, HOTTEST, maps } from "./coals.ts";

test("a coal bed keeps to its triangle budget", () => {
  assert.ok(bed().idx.length / 3 <= BED_BUDGET);
});

test("its coals are a packed heap: the bed under the logs covered, highest in the middle", () => {
  const { coals } = bed();
  // Seen from above, three in four points within 0.2 m of the middle are under a coal (its
  // round size; its longer side covers more).
  let covered = 0, points = 0;
  for (let x = -0.2; x <= 0.2; x += 0.02) {
    for (let z = -0.2; z <= 0.2; z += 0.02) {
      if (Math.hypot(x, z) > 0.2) continue;
      points++;
      if (coals.some((c) => Math.hypot(c.at[0] - x, c.at[2] - z) < c.size)) covered++;
    }
  }
  assert.ok(covered / points > 0.75, `${covered} of ${points} covered`);
  // The heap's top, on average, within 0.1 m of the middle and past 0.25 m.
  const top = (from: number, to: number) => {
    const ring = coals.filter((c) => Math.hypot(c.at[0], c.at[2]) >= from && Math.hypot(c.at[0], c.at[2]) < to);
    return ring.reduce((sum, c) => sum + c.at[1] + c.size, 0) / ring.length;
  };
  assert.ok(top(0, 0.1) > top(0.25, 1) + 0.03, "no higher in the middle");
});

test("every face of the bed faces out, so none is culled", () => {
  const { pos, idx, coals } = bed();
  const p = (i: number) => [pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]];
  for (let t = 0; t < idx.length; t += 3) {
    const [a, b, c] = [p(idx[t]), p(idx[t + 1]), p(idx[t + 2])];
    const u = b.map((q, i) => q - a[i]), w = c.map((q, i) => q - a[i]);
    const n = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
    const mid = [0, 1, 2].map((i) => (a[i] + b[i] + c[i]) / 3);
    // The mound's faces look up; a coal's away from its middle.
    const coal = coals.findLast((k) => k.from <= t / 3);
    const out = coal ? mid.map((q, i) => q - coal.at[i]) : [0, 1, 0];
    assert.ok(n[0] * out[0] + n[1] * out[1] + n[2] * out[2] > 0, `triangle ${t / 3} faces in`);
  }
});

test("its glow is never hotter than 1,440 K's orange-red", () => {
  const emissive = maps()[1];
  let lit = 0;
  for (let i = 0; i < emissive.length; i += 4) {
    const [r, g, b] = [emissive[i], emissive[i + 1], emissive[i + 2]];
    if (r < 8) continue;
    lit++;
    assert.ok(g <= r * (HOTTEST[1] / HOTTEST[0]) + 1 && b <= r * (HOTTEST[2] / HOTTEST[0]) + 1, `${r},${g},${b}`);
  }
  assert.ok(lit > emissive.length / 4 / 20, "it hardly glows");
});
