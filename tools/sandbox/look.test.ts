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

test("trunks are counted only in the middle third, by their brown hue", () => {
  const bark: [number, number, number] = [90, 60, 40];
  const leaves: [number, number, number] = [40, 70, 30];
  // Bark across the middle third only: all of it is trunk.
  near(look(picture((_, y) => (y >= H / 3 && y < (H * 2) / 3 ? bark : leaves))).trunkMiddle ?? -1, 1);
  // Bark above and below it, leaves in it: none.
  near(look(picture((_, y) => (y >= H / 3 && y < (H * 2) / 3 ? leaves : bark))).trunkMiddle ?? -1, 0);
  // Half the middle third's width bark: half.
  near(look(picture((x) => (x < W / 2 ? bark : leaves))).trunkMiddle ?? -1, 0.5);
});

test("the mean of looks is the mean of each number", () => {
  const a = look(picture(() => [0, 0, 0]));
  const b = look(picture(() => [255, 255, 255]));
  near(meanOf([a, b]).mean, 0.5);
});

test("every reference kept has every number a look has, each a number", async () => {
  const { references, reference } = await import("./look.ts");
  const keys = Object.keys(look(picture(() => [100, 120, 80]))).sort();
  // Measured since some references were made, from frames no longer to hand; a reference a
  // place is held to has what it is held on (looks.test.ts).
  const later = ["trunkMiddle", "tint", "lilac", "hardEdges", "facets", "flat"];
  assert.ok(references().includes("sotf-forest-day"));
  for (const name of references()) {
    const kept = reference(name);
    const required = keys.filter((k) => !later.includes(k));
    assert.deepEqual(Object.keys(kept.look).filter((k) => !later.includes(k)).sort(), required, name);
    assert.ok(Object.keys(kept.look).every((k) => keys.includes(k)), name);
    assert.ok(Object.values(kept.look).every((v) => typeof v === "number" && Number.isFinite(v)), name);
    assert.ok(kept.frames > 0 && kept.about.length > 0, name);
  }
});

test("the player's place is the middle of the lower half", async () => {
  const { player } = await import("./look.ts");
  const at = player(100, 100);
  assert.ok(at(50, 75) && !at(50, 25) && !at(10, 75) && !at(90, 75));
});

test("a blue sky over green land has its skyline at the land, and the bands below it", async () => {
  const { bands, skyline } = await import("./look.ts");
  // Sky down to row 30, then land darker for being nearer: far grey-green, nearer greener.
  const p = picture((_, y) => (y < 30 ? [120, 160, 230] : y < 64 ? [90, 110, 100] : [40, 90, 30]));
  for (const at of skyline(p)) near(at, 30);
  const b = bands(p);
  near(b.sky.share, 30 / H);
  near(b.far.share, 9.6 / H, 0.02);
  near(b.near.share, 1 / 3, 0.02);
  assert.ok(b.sky.blueOverGreen > 1.3 && b.near.blueOverGreen < 0.5);
  // The far land is darker than the sky by its share of the sky's brightness, and bluer than
  // the middle distance only where the middle is greener.
  assert.ok(b.farUnderSky > 0.2 && b.farUnderSky < 0.6, `${b.farUnderSky}`);
  near(b.farBluerThanMiddle, 0, 0.02);
});

test("a land with no sky in it has no sky band, and its far land under the sky reads 0", async () => {
  const { bands } = await import("./look.ts");
  const b = bands(picture(() => [40, 90, 30]));
  assert.equal(b.sky.share, 0);
  assert.equal(b.farUnderSky, 0);
});

test("a reference of open views keeps every band's numbers", async () => {
  const { references, reference } = await import("./look.ts");
  for (const name of references()) {
    const kept = reference(name);
    if (!kept.bands) continue;
    for (const k of ["sky", "far", "middle", "near"] as const) {
      assert.deepEqual(Object.keys(kept.bands[k]).sort(), ["blueOverGreen", "contrast", "mean", "saturation", "share"], name);
    }
    assert.ok(Number.isFinite(kept.bands.farUnderSky) && Number.isFinite(kept.bands.farBluerThanMiddle), name);
  }
});

/// A picture of its own size, coloured pixel by pixel.
function sized(w: number, h: number, colour: (x: number, y: number) => [number, number, number]): Picture {
  const rgba = new Uint8Array(w * h * 4);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) rgba.set([...colour(x, y), 255], (y * w + x) * 4);
  return { width: w, height: h, rgba };
}
/// A value's own noise, the same every run: a texture's grain.
const grain = (x: number, y: number) => {
  const s = Math.sin(x * 12.9898 + y * 78.233) * 43758.5453;
  return s - Math.floor(s);
};

