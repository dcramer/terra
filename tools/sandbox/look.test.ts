// The forest's look measures (look.ts) on made-up pictures whose answers are known.
// `node --test tools/sandbox/look.test.ts`.

import assert from "node:assert/strict";
import { test } from "node:test";
import { look, meanOf } from "./look.ts";
import type { Picture } from "./png.ts";

const W = 64;
const H = 96;
/// A picture coloured pixel by pixel.
function picture(colour: (x: number, y: number) => [number, number, number]): Picture {
  const rgba = new Uint8Array(W * H * 4);
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const [r, g, b] = colour(x, y);
      rgba.set([r, g, b, 255], (y * W + x) * 4);
    }
  }
  return { width: W, height: H, rgba };
}
const near = (a: number, b: number, by = 1e-3) => assert.ok(Math.abs(a - b) < by, `${a} against ${b}`);

test("a mid grey is as bright as its value, never dark or bright, and has no local contrast", () => {
  const l = look(picture(() => [128, 128, 128]));
  near(l.mean, 128 / 255);
  near(l.p5, 128 / 255);
  assert.equal(l.dark, 0);
  assert.equal(l.bright, 0);
  near(l.redOverGreen, 1);
  near(l.contrast, 0);
});

test("half black and half white: half dark, half bright, the middle percentile white", () => {
  const l = look(picture((x) => (x < W / 2 ? [0, 0, 0] : [255, 255, 255])));
  near(l.dark, 0.5);
  near(l.bright, 0.5);
  near(l.mean, 0.5);
  near(l.p5, 0);
  near(l.p95, 1);
});

test("a checker of light and dark has local contrast; a warm picture more red than green", () => {
  const checker = look(picture((x, y) => ((x + y) % 2 ? [200, 200, 200] : [100, 100, 100])));
  assert.ok(checker.contrast > 0.3, `${checker.contrast}`);
  const warm = look(picture(() => [150, 120, 60]));
  near(warm.redOverGreen, 1.25);
  near(warm.blueOverGreen, 0.5);
});

test("lit orange litter is counted only in the lower third, and masked pixels not at all", () => {
  // Orange over the whole picture: all of the lower third is lit litter.
  const orange = look(picture(() => [230, 140, 60]));
  near(orange.litLitter, 1);
  // Orange only above the lower third: none.
  const above = look(picture((_, y) => (y < (H * 2) / 3 ? [230, 140, 60] : [40, 60, 30])));
  near(above.litLitter, 0);
  // Dark orange in shade is not lit litter.
  near(look(picture(() => [90, 50, 20])).litLitter, 0);
  // A black patch masked out leaves a mid grey picture.
  const masked = look(picture((x) => (x < 8 ? [0, 0, 0] : [128, 128, 128])), (x) => x < 8);
  assert.equal(masked.dark, 0);
});

test("the mean of looks is the mean of each number", () => {
  const a = look(picture(() => [0, 0, 0]));
  const b = look(picture(() => [255, 255, 255]));
  near(meanOf([a, b]).mean, 0.5);
});

test("every reference kept has every number a look has, each a number", async () => {
  const { references, reference } = await import("./look.ts");
  const keys = Object.keys(look(picture(() => [100, 120, 80]))).sort();
  assert.ok(references().includes("sotf-forest-day"));
  for (const name of references()) {
    const kept = reference(name);
    assert.deepEqual(Object.keys(kept.look).sort(), keys, name);
    assert.ok(Object.values(kept.look).every((v) => typeof v === "number" && Number.isFinite(v)), name);
    assert.ok(kept.frames > 0 && kept.about.length > 0, name);
  }
});

test("the player's place is the middle of the lower half", async () => {
  const { player } = await import("./look.ts");
  const at = player(100, 100);
  assert.ok(at(50, 75) && !at(50, 25) && !at(10, 75) && !at(90, 75));
});
