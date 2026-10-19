// The check proved on what it is known to say (docs/fire.md, "The automatic check"; D415):
// every reference photograph holds to its own class, and the castle film r12's wall fires,
// known bad, fail the structure class. Numbers only: the photographs and stills stay outside
// the repository.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "node:test";
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
