import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { written } from "../characters/markers.ts";
import { CRITERIA, courseValues, moverSpeed, paceMatch, staleMarkers } from "./scorecard.ts";

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
    if (c.measurable) assert.ok(c.cost === "heavy" ? ["bench", "film"].includes(c.check?.by) : ["course", "markers", "paces"].includes(c.check?.by), c.id);
    else assert.ok(c.needs, `${c.id} says what it needs`);
    if (c.check?.by === "markers") assert.ok(c.check.files?.length, `${c.id} names its clip files`);
    if (c.check?.by === "paces") assert.ok(c.check.file && c.check.kind && c.check.gaits?.length, `${c.id} names its file, kind and gaits`);
    if (c.check?.by === "film") assert.ok(c.check.name, `${c.id} names its film check`);
  }
});

test("a gait's pace is matched against its kind's speed, the worst off 1 counted, a missing one never passing", () => {
  const ron = `{\n    "Man": (\n        jog: 4.3,\n        sprint: 7.4,\n    ),\n    "Troll": (\n        jog: 3.0,\n    ),\n}`;
  assert.equal(moverSpeed(ron, "Man", "sprint"), 7.4);
  assert.equal(moverSpeed(ron, "Troll", "jog"), 3.0);
  assert.equal(moverSpeed(ron, "Troll", "sprint"), undefined);
  const fight = "assets/models/character/fight.glb";
  const { worst, each } = paceMatch(fight, "Man", [{ clip: "Jog_Fwd_Loop", speed: "jog" }]);
  assert.equal(each.length, 1);
  assert.ok(worst >= 0 && Number.isFinite(worst));
  assert.equal(paceMatch(fight, "Man", [{ clip: "No_Such_Loop", speed: "jog" }]).worst, Number.POSITIVE_INFINITY);
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
