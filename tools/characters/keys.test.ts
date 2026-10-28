import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { readSpec, type Key, type V3 } from "./clip_spec.ts";
import { angleStep, between, cone, FIRST_KEY_S, GUARD_TWIST_DEG, guardNpz, judge, reachAll, type Bar, type GuardHand, type Reach } from "./keys.ts";
import { KIMODO } from "./motion.ts";

const BAR: Bar = { flex: 70, ext: 60, radial: 10, ulnar: 25, twist: [-15, 180] };
const wrist = { flex: 0, dev: 0, twist: 8, swing: 0, past: 0, elbow: [0, 0, -1] as V3 };
const GUARD: GuardHand = { grip: [0.18, 0.36, 0.05], blade: [0, 0, 1], edge: [0, 1, 0], reach: { ...wrist, side: "Right", at: 0, grip: [0, 0, 0], blade: [0, 0, 1], short: 0 }, bones: wrist };
const at = (t: number, over: Partial<Reach> = {}): Reach => ({ ...GUARD.reach, at: t, twist: 20, ...over });

test("a key past the bar is refused and the limit it is past named", () => {
  const [j] = judge(GUARD, [at(0.5, { flex: 106, dev: -60, past: 36 })], BAR);
  assert.equal(j.flags.length, 1);
  assert.match(j.flags[0], /flexed 106 \(bar 70\), ulnar 60 \(bar 25\)/);
});

test("a key sooner than the first key's bar is refused; one on it is not", () => {
  const judged = judge(GUARD, [at(FIRST_KEY_S - 0.05), at(FIRST_KEY_S + 0.2)], BAR);
  assert.match(judged[0].flags.join(), /sooner than/);
  assert.deepEqual(judge(GUARD, [at(FIRST_KEY_S)], BAR)[0].flags, []);
});

test("the last key twisted far from the guard's is cautioned, not refused; an earlier one is not", () => {
  const far = GUARD.bones.twist + GUARD_TWIST_DEG + 3;
  const judged = judge(GUARD, [at(0.5, { twist: far }), at(1.4, { twist: far })], BAR);
  assert.deepEqual(judged[0].cautions, []);
  assert.equal(judged[1].cautions.length, 1);
  assert.deepEqual(judged[1].flags, []);
});

test("between meets its ends and keeps the edge square to the blade", () => {
  const a: Key = { at: 1, grip: [0, 0, 0], blade: [0, 0, 1], edge: [0, 1, 0], yaw: -8 };
  const b: Key = { at: 2, grip: [1, 0, 0], blade: [1, 0, 0], edge: [0, 0, -1], yaw: 0 };
  assert.deepEqual(between(a, b, 0).grip, a.grip);
  assert.ok(Math.abs(between(a, b, 1).blade![0] - 1) < 1e-9);
  const mid = between(a, b, 0.5);
  assert.equal(mid.at, 1.5);
  assert.equal(mid.yaw, -4);
  const d = mid.blade!.reduce((s, x, i) => s + x * mid.edge![i], 0);
  assert.ok(Math.abs(d) < 1e-9);
});

test("a cone's directions lie within its angle of its axis, and an angle's step wraps", () => {
  const dirs = cone([0, 0, 1], 30, 10);
  assert.ok(dirs.length > 30);
  for (const v of dirs) assert.ok(v[2] >= Math.cos((30.01 * Math.PI) / 180));
  assert.equal(angleStep(-156, 165), -39);
  assert.equal(angleStep(170, -170), 20);
});

// The committed blows' keys, solved on Kimodo's skeleton where Kimodo is set up (D565 step 3).
const LIST = join(import.meta.dirname, "..", "..", "art", "animation", "sets", "fight.json");
const ready = existsSync(join(KIMODO, "venv", "bin", "python")) && existsSync(LIST.replace(/\.json$/, ""));

test("the fight's three blows check clean, and a reversed edge and a key too soon after the guard are refused", { skip: !ready && "no Kimodo" }, () => {
  const list = JSON.parse(readFileSync(LIST, "utf8"));
  const specs = ["Keyed_Slash", "Keyed_Backhand", "Keyed_Heavy"].map((c) => readSpec(c, list[c]));
  assert.ok(specs.every((s) => s.guard.clip === specs[0].guard.clip && s.guard.frame === specs[0].guard.frame));
  const npz = guardNpz(LIST, specs[0].guard.clip, (command, args) => assert.equal(spawnSync(command, args).status, 0));
  const backhand = specs[1].hand;
  // Job 8's 0.63 key with its edge turned to lead the other way (the flat backhand's fault), and
  // job 7's 0.2 s key (6 of 8 takes snapped past the wrist at 0.08-0.21 s).
  const reversed: Key = { ...backhand[2], edge: backhand[2].edge!.map((x) => -x) as V3 };
  const soon: Key = { at: 0.2, grip: [0.05, 0.38, 0.25], blade: [-0.6, 0.34, 0.72], edge: [-0.32, 0.72, -0.61] };
  const all = [...specs.flatMap((s) => s.hand), reversed, soon];
  const got = reachAll(npz, specs[0].guard.frame, all.map((k) => ({ ...k, side: "Right" as const })));
  const guard = got.guard.Right;
  assert.ok(guard.bones.past > 0, "the guard take's own wrist reads past the bar (radial 24 on its bones): the tool says so first");
  let from = 0;
  for (const s of specs) {
    const judged = judge(guard, got.keys.slice(from, (from += s.hand.length)), got.bar);
    assert.deepEqual(judged.map((j) => [...j.flags, ...j.cautions]), s.hand.map(() => []), s.name);
  }
  const [bad, early] = got.keys.slice(from);
  const kept = got.keys.slice(specs[0].hand.length, specs[0].hand.length + backhand.length);
  assert.ok(judge(guard, kept.map((k, j) => (j === 2 ? bad : k)), got.bar)[2].flags.some((f) => /past the bar/.test(f)));
  assert.ok(judge(guard, [early, ...kept], got.bar)[0].flags.some((f) => /sooner than/.test(f)));
});
