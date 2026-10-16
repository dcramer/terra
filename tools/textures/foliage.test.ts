// The foliage check's measures (foliage.ts) on made-up cards whose answers are known.
// `node --test tools/textures/foliage.test.ts`.

import assert from "node:assert/strict";
import { test } from "node:test";
import { type Cell, LIMITS, PIXELS_A_METRE_AT_2M, atlasOf, cellOf, judge, kinds, living, measure } from "./foliage.ts";

const SIZE = 512;

/** A made-up card `SIZE` across: `count` leaves in a grid, each an ellipse `long` texels long and
 * half as wide, apart from the next. `leaf(k)` gives the k-th leaf's colour (sRGB 0 to 1), its
 * normal's tilt off the card's face in degrees and the way it leans, and whether it has veins
 * and grain; `margin` draws a dark line round each, as a picture's shading between leaves. */
function card(count: number, long: number, leaf: (k: number) => { rgb: [number, number, number]; tilt: number; lean: number; grain: boolean }, margin = false): Cell {
  const rgb = new Float32Array(SIZE * SIZE * 3);
  const alpha = new Float32Array(SIZE * SIZE);
  const normal = new Float32Array(SIZE * SIZE * 3);
  for (let i = 0; i < SIZE * SIZE; i++) normal[i * 3 + 2] = 1;
  const across = Math.ceil(Math.sqrt(count));
  const step = SIZE / across;
  for (let k = 0; k < count; k++) {
    const [cx, cy] = [(k % across + 0.5) * step, (Math.floor(k / across) + 0.5) * step];
    const l = leaf(k);
    const t = (l.tilt * Math.PI) / 180;
    const n = [Math.sin(t) * Math.cos(l.lean), Math.sin(t) * Math.sin(l.lean), Math.cos(t)];
    for (let y = 0; y < SIZE; y++) {
      for (let x = 0; x < SIZE; x++) {
        const e = ((x - cx) / (long / 2)) ** 2 + ((y - cy) / (long / 4)) ** 2;
        if (e > 1) continue;
        const i = y * SIZE + x;
        alpha[i] = 1;
        // Veins every 6 texels across it and a grain from texel to texel, and its own
        // shading darker toward its edge.
        const vein = l.grain && Math.abs(((x - cx) % 6) + 6) % 6 < 1 ? 1.25 : 1;
        const grain = l.grain ? 1 + 0.12 * Math.sin(x * 2.3 + y * 1.7) * Math.cos(y * 3.1 - x * 0.7) : 1;
        const shade = l.grain ? 1 - 0.25 * e : 1;
        const dark = margin && e > 0.9 ? 0.4 : 1;
        for (let c = 0; c < 3; c++) rgb[i * 3 + c] = Math.min(1, l.rgb[c] * vein * grain * shade * dark);
        normal.set(n, i * 3);
      }
    }
  }
  return { size: SIZE, rgb, alpha, normal };
}

/** Twelve leaves, each tilted its own way, veined and grained, a little lighter or darker and
 * yellower or bluer than the next: a spray of real leaves. */
const real = (k: number) => ({
  rgb: [0.2 + 0.03 * (k % 4), 0.26 + 0.05 * (k % 3), 0.1 + 0.03 * (k % 2)].map((v) => v * (0.85 + 0.1 * (k % 4))) as [number, number, number],
  tilt: 25 + 5 * (k % 3),
  lean: k * 2.1,
  grain: true,
});
/** The same twelve, one flat fill each, all one green, flat on the card: a flat card. */
const flat = () => ({ rgb: [0.25, 0.35, 0.12] as [number, number, number], tilt: 0, lean: 0, grain: false });

test("a spray of real leaves counts its leaves and holds every number for a spray", () => {
  const m = measure(card(12, 112, real), 0.45, 0.8, true);
  assert.equal(m.leaves, 12);
  // An ellipse's long axis.
  assert.ok(Math.abs(m.leafTexels - 112) < 4, `${m.leafTexels}`);
  assert.ok(m.relief > 0.95, `relief ${m.relief}`);
  assert.ok(m.tilts > 20, `tilts ${m.tilts}`);
  assert.ok(m.spread > 0.09, `spread ${m.spread}`);
  assert.ok(m.detail > 0.015, `detail ${m.detail}`);
  assert.ok(m.leafLight > 0.08, `leaf light ${m.leafLight}`);
  assert.ok(m.leafHue > 2.5, `leaf hue ${m.leafHue}`);
  assert.ok(m.gamut > 0.85, `gamut ${m.gamut}`);
  const kind = { atlas: "foliage", form: "spray" as const, grows: "", metres: 0.8, from: "" };
  // It holds every number a spray is held to.
  assert.deepEqual(judge("real", kind, m).fails, []);
});

test("a flat card fails what makes leaves read flat, and only that", () => {
  const m = measure(card(12, 112, flat), 0.45, 0.8, true);
  assert.equal(m.relief, 0);
  assert.equal(m.tilts, 0);
  assert.ok(m.spread < 0.01 && m.detail < 0.005, `spread ${m.spread}, detail ${m.detail}`);
  assert.ok(m.leafLight < 0.01 && m.leafHue < 0.5, `leaf light ${m.leafLight}, hue ${m.leafHue}`);
  const kind = { atlas: "foliage", form: "spray" as const, grows: "", metres: 0.8, from: "" };
  // Its picture's leaves, one fill each and apart, are still counted.
  assert.equal(m.leaves, 12);
  assert.deepEqual(judge("flat", kind, m).fails.sort(), ["detail", "leafHue", "leafLight", "relief", "spread", "tilts"]);
});

