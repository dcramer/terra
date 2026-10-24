import assert from "node:assert/strict";
import { test } from "node:test";
import { blackbody, FUELS, ramp, table, texture } from "./ramp.ts";

test("the ramp is a blackbody's colour: white at D65, red with no blue at 1000 K", () => {
  const d65 = blackbody(6504);
  for (const c of d65) assert.ok(Math.abs(c - 1) < 0.05, `D65 ${d65}`);
  const [r, g, b] = blackbody(1000);
  assert.ok(r > 4 * g && b === 0);
});

test("the ramp brightens with heat, keeps its chroma as it dims and covers most when hot", () => {
  let last = -1;
  for (let h = 0; h <= 1; h += 0.1) {
    const [r, g, b] = ramp(h);
    const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    assert.ok(lum > last);
    last = lum;
    assert.ok(r > g && g >= b, "red leads at every heat, never grey");
  }
  assert.ok(ramp(0)[3] < 0.5, "a dull red rim covers least");
  assert.equal(ramp(1)[3], 1, "the hottest covers what is behind it");
});

test("the hottest is the photographs' white-yellow core, the orange keeps almost no blue", () => {
  // Night photographs' campfire core, sRGB 253,224,133: blue about a quarter of red, linear.
  const [r, g, b] = ramp(1);
  assert.ok(b / r > 0.15 && b / r < 0.35, `hottest B/R ${b / r}`);
  assert.ok(g / r > 0.7 && g / r <= 1, `hottest G/R ${g / r}`);
  const [ro, , bo] = ramp(0.6);
  assert.ok(bo / ro < 0.03, `orange B/R ${bo / ro}`);
});

test("the ramp's texture decodes to the ramp: sRGB over its scale, the dim red end kept", () => {
  const { picture, scale } = texture();
  const linear = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
  const wood = table(FUELS.wood);
  for (const i of [0, 40, 128, 255]) {
    for (let k = 0; k < 3; k++) {
      const got = linear(picture.rgba[i * 4 + k] / 255) * scale;
      const want = wood[i * 4 + k];
      assert.ok(Math.abs(got - want) <= 0.03 * want + 2e-3, `heat ${i} channel ${k}: ${got} for ${want}`);
    }
    assert.ok(Math.abs(picture.rgba[i * 4 + 3] / 255 - wood[i * 4 + 3]) < 0.003);
  }
});

test("a campfire's flame is the blackbody's own: a yellow core, an orange body, deep red tips", () => {
  // In display terms over its brightest channel, as the step-back measured the frame.
  const shown = (heat: number, fuel = FUELS.hearth) => {
    const c = ramp(heat, fuel).slice(0, 3);
    const m = Math.max(...c);
    const e = c.map((v) => (v / m <= 0.0031308 ? 12.92 * (v / m) : 1.055 * (v / m) ** (1 / 2.4) - 0.055));
    const [r, g, b] = e;
    return { hue: (60 * (g - b)) / (r - b), sat: (r - b) / r };
  };
  // Unwashed (PALE 0): washed toward cream, its flame read pale and hazy from above, where
  // e9e5b733's saturated, crisper tongues scored higher (campfire net round).
  const unwashed = { ...FUELS.hearth, pale: 0 };
  for (const h of [0.75, 0.85, 0.9, 0.95, 1]) assert.deepEqual(shown(h), shown(h, unwashed), `washed at ${h}`);
  // The core over the coals a saturated yellow, not white nor pure; only the hottest paler.
  const core = shown(0.95);
  assert.ok(core.hue > 40 && core.hue < 50 && core.sat > 0.65 && core.sat < 0.8, `core hue ${core.hue} saturation ${core.sat}`);
  assert.ok(shown(1).sat < 0.6, `hottest saturation ${shown(1).sat}`);
  // Its body orange and full, its cooler tips the blackbody's deep red.
  const body = shown(0.9);
  assert.ok(body.hue > 28 && body.hue < 38 && body.sat > 0.95, `body hue ${body.hue} saturation ${body.sat}`);
  assert.ok(shown(0.75).sat > 0.95 && shown(0.75).hue < 20, `tip ${JSON.stringify(shown(0.75))}`);
});
