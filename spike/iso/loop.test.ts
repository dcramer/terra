// loop.ts's tests, with no worker: the ratchet's rule (D634), the blind key, the parse of a
// biome file and of the refs' stats, and the line a biome prints. Run: node --test spike/iso/loop.test.ts

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { barOf, blindOrder, delta, EPS, type History, isChanged, newerKept, NOISE_MARGIN, noiseOf, outcome, type Pair, parseBiome, PIXEL_SHARE, PLAIN, pixelShare, refRanges, reserveRound, type Round, runNotes, script, shotAgainst, type Side, staleAgainst, statsLine, testResult, updateHistory, verdict, winnerOf } from "./loop.ts";

const pair = (a: Side, winner?: Side | "same", margin?: Pair["margin"]): Pair => ({ folder: "f", brief: "b", a, winner, margin });
const changed = (...pairs: Pair[]) => ({ changed: true, pairs });

test("a loss is clear or strong, or slight twice with the order swapped", () => {
  assert.equal(outcome(changed(pair("new", "kept", "clear"))), "loss");
  assert.equal(outcome(changed(pair("kept", "kept", "strong"))), "loss");
  assert.equal(outcome(changed(pair("new", "kept", "slight"))), "pending", "a slight loss waits for its swap");
  assert.equal(outcome(changed(pair("new", "kept", "slight"), pair("kept", "kept", "slight"))), "loss");
  assert.equal(outcome(changed(pair("new", "kept", "slight"), pair("kept", "new", "slight"))), "tie");
  assert.equal(outcome(changed(pair("new", "kept", "slight"), pair("kept", "same"))), "tie");
  assert.equal(outcome(changed(pair("new", "new", "clear"))), "win");
  assert.equal(outcome(changed(pair("new", "same"))), "tie");
  assert.equal(outcome(changed(pair("new"))), "pending");
});

test("a slight win waits for its swap and wins only when the same side wins both orders", () => {
  assert.equal(outcome(changed(pair("new", "new", "slight"))), "pending");
  assert.equal(outcome(changed(pair("new", "new", "slight"), pair("kept", "new", "slight"))), "win");
  // The same letter twice (A, new then kept): the judge's position bias, lead 8's 0930 and 0959.
  assert.equal(outcome(changed(pair("new", "new", "slight"), pair("kept", "kept", "slight"))), "tie");
  assert.equal(outcome(changed(pair("new", "new", "slight"), pair("kept", "same"))), "tie");
});

test("a round whose changed biomes only tied is TIED, never a KEEP; an unchanged round still keeps", () => {
  const still = { changed: false, pairs: [] };
  const bias = changed(pair("new", "new", "slight"), pair("kept", "kept", "slight"));
  assert.equal(verdict({ ruin: changed(pair("new", "same")), forest: still }).result, "TIED");
  assert.deepEqual(verdict({ ruin: bias, forest: still }).tied, ["ruin"]);
  assert.equal(verdict({ ruin: bias, marsh: changed(pair("new", "new", "clear")) }).result, "KEEP", "a win elsewhere carries the round");
  assert.equal(verdict({ ruin: still, forest: still }).result, "KEEP", "nothing moved: the kept stills stay the same look");
});

