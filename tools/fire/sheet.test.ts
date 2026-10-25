// The fire sheet's lit-ground number on pictures whose answers are known by construction.
import assert from "node:assert/strict";
import { test } from "node:test";
import type { Picture } from "../sandbox/png.ts";
import { judge, type Metrics } from "./measure.ts";
import { measure } from "./measure.ts";
import { calls, FIRES, floored, sampled, shotsOf, surround } from "./sheet.ts";

/// A picture of one colour, with a flame-white box and, if asked, the band below it lit.
function picture(ground: [number, number, number], lit: boolean): Picture {
  const [width, height] = [90, 90];
  const rgba = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++) {
      const inFlame = x >= 30 && x < 60 && y >= 20 && y < 50;
      const below = y >= 50 && lit;
      const [r, g, b] = inFlame ? [255, 255, 255] : below ? ground : [0, 0, 0];
      rgba.set([r, g, b, 255], 4 * (y * width + x));
    }
  return { width, height, rgba };
}

const BOX = { x: 30, y: 20, w: 30, h: 30 };

test("the band round a fire leaves its flame out, so a bright flame over dark ground reads dark", () => {
  assert.equal(surround(picture([0, 0, 0], false), BOX).light, 0);
});

test("ground the fire lights orange reads lit and warm", () => {
  const { light, warmth } = surround(picture([255, 120, 40], true), BOX);
  assert.ok(light > 0.1, `${light}`);
  assert.ok(warmth > 5, `${warmth}`);
  const grey = surround(picture([128, 128, 128], true), BOX);
  assert.ok(Math.abs(grey.warmth - 1) < 1e-9);
});

test("a fire's own floor lets its soot under the photographs' least, its most and the other numbers still the class's", () => {
  const bands = { soot: { median: 0.4, min: 0.13, max: 0.92 }, dim: { median: 20, min: 5, max: 63 } };
  const m = { soot: 0.05, dim: -20 } as Metrics;
  const by = (b: Parameters<typeof judge>[1]) => Object.fromEntries(judge(m, b).map((h) => [h.called, h.holds]));
  assert.deepEqual(by(bands), { "dim %": false, "soot/flame": false });
  assert.deepEqual(by(floored(bands, { soot: 0 })), { "dim %": false, "soot/flame": true });
  assert.equal(judge({ soot: 1.2 } as Metrics, floored(bands, { soot: 0 })).find((h) => h.called === "soot/flame")?.holds, false);
});

test("a row's numbers are the mean of its shots', each measured in its own crop", () => {
  // Two shots of a flame, the second taller and higher, over differently lit ground.
  const flame = (top: number, h: number, ground: [number, number, number]) => {
    const pic = picture(ground, true);
    for (let y = top; y < top + h; y++)
      for (let x = 30; x < 60; x++) pic.rgba.set([255, y % 3 ? 190 : 120, 40, 255], 4 * (y * pic.width + x));
    return { pic, box: { x: 30, y: top, w: 30, h } };
  };
  const shots = [flame(20, 30, [80, 40, 20]), flame(10, 40, [200, 100, 40])];
  const one = shots.map(({ pic, box }) => measure(pic, box));
  const { metrics, ground } = sampled(shots);
  assert.notEqual(one[0].yellow, one[1].yellow);
  for (const k of ["yellow", "orange", "soot", "fine", "flamePx"] as const) assert.ok(Math.abs(metrics[k] - (one[0][k] + one[1][k]) / 2) < 1e-9, k);
  const lights = shots.map(({ pic, box }) => surround(pic, box).light);
  assert.notEqual(lights[0], lights[1]);
  assert.ok(Math.abs(ground.light - (lights[0] + lights[1]) / 2) < 1e-12);
});

test("each fire is shot five times over two seconds, its own camera held through every shot", () => {
  const all = calls();
  for (const fire of FIRES) {
    const shots = shotsOf(fire);
    assert.ok(shots.length >= 5 && (shots.at(-1)! - shots[0]) / 60 >= 2, fire.name);
    for (const f of shots) {
      assert.ok(all.some((c) => c.frame === f && c.method === "fire.seen"), `${fire.name} ${f}`);
      const looks = all.filter((c) => c.frame === f && c.method === "build.look");
      assert.deepEqual(looks.map((c) => c.params), [{ from: fire.eye, at: fire.at }], `${fire.name} ${f}: one camera, its own`);
    }
  }
});
