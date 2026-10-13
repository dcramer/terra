import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { type Scan, scanArgs } from "./scans.ts";

const ROOT = join(import.meta.dirname, "..", "..");

test("a scan is cut to its budget, its largest piece kept in nature unless it says", () => {
  const first = scanArgs("Rock_1", { from: "boulder_01" }, "boulder_01.gltf");
  assert.deepEqual(first.args.slice(2), ["500", "--loose", "--crease", "50"]);
  assert.equal(first.args[0], "boulder_01.gltf");
  assert.ok(first.piece.endsWith("Rock_1_1.glb"));
  assert.ok(first.out.endsWith(join("assets", "models", "nature", "Rock_1.glb")));
  const other = scanArgs("Pebble", { from: "x", piece: 2, budget: 120, into: "items" }, "x.gltf");
  assert.equal(other.args[2], "120");
  assert.ok(other.piece.endsWith("Pebble_3.glb"));
  assert.ok(other.out.endsWith(join("assets", "models", "items", "Pebble.glb")));
});

test("every scan listed has been made", () => {
  const list: Record<string, Scan> = JSON.parse(readFileSync(join(ROOT, "art", "scans.json"), "utf8"));
  for (const [file, scan] of Object.entries(list)) {
    assert.ok(existsSync(join(ROOT, "assets", "models", scan.into ?? "nature", `${file}.glb`)), `${file} not made`);
  }
});
