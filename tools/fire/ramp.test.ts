import assert from "node:assert/strict";
import { test } from "node:test";
import { blackbody, FUELS, ramp, table, texture } from "./ramp.ts";

test("the ramp is a blackbody's colour: white at D65, red with no blue at 1000 K", () => {
  const d65 = blackbody(6504);
  for (const c of d65) assert.ok(Math.abs(c - 1) < 0.05, `D65 ${d65}`);
  const [r, g, b] = blackbody(1000);
  assert.ok(r > 4 * g && b === 0);
});

test("the ramp brightens with heat, keeps its chroma as it dims and adds light when hot", () => {
  let last = -1;
  for (let h = 0; h <= 1; h += 0.1) {
    const [r, g, b] = ramp(h);
    const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    assert.ok(lum > last);
    last = lum;
    assert.ok(r > g && g >= b, "red leads at every heat, never grey");
  }
  assert.equal(ramp(0)[3], 1);
  assert.equal(ramp(1)[3], 0);
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
