import { test } from "node:test";
import assert from "node:assert/strict";
import { sporadic } from "./aa.ts";

// A 10x10 RGB picture, every channel `v`.
const flat = (v: number) => new Uint8Array(300).fill(v);

test("a frame whose light changed everywhere is sporadic, a flame-sized patch is not", () => {
  const base = flat(100);
  const patch = flat(100);
  for (let p = 0; p < 20; p++) patch[3 * p] = 200; // a fifth of the picture: the fire's patch
  const brighter = flat(101); // one step brighter in every pixel: a whole frame's exposure
  const pairs = [
    { frame: 8, a: base, b: brighter, w: 10 },
    { frame: 4, a: base, b: base, w: 10 },
    { frame: 6, a: base, b: patch, w: 10 },
    { frame: 2, a: brighter, b: base, w: 10 },
  ];
  assert.deepEqual(sporadic(pairs), [2, 8]);
});

test("the share decides: just over half the picture counts, just under does not", () => {
  const base = flat(0);
  const over = flat(0);
  const under = flat(0);
  for (let p = 0; p < 51; p++) over[3 * p + 2] = 9;
  for (let p = 0; p < 49; p++) under[3 * p + 1] = 9;
  assert.deepEqual(sporadic([{ frame: 1, a: base, b: over, w: 10 }, { frame: 3, a: base, b: under, w: 10 }]), [1]);
});
