import { test } from "node:test";
import assert from "node:assert/strict";
import { topRow } from "./tipstop.ts";

// A w x h mask with a filled box [x0, x1) x [y0, y1) per entry.
const mask = (w: number, h: number, boxes: number[][]) =>
  Array.from({ length: h }, (_, y) =>
    Array.from({ length: w }, (_, x) => boxes.some(([x0, y0, x1, y1]) => x >= x0 && x < x1 && y >= y0 && y < y1)),
  );

test("a lone flame's top is its first row", () => {
  assert.equal(topRow(mask(40, 60, [[10, 20, 30, 60]])), 20);
});

test("a small wisp above the flame does not set the top", () => {
  // body 20 x 40 = 800 px from row 20; wisp 4 x 2 = 8 px (1%) at row 3
  const m = mask(40, 60, [[10, 20, 30, 60], [15, 3, 19, 5]]);
  assert.equal(topRow(m), 20);
  assert.equal(topRow(m, 0), 3, "share 0 keeps the old rule");
});

test("a detached tongue big enough to count sets the top", () => {
  // body 800 px; a shed tongue 6 x 8 = 48 px (5.6%) at row 6
  assert.equal(topRow(mask(40, 60, [[10, 20, 30, 60], [14, 6, 20, 14]])), 6);
});

test("a row needs 3 pixels of a kept blob", () => {
  // body's first row is a 2-pixel tip at row 10, full from row 11
  assert.equal(topRow(mask(40, 60, [[19, 10, 21, 11], [10, 11, 30, 60]])), 11);
});

test("no flame gives -1", () => {
  assert.equal(topRow(mask(10, 10, [])), -1);
});
