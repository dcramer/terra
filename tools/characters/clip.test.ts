import assert from "node:assert/strict";
import { test } from "node:test";
import { choose } from "./clip_spec.ts";
import { scores, scoringSet, takeName } from "./clip.ts";
import { kept } from "./motion.ts";

test("the scoring set holds each take as a move by a name moves.ts reads, from its first frame to its last", () => {
  const text = scoringSet([
    { clip: "Keyed_Backhand", take: 0, end: 1.567, active: [0.7, 0.9] },
    { clip: "Keyed_Backhand", take: 1, end: 1.6 },
  ]);
  // moves.ts reads a move's name as \w+, so a take's `#` stays in its clip only.
  const names = [...text.matchAll(/"(\w+)":\s*\(/g)].map((m) => m[1]);
  assert.deepEqual(names, ["Keyed_Backhand_0", "Keyed_Backhand_1"]);
  assert.match(text, /"Keyed_Backhand_0": \(clip: "Keyed_Backhand#0", from: 0\.00, active: \(0\.70, 0\.90\), done: 1\.57,/);
  // A take that never strikes has no active frames to check.
  assert.doesNotMatch(text.split("\n").find((l) => l.includes("Keyed_Backhand_1"))!, /active/);
  assert.equal(takeName("Keyed_Backhand", 1), "Keyed_Backhand#1");
});

test("each take's score is its own rows' failures with what the step found, and the first that passes is kept", () => {
  const rows = [
    { move: "Keyed_Backhand_0", clip: "Keyed_Backhand#0", bad: ["sweeps 30 cm to the left, not left to right (spec)"], gaps: [0.02, 0.04] as [number, number] },
    { move: "Keyed_Backhand_1", clip: "Keyed_Backhand#1", bad: [], gaps: [0.03, 0.08] as [number, number] },
    { move: "Keyed_Backhand_2", clip: "Keyed_Backhand#2", bad: [], gaps: [0.01, 0.02] as [number, number] },
    // Another clip's take, and a row of a clip shown besides the moves: neither is a take.
    { move: "Keyed_Slash_0", clip: "Keyed_Slash#0", bad: [] },
    { clip: "Keyed_Backhand#1", bad: ["hands 3.0 cm deep"] },
  ];
  const got = scores(rows, "Keyed_Backhand", new Map([[1, ["never strikes"]]]));
  assert.deepEqual(got.map((s) => [s.take, s.failures.length, s.off]), [[0, 1, 0.04], [1, 1, 0.08], [2, 0, 0.02]]);
  assert.equal(choose(got)!.take, 2);
});

test("a kept take is the entry's, with as many samples as make it again", () => {
  assert.deepEqual(kept({ seed: 2 }, 1), { seed: 2, take: 1 });
  assert.deepEqual(kept({ seed: 2 }, 4), { seed: 2, take: 4, samples: 5 });
  assert.deepEqual(kept({ seed: 2, samples: 6 }, 1), { seed: 2, samples: 6, take: 1 });
});
