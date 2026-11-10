// The hold solver (solve.ts): a planted fault with a known answer is found, the same spec gives
// the same bytes, a parameter left at a bound is said, and a bad spec is an error that says what
// to do. Only the last test starts moves.ts (three processes, a clip each); none needs a GPU.

import assert from "node:assert/strict";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { Glb, type Q, qinv, qmul } from "./glb.ts";
import type { Measure } from "./holds.ts";
import { pool } from "./holds.ts";
import { type Dim, type Spec, apply, dimsOf, holdsMeasure, readSpec, report, scored, search, solve } from "./solve.ts";
import { FIGHT, type Tune, aimOf, anglesOf, heldPose, readSets, setsWith } from "./tune.ts";

const ROOT = join(import.meta.dirname, "..", "..");
const OUT = join(ROOT, "tools", "characters", "out", "solve-test");
const off = (a: Q, b: Q) => (2 * Math.acos(Math.min(1, Math.abs(qmul(qinv(a), b)[3]))) * 180) / Math.PI;
const m = (look: string, measure: string, value: number, bar: number | null, clip = "Jog_Fwd_Loop"): Measure => ({ look, measure, body: "base", clip, time: null, value, bar, unit: "deg", ok: bar === null ? null : value <= bar, where: "" });

test("a hand rolled and tipped off a passing pose is dialled back to within half a step", async () => {
  const body = new Glb(join(ROOT, "assets/models/character/base.glb"));
  const clips = new Glb(join(ROOT, "assets/models/character/fight.glb"));
  const sets = readSets();
  const hold = sets.Carry_Tool.hold![0];
  const roll = hold.roll!.hand_r;
  const aim = anglesOf(hold.aim!.hand_r);
  const want = heldPose(body, clips, "Carry_Tool", sets.Carry_Tool).get("hand_r")!;
  // The planted fault: the committed pose with its hand rolled 29.7 degrees more and aimed 9.8
  // degrees higher. The bar is half a degree of the hand off the committed pose's.
  const planted = structuredClone(sets);
  planted.Carry_Tool.hold![0].roll!.hand_r = roll + 29.7;
  planted.Carry_Tool.hold![0].aim!.hand_r = aimOf(aim.up + 9.8, aim.round);
  const [R, U] = ["pose.Carry_Tool.roll.hand_r", "pose.Carry_Tool.aim.hand_r.up"];
  const spec: Spec = { looks: { stone_axe: ["Jog_Fwd_Loop"] }, measures: ["carry.*"], parameters: [{ name: R, lo: roll - 45, hi: roll + 45, step: 0.5 }, { name: U, lo: aim.up - 20, hi: aim.up + 20, step: 0.5 }] };
  const measure = async (tune: Tune) => [m("stone_axe", "carry.hand_off", off(heldPose(body, clips, "Carry_Tool", setsWith(tune, planted).sets.Carry_Tool).get("hand_r")!, want), 0.5)];
  const solved = await solve(spec, measure, { sets: planted });
  assert.ok(solved.start.score[0] > 25, `the planted pose starts ${solved.start.score[0]} over`);
  assert.equal(solved.best.score[0], 0);
  const [r, u] = solved.best.at;
  assert.ok(Math.abs(r - roll) <= 0.25 && Math.abs(u - aim.up) <= 0.25, `roll ${r} for ${roll}, up ${u} for ${aim.up}`);
  assert.ok(solved.trace.length <= 160, `${solved.trace.length} evaluations`);
  // The edits are the answer, where each lives.
  assert.deepEqual(solved.edits.map((e) => [e.file, e.key]).sort(), [[FIGHT, "Carry_Tool.hold[0].aim.hand_r"], [FIGHT, "Carry_Tool.hold[0].roll.hand_r"]]);
  assert.equal(solved.edits.find((e) => e.key.endsWith("roll.hand_r"))!.value, r);
  // The same spec, the same bytes; a report within the 25 lines.
  const again = await solve(spec, measure, { sets: planted });
  const lines = report("planted", solved);
  assert.equal(lines.join("\n"), report("planted", again).join("\n"));
  assert.ok(lines.length <= 24 && lines.at(-1)!.startsWith("✓ solve"), lines.join("\n"));
  assert.ok(lines.some((l) => /stone_axe carry.hand_off: \d+\.\d deg \(base, Jog_Fwd_Loop\) -> 0\.\d deg/.test(l)), lines.join("\n"));
});

