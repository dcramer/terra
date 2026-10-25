import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { CRITERIA, courseValues } from "./scorecard.ts";

test("the course's numbers are read from its lines and nothing else", () => {
  const out = ["running 1 test", "scorecard\ttunnel.wall\t2", "scorecard\tjump.apex\t0.046875", "scorecard\tbad\tNaN", "test x ... ok"].join("\n");
  const values = courseValues(out);
  assert.deepEqual([...values.entries()], [
    ["tunnel.wall", 2],
    ["jump.apex", 0.047],
  ]);
});

test("every criterion has its fields, a measurable one its course check (a heavy one the bench), the rest what they need", () => {
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
    if (c.measurable) assert.equal(c.check?.by, c.cost === "heavy" ? "bench" : "course", c.id);
    else assert.ok(c.needs, `${c.id} says what it needs`);
  }
});
