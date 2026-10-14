// Each measure artifacts.ts holds frames to finds the fault planted in made-up frames, and
// passes frames without it. `node --test tools/sandbox/frames.test.ts`.

import assert from "node:assert/strict";
import { test } from "node:test";
import { type Grey, blur, changed, flicked, grey, jumped, mean, share, trailing } from "./frames.ts";
import { readPng, writePng } from "./png.ts";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const W = 64;
const H = 64;
/// A frame: a gradient, shifted so far to the right, with a patch so bright where given.
function frame(shift = 0, patch?: { x: number; y: number; side: number; value: number }): Grey {
  const v = new Float32Array(W * H);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) v[y * W + x] = 0.2 + (0.4 * ((x + shift) % W)) / W;
  if (patch) for (let y = patch.y; y < patch.y + patch.side; y++) for (let x = patch.x; x < patch.x + patch.side; x++) v[y * W + x] = patch.value;
  return { width: W, height: H, v };
}

test("a still picture is unchanged, and acne flickering on it is found", () => {
  assert.equal(share(changed(frame(), frame())), 0);
  // An eighth of a face flickering dark: 64 of 4,096 pixels.
  const acne = frame(0, { x: 10, y: 10, side: 8, value: 0.05 });
  assert.ok(Math.abs(share(changed(frame(), acne)) - (100 * 64) / (W * H)) < 1e-9);
});

test("a camera moving smoothly does not flicker; a patch there for one frame does", () => {
  const moving = [0, 1, 2].map((s) => blur(frame(s), 1));
  assert.equal(share(flicked(moving[0], moving[1], moving[2])), 0);
  const flash = blur(frame(1, { x: 20, y: 20, side: 6, value: 0.95 }), 1);
  assert.ok(share(flicked(moving[0], flash, moving[2])) > 0.5);
});

test("a piece settling in a frame does not trail; one fading in over frames does", () => {
  const before = frame();
  const after = frame(0, { x: 5, y: 5, side: 10, value: 0.9 });
  const quiet = changed(after, after);
  // In at once, then still.
  assert.equal(share(trailing(changed(before, after), quiet, quiet)), 0);
  // A third of the way each frame, as a history that is not rejected trails.
  const steps = [0, 1, 2, 3].map((k) => {
    const g = frame();
    for (let i = 0; i < g.v.length; i++) g.v[i] += (k / 3) * (after.v[i] - g.v[i]);
    return g;
  });
  const d = [1, 2, 3].map((k) => changed(steps[k - 1], steps[k]));
  assert.ok(share(trailing(d[0], d[1], d[2])) > 2);
});

test("a room's light switched at once jumps the picture's brightness; faded it does not", () => {
  const lit = frame();
  const dim = { ...lit, v: lit.v.map((x) => x * 0.6) };
  assert.ok(jumped(mean(lit), mean(dim)) > 30);
  const step = { ...lit, v: lit.v.map((x) => x * 0.99) };
  assert.ok(jumped(mean(lit), mean(step)) < 2);
});

test("a picture written as a PNG reads back the same, and halves to its brightness", () => {
  const rgba = new Uint8Array(4 * 4 * 4);
  for (let i = 0; i < 16; i++) rgba.set([i * 16, 255 - i * 16, 7, 255], i * 4);
  const path = join(mkdtempSync(join(tmpdir(), "png-")), "a.png");
  writePng(path, { width: 4, height: 4, rgba });
  const back = readPng(path);
  assert.deepEqual([back.width, back.height], [4, 4]);
  assert.deepEqual([...back.rgba], [...rgba]);
  assert.equal(grey(back).v.length, 4);
});
