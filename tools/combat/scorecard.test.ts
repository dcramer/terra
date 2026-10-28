import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { CRITERIA, type Criterion, judge, measuredValues, miss, movesValue, readBaseline, readSet, rebank, rebaseline, testVerdicts, timing, writeBaseline } from "./scorecard.ts";

const c = (bar: Criterion["bar"], extra: Partial<Criterion> = {}): Criterion => ({ id: "x", noticed: 3, notice: "", bar, unit: "s", source: "", check: null, cost: "cheap", measurable: true, ...extra });

test("a miss is how far outside the bar, 0 within it", () => {
  assert.equal(miss(0.3, { within: [0.22, 0.32] }), 0);
  assert.ok(Math.abs(miss(0.41, { within: [0.22, 0.32] }) - 0.09) < 1e-9);
  assert.ok(Math.abs(miss(0.12, { within: [0.22, 0.32] }) - 0.1) < 1e-9);
  assert.equal(miss(3, { at_most: 2 }), 1);
  assert.equal(miss(-4, { at_most: 2 }), 0);
  assert.equal(miss(0.7, { at_least: 0.8 }) > 0, true);
  assert.equal(miss(1, { pass: true }), 1);
});

test("the ratchet holds a criterion at its baseline: worse fails, better and held pass", () => {
  const bar = c({ at_most: 1 });
  const at = (v: number) => ({ value: v, why: "-" });
  assert.equal(judge(bar, 12, at(12)).status, "held");
  assert.equal(judge(bar, 11, at(12)).status, "better");
  assert.equal(judge(bar, 12.5, at(12)).status, "worse");
  assert.equal(judge(bar, 0.5, at(12)).status, "pass");
  // Within the bar, a move within it is no change; out of it again is worse.
  assert.equal(judge(bar, -2, at(-3)).status, "pass");
  assert.equal(judge(bar, 1.5, at(-3)).status, "worse");
  // No baseline: within the bar passes, outside it must be raised.
  assert.equal(judge(bar, 0.5, undefined).status, "pass");
  assert.equal(judge(bar, 3, undefined).status, "new");
  // A slack takes a noisy number's wobble; no number is a failure.
  assert.equal(judge(c({ at_most: 0 }, { slack: 2 }), 67, at(66)).status, "held");
  assert.equal(judge(bar, undefined, at(1)).status, "missing");
  assert.equal(judge(c(null, { measurable: false }), undefined, undefined).status, "unmeasured");
});

test("a raise writes only what got worse, a lowering only what got better, a new row its first number", () => {
  const held = new Map([
    ["a", { value: 10, why: "first" }],
    ["b", { value: 10, why: "first" }],
  ]);
  const bar = c({ at_most: 1 });
  const results = [
    { ...judge({ ...bar, id: "a" }, 12, held.get("a")) },
    { ...judge({ ...bar, id: "b" }, 8, held.get("b")) },
    // Not held yet: one outside its bar, one within it.
    { ...judge({ ...bar, id: "n" }, 5, undefined) },
    { ...judge({ ...bar, id: "p" }, 0.5, undefined) },
  ];
  const raised = rebaseline(held, results, "raise", "the slash's new wind-up", "first number 2026-10-28");
  assert.deepEqual(raised.get("a"), { value: 12, why: "the slash's new wind-up" });
  assert.deepEqual(raised.get("b"), { value: 10, why: "first" });
  // A new row never takes the raise's why (e747fa20 gave six held.* rows shield_elbow's).
  assert.deepEqual(raised.get("n"), { value: 5, why: "first number 2026-10-28" });
  assert.deepEqual(raised.get("p"), { value: 0.5, why: "first number 2026-10-28" });
  const lowered = rebaseline(held, results, "lower", "nightly", "first number 2026-10-28");
  assert.deepEqual(lowered.get("a"), { value: 10, why: "first" });
  assert.equal(lowered.get("b")?.value, 8);
  // Nightly banks the new rows too, under their own why.
  assert.deepEqual(lowered.get("n"), { value: 5, why: "first number 2026-10-28" });
  assert.deepEqual(lowered.get("p"), { value: 0.5, why: "first number 2026-10-28" });
  const text = writeBaseline(raised, ["b", "a"]);
  assert.equal(text.split("\n")[1], "b\t10\tfirst");
  assert.deepEqual(readBaseline(text), raised);
});

