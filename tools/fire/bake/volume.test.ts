import assert from "node:assert/strict";
import { test } from "node:test";
import { coarse, crossfade, seam, tile, untile } from "./volume.ts";

// A slowly wandering field: each frame the last plus a little noise, stronger and weaker by turns
// as a gusting fire's is, so a loop cut from it jumps at the seam by the whole drift over the
// loop, unless the crossfade hides it.
function wander(frames: number, n: number, seed = 7): Float32Array[] {
  let s = seed;
  const rnd = () => ((s = (s * 1103515245 + 12345) >>> 0) / 2 ** 32) - 0.5;
  const out: Float32Array[] = [];
  let cur = new Float32Array(n).map(() => 0.5 + rnd() * 0.2);
  for (let f = 0; f < frames; f++) {
    const gust = 1 + 0.6 * Math.sin(f / 4);
    cur = cur.map((x) => Math.min(1, Math.max(0, x + rnd() * 0.04 * gust)));
    out.push(cur);
  }
  return out;
}

test("the crossfade blends the frames after the loop into its start, so its seam is the sim's own step", () => {
  const raw = wander(70, 64);
  const loop = crossfade(raw, 60, 10);
  assert.equal(loop.length, 60);
  // Frame 3 is a tenth-and-a-bit of the way back to the start: raw[63] (1 - t) + raw[3] t, t = 4/11.
  const t = 4 / 11;
  for (let p = 0; p < 64; p++) assert.ok(Math.abs(loop[3][p] - (raw[63][p] * (1 - t) + raw[3][p] * t)) < 1e-6);
  // Past the fade the frames are the sim's own.
  assert.equal(loop[10], raw[10]);
  assert.throws(() => crossfade(raw, 65, 10));
});

test("a cut loop fails the seam's check and the crossfaded one passes", () => {
  const raw = wander(260, 512);
  const cut = seam(raw.slice(0, 200));
  assert.ok(!cut.pass && cut.ratio > 3, `the cut loop's seam is ${cut.ratio.toFixed(2)}x its mean step`);
  const faded = seam(crossfade(raw, 200, 30));
  assert.ok(faded.pass, `the crossfaded seam is ${faded.ratio.toFixed(2)}x its mean step`);
});

test("frames tiled into one texture come back out, and averaged blocks are the cells' mean", () => {
  const cells: [number, number, number] = [4, 8, 3];
  const frames = Array.from({ length: 7 }, (_, f) => new Float32Array(96).map((_, i) => f * 1000 + i));
  const { data, size } = tile(frames, cells, 3);
  assert.deepEqual(size, [12, 24, 3]);
  // Frame 4 is column 1, row 1: its cell (x 2, y 5, z 1) at texel (6, 13, 1).
  assert.equal(data[(1 * 24 + 13) * 12 + 6], 4000 + (1 * 8 + 5) * 4 + 2);
  const back = untile(data, size, cells, 3, 7);
  for (let f = 0; f < 7; f++) assert.deepEqual([...back[f]], [...frames[f]]);
  const [c] = coarse([new Float32Array(96).map((_, i) => (i < 48 ? 1 : 3))], cells, 2);
  // 2 x 4 x 1 blocks of 2 cells a side over z 0-1 (z 2 dropped): 1s below, the upper half's
  // last four rows of z 1 3s.
  assert.deepEqual([...c], [1, 1, 1, 1, 2, 2, 2, 2]);
});
