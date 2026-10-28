import assert from "node:assert/strict";
import { test } from "node:test";
import { fitFlow, halves, warpError } from "./pack.ts";
import { fitFrame, packSmoke, puffOf, smokeFrame } from "./smoke.ts";
import { bakeBillow, half } from "./billow.ts";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Recipe } from "./bake.ts";

test("a half float written is the half float read", () => {
  const values = [0, 1, -2, 0.333251953125, 6.103515625e-5, 1e-6, 65504];
  const back = halves(Buffer.from(new Uint16Array(values.map(half)).buffer));
  values.forEach((v, i) => assert.ok(Math.abs(back[i] - v) <= Math.abs(v) * 1e-3 + 1e-7, `${v} -> ${back[i]}`));
});

test("a billow is opaque in its middle, clear at the frame's corner, and shades itself", () => {
  const out = mkdtempSync(join(tmpdir(), "billow-"));
  const r = { name: "t", size: 48, frames: 1, camera: { center: [0, 0], extent: 3 }, billow: { seed: 7, lumps: 4, density: 14, amplitude: 0.45, frequency: 2.6 } };
  bakeBillow(r as unknown as Recipe & { billow: typeof r.billow }, out);
  const data = halves(readFileSync(join(out, "raw.bin")));
  rmSync(out, { recursive: true });
  const n = 48;
  const per = n * n * 3;
  const fr = smokeFrame(data.subarray(0, per), data.subarray(per, 2 * per), data.subarray(2 * per, 3 * per), n * n);
  const mid = (n / 2) * n + n / 2;
  assert.ok(fr.alpha[mid] > 0.95, `middle alpha ${fr.alpha[mid]}`);
  assert.equal(fr.alpha[0], 0);
  // Lit from the top (map 0's G), its upper half is brighter than its lower; from below (map
  // 1's G) the other way round.
  const half_ = (map: Float32Array, top: boolean) => {
    let s = 0;
    for (let p = 0; p < n * n; p++) if (Math.floor(p / n) < n / 2 === top) s += map[p] * fr.alpha[p];
    return s;
  };
  assert.ok(half_(fr.maps[1], true) > 2 * half_(fr.maps[1], false), "lit from the top");
  assert.ok(half_(fr.maps[4], false) > 2 * half_(fr.maps[4], true), "lit from below");
});

test("a billow thins and opens as it ages: its last frame is see-through where its first is solid", () => {
  const out = mkdtempSync(join(tmpdir(), "billow-"));
  const r = { name: "t", size: 48, frames: 2, camera: { center: [0, 0], extent: 3 }, billow: { seed: 7, lumps: 4, density: 14, amplitude: 0.45, frequency: 2.6 } };
  bakeBillow(r as unknown as Recipe & { billow: typeof r.billow }, out);
  const data = halves(readFileSync(join(out, "raw.bin")));
  rmSync(out, { recursive: true });
  const n = 48;
  const per = n * n * 3;
  // The mean alpha over the texels a frame covers at all, and the share of them it covers
  // nearly solid.
  const cover = (f: number) => {
    const fr = smokeFrame(data.subarray((f * 3) * per, (f * 3 + 1) * per), data.subarray((f * 3 + 1) * per, (f * 3 + 2) * per), data.subarray((f * 3 + 2) * per, (f * 3 + 3) * per), n * n);
    const held = fr.alpha.filter((a) => a > 0.02);
    return { mean: held.reduce((s, a) => s + a, 0) / held.length, solid: held.filter((a) => a > 0.95).length / held.length };
  };
  const [young, old] = [cover(0), cover(1)];
  assert.ok(young.solid > 0.5, `young solid ${young.solid}`);
  assert.ok(old.solid < 0.1, `old solid ${old.solid}`);
  assert.ok(old.mean < 0.6 * young.mean, `mean alpha young ${young.mean}, old ${old.mean}`);
});

test("a fitted smoke set whose puff reaches its frame's border is refused, one inside it packed", () => {
  const raw = mkdtempSync(join(tmpdir(), "billow-"));
  const out = mkdtempSync(join(tmpdir(), "packed-"));
  const r = { name: "t", class: "smoke", kind: "oneshot", six_way: true, size: 48, frames: 1, columns: 1, camera: { center: [0, 0], extent: 3 }, billow: { seed: 7, lumps: 4, density: 14, amplitude: 0.2, frequency: 2.2 } };
  bakeBillow(r as unknown as Recipe & { billow: typeof r.billow }, raw);
  try {
    // Its 90% radius at 0.6 of the frame puts its outer lumps past the border; at 0.3, inside.
    assert.throws(() => packSmoke({ ...r, fit: { frame: 32, radius: 0.6 } } as unknown as Recipe, raw, out), /reaches its border/);
    packSmoke({ ...r, fit: { frame: 32, radius: 0.3 } } as unknown as Recipe, raw, out);
  } finally {
    rmSync(raw, { recursive: true });
    rmSync(out, { recursive: true });
  }
});

