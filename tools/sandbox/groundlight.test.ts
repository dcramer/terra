import assert from "node:assert/strict";
import { test } from "node:test";
import { STONE_HUE, STONE_SATURATION, stoneHue } from "./groundlight.ts";
import type { Picture } from "./png.ts";

/// A picture of dark grass with every stone box's brightest fifth painted `lit`.
function painted(lit: number[]): Picture {
  const width = 200;
  const height = 100;
  const rgba = new Uint8Array(width * height * 4);
  for (let i = 0; i < width * height; i++) rgba.set([20, 24, 12, 255], i * 4);
  // Stripes a tenth of the rows, so a fifth of each box is lit wherever the box falls.
  for (let y = 0; y < height; y += 5) for (let x = 0; x < width; x++) rgba.set([...lit, 255], (y * width + x) * 4);
  return { width, height, rgba };
}

const orange = (c: number[]) => {
  const s = stoneHue(painted(c));
  return s.hue >= STONE_HUE[0] && s.hue <= STONE_HUE[1] && s.saturation >= STONE_SATURATION;
};

test("firelit stone read orange passes", () => {
  // Round 27's spread light on the stones, and a plain firelit orange.
  assert.ok(orange([207, 141, 73]));
  assert.ok(orange([200, 120, 50]));
});

test("salmon, pink and white-hot stones fail", () => {
  // Main 6fab1cba's stones, past the tonemapper's shoulder, and a pink.
  assert.ok(!orange([236, 165, 104]));
  assert.ok(!orange([230, 140, 130]));
  assert.ok(!orange([250, 240, 230]));
});

test("the hue is measured from the brightest pixels, not the grass", () => {
  const s = stoneHue(painted([200, 120, 50]));
  assert.ok(Math.abs(s.rgb[0] - 200) < 1 && Math.abs(s.rgb[2] - 50) < 1);
});
