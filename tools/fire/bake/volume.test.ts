import assert from "node:assert/strict";
import { test } from "node:test";
import { coarse, crossfade, flameHeight, seam, tile, tips, untile } from "./volume.ts";

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

// A flame of columns on a 20 x 20 x 40 grid: a broad body 20 cells tall, and on it three tongues
// of their own heights and one stub below three quarters of the flame's height.
function tongues(): { flame: Float32Array; vel: Float32Array; cells: [number, number, number] } {
  const cells: [number, number, number] = [20, 20, 40];
  const flame = new Float32Array(20 * 20 * 40);
  const vel = new Float32Array(flame.length * 3);
  const column = (x0: number, y0: number, r: number, top: number) => {
    for (let z = 0; z < top; z++)
      for (let y = y0 - r; y <= y0 + r; y++)
        for (let x = x0 - r; x <= x0 + r; x++) {
          // Rounded: the middle column is the highest, its neighbours a cell lower.
          const lower = Math.max(Math.abs(x - x0), Math.abs(y - y0));
          if (z < top - lower) flame[(z * 20 + y) * 20 + x] = 0.5;
        }
  };
  column(10, 10, 6, 20);
  column(4, 4, 1, 38);
  column(15, 5, 1, 34);
  column(14, 15, 1, 32);
  column(5, 15, 1, 24);
  // The gas rises at 4 cells a frame and drifts downwind at 1.
  for (let p = 0; p < flame.length; p++) [vel[p * 3], vel[p * 3 + 2]] = [1, 4];
  return { flame, vel, cells };
}

test("a frame's tongue tips are its highest local tops above three quarters of its height, with the gas's speed there", () => {
  const { flame, vel, cells } = tongues();
  const tall = 38;
  const listed = tips(flame, vel, cells, tall, 0.02, [-0.2, -0.2, 0], 0.02 * 30);
  // The three tongues, highest first; the stub (24 < 28.5) and the body's top are not tips.
  assert.deepEqual(
    listed.map((t) => t.slice(0, 3)),
    [
      [-0.2 + 4.5 * 0.02, -0.2 + 4.5 * 0.02, 0.76],
      [-0.2 + 15.5 * 0.02, -0.2 + 5.5 * 0.02, 0.68],
      [-0.2 + 14.5 * 0.02, -0.2 + 15.5 * 0.02, 0.64],
    ].map((t) => t.map((v) => Number(v.toFixed(3)))),
  );
  // 4 cells a frame of 2 cm at 30 frames a second is 2.4 m/s up, 1 is 0.6 downwind.
  for (const t of listed) assert.deepEqual(t.slice(3), [0.6, 0, 2.4]);
  // A flame with only its body has no tongue above three quarters of it, and no frame lists more than three.
  const body = new Float32Array(flame.length);
  for (let p = 0; p < 20 * 20 * 15; p++) body[p] = 0.5;
  assert.deepEqual(tips(body, vel, cells, 38, 0.02, [0, 0, 0], 1), []);
  assert.ok(tips(flame, vel, cells, 20, 0.02, [0, 0, 0], 1).length === 3);
});

test("the flame's height is the 98th percentile of its cells' tops over the loop", () => {
  const cells: [number, number, number] = [10, 10, 50];
  // Every cell of the lowest 30 levels, and one column to 50: 3000 cells under 30, 20 over.
  const f = new Float32Array(10 * 10 * 50);
  for (let p = 0; p < 10 * 10 * 30; p++) f[p] = 1;
  for (let z = 30; z < 50; z++) f[z * 100] = 1;
  assert.equal(flameHeight([f], cells), 30);
  // Below the edge is not flame.
  assert.equal(flameHeight([f.map((x) => x * 0.05)], cells), 0);
});
