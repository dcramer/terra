import assert from "node:assert/strict";
import { test } from "node:test";
import { DXGI_BC5_UNORM, dds3d, decodeBc4, decodeBc5, encodeBc4, encodeBc5, readDds3d } from "./bc5.ts";

// Sixteen 3-bit indices, texel 0 in the lowest bits, as the block's last six bytes.
function indices(idx: number[]): number[] {
  let lo = 0;
  let hi = 0;
  for (let t = 0; t < 8; t++) lo |= idx[t] << (3 * t);
  for (let t = 0; t < 8; t++) hi |= idx[8 + t] << (3 * t);
  return [lo & 255, (lo >> 8) & 255, lo >> 16, hi & 255, (hi >> 8) & 255, hi >> 16];
}

test("a BC4 block decodes as Direct3D's palettes say", () => {
  // e0 > e1: eight entries, the six between in sevenths from e0.
  const eight = new Uint8Array([255, 0, ...indices([0, 1, 2, 3, 4, 5, 6, 7, 2, 2, 2, 2, 7, 7, 7, 7])]);
  const out = new Float32Array(16);
  decodeBc4(eight, 0, out);
  const want = [1, 0, 6 / 7, 5 / 7, 4 / 7, 3 / 7, 2 / 7, 1 / 7, 6 / 7, 6 / 7, 6 / 7, 6 / 7, 1 / 7, 1 / 7, 1 / 7, 1 / 7];
  for (let t = 0; t < 16; t++) assert.ok(Math.abs(out[t] - want[t]) < 1e-6, `texel ${t}: ${out[t]} for ${want[t]}`);
  // e0 <= e1: four between in fifths, then 0 and 1 whatever the endpoints.
  const six = new Uint8Array([50, 100, ...indices([0, 1, 2, 3, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0])]);
  decodeBc4(six, 0, out);
  const w6 = [50, 100, 60, 70, 80, 90].map((x) => x / 255).concat([0, 1]);
  for (let t = 0; t < 8; t++) assert.ok(Math.abs(out[t] - w6[t]) < 1e-6, `texel ${t}: ${out[t]} for ${w6[t]}`);
});

test("a block of palette values, one value, or two encodes exactly", () => {
  const out = new Uint8Array(8);
  const back = new Float32Array(16);
  const pal = [200, 40].concat([2, 3, 4, 5, 6, 7].map((i) => ((8 - i) * 200 + (i - 1) * 40) / 7)).map((x) => x / 255);
  const blocks = [
    Array.from({ length: 16 }, (_, t) => pal[(t * 5) % 8]),
    Array.from({ length: 16 }, () => 0.5),
    Array.from({ length: 16 }, (_, t) => (t % 3 ? 0 : 0.8)),
    Array.from({ length: 16 }, (_, t) => (t < 4 ? 0 : t < 8 ? 1 : 0.4)),
  ];
  for (const v of blocks) {
    encodeBc4(v, out, 0);
    decodeBc4(out, 0, back);
    // Exact to the 8-bit endpoints' rounding (0.5 / 255 at most for a one or two value block).
    for (let t = 0; t < 16; t++) assert.ok(Math.abs(back[t] - v[t]) <= 0.5 / 255 + 1e-6, `${v}: texel ${t} ${back[t]}`);
  }
});

test("random blocks come back within half a palette step", () => {
  let s = 12345;
  const rnd = () => ((s = (s * 1103515245 + 12345) >>> 0) / 2 ** 32);
  const out = new Uint8Array(8);
  const back = new Float32Array(16);
  for (let n = 0; n < 4000; n++) {
    const lo = rnd() * 0.6;
    const span = rnd() * (1 - lo);
    const v = Array.from({ length: 16 }, () => lo + rnd() * span);
    encodeBc4(v, out, 0);
    decodeBc4(out, 0, back);
    let sse = 0;
    let max = 0;
    for (let t = 0; t < 16; t++) {
      const e = Math.abs(back[t] - v[t]);
      sse += e * e;
      max = Math.max(max, e);
    }
    // Eight entries over the range: half a step is span / 14. The fit is least squares, so it
    // may trade a little of the largest error for less in all (by a fifth at most), plus the
    // endpoints' rounding; the rms no more than a uniform error's over one step.
    assert.ok(max <= (1.2 * span) / 14 + 1 / 255, `block ${n}: max error ${max} over a span of ${span}`);
    assert.ok(Math.sqrt(sse / 16) <= span / 14 / Math.sqrt(3) + 0.5 / 255, `block ${n}: rms ${Math.sqrt(sse / 16)}`);
  }
});

test("a 3D texture of two channels round-trips through BC5, the flame's edge kept", () => {
  // A blob of flame (falling to 0 off a sphere) and a temperature gradient up the volume.
  const w = 16;
  const h = 12;
  const d = 5;
  const r = new Float32Array(w * h * d);
  const g = new Float32Array(w * h * d);
  for (let z = 0; z < d; z++)
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        const i = (z * h + y) * w + x;
        const q = Math.hypot(x - 7.5, y - 5.5, (z - 2) * 2) / 6;
        r[i] = Math.max(0, 1 - q * q);
        g[i] = (z + y / h) / d;
      }
  const data = encodeBc5(r, g, w, h, d);
  assert.equal(data.length, (w / 4) * (h / 4) * d * 16);
  const [r2, g2] = decodeBc5(data, w, h, d);
  let sum = 0;
  let max = 0;
  for (let i = 0; i < r.length; i++) {
    const e = Math.max(Math.abs(r2[i] - r[i]), Math.abs(g2[i] - g[i]));
    sum += e;
    max = Math.max(max, e);
    // Where there is no flame it stays none: the edge does not glow.
    if (r[i] === 0) assert.equal(r2[i], 0, `texel ${i}: ${r2[i]} where there was no flame`);
  }
  assert.ok(sum / r.length < 2 / 255, `mean error ${(sum / r.length) * 255} / 255`);
  // The steepest blocks span nearly 0 to 1, so the random blocks' bound at a span of 1.
  assert.ok(max <= 1.2 / 14 + 1 / 255, `max error ${max * 255} / 255`);
});

test("the DDS file is a DX10 volume of the format and size it was given", () => {
  const data = new Uint8Array(2 * 2 * 3 * 16).map((_, i) => i & 255);
  const file = dds3d(DXGI_BC5_UNORM, 8, 8, 3, data);
  assert.equal(file.length, 148 + data.length);
  assert.equal(file.readUInt32LE(112) & 0x200000, 0x200000); // DDSCAPS2_VOLUME
  assert.equal(file.readUInt32LE(8) & 0x800000, 0x800000); // DDSD_DEPTH
  assert.equal(file.readUInt32LE(132), 4); // D3D10_RESOURCE_DIMENSION_TEXTURE3D
  assert.equal(file.readUInt32LE(140), 1); // one array element
  const back = readDds3d(file);
  assert.deepEqual([back.format, back.w, back.h, back.d], [DXGI_BC5_UNORM, 8, 8, 3]);
  assert.deepEqual([...back.data], [...data]);
  assert.throws(() => dds3d(DXGI_BC5_UNORM, 8, 8, 3, data.subarray(1)));
});
