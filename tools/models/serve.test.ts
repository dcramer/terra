import assert from "node:assert/strict";
import { join } from "node:path";
import { test } from "node:test";
import { models, servedFile } from "./serve.ts";

const root = join(import.meta.dirname, "..", "..");

test("serves only what is under assets/models and art", () => {
  assert.equal(servedFile(root, "assets/models/props/Forge.glb"), join(root, "assets/models/props/Forge.glb"));
  assert.equal(servedFile(root, "../.env"), undefined);
  assert.equal(servedFile(root, "assets/models/../../.env"), undefined);
  assert.equal(servedFile(root, "assets/data/items.ron"), undefined);
  assert.equal(servedFile(root, "%2e%2e/.env"), undefined);
});

test("lists the game's models before the source art", () => {
  const listed = models(root).map((m) => m.path);
  assert.ok(listed.includes("assets/models/props/Forge.glb"));
  assert.ok(listed.indexOf("assets/models/props/Forge.glb") < listed.findIndex((p) => p.startsWith("art/")));
});
