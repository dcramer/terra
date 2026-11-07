// loop.ts's tests, with no worker: the ratchet's rule (D634), the blind key, the parse of a
// biome file and of the refs' stats, and the line a biome prints. Run: node --test spike/iso/loop.test.ts

import assert from "node:assert/strict";
import { test } from "node:test";
import { blindOrder, delta, isChanged, outcome, type Pair, parseBiome, pixelShare, refRanges, runNotes, type Side, statsLine, testResult, verdict, winnerOf } from "./loop.ts";

const pair = (a: Side, winner?: Side | "same", margin?: Pair["margin"]): Pair => ({ folder: "f", brief: "b", a, winner, margin });
const changed = (...pairs: Pair[]) => ({ changed: true, pairs });

test("a loss is clear or strong, or slight twice with the order swapped", () => {
  assert.equal(outcome(changed(pair("new", "kept", "clear"))), "loss");
  assert.equal(outcome(changed(pair("kept", "kept", "strong"))), "loss");
  assert.equal(outcome(changed(pair("new", "kept", "slight"))), "pending", "a slight loss waits for its swap");
  assert.equal(outcome(changed(pair("new", "kept", "slight"), pair("kept", "kept", "slight"))), "loss");
  assert.equal(outcome(changed(pair("new", "kept", "slight"), pair("kept", "new", "slight"))), "tie");
  assert.equal(outcome(changed(pair("new", "kept", "slight"), pair("kept", "same"))), "tie");
  assert.equal(outcome(changed(pair("new", "new", "slight"))), "win");
  assert.equal(outcome(changed(pair("new", "same"))), "tie");
  assert.equal(outcome(changed(pair("new"))), "pending");
});

test("an unchanged biome ties with no judge; one with no kept still is new", () => {
  assert.equal(outcome({ changed: false, pairs: [] }), "tie");
  assert.equal(outcome({ changed: null, pairs: [] }), "new");
});

test("the round keeps only when no biome lost, and waits on an unanswered pair", () => {
  const tie = { changed: false, pairs: [] };
  assert.equal(verdict({ ruin: tie, forest: changed(pair("new", "new", "clear")) }).result, "KEEP");
  const lost = verdict({ ruin: tie, forest: changed(pair("new", "new", "clear")), marsh: changed(pair("kept", "kept", "clear")) });
  assert.equal(lost.result, "REVERT");
  assert.deepEqual(lost.losers, ["marsh"]);
  const waits = verdict({ ruin: changed(pair("new", "kept", "slight")), forest: changed(pair("new", "new", "strong")) });
  assert.equal(waits.result, null);
  assert.deepEqual(waits.pending, ["ruin"]);
});

test("the blind key: both orders come up, the second pair swaps the first, A and B map through it", () => {
  let seed = 7;
  const rng = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  const firsts = new Set(Array.from({ length: 40 }, () => blindOrder(rng)));
  assert.deepEqual([...firsts].sort(), ["kept", "new"]);
  assert.equal(blindOrder(rng, "new"), "kept");
  assert.equal(blindOrder(rng, "kept"), "new");
  assert.equal(winnerOf("A", "kept"), "kept");
  assert.equal(winnerOf("b", "kept"), "new");
  assert.equal(winnerOf("same", "new"), "same");
  assert.throws(() => winnerOf("C", "new"), /A, B or same/);
});

test("a biome file's refs and kinds are read past its comments", () => {
  const ron = `// refs: "not-this", kind: [Day]\n(\n    kind: [Night, Rain],\n    refs: "neon-street", // the street\n    layout: Ruin,\n)`;
  assert.deepEqual(parseBiome(ron), { refs: "neon-street", kind: ["Night", "Rain"] });
  assert.deepEqual(parseBiome("(layout: Ruin)"), { refs: null, kind: [] });
});

