import assert from "node:assert/strict";
import { test } from "node:test";
import { blackbody, ramp } from "./ramp.ts";

test("the ramp is a blackbody's colour: white at D65, red with no blue at 1000 K", () => {
  const d65 = blackbody(6504);
  for (const c of d65) assert.ok(Math.abs(c - 1) < 0.05, `D65 ${d65}`);
  const [r, g, b] = blackbody(1000);
  assert.ok(r > 4 * g && b === 0);
});

test("the ramp brightens with heat, keeps its chroma as it dims and adds light when hot", () => {
  let last = -1;
  for (let h = 0; h <= 1; h += 0.1) {
    const [r, g, b] = ramp(h);
    const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    assert.ok(lum > last);
    last = lum;
    assert.ok(r > g && g >= b, "red leads at every heat, never grey");
  }
  assert.equal(ramp(0)[3], 1);
  assert.equal(ramp(1)[3], 0);
});
