// stats.ts's tests, on images made here whose measures are known by construction: a PNG
// written with each row's filter in turn decodes to its pixels, a flat grey measures its own
// luminance with no detail and no saturation, and a frame half black and half white keeps
// its halves through Pillow's Lanczos downscale. (stats.py's own numbers on a still match to
// the fourth place: the README.)

import assert from "node:assert/strict";
import { test } from "node:test";
import { deflateSync } from "node:zlib";
import { decodePng, measure, resizeLanczos } from "./stats.ts";

/** An RGB PNG whose rows use filters none, sub and up in turn (no CRCs: the decoder skips them). */
function png(width: number, height: number, px: (x: number, y: number) => [number, number, number]): Buffer {
  const stride = width * 3;
  const rows: number[] = [];
  let prev = new Array(stride).fill(0);
  for (let y = 0; y < height; y++) {
    const line: number[] = [];
    for (let x = 0; x < width; x++) line.push(...px(x, y));
    const filter = y % 3;
    rows.push(filter);
    for (let i = 0; i < stride; i++) {
      const pred = filter === 1 ? (i >= 3 ? line[i - 3] : 0) : filter === 2 ? prev[i] : 0;
      rows.push((line[i] - pred) & 255);
    }
    prev = line;
  }
  const chunk = (kind: string, body: Buffer) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(body.length);
    return Buffer.concat([len, Buffer.from(kind, "latin1"), body, Buffer.alloc(4)]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr.set([8, 2, 0, 0, 0], 8);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(Buffer.from(rows))),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

test("a PNG decodes to its pixels through every row filter", () => {
  const im = decodePng(png(7, 6, (x, y) => [x * 30, y * 40, (x * y * 13) & 255]));
  assert.equal(im.width, 7);
  for (let y = 0; y < 6; y++) {
    for (let x = 0; x < 7; x++) {
      assert.deepEqual([...im.data.subarray((y * 7 + x) * 3, (y * 7 + x) * 3 + 3)], [x * 30, y * 40, (x * y * 13) & 255]);
    }
  }
});

test("a flat grey measures its own luminance, no detail and no saturation", () => {
  const s = measure(decodePng(png(960, 540, () => [128, 128, 128])));
  const l = ((128 / 255 + 0.055) / 1.055) ** 2.4;
  assert.ok(Math.abs(s.luma_p50 - l) < 1e-4 && Math.abs(s.luma_mean - l) < 1e-4, `${s.luma_p50} against ${l}`);
  assert.equal(s.fine_detail, 0);
  assert.equal(s.saturation, 0);
  assert.equal(s.mid_share, 1);
});

test("Lanczos keeps a half-black, half-white frame's halves", () => {
  const im = resizeLanczos(decodePng(png(1920, 1080, (x) => (x < 960 ? [0, 0, 0] : [255, 255, 255]))), 960, 540);
  assert.equal(im.data[(270 * 960 + 100) * 3], 0);
  assert.equal(im.data[(270 * 960 + 900) * 3], 255);
  const s = measure(im);
  assert.ok(Math.abs(s.dark_share - 0.5) < 0.01 && Math.abs(s.bright_share - 0.5) < 0.01, JSON.stringify(s));
});
