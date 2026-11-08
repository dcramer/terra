import assert from "node:assert/strict";
import { test } from "node:test";
import { crop, sheet, small } from "./sheet.ts";
import type { Picture } from "./png.ts";

/// A picture of one colour.
const flat = (width: number, height: number, [r, g, b]: number[]): Picture => {
  const rgba = new Uint8Array(width * height * 4);
  for (let at = 0; at < rgba.length; at += 4) rgba.set([r, g, b, 255], at);
  return { width, height, rgba };
};
const pixel = (picture: Picture, x: number, y: number) => [...picture.rgba.subarray((y * picture.width + x) * 4, (y * picture.width + x) * 4 + 3)];

test("stills are laid out in order, so many to a row, with a dark line between", () => {
  const tiles = [[200, 0, 0], [0, 200, 0], [0, 0, 200]].map((colour) => flat(4, 3, colour));
  const laid = sheet(tiles, 2);
  assert.deepEqual([laid.width, laid.height], [4 * 2 + 2, 3 * 2 + 2]);
  assert.deepEqual(pixel(laid, 0, 0), [200, 0, 0]);
  assert.deepEqual(pixel(laid, 6, 0), [0, 200, 0]);
  assert.deepEqual(pixel(laid, 0, 5), [0, 0, 200]);
  assert.deepEqual(pixel(laid, 4, 0), [0, 0, 0], "the line between");
  assert.deepEqual(pixel(laid, 6, 5), [0, 0, 0], "the empty place after the last");
});

test("a still is cut to a part, made small by the mean of what each pixel covers, and lifted by stops", () => {
  const still = flat(8, 4, [40, 40, 40]);
  still.rgba.set([240, 40, 40, 255], (1 * 8 + 5) * 4);
  const part = crop(still, [4, 2, 4, 0]);
  assert.deepEqual([part.width, part.height], [4, 2]);
  assert.deepEqual(pixel(part, 1, 1), [240, 40, 40]);
  const half = small(part, 2);
  assert.deepEqual([half.width, half.height], [2, 1]);
  assert.deepEqual(pixel(half, 0, 0), [90, 40, 40], "the mean of three dark pixels and the bright one");
  assert.deepEqual(pixel(small(flat(2, 2, [40, 100, 200]), 2, 1), 0, 0), [80, 200, 255], "a stop doubles, to white at most");
});