test("the search is the seed's: the same trace twice, another seed another, the known best found", async () => {
  // Two basins: the lower one at (37, -12), a shallower one at (-30, 30) a sample can start in.
  const dims: Dim[] = [{ name: "a", start: 0, lo: -50, hi: 50, step: 1 }, { name: "b", start: 3, lo: -47, hi: 53, step: 1 }];
  const f = ([a, b]: number[]) => Math.min(Math.abs(a - 37) + Math.abs(b + 12), 5 + Math.abs(a + 30) + Math.abs(b - 30));
  const evaluate = async (points: number[][]) => points.map((p) => ({ score: [f(p)], detail: null }));
  const run = (seed: number) => search(dims, evaluate, { seed, samples: 16, starts: 3, budget: 400 });
  const [one, two, other] = [await run(7), await run(7), await run(8)];
  assert.deepEqual(one.best.at, [37, -12]);
  assert.deepEqual(one.trace.map((t) => t.at), two.trace.map((t) => t.at));
  assert.notDeepEqual(one.trace.map((t) => t.at), other.trace.map((t) => t.at));
  assert.deepEqual(one.trace[0].at, [0, 3]);
  assert.equal(new Set(one.trace.map((t) => t.at.join())).size, one.trace.length);
  // A budget is kept.
  assert.ok((await search(dims, evaluate, { seed: 7, samples: 16, starts: 3, budget: 30 })).trace.length <= 30);
});

test("the objective is over the bars first, then the thinnest margin, then the nearest; a bound is said", async () => {
  const spec: Spec = { looks: { basket: ["Jog_Fwd_Loop"] }, hold: { knife: ["Jog_Fwd_Loop"] }, measures: ["carry.*"], parameters: [] };
  const got = scored([m("basket", "carry.ground", 4, 0), m("basket", "carry.ground", 6, 0, "Sprint_Loop"), m("basket", "carry.legs", -3, 1), m("basket", "use.trunk", 50, 1), m("basket", "carry.out", 70, null), m("knife", "carry.trunk", 2.5, 1), m("torch", "carry.trunk", 9, 1)], spec);
  // The worst of a measure counts once (6 over 0), a held look's excess counts (1.5), a look or
  // measure the spec does not name does not; the margin is the improved looks' alone.
  assert.equal(got.over, 7.5);
  assert.equal(got.margin, -6);
  assert.equal(got.worst.get("basket carry.ground")!.clip, "Sprint_Loop");
  // The give's back at 40 would be best, but a held look fails past 30: the answer stays at 30,
  // and give.out, wanted past its bound, is left at it and said.
  const give: Spec = { looks: { basket: ["Jog_Fwd_Loop"] }, hold: { knife: ["Jog_Fwd_Loop"] }, measures: ["carry.*"], parameters: [{ name: "give.back", lo: 0, hi: 60, step: 1 }, { name: "give.out", lo: 10, hi: 30, step: 1 }] };
  const measure = async (t: Tune) => [m("basket", "carry.ground", Math.abs(t["give.back"] - 40) + (99 - t["give.out"]) / 10 - 9, 0), m("knife", "carry.trunk", 1 + 5 * (t["give.back"] - 30), 1)];
  const solved = await solve(give, measure);
  assert.deepEqual(solved.best.at, [30, 30]);
  const lines = report("give", solved);
  assert.ok(lines.some((l) => l.includes("give.out") && l.includes("left at its upper bound 30")), lines.join("\n"));
  assert.ok(!lines.some((l) => l.includes("give.back") && l.includes("bound")), lines.join("\n"));
  assert.deepEqual(solved.edits.map((e) => e.key).sort(), ["GIVE_BACK_DEG", "GIVE_OUT_DEG"]);
});

test("a bad spec is an error that says what to do", async () => {
  mkdirSync(OUT, { recursive: true });
  const write = (spec: unknown) => {
    writeFileSync(join(OUT, "spec.json"), JSON.stringify(spec));
    return join(OUT, "spec.json");
  };
  const good = { looks: { basket: ["Jog_Fwd_Loop"] }, measures: ["carry.*"], parameters: [{ name: "give.back", lo: 0, hi: 60, step: 1 }] };
  assert.doesNotThrow(() => readSpec(write(good)));
  assert.throws(() => readSpec(write({ ...good, looks: { baskit: ["Jog_Fwd_Loop"] } })), /no held look baskit .*D556.*name any of .*basket/);
  assert.throws(() => readSpec(write({ ...good, hold: { knife: ["Jogging"] } })), /knife .*Jogging is not .*D556.*Jog_Fwd_Loop/);
  assert.throws(() => readSpec(write({ ...good, bodies: ["giant"] })), /no body giant .*D556.*base/);
  assert.throws(() => dimsOf({ ...good, parameters: [{ name: "give.lift", lo: 0, hi: 1, step: 0.1 }] }), /no parameter give.lift.*D556.*give.back/);
  assert.throws(() => dimsOf({ ...good, parameters: [{ name: "give.back", lo: 40, hi: 60, step: 1 }] }), /give.back starts at 25.*outside its bounds.*widen/);
  await assert.rejects(solve({ ...good, measures: ["carried.*"] }, async () => [m("basket", "carry.ground", 1, 0)]), /match no barred measure.*D556.*basket carry.ground/);
  rmSync(OUT, { recursive: true, force: true });
});

