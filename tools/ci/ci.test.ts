import assert from "node:assert/strict";
import { test } from "node:test";
import { marker, parse, plan, type Issue } from "./ci.ts";

const sha = "0123456789abcdef0123456789abcdef01234567";
// As nightly.rs's `failure` writes it.
const failed = `failed at ${sha} (2026-10-19 04:00 UTC): fuzz, docs
  fuzz first fails at 89abcdef01 Roofs lie flat
    seed 7: z-fighting 3, was 0
    seed 9: gap 1, was 0

docs:
    docs: 1 faults
stages: ✓ clippy, ✓ tests, ✗ fuzz, ✗ docs
logs: .git/terra-nightly/logs/
`;

test("a failed report is read: each failing stage, its lines, the first's bisect, every stage's verdict", () => {
  const r = parse(failed)!;
  assert.equal(r.passed, false);
  assert.equal(r.commit, sha);
  assert.deepEqual(r.failures, [
    { stage: "fuzz", found: "first fails at 89abcdef01 Roofs lie flat", lines: ["seed 7: z-fighting 3, was 0", "seed 9: gap 1, was 0"] },
    { stage: "docs", lines: ["docs: 1 faults"] },
  ]);
  assert.deepEqual(r.stages.filter((s) => s.ok).map((s) => s.name), ["clippy", "tests"]);
});

test("a passed report is read, and anything else is not a report", () => {
  assert.equal(parse(`passed at ${sha} (now): nothing to lower\n`)!.passed, true);
  assert.equal(parse("nightly: origin/main was run already"), undefined);
});

test("a stage newly failing gets an issue, one already filed a comment only when its lines change", () => {
  const r = parse(failed)!;
  const first = plan(r, [], {}, "box");
  assert.deepEqual(first.actions.map((a) => [a.kind, a.stage]), [["create", "fuzz"], ["create", "docs"]]);
  const open: Issue[] = [
    { number: 1, title: "nightly: fuzz fails", body: marker("fuzz") },
    { number: 2, title: "nightly: docs fails", body: marker("docs") },
  ];
  assert.deepEqual(plan(r, open, first.posted, "box").actions, []);
  const changed = parse(failed.replace("gap 1", "gap 2"))!;
  assert.deepEqual(plan(changed, open, first.posted, "box").actions.map((a) => [a.kind, a.stage]), [["comment", "fuzz"]]);
});

test("an issue closes when its stage passes, and a run that failed to start closes nothing else", () => {
  const open: Issue[] = [
    { number: 1, title: "", body: marker("fuzz") },
    { number: 2, title: "", body: marker("tests") },
    { number: 3, title: "", body: marker("run") },
  ];
  const r = parse(failed)!;
  assert.deepEqual(plan(r, open, {}, "box").actions.filter((a) => a.kind === "close").map((a) => a.stage), ["tests", "run"]);
  const passed = parse(`passed at ${sha} (now): nothing to lower\n`)!;
  assert.deepEqual(plan(passed, open, {}, "box").actions.map((a) => a.kind), ["close", "close", "close"]);
  const noStart = { passed: false, commit: sha, when: "now", summary: "run", failures: [{ stage: "run", lines: ["✗ nightly: cannot fetch"] }], stages: [] };
  assert.deepEqual(plan(noStart, open, {}, "box").actions.map((a) => [a.kind, a.stage]), [["comment", "run"]]);
});
