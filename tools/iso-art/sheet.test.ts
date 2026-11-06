// sheet.ts's tests, on frames made here whose content is known by construction: a figure on
// plain ground is found to its pixel (and a JPEG-like speckle is not content), and the shelf
// never lays a tile past the width nor over another.

import assert from "node:assert/strict";
import { test } from "node:test";
import { contentBox, crop, shelf, union } from "./sheet.ts";
import type { Rgb } from "./stats.ts";

function frame(width: number, height: number, px: (x: number, y: number) => [number, number, number]): Rgb {
  const data = new Uint8Array(width * height * 3);
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) data.set(px(x, y), (y * width + x) * 3);
  return { width, height, data };
}

test("a figure on plain ground is cropped to it and its margin, speckle left out", () => {
  // Grey ground, a dark figure at x 40..59, y 30..79, and lone bright pixels that are noise.
  const noise = new Set(["5,5", "90,12", "70,95"]);
  const im = frame(100, 100, (x, y) => (x >= 40 && x < 60 && y >= 30 && y < 80 ? [60, 40, 30] : noise.has(`${x},${y}`) ? [255, 255, 255] : [120, 120, 120]));
  assert.deepEqual(contentBox(im, 14, 0), [40, 30, 60, 80]);
  assert.deepEqual(contentBox(im, 14, 8), [32, 22, 68, 88]);
  const c = crop(im, contentBox(im, 14, 0));
  assert.equal(c.width * c.height * 3, c.data.length);
  assert.deepEqual([...c.data.subarray(0, 3)], [60, 40, 30]);
});

test("plain ground keeps the whole frame, and boxes join", () => {
  assert.deepEqual(contentBox(frame(20, 10, () => [90, 90, 90])), [0, 0, 20, 10]);
  assert.deepEqual(union([[5, 6, 10, 12], [2, 8, 7, 20]]), [2, 6, 10, 20]);
});

test("the shelf keeps every tile inside the width and apart", () => {
  const sizes = [300, 120, 500, 80, 260, 410, 90].map((w, i) => ({ w, h: 50 + ((i * 37) % 90) }));
  const { at, width, height } = shelf(sizes, 700, 10);
  assert.ok(width <= 700);
  at.forEach(([x, y], i) => {
    assert.ok(x + sizes[i].w <= 700 && y + sizes[i].h <= height);
    at.forEach(([x2, y2], j) => {
      if (j <= i) return;
      const apart = x + sizes[i].w <= x2 || x2 + sizes[j].w <= x || y + sizes[i].h <= y2 || y2 + sizes[j].h <= y;
      assert.ok(apart, `tiles ${i} and ${j} overlap`);
    });
  });
});