test("--apply's edits change the named values alone and keep the rest of each file", () => {
  const dir = (f: string) => join(OUT, f);
  for (const d of ["art/animation/sets", "crates/terra_walker/src", "tools/characters"]) mkdirSync(dir(d), { recursive: true });
  const fight = '{\n  "Carry_Tool": {\n    "swing": 0.25,\n    "hold": [\n      {\n        "aim": {\n          "hand_r": [\n            -0.261,\n            -0.965,\n            0.0\n          ]\n        },\n        "roll": {\n          "hand_r": 8.0,\n          "twist": 7.7\n        }\n      }\n    ]\n  }\n}\n';
  writeFileSync(dir(FIGHT), fight);
  writeFileSync(dir("crates/terra_walker/src/carry_arm.rs"), "/// How far.\npub const GIVE_BACK_DEG: f32 = 25.0;\npub const GIVE_OUT_DEG: f32 = 25.0;\n");
  writeFileSync(dir("tools/characters/grip.ts"), "export const PALM_M = 0.021;\nexport const FIT_M = 0.0206;\n");
  apply([{ file: FIGHT, key: "Carry_Tool.hold[0].roll.hand_r", value: 11.5 }, { file: FIGHT, key: "Carry_Tool.hold[0].aim.hand_r", value: [0.1, -0.9, 0.2] }, { file: "crates/terra_walker/src/carry_arm.rs", key: "GIVE_BACK_DEG", value: 31 }, { file: "tools/characters/grip.ts", key: "FIT_M", value: 0.0195 }], OUT);
  assert.equal(readFileSync(dir(FIGHT), "utf8"), fight.replace('"hand_r": 8.0', '"hand_r": 11.5').replace("-0.261,\n            -0.965,\n            0.0\n", "0.1,\n            -0.9,\n            0.2\n"));
  assert.equal(readFileSync(dir("crates/terra_walker/src/carry_arm.rs"), "utf8"), "/// How far.\npub const GIVE_BACK_DEG: f32 = 31.0;\npub const GIVE_OUT_DEG: f32 = 25.0;\n");
  assert.equal(readFileSync(dir("tools/characters/grip.ts"), "utf8"), "export const PALM_M = 0.021;\nexport const FIT_M = 0.0195;\n");
  assert.throws(() => apply([{ file: "crates/terra_walker/src/carry_arm.rs", key: "GIVE_LIFT", value: 1 }], OUT), /no constant GIVE_LIFT.*D556.*by hand/);
  rmSync(OUT, { recursive: true, force: true });
});

test("holds.ts's rows measure a point: the give named at its committed value is the committed hold, and less of it changes the rise", async () => {
  mkdirSync(OUT, { recursive: true });
  const spec: Spec = { bodies: ["base"], looks: { stone_axe: ["Crouch_Idle_Loop"] }, measures: ["carry.*"], parameters: [{ name: "give.back", lo: 0, hi: 60, step: 1 }] };
  writeFileSync(join(OUT, "solve.log"), "");
  const measure = holdsMeasure(spec, { log: join(OUT, "solve.log"), out: OUT }, pool(3));
  const start = dimsOf(spec)[0].start;
  const rise = (ms: Measure[]) => ms.find((x) => x.measure === "carry.rise")!;
  const [plain, named, less] = await Promise.all([measure({}), measure({ "give.back": start }), measure({ "give.back": 0 })]);
  assert.equal(rise(plain).bar, 10);
  assert.equal(rise(plain).clip, "Crouch_Idle_Loop");
  assert.deepEqual(named.map((x) => [x.measure, x.value]), plain.map((x) => [x.measure, x.value]));
  assert.ok(rise(less).value > rise(plain).value + 5, `rise ${rise(plain).value} with the give, ${rise(less).value} without`);
  assert.equal(scored(plain, spec).over, 0);
  rmSync(OUT, { recursive: true, force: true });
});
