// The check proved on what it is known to say (docs/fire.md, "The automatic check"; D415):
// every reference photograph holds to its own class, and the castle film r12's wall fires,
// known bad, fail the structure class. Numbers only: the photographs and stills stay outside
// the repository.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "node:test";
import { COLOURS, groundCut, widened } from "./games.ts";
import { judge, type Metrics, references } from "./measure.ts";

const refs = references();

test("every class the fire sheet holds a fire to has at least eight photographs", () => {
  for (const cls of ["campfire", "torch", "structure", "flamethrower", "fireball"]) {
    assert.ok((refs.classes[cls]?.photos ?? 0) >= 8, `${cls}: ${refs.classes[cls]?.photos ?? 0} photographs`);
  }
});

test("every reference photograph holds to its own class", () => {
  for (const photo of refs.photos.filter((p) => p.class !== "excluded")) {
    const out = judge(photo.metrics, refs.classes[photo.class].bands).filter((h) => !h.holds);
    assert.deepEqual(out, [], `${photo.file} out of ${photo.class}`);
  }
});

test("the castle film r12's wall fires fail the structure class", () => {
  const { walls } = JSON.parse(readFileSync(resolve(import.meta.dirname, "r12-walls.json"), "utf8")) as { walls: { still: string; metrics: Metrics }[] };
  assert.equal(walls.length, 3);
  for (const wall of walls) {
    const out = judge(wall.metrics, refs.classes.structure.bands).filter((h) => !h.holds);
    assert.ok(out.length > 0, `${wall.still} passed`);
  }
});

test("every AAA game frame a class counts holds to that class's colour (D423)", () => {
  const games = refs.games ?? [];
  assert.ok(games.filter((g) => g.class === "campfire").length >= 8, "the campfire's game frames");
  for (const g of games.filter((g) => g.class !== "excluded")) {
    assert.ok(g.metrics.flamePx >= 5000, `${g.file}: ${g.metrics.flamePx} flame pixels`);
    // Only colour: a frame's shape and scale numbers take its drawn box and the proxy's edge.
    const colour = Object.fromEntries(COLOURS.map((k) => [k, refs.classes[g.class].bands[k]]));
    const out = judge(g.metrics, colour).filter((h) => !h.holds);
    assert.deepEqual(out, [], `${g.file}'s colour out of ${g.class}`);
  }
});

test("a class's colour takes its game frames' limits only where they lie outside the photographs'", () => {
  const band = (min: number, max: number) => ({ median: (min + max) / 2, min, max });
  const bands = Object.fromEntries(COLOURS.map((k) => [k, band(10, 20)]));
  bands.soot = band(0.1, 0.5);
  const frame = (v: Partial<Metrics>) => ({ ...Object.fromEntries(COLOURS.map((k) => [k, 15])), soot: 0.9, ...v }) as Metrics;
  const out = widened(bands, [frame({ pale: 30 }), frame({ pale: 12, yellow: 4 })]);
  assert.deepEqual([out.pale.min, out.pale.max], [10, 30]);
  assert.deepEqual([out.yellow.min, out.yellow.max], [4, 20]);
  assert.deepEqual([out.red.min, out.red.max], [10, 20]);
  assert.deepEqual(out.soot, bands.soot, "shape and scale stay the photographs'");
});

test("the ground-cut proxy keeps the flame and leaves out the lit ground round it", () => {
  const [w, h] = [40, 40];
  const rgba = new Uint8Array(4 * w * h);
  for (let p = 0; p < w * h; p++) {
    const [x, y] = [p % w, Math.floor(p / w)];
    // Ground lit orange everywhere; a bright flame in the box's middle, lit ground at its edge.
    const flame = x >= 15 && x < 25 && y >= 10 && y < 30;
    rgba.set(flame ? [255, 170, 40, 255] : [180, 90, 30, 255], 4 * p);
  }
  const matte = groundCut({ width: w, height: h, rgba }, { x: 10, y: 10, w: 20, h: 20 });
  const kept = (x: number, y: number) => matte.rgba[4 * (y * w + x)] === 255;
  assert.ok(kept(20, 20), "the flame");
  assert.ok(!kept(12, 20), "the lit ground inside the box");
});
