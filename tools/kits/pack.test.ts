// node --test tools/kits/pack.test.ts
import { strict as assert } from "node:assert";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { bill, leavingNothingIfItFails, materialFile, nextId, packFile, underFor } from "./pack.ts";

test("a cost is written as the kit's bill of fractions", () => {
  assert.equal(bill("Stone 1/4"), '[("Stone", 1, 4)]');
  assert.equal(bill("Wood 1/64, Fibre 1/64"), '[("Wood", 1, 64), ("Fibre", 1, 64)]');
  assert.throws(() => bill("a quarter of a stone"));
});

test("a new material takes the next id in its range, roofs from 2048", () => {
  assert.equal(nextId([1, 2, 7, 2048, 2050], false), 8);
  assert.equal(nextId([1, 2, 7, 2048, 2050], true), 2051);
  assert.equal(nextId([1, 2], true), 2048);
});

test("a material's file fits on one screen and names its pattern and pack", () => {
  const text = materialFile({ name: "river_cobble", title: "River cobble", source: "polyhaven:river_small_rocks", id: 8, pattern: "block", cost: "Stone 1/4", steps: 1, weight: 3, roof: 2049 });
  const lines = text.split("\n").filter((l) => l.trim() && !l.trim().startsWith("//"));
  assert.ok(lines.length <= 20, text);
  assert.match(text, /pattern: "block",/);
  assert.match(text, /pack: "river_cobble",/);
  assert.match(text, /roof_kind: Some\(2049\),/);
});

test("a block pack fills its walls and floors with the set, parallax on its walls", () => {
  const text = packFile({ name: "cut_sandstone", source: "x", pattern: "block", metres: 2, depth: 0.015, mean: [0.6, 0.5, 0.4] });
  assert.match(text, /Masonry: \(path: "materials\/cut_sandstone", metres: \(2.0, 2.0\), depth: 0.015\),/);
  assert.match(text, /Floor: \(path: "materials\/cut_sandstone", metres: \(2.0, 2.0\)\),/);
  assert.match(text, /light: \(0.6, 0.5, 0.4\)/);
  const over = packFile({ name: "limed", source: "x", pattern: "block", metres: 2, depth: 0, mean: [0.8, 0.8, 0.8], under: "packed_earth" });
  assert.match(over, /pack: "packed_earth",/);
});

test("a wood is laid over the pine logs' pack and a covering over the shakes', unless told", () => {
  assert.equal(underFor("coursed"), "pine_logs");
  assert.equal(underFor("roof"), "shakes");
  assert.equal(underFor("block"), undefined);
  assert.equal(underFor("roof", "slate"), "slate");
  assert.equal(underFor("coursed", "none"), undefined);
  const text = packFile({ name: "oak_logs", source: "x", pattern: "coursed", metres: 2, depth: 0, mean: [0.5, 0.4, 0.3], under: underFor("coursed") });
  assert.match(text, /pack: "pine_logs",/);
});

test("a run that fails leaves nothing behind: what it made gone, what it changed as it was", async () => {
  const root = mkdtempSync(join(tmpdir(), "pack-test-"));
  try {
    const [made, changed, folder] = [join(root, "new.ron"), join(root, "README.md"), join(root, "set")];
    writeFileSync(changed, "as it was\n");
    await assert.rejects(
      leavingNothingIfItFails([made, changed, folder], async () => {
        writeFileSync(made, "half made");
        writeFileSync(changed, "changed");
        mkdirSync(folder);
        writeFileSync(join(folder, "color.jpg"), "x");
        throw new Error("the checker fails");
      }),
    );
    assert.ok(!existsSync(made) && !existsSync(folder));
    assert.equal(readFileSync(changed, "utf8"), "as it was\n");
    // And one that works keeps what it made.
    await leavingNothingIfItFails([made], async () => writeFileSync(made, "made"));
    assert.equal(readFileSync(made, "utf8"), "made");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
