// The fire measurer's numbers on pictures whose answers are known by construction.
import assert from "node:assert/strict";
import { test } from "node:test";
import type { Picture } from "../sandbox/png.ts";
import { fromPpm, judge, measure, parseBox } from "./measure.ts";

type Rgb = [number, number, number];
const YELLOW: Rgb = [255, 200, 40];
const ORANGE: Rgb = [230, 130, 30];
const RED: Rgb = [200, 40, 20];
const DIM_RED: Rgb = [130, 40, 10];
const BLACK: Rgb = [10, 10, 10];
const GREY: Rgb = [128, 128, 128];

/// A picture painted pixel by pixel.
function paint(width: number, height: number, at: (x: number, y: number) => Rgb): Picture {
  const rgba = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++) {
      const [r, g, b] = at(x, y);
      rgba.set([r, g, b, 255], 4 * (y * width + x));
    }
  return { width, height, rgba };
}
const inside = (x: number, y: number, x0: number, y0: number, x1: number, y1: number) => x >= x0 && x < x1 && y >= y0 && y < y1;

test("a flame all of one hue is all of its band", () => {
  const m = measure(paint(64, 64, () => YELLOW));
  assert.equal(m.yellow, 100);
  assert.equal(m.redOrange, 0);
  assert.equal(m.flamePx, 64 * 64);
});

test("half red and half orange is half each, all red plus orange", () => {
  const m = measure(paint(64, 64, (x) => (x < 32 ? RED : ORANGE)));
  assert.equal(m.red, 50);
  assert.equal(m.orange, 50);
  assert.equal(m.redOrange, 100);
});

test("grey and dark pixels are not flame", () => {
  const m = measure(paint(64, 64, (x) => (x < 32 ? GREY : BLACK)));
  assert.equal(m.flamePx, 0);
});

test("a lit pink stone by a fire is not flame; the flame beside it is", () => {
  const STONE: Rgb = [252, 233, 222];
  const PINK: Rgb = [195, 114, 123];
  const m = measure(paint(64, 64, (x) => (x < 16 ? STONE : x < 32 ? PINK : YELLOW)));
  assert.equal(m.flamePx, 32 * 64);
  assert.equal(m.red, 0);
});

test("a flame ringed by black meets soot at all its edge; on grey, at none", () => {
  const ringed = measure(paint(100, 100, (x, y) => (inside(x, y, 30, 30, 70, 70) ? YELLOW : BLACK)));
  assert.ok(ringed.edgeSoot > 95, `edge soot ${ringed.edgeSoot}`);
  assert.ok(ringed.soot > 0.1, `soot ${ringed.soot}`);
  const clean = measure(paint(100, 100, (x, y) => (inside(x, y, 30, 30, 70, 70) ? YELLOW : GREY)));
  assert.equal(clean.edgeSoot, 0);
  assert.equal(clean.soot, 0);
});

// Scaled to a fixed height, the wide one read a third the tall one's soot by construction
// (D433): a 3:1 flame's border to the dark round it is the same however it is turned.
test("a wide, low flame and a tall, narrow one ringed alike by black score the same soot", () => {
  const wide = measure(paint(200, 120, (x, y) => (inside(x, y, 40, 45, 160, 85) ? YELLOW : BLACK)));
  const tall = measure(paint(120, 200, (x, y) => (inside(x, y, 45, 40, 85, 160) ? YELLOW : BLACK)));
  assert.ok(wide.soot > 0.1, `soot ${wide.soot}`);
  assert.ok(Math.abs(wide.soot - tall.soot) < 0.01 * tall.soot, `wide ${wide.soot}, tall ${tall.soot}`);
  assert.ok(Math.abs(wide.edgeSoot - tall.edgeSoot) < 1, `edge soot wide ${wide.edgeSoot}, tall ${tall.edgeSoot}`);
});

test("one dark hole in a square flame is one pocket a flame-square, at any size", () => {
  for (const s of [40, 160]) {
    const pic = paint(s, s, (x, y) => (inside(x, y, 0.4 * s, 0.4 * s, 0.6 * s, 0.6 * s) ? BLACK : YELLOW));
    const m = measure(pic);
    assert.ok(Math.abs(m.pockets - 1) < 0.1, `size ${s}: pockets ${m.pockets}`);
  }
});

