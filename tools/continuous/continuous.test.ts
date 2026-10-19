import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { type Check, due, scopePaths, verdict } from "./continuous.ts";

const fire: Check = { name: "fire", does: "", run: ["cargo", "xtask", "fire", "--gate"], reads: "FIRE" };
const a = "a".repeat(40);
const b = "b".repeat(40);
const c = "c".repeat(40);

test("every check's paths are read from scope.rs, where the check's stages read them too", () => {
  const scope = readFileSync(join(import.meta.dirname, "../../xtask/src/scope.rs"), "utf8");
  const checks: Check[] = JSON.parse(readFileSync(join(import.meta.dirname, "checks.json"), "utf8"));
  for (const check of checks.filter((c) => c.reads)) assert.ok(scopePaths(scope, check.reads!).length, check.name);
  assert.ok(scopePaths(scope, "FIRE").includes("tools/fire/"));
  assert.deepEqual(scopePaths('const X: &[&str] = &[\n    "a/",\n    "b.rs",\n];', "X"), ["a/", "b.rs"]);
});

test("a check runs the first time, when main moved over what it reads, or when its timer is up", () => {
  const now = Date.parse("2026-10-19T12:00Z");
  const last = { at: a, when: "2026-10-19T11:00:00.000Z", passed: true, good: a, said: "" };
  assert.equal(due(fire, undefined, a, true, now), "first run");
  assert.equal(due(fire, last, a, true, now), undefined);
  assert.equal(due(fire, last, b, false, now), undefined);
  assert.equal(due(fire, last, b, true, now), "main moved");
  assert.equal(due({ ...fire, hours: 1 }, last, a, false, now), "1 h since the last");
});

test("a failure after a pass names the range since the last pass and the commits in it touching what it reads", () => {
  const passed = verdict(fire, undefined, a, "2026-10-19T01:00:00.000Z", true, "✓ fire", []);
  assert.equal(passed.record.good, a);
  assert.match(passed.line, /^✓ fire 2026-10-19 01:00 UTC at aaaaaaaaaa: ✓ fire$/);
  const broke = verdict(fire, passed.record, c, "2026-10-19T02:00:00.000Z", false, "✗ fire: 1 worse than held", ["ccc Hotter flames", "bbb Smoke"]);
  assert.equal(broke.record.good, a);
  assert.match(
    broke.line,
    /^✗ fire 2026-10-19 02:00 UTC at cccccccccc: regressed in aaaaaaaaaa\.\.cccccccccc: ✗ fire: 1 worse than held, 2 commits touching what it reads:\n {4}ccc Hotter flames\n {4}bbb Smoke$/,
  );
  const still = verdict(fire, broke.record, c, "2026-10-19T03:00:00.000Z", false, "✗", []);
  assert.match(still.line, /still fails in aaaaaaaaaa\.\.cccccccccc/);
  assert.match(verdict(fire, still.record, c, "2026-10-19T04:00:00.000Z", true, "✓", []).line, /passes again/);
  assert.match(verdict(fire, undefined, a, "2026-10-19T01:00:00.000Z", false, "✗", []).line, /no passing run to compare/);
});