test("a picture's leaves are counted apart by the dark margins between them where they touch", () => {
  // Nine leaves so long they overlap their neighbours, flat on the card, each its own shade.
  const shaded = (k: number) => ({ rgb: [0.15 + 0.02 * k, 0.3 + 0.025 * k, 0.1] as [number, number, number], tilt: 0, lean: 0, grain: false });
  const touching = measure(card(9, 184, shaded, true), 0.45, 0.8, true);
  assert.ok(touching.leaves >= 8 && touching.leaves <= 10, `${touching.leaves}`);
});

test("leaves tilted alike are one plane to the light; each tilted its own way, apart", () => {
  const alike = measure(card(12, 112, (k) => ({ ...real(k), tilt: 30, lean: 1 })), 0.45, 0.8, true);
  // Tilted off the card's face, so relief, but all one way, so no tilt from leaf to leaf.
  assert.ok(alike.relief > 0.95);
  assert.ok(alike.tilts < 1, `${alike.tilts}`);
});

test("a living leaf's green is in the gamut; pastel mint, neon and grey are not", () => {
  assert.ok(living(0.32, 0.42, 0.2));
  assert.ok(living(0.13, 0.2, 0.15));
  // The composed cards' mint before D292: pale, luma past 0.45.
  assert.ok(!living(0.5, 0.62, 0.45));
  assert.ok(!living(0.2, 1, 0.05));
  assert.ok(!living(0.4, 0.42, 0.4));
  const mint = measure(card(12, 112, (k) => ({ ...real(k), rgb: [0.5, 0.64, 0.42] })), 0.45, 0.8, true);
  assert.ok(mint.gamut < LIMITS.gamut!.over!, `${mint.gamut}`);
});

test("a card is sharp at 2 m while it has a texel for each pixel it covers there", () => {
  // 512 texels over a card as wide as 512 pixels at 2 m: one each.
  const metres = SIZE / PIXELS_A_METRE_AT_2M;
  const m = measure(card(4, 140, real), 0.45, metres, true);
  assert.ok(Math.abs(m.sharpness - 1) < 1e-6, `${m.sharpness}`);
  assert.ok(measure(card(4, 140, real), 0.45, 2 * metres, true).sharpness < 0.51);
});

test("a composed card's leaves are as long as its recipe laid them, not as overlaps cut them", () => {
  const m = measure(card(12, 112, real), 0.45, 0.8, true, 0.5);
  assert.equal(m.leafTexels, SIZE / 2);
});

test("a cell is read from its atlas at its corner, its normal unpacked from OpenGL's convention", () => {
  const rgba = new Uint8Array(4 * 4 * 4);
  const normals = new Uint8Array(4 * 4 * 4).fill(128);
  // The cell at (0.5, 0.5), a half across: its top-left texel is the atlas's (2, 2).
  rgba.set([255, 0, 0, 255], (2 * 4 + 2) * 4);
  normals.set([255, 128, 128, 255], (2 * 4 + 2) * 4);
  const c = cellOf({ width: 4, height: 4, rgba }, { width: 4, height: 4, rgba: normals }, [0.5, 0.5], 0.5);
  assert.equal(c.size, 2);
  assert.deepEqual([...c.rgb.subarray(0, 3)], [1, 0, 0]);
  assert.equal(c.alpha[0], 1);
  assert.ok(c.normal[0] > 0.99, `${c.normal.subarray(0, 3)}`);
});

test("a known failure passes the check until it is fixed, and fails it once it is", () => {
  const kind = { atlas: "foliage", form: "spray" as const, grows: "", metres: 0.8, from: "" };
  const m = measure(card(12, 112, flat), 0.45, 0.8, false);
  const j = judge("flat", kind, m, { relief: "a picture", spread: "a picture" }, { thickness: "none have one" });
  assert.ok(j.known.includes("relief") && j.known.includes("thickness"));
  assert.ok(!j.known.includes("tilts"));
  assert.deepEqual(j.fixed, []);
  const fixed = judge("real", kind, measure(card(12, 112, real), 0.45, 0.8, true), { relief: "a picture" });
  assert.deepEqual(fixed.fixed, ["relief"]);
});

test("every kind is a card of its atlas, and every card of the foliage atlas a kind", () => {
  const k = kinds();
  const atlases = Object.fromEntries(Object.entries(k.atlases).map(([n, f]) => [n, atlasOf(f)]));
  for (const [name, kind] of Object.entries(k.kinds)) {
    assert.ok(atlases[kind.atlas]?.cards.some((c) => c.name === name), `${name} in ${kind.atlas}`);
    assert.ok(kind.metres > 0 && kind.grows && kind.from, name);
  }
  for (const c of atlases.foliage.cards) assert.ok(k.kinds[c.name], `the foliage atlas's ${c.name} has no kind in foliage.json`);
  // A composed card's leaf length is read; a picture has none.
  assert.ok(atlases.foliage.cards.find((c) => c.name === "oak")!.leaf! > 0.05);
  assert.equal(atlases.foliage.cards.find((c) => c.name === "herb")!.leaf, undefined);
  for (const [card, known] of Object.entries(k.known)) {
    assert.ok(card === "*" || k.kinds[card], `known failures of ${card}, no such kind`);
    for (const f of Object.keys(known)) assert.ok(f in LIMITS, `${card}: ${f} is no measure`);
  }
});
