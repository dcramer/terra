import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { test } from "node:test";
import { ours, PROBES, units } from "./rebuilds.ts";

const ROOT = resolve(import.meta.dirname, "../..");

test("the timings report's units are read, and only the workspace's crates are kept", () => {
  const html = `<script>\nconst UNIT_DATA = [\n  {"name": "terra_build", "target": "", "mode": "todo", "duration": 12.5},\n  {"name": "terra_build", "target": " build-script", "mode": "todo", "duration": 1},\n  {"name": "bevy_ecs", "target": "", "mode": "todo", "duration": 30}\n];\nconst CONCURRENCY_DATA = [];`;
  assert.deepEqual(ours(units(html)), { terra_build: 12.5 });
});

test("every probe names a file that is there", () => {
  for (const [name, path] of Object.entries(PROBES)) assert.ok(existsSync(join(ROOT, path)), `${name}: ${path}`);
});
