import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { written } from "../characters/markers.ts";
import { type Criterion, judge, line } from "../scorecard/ratchet.ts";
import { CRITERIA, courseValues, filmValues, staleMarkers } from "./scorecard.ts";

test("the course's numbers are read from its lines and nothing else", () => {
  const out = ["running 1 test", "scorecard\ttunnel.wall\t2", "scorecard\tjump.apex\t0.046875", "scorecard\tbad\tNaN", "test x ... ok"].join("\n");
  const values = courseValues(out);
  assert.deepEqual([...values.entries()], [
    ["tunnel.wall", 2],
    ["jump.apex", 0.047],
  ]);
});

test("every criterion has its fields, a measurable one its course check (a heavy one the bench or the film), the rest what they need", () => {
  const file = JSON.parse(readFileSync(CRITERIA, "utf8"));
  assert.ok(file.course.crate && file.course.test);
  const ids = new Set<string>();
  assert.ok(file.criteria.length > 0);
  for (const c of file.criteria) {
    assert.ok(!ids.has(c.id), `${c.id} twice`);
    ids.add(c.id);
    assert.ok(c.noticed >= 1 && c.noticed <= 5, c.id);
    assert.ok(c.notice && c.source && "bar" in c, c.id);
    assert.ok(c.cost === "cheap" || c.cost === "heavy", c.id);
    if (c.measurable) assert.ok(c.cost === "heavy" ? ["bench", "film"].includes(c.check?.by) : ["course", "markers", "test"].includes(c.check?.by), c.id);
    else assert.ok(c.needs, `${c.id} says what it needs`);
    if (c.check?.by === "markers") assert.ok(c.check.files?.length, `${c.id} names its clip files`);
    if (c.check?.by === "test") assert.ok(c.check.crate && c.check.test, `${c.id} names its crate and test`);
    if (c.check?.by === "film") assert.ok(c.check.name, `${c.id} names its film check`);
  }
});

test("a clip file re-exported without its baked foot locks is counted, one as baked is not", () => {
  const fight = "assets/models/character/fight.glb";
  const dir = mkdtempSync(join(tmpdir(), "markers-"));
  const stripped = join(dir, "fight.glb");
  writeFileSync(stripped, written(fight, {}));
  try {
    assert.deepEqual(staleMarkers([fight, stripped]), [stripped]);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("a film check with no number (a run that did not finish) leaves its row unmeasured with the check's line as its note, the others judged", () => {
  const criteria = [
    { id: "feet.sliding", measurable: true, check: { by: "film", name: "feet sliding" } },
    { id: "swim.pose", measurable: true, check: { by: "film", name: "swim pose" } },
  ] as unknown as Criterion[];
  // As yard.ts writes them: JSON writes a NaN as null.
  const checks = JSON.parse(
    JSON.stringify([
      { name: "feet sliding", ok: true, line: "0 of 66 plants slid", value: 0, faults: 0 },
      { name: "swim pose", ok: false, line: "✗ not measured: the lake run did not finish (its log lake.log)", value: NaN, faults: 1 },
    ]),
  );
  const values = new Map<string, number>();
  const notes = new Map<string, string>();
  filmValues(criteria, checks, values, notes);
  assert.deepEqual([...values.entries()], [["feet.sliding", 0]]);
  assert.deepEqual([...notes.entries()], [["swim.pose", "the lake run did not finish (its log lake.log)"]]);
  const result = { ...judge(criteria[1], values.get("swim.pose"), undefined), note: notes.get("swim.pose") };
  assert.equal(line(criteria[1], result, "cargo xtask controller", ""), "✗ swim.pose: not measured (the lake run did not finish (its log lake.log))");
});
