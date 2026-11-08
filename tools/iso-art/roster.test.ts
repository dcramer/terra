// Every biome roster's walk speed is its model's measured one (roster.ts).

import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { ROOT } from "./iso-art.ts";
import { rosterWalks, staleWalks } from "./roster.ts";

const BIOMES = join(ROOT, "crates", "terra_iso", "biomes");
const MODELS = join(ROOT, "assets", "models", "iso");

test("every biome roster's walk is its model's measured walk", () => {
  assert.deepEqual(staleWalks(BIOMES, MODELS), []);
});

test("a roster walk from before the planted-foot rule is named stale, with the speed to set", () => {
  const dir = mkdtempSync(join(tmpdir(), "roster-"));
  const ron = `foes: [\n    (model: "models/iso/treant-a.glb", at: (6.5, -5.0), walk: 1.06, radius: 0.65),\n]`;
  writeFileSync(join(dir, "planted.ron"), ron);
  assert.deepEqual(rosterWalks(ron), [{ line: 2, model: "treant-a", walk: 1.06 }]);
  const stale = staleWalks(dir, MODELS);
  assert.equal(stale.length, 1);
  assert.match(stale[0], /planted\.ron:2: treant-a's walk is 1\.06 m\/s, but its model walks 0\.96/);
});