test("verdict refuses to keep a tied round's stills, and --revert records it with the kept still untouched", () => {
  const dir = mkdtempSync(join(tmpdir(), "iso-verdict-"));
  try {
    const keptStill = join(dir, "kept", "ruin.png");
    const roundStill = join(dir, "r1-ruin.png");
    mkdirSync(join(dir, "kept"));
    writeFileSync(keptStill, "kept look");
    writeFileSync(roundStill, "reverted look");
    const h: History = {
      kept: { ruin: { round: "r0", still: keptStill, stats: {} as never } },
      rounds: {
        r0: { at: "2026-11-07T08:00:00Z", rev: "a", dirty: false, job: "j", biomes: {}, verdict: { result: "KEEP", losers: [], at: "2026-11-07T08:01:00Z" } },
        r1: { at: "2026-11-07T09:00:00Z", rev: "b", dirty: true, job: "j", biomes: { ruin: { still: roundStill, stats: {} as never, changed: true, against: "r0", pairs: [pair("new", "new", "slight"), pair("kept", "kept", "slight")] } } },
      },
    };
    writeFileSync(join(dir, "history.json"), JSON.stringify(h));
    const run = (...args: string[]) => spawnSync(process.execPath, [join(import.meta.dirname, "loop.ts"), "verdict", ...args], { env: { ...process.env, ISO_KIT_HOME: dir }, encoding: "utf8" });
    const refused = run("r1");
    assert.equal(refused.status, 1, refused.stdout + refused.stderr);
    assert.match(refused.stdout, /only tied.*--revert/s);
    assert.equal(readFileSync(keptStill, "utf8"), "kept look", "the tie's still did not become the kept one");
    assert.equal(JSON.parse(readFileSync(join(dir, "history.json"), "utf8")).rounds.r1.verdict, undefined);
    const reverted = run("r1", "--revert");
    assert.equal(reverted.status, 0, reverted.stdout + reverted.stderr);
    const after: History = JSON.parse(readFileSync(join(dir, "history.json"), "utf8"));
    assert.equal(after.rounds.r1.verdict?.result, "REVERT");
    assert.equal(after.kept.ruin.round, "r0");
    assert.equal(readFileSync(keptStill, "utf8"), "kept look");
    // A revert copies nothing, so it is taken on a round older than another biome's kept still
    // (lead 9: 261107-1024's ruin tied after 1029's neon street was kept).
    h.rounds.r1.biomes.forest = { still: roundStill, stats: {} as never, changed: false, pairs: [] };
    h.rounds.r2 = { at: "2026-11-07T09:30:00Z", rev: "c", dirty: true, job: "j", biomes: {}, verdict: { result: "KEEP", losers: [], at: "2026-11-07T09:40:00Z" } };
    h.kept.forest = { round: "r2", still: keptStill, stats: {} as never };
    writeFileSync(join(dir, "history.json"), JSON.stringify(h));
    assert.match(run("r1").stdout, /older than the kept still of forest/);
    const late = run("r1", "--revert");
    assert.equal(late.status, 0, late.stdout + late.stderr);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
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

test("two shoots in one minute get their own rounds, and neither's history entry is lost", () => {
  const kit = mkdtempSync(join(tmpdir(), "iso-loop-"));
  const file = join(kit, "history.json");
  const entry = (still: string) => ({ still, stats: {}, pairs: [] }) as unknown as Round["biomes"][string];
  const round = (b: string): Round => ({ at: "t", rev: "r", dirty: true, job: "j", biomes: { [b]: entry(`${b}.png`) } });
  writeFileSync(file, JSON.stringify({ kept: {}, rounds: { old: round("forest") } }));
  // Both read the history at their start, before either wrote.
  const seen: History = JSON.parse(readFileSync(file, "utf8"));
  const first = reserveRound(kit, "261107-0250", (n) => !!seen.rounds[n]);
  const second = reserveRound(kit, "261107-0250", (n) => !!seen.rounds[n]);
  assert.equal(first, "261107-0250");
  assert.equal(second, "261107-0250-2");
  assert.equal(reserveRound(kit, "old", (n) => !!seen.rounds[n]), "old-2", "a name in the history is taken even with no folder");
  // A record lands in between; then each shoot writes minutes after it read.
  updateHistory(file, (h) => {
    h.rounds.old.biomes.forest.score = { overall: 5, gaps: "g" };
  });
  updateHistory(file, (h) => {
    h.rounds[first] = round("ruin");
  });
  updateHistory(file, (h) => {
    h.rounds[second] = round("marsh");
  });
  const now: History = JSON.parse(readFileSync(file, "utf8"));
  assert.deepEqual(Object.keys(now.rounds).sort(), ["261107-0250", "261107-0250-2", "old"]);
  assert.ok(now.rounds[first].biomes.ruin && now.rounds[second].biomes.marsh);
  assert.equal(now.rounds.old.biomes.forest.score?.overall, 5);
  rmSync(kit, { recursive: true });
});

test("a verdict on a round older than a biome's kept still is refused, a newer or the same round's is not", () => {
  const entry = { still: "s", stats: {}, pairs: [] } as unknown as Round["biomes"][string];
  const round = (at: string, ...bs: string[]): Round => ({ at, rev: "r", dirty: false, job: "j", biomes: Object.fromEntries(bs.map((b) => [b, entry])) });
  const kept = (r: string) => ({ round: r, still: "k", stats: {} }) as unknown as History["kept"][string];
  const h = {
    rounds: { old: round("2026-11-07T05:00:00Z", "forest", "ruin"), mid: round("2026-11-07T06:00:00Z", "forest"), new: round("2026-11-07T07:00:00Z", "marsh") },
    kept: { forest: kept("mid"), ruin: kept("j1"), marsh: kept("new") },
  };
  assert.deepEqual(newerKept(h, "old"), ["forest"], "mid's forest is newer than old's: keeping old would undo it");
  assert.deepEqual(newerKept(h, "mid"), [], "the round that is kept may be judged again");
  assert.deepEqual(newerKept(h, "new"), []);
  assert.deepEqual(newerKept(h, "gone"), []);
});

test("a round shot against a kept still since replaced keeps none of its ties and is refused for a change", () => {
  const e = (changed: boolean, against?: string) => ({ still: "s", stats: {}, pairs: [], changed, against }) as unknown as Round["biomes"][string];
  const kept = (r: string) => ({ round: r, still: "k", stats: {} }) as unknown as History["kept"][string];
  const h: Pick<History, "kept" | "rounds"> = {
    rounds: {
      base: { at: "08:00", rev: "r", dirty: false, job: "j", biomes: { marsh: e(false), ruin: e(false) }, verdict: { result: "KEEP", losers: [], at: "08:10" } },
      // Both shot against base; marsh's change is then kept, after ruin's round began.
      wet: { at: "08:20", rev: "r", dirty: true, job: "j", biomes: { marsh: e(true, "base"), ruin: e(false, "base") }, verdict: { result: "KEEP", losers: [], at: "08:40" } },
      grime: { at: "08:25", rev: "r", dirty: true, job: "j", biomes: { marsh: e(false), ruin: e(true) } },
      late: { at: "08:30", rev: "r", dirty: true, job: "j", biomes: { marsh: e(true, "base") } },
    },
    kept: { marsh: kept("wet"), ruin: kept("wet") },
  };
  assert.equal(shotAgainst(h, "grime", "marsh"), "base", "inferred from the last keep before it was shot");
  assert.deepEqual(staleAgainst(h, "grime"), { ties: ["marsh"], changed: [] }, "grime's marsh tied base's still, not wet's; ruin's kept moved to wet only as a tie, the same look");
  assert.deepEqual(staleAgainst(h, "late"), { ties: [], changed: ["marsh"] });
  assert.deepEqual(staleAgainst(h, "wet"), { ties: [], changed: [] }, "the kept round itself is current");
});

test("a biome's own noise lifts its bars: a plain re-shoot within it is the same, a change past it is not", () => {
  const im = (v: number, n = 0) => {
    const data = new Uint8Array(300).fill(v);
    data.fill(v + 40, 0, n * 3);
    return { width: 10, height: 10, data };
  };
  // Three shots of one commit: two pixels of a hundred differ between the first two, one more
  // with the third (the neon street's rain and probe, in small).
  const n = noiseOf([im(10), im(10, 2), im(10, 3)], "abc", "t");
  assert.equal(n.share, 0.03);
  assert.equal(n.shots, 3);
  const bar = barOf(n);
  assert.ok(NOISE_MARGIN > 1);
  assert.equal(bar.share, 0.03 * NOISE_MARGIN, "the most two shots differ by, times the margin");
  assert.equal(isChanged({}, 0.03 * NOISE_MARGIN - 0.001, bar), false, "a re-shoot within the noise is the same");
  assert.equal(isChanged({}, 0.03), true, "the plain bar would have asked for judges");
  assert.equal(isChanged({}, 0.03 * NOISE_MARGIN + 0.001, bar), true);
  // A measure's spread lifts only that measure's bar; the plain one stays the floor.
  const spread = barOf({ ...n, spread: { dark_share: 0.004, saturation: 0.0001 }, share: 0.001 });
  assert.equal(spread.share, PIXEL_SHARE);
  assert.equal(spread.eps.dark_share, Math.round(0.004 * NOISE_MARGIN * 1e4) / 1e4);
  assert.equal(spread.eps.saturation, EPS);
  assert.equal(isChanged({ dark_share: 0.004 * NOISE_MARGIN - 0.0005 }, 0, spread), false);
  assert.equal(isChanged({ dark_share: 0.004 * NOISE_MARGIN + 0.0005 }, 0, spread), true);
  assert.equal(isChanged({ luma_p95: 0.002 }, 0, spread), true, "a measure with no spread keeps the plain bar");
  assert.equal(barOf(undefined), PLAIN);
  assert.throws(() => noiseOf([im(1)], "abc", "t"), /two shots/);
  const line = statsLine("neon-street", S, null, { round: "k", delta: {}, share: 0.03, bar });
  assert.match(line, new RegExp(`same as kept k \\(its noise: ${(3 * NOISE_MARGIN).toFixed(2)}% px\\)$`));
});

test("the noise script shoots each biome N times and no film", () => {
  const sh = script(["ruin", "marsh"], true, 3);
  for (const b of ["ruin", "marsh"]) for (const n of [1, 2, 3]) assert.ok(sh.includes(`--shot $d/${b}-${n}.png`), `${b} ${n}`);
  assert.ok(!sh.includes("--film"));
  assert.ok(script(["ruin"], false).includes("--shot $d/ruin.png"));
});

test("a re-judge replaces a pair's answer, keeps the earlier one and warns when the verdict no longer follows", () => {
  const dir = mkdtempSync(join(tmpdir(), "iso-rejudge-"));
  try {
    // Round 261107-1544's shape: kept on a non-blind judge's slight win both orders.
    const h: History = {
      kept: {},
      rounds: {
        r1: { at: "2026-11-07T09:00:00Z", rev: "b", dirty: false, job: "j", biomes: { forest: { still: "s", stats: {} as never, changed: true, pairs: [pair("kept", "new", "slight"), pair("new", "new", "slight")] } }, verdict: { result: "KEEP", losers: [], at: "2026-11-07T09:10:00Z" } },
      },
    };
    writeFileSync(join(dir, "history.json"), JSON.stringify(h));
    const run = (...args: string[]) => spawnSync(process.execPath, [join(import.meta.dirname, "loop.ts"), "record", "r1", "forest", "pair", ...args], { env: { ...process.env, ISO_KIT_HOME: dir }, encoding: "utf8" });
    const plain = run("same", "slight");
    assert.equal(plain.status, 1, "without --rejudge an answered pair is not overwritten");
    assert.match(plain.stdout, /--rejudge/);
    assert.equal(run("A", "slight", "--rejudge", "3").status, 1, "there is no pair 3");
    const first = run("same", "slight", "--rejudge");
    assert.equal(first.status, 0, first.stdout + first.stderr);
    assert.match(first.stdout, /pair 1 re-judged.*\n.*warning: r1's verdict was KEEP, its pairs now say TIED/);
    const second = run("--rejudge", "2", "same", "slight");
    assert.equal(second.status, 0, second.stdout + second.stderr);
    const after: History = JSON.parse(readFileSync(join(dir, "history.json"), "utf8"));
    const [p1, p2] = after.rounds.r1.biomes.forest.pairs;
    assert.deepEqual([p1.winner, p1.earlier], ["same", [{ winner: "new", margin: "slight" }]]);
    assert.deepEqual([p2.winner, p2.earlier], ["same", [{ winner: "new", margin: "slight" }]]);
    assert.equal(outcome(after.rounds.r1.biomes.forest), "tie");
    // A blind answer that agrees with the verdict warns of nothing.
    h.rounds.r1.biomes.forest.pairs = [pair("kept", "new", "clear")];
    writeFileSync(join(dir, "history.json"), JSON.stringify(h));
    const agrees = run("B", "strong", "--rejudge", "1");
    assert.equal(agrees.status, 0, agrees.stdout + agrees.stderr);
    assert.doesNotMatch(agrees.stdout, /warning/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("drop closes a round no judge answered as DROP, never one a judge answered unless its kept still was replaced", () => {
  const dir = mkdtempSync(join(tmpdir(), "iso-drop-"));
  try {
    const h: History = {
      kept: { ruin: { round: "r0", still: "k", stats: {} as never } },
      rounds: {
        r0: { at: "2026-11-07T08:00:00Z", rev: "a", dirty: false, job: "j", biomes: { ruin: { still: "s0", stats: {} as never, changed: true, pairs: [pair("new", "new", "clear")] } }, verdict: { result: "KEEP", losers: [], at: "2026-11-07T08:01:00Z" } },
        r1: { at: "2026-11-07T09:00:00Z", rev: "b", dirty: false, job: "j", biomes: { ruin: { still: "s1", stats: {} as never, changed: true, against: "r0", pairs: [pair("new")] } } },
        r2: { at: "2026-11-07T09:10:00Z", rev: "c", dirty: false, job: "j", biomes: { ruin: { still: "s2", stats: {} as never, changed: true, against: "r0", pairs: [pair("new", "kept", "slight"), pair("kept")] } } },
      },
    };
    writeFileSync(join(dir, "history.json"), JSON.stringify(h));
    const run = (...args: string[]) => spawnSync(process.execPath, [join(import.meta.dirname, "loop.ts"), "drop", ...args], { env: { ...process.env, ISO_KIT_HOME: dir }, encoding: "utf8" });
    assert.equal(run("r1").status, 1, "a drop says why");
    const answered = run("r2", "superseded");
    assert.equal(answered.status, 1, answered.stdout);
    assert.match(answered.stdout, /closes by its pairs.*verdict r2 --revert/s);
    const dropped = run("r1", "a plain re-shoot");
    assert.equal(dropped.status, 0, dropped.stdout + dropped.stderr);
    const after: History = JSON.parse(readFileSync(join(dir, "history.json"), "utf8"));
    assert.deepEqual([after.rounds.r1.verdict?.result, after.rounds.r1.verdict?.dropped], ["DROP", "a plain re-shoot"]);
    assert.equal(after.kept.ruin.round, "r0", "a dropped round's still is never kept");
    assert.match(run("r1", "again").stdout, /closed already \(DROP: a plain re-shoot\)/);
    // r2's answer was against r0's still; once r3 replaced it, the answer says nothing and r2 drops.
    after.rounds.r3 = { at: "2026-11-07T09:30:00Z", rev: "d", dirty: false, job: "j", biomes: { ruin: { still: "s3", stats: {} as never, changed: true, against: "r0", pairs: [pair("new", "new", "clear")] } }, verdict: { result: "KEEP", losers: [], at: "2026-11-07T09:40:00Z" } };
    after.kept.ruin = { round: "r3", still: "k", stats: {} as never };
    writeFileSync(join(dir, "history.json"), JSON.stringify(after));
    const stale = run("r2", "judged against a replaced still");
    assert.equal(stale.status, 0, stale.stdout + stale.stderr);
    // A later shoot measures against the last KEEP, never a DROP made after it.
    after.rounds.r4 = { at: "2026-11-07T10:00:00Z", rev: "e", dirty: false, job: "j", biomes: { ruin: { still: "s4", stats: {} as never, changed: false, pairs: [] } } };
    after.rounds.r1.verdict = { result: "DROP", losers: [], at: "2026-11-07T09:50:00Z", dropped: "x" };
    assert.equal(shotAgainst(after, "r4", "ruin"), "r3");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