test("the refs' ranges span the named refs in stats.json and name the ones it lacks", () => {
  const all = { a: { luma_p95: 0.2, fine_detail: 0.13 }, b: { luma_p95: 0.5, fine_detail: 0.2 }, other: { luma_p95: 0.9 } };
  const r = refRanges(all, ["a", "b", "c"]);
  assert.deepEqual(r.ranges?.luma_p95, [0.2, 0.5]);
  assert.deepEqual(r.ranges?.fine_detail, [0.13, 0.2]);
  assert.deepEqual(r.missing, ["c"]);
  assert.equal(refRanges(all, []).ranges, null, "a biome with no refs yet");
});

const S = { luma_p5: 0, luma_p50: 0.012, luma_p95: 0.1645, luma_mean: 0.0379, fine_detail: 0.1121, saturation: 0.5293, dark_share: 0.8165, mid_share: 0.1724, bright_share: 0.0111 };

test("the GPU's noise between two fixed-step shots is no change; a moved measure or patch of pixels is", () => {
  const noise = delta({ ...S, saturation: 0.5294 }, S);
  assert.equal(isChanged(noise, 0.00007), false);
  assert.equal(isChanged(delta({ ...S, luma_p95: 0.17 }, S), 0), true);
  assert.equal(isChanged(noise, 0.01), true);
  const im = (v: number) => ({ width: 2, height: 2, data: new Uint8Array(12).fill(v) });
  assert.equal(pixelShare(im(10), im(12)), 0, "two levels is noise");
  assert.equal(pixelShare(im(10), im(13)), 1);
  assert.equal(pixelShare(im(10), { width: 4, height: 1, data: new Uint8Array(12) }), 1, "another size");
});

test("a biome's line marks a measure outside its refs and says what moved from the kept still", () => {
  const ranges = { fine_detail: [0.13, 0.23] as [number, number], luma_p95: [0.1, 0.2] as [number, number] };
  const line = statsLine("ruin", S, ranges, { round: "j1", delta: { luma_p95: 0.0035, fine_detail: 0 }, share: 0.02 });
  assert.match(line, /^ruin: /);
  assert.match(line, /fine 0\.1121↓ \(0\.130-0\.230\)/);
  assert.match(line, /p95 0\.1645 \(0\.100-0\.200\)/);
  assert.match(line, /vs kept j1: p95 \+0\.0035, 2\.00% px differ$/);
  assert.match(statsLine("ruin", S, null, { round: "j1", delta: {}, share: 0 }), /\(no refs yet\) \| same as kept j1$/);
  assert.match(statsLine("x", S, null, null), /no kept still: new$/);
});

test("the spike's tests are read from cargo's log", () => {
  assert.deepEqual(testResult("test result: ok. 81 passed; 0 failed; 0 ignored\ntest result: ok. 2 passed; 0 failed;"), { passed: 83, failed: false });
  assert.equal(testResult("test result: FAILED. 80 passed; 1 failed;").failed, true);
  assert.equal(testResult("error[E0425]: cannot find value").failed, true);
  assert.equal(testResult("").failed, true, "no tests ran");
});

test("a run log's budgets and shader errors reach the round", () => {
  const log = [
    "\x1b[2m2026-11-07T08:45:16.36Z\x1b[0m \x1b[33m WARN\x1b[0m bevy_audio: No audio device found.",
    "weather budget (neon-street): rain 5600 streaks",
    "\x1b[2m2026-11-07T08:45:16.91Z\x1b[0m \x1b[33m WARN\x1b[0m bevy_shader::shader_cache: Shader `w.wesl` has an unresolved import:",
    "error: module not found: `bevy_pbr::mesh_view_bindings`",
    "\x1b[2m2026-11-07T08:45:16.97Z\x1b[0m \x1b[33m WARN\x1b[0m bevy_shader::shader_cache: Shader `w.wesl` has an unresolved import:",
    "error: module not found: `bevy_pbr::mesh_view_bindings`",
  ].join("\n");
  assert.deepEqual(runNotes(log), [
    "weather budget (neon-street): rain 5600 streaks",
    "WARN bevy_shader::shader_cache: Shader `w.wesl` has an unresolved import: error: module not found: `bevy_pbr::mesh_view_bindings`",
  ]);
});