test("a flat saturated orange is a debug tint; the same orange textured, or a flat sky blue, is not", async () => {
  const { tints } = await import("./look.ts");
  near(tints(picture(() => [230, 110, 40])).tint, 1);
  const textured = tints(picture((x, y) => {
    const k = 0.7 + 0.6 * grain(x, y);
    return [230 * k, 110 * k, 40 * k].map(Math.round) as [number, number, number];
  }));
  near(textured.tint, 0);
  near(tints(picture(() => [70, 120, 200])).tint, 0);
  // A grey picture with an orange quarter: the quarter's squares.
  near(tints(sized(64, 64, (x, y) => (x < 32 && y < 32 ? [230, 110, 40] : [128, 128, 128]))).tint, 0.25);
});

test("lilac is lilac and nothing else is", async () => {
  const { tints } = await import("./look.ts");
  near(tints(picture(() => [170, 140, 190])).lilac, 1);
  near(tints(picture(() => [140, 160, 120])).lilac, 0);
  near(tints(picture(() => [90, 110, 200])).lilac, 0);
  near(tints(sized(64, 64, (x) => (x < 16 ? [170, 140, 190] : [128, 128, 128]))).lilac, 0.25);
});

test("a shadow's edge is hard when it falls in a pixel or two, soft over thirty, and a colour's edge is no shadow", async () => {
  const { hardEdges } = await import("./look.ts");
  const green = (v: number): [number, number, number] => [Math.round(0.6 * v), Math.round(v), Math.round(0.4 * v)];
  // Lit grass and its shadow, the same green a quarter as bright.
  near(hardEdges(sized(200, 40, (x) => green(x < 100 ? 200 : 50))), 1);
  near(hardEdges(sized(200, 40, (x) => green(x < 85 ? 200 : x > 115 ? 50 : 200 - (150 * (x - 85)) / 30))), 0);
  // Two pixels of blur is still hard.
  near(hardEdges(sized(200, 40, (x) => green(x < 100 ? 200 : x > 101 ? 50 : 125))), 1);
  // Bright green grass against a dark red trunk is a thing against a thing: counted neither way.
  near(hardEdges(sized(200, 40, (x) => (x < 100 ? green(200) : [60, 20, 15]))), 0);
  // Along each row a shadow falls hard and lifts again softly: half its edges hard.
  near(hardEdges(sized(400, 40, (x) => green(x < 100 ? 200 : x < 200 ? 50 : x < 230 ? 50 + (150 * (x - 200)) / 30 : 200))), 0.5, 0.05);
});

test("a long slanting straight edge across the ground is a facet; a wavy one, a level one and an upright one are not", async () => {
  const { facets } = await import("./look.ts");
  const [w, h] = [384, 216];
  const red: [number, number, number] = [150, 40, 30];
  const green: [number, number, number] = [60, 140, 50];
  // A line at 40 degrees from the bottom edge up through the lower half.
  const slant = (x: number, y: number) => (h - y) < (x - 60) * Math.tan((40 * Math.PI) / 180);
  const straight = facets(sized(w, h, (x, y) => (slant(x, y) ? red : green)));
  // Its length in the lower half is about 108 / sin 40 pixels, 0.44 of the width, give or take its ends.
  assert.ok(straight > 0.3 && straight < 0.6, `${straight}`);
  // The same edge, wavy along its length.
  const wave = (x: number, y: number) => (h - y) < (x - 60 + 18 * Math.sin(y / 5)) * Math.tan((40 * Math.PI) / 180);
  const wavy = facets(sized(w, h, (x, y) => (wave(x, y) ? red : green)));
  assert.ok(wavy < 0.05, `${wavy}`);
  near(facets(sized(w, h, (_, y) => (y < 160 ? green : red))), 0);
  near(facets(sized(w, h, (x) => (x < 200 ? green : red))), 0);
  // Above the lower half, nothing counts.
  const above = (x: number, y: number) => y < h / 2 - 8 && slant(x, y + h / 2);
  near(facets(sized(w, h, (x, y) => (above(x, y) ? red : green))), 0);
});

test("a flat fill below the skyline is flat colour; grain is not, and black is left out", async () => {
  const { flat } = await import("./look.ts");
  near(flat(picture(() => [120, 100, 80])), 1);
  near(flat(picture((x, y) => {
    const k = 0.8 + 0.4 * grain(x, y);
    return [120 * k, 100 * k, 80 * k].map(Math.round) as [number, number, number];
  })), 0);
  near(flat(picture(() => [8, 8, 8])), 0);
  // Half flat, half grain: half.
  near(flat(sized(64, 64, (x, y) => {
    const k = x < 32 ? 1 : 0.8 + 0.4 * grain(x, y);
    return [120 * k, 100 * k, 80 * k].map(Math.round) as [number, number, number];
  })), 0.5);
});
