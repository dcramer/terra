import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { type Criterion, judge } from "../scorecard/ratchet.ts";
import { CRITERIA, courseValues, ranked, traceLines, worstLine } from "./scorecard.ts";

const row = (id: string, noticed: number, at_most: number): Criterion => ({ id, noticed, notice: id, bar: { at_most }, unit: "", source: "", check: { by: "course" }, cost: "cheap", measurable: true });

test("the course's numbers are read from its scorecard lines and nothing else", () => {
  const out = ["anim\tstop.jog\tpop 1.55", "scorecard\tpop.fight\t13.6031", "scorecard\tbad\tNaN"].join("\n");
  assert.deepEqual([...courseValues(out).entries()], [["pop.fight", 13.603]]);
});

test("every criterion has its fields and a course check, each id once", () => {
  const file = JSON.parse(readFileSync(CRITERIA, "utf8"));
  assert.ok(file.course.crate && file.course.test);
  const ids = new Set<string>();
  for (const c of file.criteria) {
    assert.ok(!ids.has(c.id), `${c.id} twice`);
    ids.add(c.id);
    assert.ok(c.noticed >= 1 && c.noticed <= 5 && c.notice && c.source && c.bar, c.id);
    assert.equal(c.check.by, "course", c.id);
  }
});

test("the failing rows rank most noticed first, then most bars out; passing rows not at all", () => {
  const criteria = [row("a", 3, 1), row("b", 5, 1), row("c", 5, 0.1), row("d", 5, 10)];
  const values = [9, 2, 0.5, 1];
  const results = criteria.map((c, i) => judge(c, values[i], undefined));
  assert.deepEqual(
    ranked(criteria, results).map((r) => [r.c.id, r.out]),
    [
      ["c", 4],
      ["b", 1],
      ["a", 8],
    ],
  );
});

test("a row's worst transition is the one of its family with the largest number", () => {
  const out = [
    "anim\tinto.block\tfight\tLow_Guard -> Clear_Block\tpop 3.71 (lowerarm_r)",
    "anim\tinto.roll\tfight\tLow_Guard -> Fight_Roll\tpop 13.60 (upperarm_r at +0.00 s)\tslide 20.6 cm",
    "anim\tstop.jog\tstop\tJog -> Idle\tpop 20 (Head)",
    "anim\tseam.Sprint_Loop\tseam\tSprint_Loop\tpop 2.79 (upperarm_l)",
  ].join("\n");
  assert.match(worstLine(out, "pop.fight")!, /^into\.roll/);
  assert.match(worstLine(out, "seam.loops")!, /Sprint_Loop/);
  assert.equal(worstLine(out, "stop.rest"), undefined);
});

test("a transition's trace is its own lines, its name dropped, and not a longer name's", () => {
  const out = ["trace\tinto.slash\t+0.000\tOpen_Slash 0.000", "trace\tinto.slash.2\t+0.000\tx", "anim\tinto.slash\tfight"].join("\n");
  assert.deepEqual(traceLines(out, "into.slash"), ["+0.000  Open_Slash 0.000"]);
});
