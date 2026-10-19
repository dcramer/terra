// The fire sheet's lit-ground number on pictures whose answers are known by construction.
import assert from "node:assert/strict";
import { test } from "node:test";
import type { Picture } from "../sandbox/png.ts";
import { surround } from "./sheet.ts";

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