// The castle rounds' measure.mjs filled pockets at the photograph's own size, and on a
// frame-filling dark hole its fill outgrew an array ("Invalid array length"); here the fill
// is at the flame's 256 px square, so a hole of millions of pixels is one pocket like any other.
test("a dark hole filling most of a large flame is one pocket, measured at once", () => {
  const pic = paint(2400, 1800, (x, y) => (inside(x, y, 40, 40, 2360, 1760) ? BLACK : YELLOW));
  const started = Date.now();
  const m = measure(pic);
  assert.ok(Math.abs(m.pockets - 1) < 0.1, `pockets ${m.pockets}`);
  assert.ok(Date.now() - started < 3000, `took ${Date.now() - started} ms`);
});

test("a flame half as bright as its brightest part below the knee is dim there", () => {
  const m = measure(paint(64, 64, (x) => (x < 32 ? YELLOW : DIM_RED)));
  assert.equal(m.dim, 50);
});

test("fine stripes put more of the detail fine than a broad gradient does", () => {
  const stripes = measure(paint(128, 128, (x) => (x % 4 < 2 ? YELLOW : DIM_RED)));
  const broad = measure(paint(128, 128, (x) => (x < 64 ? YELLOW : DIM_RED)));
  assert.ok(stripes.fine > broad.fine + 20, `stripes ${stripes.fine}, broad ${broad.fine}`);
  assert.ok(broad.coarse > stripes.coarse, `broad ${broad.coarse}, stripes ${stripes.coarse}`);
});

test("a crop measures only what it holds", () => {
  const pic = paint(100, 50, (x) => (x < 50 ? RED : YELLOW));
  assert.equal(measure(pic, parseBox("40x50+60+0")).yellow, 100);
  assert.equal(measure(pic, parseBox("40x50+0+0")).red, 100);
});

test("a number out of its class's widened range fails, one inside holds", () => {
  const m = measure(paint(64, 64, () => YELLOW));
  const held = judge(m, { yellow: { median: 10, min: 5, max: 20 }, red: { median: 50, min: 0, max: 60 } });
  assert.deepEqual(
    held.map((h) => [h.called, h.holds]),
    [
      ["yellow %", false],
      ["red %", true],
    ],
  );
});

test("a PPM reads as the picture it holds", () => {
  const head = new TextEncoder().encode("P6\n2 1\n255\n");
  const pic = fromPpm(new Uint8Array([...head, 1, 2, 3, 4, 5, 6]));
  assert.deepEqual([pic.width, pic.height, [...pic.rgba]], [2, 1, [1, 2, 3, 255, 4, 5, 6, 255]]);
});

test("with the flame matte, lit ground of a flame's colour outside it is not flame (D433)", () => {
  // A red and orange flame in the upper half on black, the lower part yellow lit ground, which
  // the colour rule alone takes for flame; the matte white over the flame, soft at its edge.
  const flame = (x: number, y: number) => inside(x, y, 16, 4, 48, 30);
  const pic = paint(64, 64, (x, y) => (flame(x, y) ? (x < 32 ? RED : ORANGE) : y >= 40 ? YELLOW : BLACK));
  const matte = paint(64, 64, (x, y) => (flame(x, y) ? [255, 255, 255] : inside(x, y, 15, 3, 49, 31) ? [100, 100, 100] : [0, 0, 0]));
  assert.ok(measure(pic).yellow > 50, "without the matte the ground counts");
  const m = measure(pic, undefined, matte);
  assert.equal(m.flamePx, 32 * 26);
  assert.equal(m.yellow, 0);
  assert.equal(m.red, 50);
  assert.equal(m.redOrange, 100);
  // A crop is cut from the matte where it is cut from the picture.
  const cropped = measure(pic, parseBox("40x60+12+2"), matte);
  assert.equal(cropped.flamePx, 32 * 26);
  assert.equal(cropped.yellow, 0);
});
