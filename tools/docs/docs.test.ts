import assert from "node:assert/strict";
import { test } from "node:test";
import { anchors, brokenLinks, decisionTable, nextDecision, unindexed, unknownDecisions } from "./docs.ts";

test("headings anchor as GitHub makes them, repeats numbered", () => {
  const a = anchors("# Worlds: the realms\n## Size\n## Size\n## What `it` asks, (D4)\n```\n# not a heading\n```\n");
  assert.deepEqual([...a], ["worlds-the-realms", "size", "size-1", "what-it-asks-d4"]);
});

test("a link to a missing file or heading is a fault; the web and code are not looked at", () => {
  const files: Record<string, string> = { "docs/a.md": "## Water\n", "docs/b.md": "" };
  const read = (p: string) => files[p];
  const text = "[ok](a.md#water) [gone](c.md) [no](a.md#size) [web](https://x.y/z) `[code](nope.md)` [self](#here)\n## Here\n";
  assert.deepEqual(brokenLinks("docs/b.md", text, (p) => (p === "docs/b.md" ? text : read(p))), [
    "docs/b.md: links to c.md, which does not exist",
    "docs/b.md: links to a.md#size, which has no such heading",
  ]);
});

test("a decision row needs its seven cells, a known owner, status and who decided", () => {
  const table = [
    "| D1 | Team | Design | Developers | Decided | Two. | Why. |",
    "| D2 | Bevy | Tech | Agent | Leaning | Bevy. | Why. |",
    "| D3 | Old | Design | — | Superseded by D1, D2 | Was. | Why. |",
    "| D4 | Odd | Design | Agent | Open | Maybe. | Why. |",
    "| D5 | Pipe | Tech | Agent | Decided | a | b | Why. |",
    "| D1 | Again | Art | Someone | Done | x | y |",
  ].join("\n");
  const { faults, ids } = decisionTable(table);
  assert.deepEqual([...ids], ["D1", "D2", "D3", "D4", "D5"]);
  assert.deepEqual(faults, [
    'docs/decisions.md: D4 has By "Agent": Developers or Agent, or — when Open, Rejected or Superseded',
    "docs/decisions.md: D5 has 8 cells, not 7 (a | in its text?)",
    'docs/decisions.md: D1 has Owner "Art", not Design or Tech',
    'docs/decisions.md: D1 has Status "Done"',
    'docs/decisions.md: D1 has By "Someone": Developers or Agent, or — when Open, Rejected or Superseded',
    'docs/decisions.md: D1 is in the table twice, "Team" and "Again": renumber the one you added to D6, the next free number',
  ]);
});

test("the next free decision number is one past the highest, gaps left alone", () => {
  assert.equal(nextDecision(["D1", "D12", "D3"]), 13);
  assert.equal(nextDecision([]), 1);
});

test("citing a decision the table lacks is a fault, once per file", () => {
  assert.deepEqual(unknownDecisions("x.md", "D1, D9 and D9 again; not DD9 or D1234", new Set(["D1"])), [
    "x.md: cites D9, which is not in docs/decisions.md",
  ]);
});

test("an index must name every entry of its folder", () => {
  assert.deepEqual(unindexed("r/README.md", "[a](a.md)", ["a.md", "b.md"]), ["r/README.md: does not list b.md"]);
});