test("a six-way frame's alpha is the glow pass's G and each map its light over alpha", () => {
  // Two texels: smoke half covering, lit 0.1 from the right and 0.12 from the front; and none.
  const light0 = new Float32Array([0.1, 0.05, 0.02, 0, 0, 0]);
  const light1 = new Float32Array([0.03, 0.04, 0.12, 0, 0, 0]);
  const glow = new Float32Array([0.25, 0.5, 0, 0, 0, 0]);
  const fr = smokeFrame(light0, light1, glow, 2);
  assert.equal(fr.alpha[0], 0.5);
  assert.ok(Math.abs(fr.maps[0][0] - 0.2) < 1e-6 && Math.abs(fr.maps[5][0] - 0.24) < 1e-6);
  assert.ok(Math.abs(fr.glow[0] - 0.5) < 1e-6);
  assert.equal(fr.alpha[1], 0);
  assert.equal(fr.maps[0][1], 0);
});

test("half floats decode as IEEE 754 says", () => {
  const bits = new Uint16Array([0x3c00, 0xc000, 0x3555, 0x0001, 0x7bff, 0x0000]);
  const got = halves(Buffer.from(bits.buffer));
  assert.deepEqual([...got.slice(0, 2)], [1, -2]);
  assert.ok(Math.abs(got[2] - 1 / 3) < 1e-3);
  assert.equal(got[3], 2 ** -24);
  assert.equal(got[4], 65504);
  assert.equal(got[5], 0);
});

test("the flow that moved a picture carries it onto the next, and the fit finds it", () => {
  const n = 48;
  const blob = (cx: number, cy: number) =>
    Float32Array.from({ length: n * n }, (_, p) => Math.exp(-(((p % n) - cx) ** 2 + (Math.floor(p / n) - cy) ** 2) / 30));
  // The blob rises 3 texels a frame: flow (0, -3/n) frame widths.
  const a = blob(24, 30);
  const b = blob(24, 27);
  const mx = new Float32Array(n * n);
  const my = new Float32Array(n * n).fill(-3 / n);
  assert.ok(warpError(a, b, mx, my, n, 1) < 0.01 * warpError(a, b, mx, my, n, 0));
  const { k, gain } = fitFlow([[a, b, mx, my]], n);
  assert.ok(Math.abs(k - 1) < 0.05, `fitted ${k}`);
  assert.ok(gain < 0.01);
});

test("a fitted smoke frame has its puff in the middle at the share of the frame asked, its light kept", () => {
  // A disc of smoke radius 10 texels at (20, 44) of a 64-texel frame, lit 0.3 from the right.
  const n = 64;
  const alpha = new Float32Array(n * n);
  const maps = Array.from({ length: 6 }, () => new Float32Array(n * n));
  for (let p = 0; p < n * n; p++) {
    if (Math.hypot((p % n) + 0.5 - 20, Math.floor(p / n) + 0.5 - 44) < 10) {
      alpha[p] = 1;
      maps[0][p] = 0.3;
    }
  }
  const fr = { maps, alpha, glow: new Float32Array(n * n) };
  const found = puffOf(alpha, n);
  assert.ok(Math.abs(found.x - 20) < 0.2 && Math.abs(found.y - 44) < 0.2, `${found.x}, ${found.y}`);
  // 90% of a disc's area lies within 0.95 of its radius.
  assert.ok(Math.abs(found.radius - 9.5) < 0.6, `${found.radius}`);
  const fitted = fitFrame(fr, n, 32, 0.4);
  const puff = puffOf(fitted.alpha, 32);
  assert.ok(Math.abs(puff.x - 16) < 0.3 && Math.abs(puff.y - 16) < 0.3, `${puff.x}, ${puff.y}`);
  assert.ok(Math.abs(puff.radius - 0.4 * 32) < 1, `${puff.radius}`);
  // Inside the puff its light over alpha is what it was; its corners empty.
  assert.ok(Math.abs(fitted.maps[0][16 * 32 + 16] - 0.3) < 1e-4);
  assert.equal(fitted.alpha[0], 0);
});
