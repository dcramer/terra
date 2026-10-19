import assert from "node:assert/strict";
import { test } from "node:test";
import { fitFlow, halves, warpError } from "./pack.ts";
import { smokeFrame } from "./smoke.ts";

test("a six-way frame's alpha is the glow pass's G and each map its light over alpha", () => {
  // Two texels: smoke half covering, lit 0.1 from the right and 0.12 from the front; and none.
  const light0 = new Float32Array([0.1, 0.05, 0.02, 0, 0, 0]);
  const light1 = new Float32Array([0.03, 0.04, 0.12, 0, 0, 0]);
  const glow = new Float32Array([0.25, 0.5, 0, 0, 0, 0]);
  const fr = smokeFrame(light0, light1, glow, 2);
  assert.equal(fr.alpha[0], 0.5);
  assert.ok(Math.abs(fr.maps[0][0] - 0.2) < 1e-6 && Math.abs(fr.maps[5][0] - 0.24) < 1e-6);
  assert.ok(Math.abs(fr.glow[0] - 0.5) < 1e-6);
  assert.equal(fr.alpha[1], 0);
  assert.equal(fr.maps[0][1], 0);
});

test("half floats decode as IEEE 754 says", () => {
  const bits = new Uint16Array([0x3c00, 0xc000, 0x3555, 0x0001, 0x7bff, 0x0000]);
  const got = halves(Buffer.from(bits.buffer));
  assert.deepEqual([...got.slice(0, 2)], [1, -2]);
  assert.ok(Math.abs(got[2] - 1 / 3) < 1e-3);
  assert.equal(got[3], 2 ** -24);
  assert.equal(got[4], 65504);
  assert.equal(got[5], 0);
});

test("the flow that moved a picture carries it onto the next, and the fit finds it", () => {
  const n = 48;
  const blob = (cx: number, cy: number) =>
    Float32Array.from({ length: n * n }, (_, p) => Math.exp(-(((p % n) - cx) ** 2 + (Math.floor(p / n) - cy) ** 2) / 30));
  // The blob rises 3 texels a frame: flow (0, -3/n) frame widths.
  const a = blob(24, 30);
  const b = blob(24, 27);
  const mx = new Float32Array(n * n);
  const my = new Float32Array(n * n).fill(-3 / n);
  assert.ok(warpError(a, b, mx, my, n, 1) < 0.01 * warpError(a, b, mx, my, n, 0));
  const { k, gain } = fitFlow([[a, b, mx, my]], n);
  assert.ok(Math.abs(k - 1) < 0.05, `fitted ${k}`);
  assert.ok(gain < 0.01);
});