test("--first writes held rows again as first numbers, their values kept, never a worse one", () => {
  const held = new Map([
    ["a", { value: 10, why: "another row's raise" }],
    ["b", { value: 10, why: "another row's raise" }],
    ["c", { value: 3, why: "kept" }],
  ]);
  const bar = c({ at_most: 1 });
  const results = [judge({ ...bar, id: "a" }, 9, held.get("a")), judge({ ...bar, id: "b" }, 12, held.get("b")), judge({ ...bar, id: "c" }, 3, held.get("c"))];
  const ok = rebank(held, results, ["a"], "first number 2026-10-28");
  assert.deepEqual(ok.refused, []);
  // Its value as held: lowering is nightly's.
  assert.deepEqual(ok.next.get("a"), { value: 10, why: "first number 2026-10-28" });
  assert.deepEqual(ok.next.get("c"), { value: 3, why: "kept" });
  const bad = rebank(held, results, ["b", "x"], "first number 2026-10-28");
  assert.equal(bad.refused.length, 2);
  assert.deepEqual(bad.next.get("b"), { value: 10, why: "another row's raise" });
});

test("timings are real seconds from the move set, as the Enshrouded comparison reads them", () => {
  const set = readSet(`(
    moves: {
        "slash": (clip: "A", from: 0.15, speed: 1.35, active: (0.70, 0.92), done: 1.55, cancel: 1.02, next: "backhand", hit_stop: 0.1,
            warp: (within: 3.2, cone: 70, stop_at: 1.45, most_mps: 4.0)),
        "backhand": (clip: "B", from: 0.2, speed: 1.35, active: (0.70, 0.90), done: 1.5, cancel: 0.98),
        "roll": (clip: "Roll", from: 0.05, speed: 1.45, done: 1.2, cancel: 1.0),
    },
  )`);
  const near = (a: number | undefined, b: number) => assert.ok(a !== undefined && Math.abs(a - b) < 0.002, `${a} against ${b}`);
  near(timing(set, { measure: "idle_to_hit", move: "slash" }), 0.407);
  near(timing(set, { measure: "hit_to_hit", move: "slash", next: "backhand" }), 0.607);
  near(timing(set, { measure: "total", move: "roll" }), 0.793);
  near(timing(set, { measure: "cancel", move: "roll" }), 0.655);
  near(timing(set, { measure: "chain_blows", move: "slash" }), 2);
  near(timing(set, { measure: "hit_stop", move: "slash" }), 0.1);
  near(timing(set, { measure: "spacing" }), 1.45 / 1.8);
  assert.equal(timing(set, { measure: "idle_to_hit", move: "roll" }), undefined);
});

test("cargo's lines and the move check's rows give each criterion its number", () => {
  const verdicts = testVerdicts("running 2 tests\ntest a::b ... ok\ntest a::c ... FAILED\n");
  assert.equal(verdicts.get("a::b"), 0);
  assert.equal(verdicts.get("a::c"), 1);
  // What a passing test measured, printed beside libtest's own progress dots.
  const measured = measuredValues("running 2 tests\nmeasured hit.kick.peak 0.8\n.measured hit.flash.frames 3\n.\ntest result: ok. 2 passed; 0 failed; 0 measured; 9 filtered out\n");
  assert.deepEqual([...measured], [["hit.kick.peak", 0.8], ["hit.flash.frames", 3]]);
  const rows = [
    { move: "slash", clip: "Fight_Slash", measured: { held_cm: 12 } },
    { clip: "Fight_Slash", measured: { held_cm: 1 } },
  ];
  assert.equal(movesValue(rows, { row: "slash", measure: "held_cm" }), 12);
  assert.equal(movesValue(rows, { row: "Fight_Slash", measure: "held_cm" }), 1);
});

test("every criterion is whole: an id once, a notice, a source, and a bar and check when measurable", () => {
  const criteria: Criterion[] = JSON.parse(readFileSync(CRITERIA, "utf8")).criteria;
  const ids = new Set<string>();
  for (const each of criteria) {
    assert.ok(!ids.has(each.id), `${each.id} twice`);
    ids.add(each.id);
    assert.ok(each.notice && each.source, each.id);
    assert.ok(each.noticed >= 1 && each.noticed <= 5, each.id);
    assert.ok(["cheap", "heavy"].includes(each.cost), each.id);
    if (each.measurable) assert.ok(each.bar && each.check, each.id);
    else assert.ok(each.needs, `${each.id} says what it needs to be measured`);
  }
});
