// Hold parameters tried for one run (tune.ts): the defaults are the committed numbers, a tuned
// pose is the baked one when nothing is changed, and a changed number changes what is measured.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { GIVE } from "./carry_arm.ts";
import { Glb, type Q, qinv, qmul } from "./glb.ts";
import { FIT_M, PALM_M } from "./grip.ts";
import { aimOf, anglesOf, editsOf, heldPose, paramOf, readSets, setsWith } from "./tune.ts";

const ROOT = join(import.meta.dirname, "..", "..");
const OUT = join(ROOT, "tools", "characters", "out");
const sets = readSets();
const off = (a: Q, b: Q) => (2 * Math.acos(Math.min(1, Math.abs(qmul(qinv(a), b)[3]))) * 180) / Math.PI;

test("a pose laid from the committed aims and rolls is the baked pose, to a hundredth of a degree", () => {
  const body = new Glb(join(ROOT, "assets/models/character/base.glb"));
  const clips = new Glb(join(ROOT, "assets/models/character/fight.glb"));
  const poses = Object.keys(sets).filter((k) => k.startsWith("Carry_") && sets[k].hold?.every((h) => !h.turn && !h.shield));
  assert.ok(poses.includes("Carry_Tool") && poses.includes("Carry_Basket"), `${poses}`);
  let worst = 0;
  for (const name of poses) {
    const baked = clips.clip(name)!;
    for (const [bone, q] of heldPose(body, clips, name, sets[name])) worst = Math.max(worst, off(q, baked.tracks.get(bone)!.r!(0) as Q));
  }
  assert.ok(worst < 0.01, `${worst} degrees off at worst`);
  // The planted fault: a hand rolled 5 degrees more is that far off the baked one.
  const rolled = setsWith({ "pose.Carry_Tool.roll.hand_r": sets.Carry_Tool.hold![0].roll!.hand_r + 5 }).sets;
  const hand = heldPose(body, clips, "Carry_Tool", rolled.Carry_Tool).get("hand_r")!;
  assert.ok(Math.abs(off(hand, clips.clip("Carry_Tool")!.tracks.get("hand_r")!.r!(0) as Q) - 5) < 0.01);
});

test("a parameter's default is the committed number, and an edit names where it lives", () => {
  const rs = readFileSync(join(ROOT, "crates/terra_walker/src/carry_arm.rs"), "utf8");
  assert.equal(paramOf("give.out").start, Number(/GIVE_OUT_DEG: f32 = ([0-9.]+)/.exec(rs)![1]));
  assert.equal(paramOf("give.out").start, GIVE.out);
  assert.deepEqual([paramOf("seat.fit").start, paramOf("seat.palm").start], [FIT_M, PALM_M]);
  const aim = sets.Carry_Basket.hold![1].aim!.upperarm_l;
  assert.deepEqual(paramOf("pose.Carry_Basket.aim.upperarm_l.up").start, anglesOf(aim).up);
  assert.equal(paramOf("pose.Carry_Basket.roll.hand_l").start, sets.Carry_Basket.hold![1].roll!.hand_l);
  assert.equal(paramOf("pose.Carry_Basket.swing").start, sets.Carry_Basket.swing);
  // An aim's angles give its vector back as the file holds it.
  const { up, round } = anglesOf(aim);
  assert.deepEqual(aimOf(up, round).map((x, i) => Math.abs(x - aim[i]) < 0.0011), [true, true, true]);
  const tune = { "pose.Carry_Basket.aim.upperarm_l.up": -60, "give.out": 30, "seat.fit": 0.02 };
  assert.deepEqual(editsOf(tune), [
    { file: "art/animation/sets/fight.json", key: "Carry_Basket.hold[1].aim.upperarm_l", value: aimOf(-60, round) },
    { file: "crates/terra_walker/src/carry_arm.rs", key: "GIVE_OUT_DEG", value: 30 },
    { file: "crates/terra_walker/src/grip.rs", key: "FIT_M", value: 0.02 },
    { file: "tools/characters/grip.ts", key: "FIT_M", value: 0.02 },
  ]);
  // Only what is tuned changes.
  const laid = setsWith(tune).sets;
  laid.Carry_Basket.hold![1].aim!.upperarm_l = aim;
  assert.deepEqual(laid, sets);
});

test("an unknown parameter is an error that says what there is (D556)", () => {
  assert.throws(() => paramOf("give.elbows"), /no parameter give\.elbows: .*Name one of: give\.elbow, give\.back, give\.out/);
  assert.throws(() => paramOf("pose.Carry_Baskets.roll.hand_l"), /no clip Carry_Baskets with a hold.*pose\.Carry_Basket/);
  assert.throws(() => paramOf("pose.Carry_Basket.roll.thumb_01_l"), /give no roll for thumb_01_l.*pose\.Carry_Basket\.roll\.hand_l/);
  assert.throws(() => paramOf("pose.Carry_Basket.aim.hand_l"), /an aim has \.up and \.round/);
  assert.throws(() => paramOf("grip.fit"), /seat\.fit/);
});

test("moves.ts --tune: the committed numbers named change nothing, another number does", () => {
  mkdirSync(OUT, { recursive: true });
  const run = (tag: string, tune?: Record<string, number>) => {
    const json = join(OUT, `tune-test-${tag}.json`);
    const args = ["--set", "none", "--carry", "--arms", "--hold", "socket.hand.R=assets/models/held/stone_axe.glb", "--also", "Crouch_Idle_Loop", "--template", "female", "--json", json];
    execFileSync(process.execPath, [join(ROOT, "tools/characters/moves.ts"), ...args, ...(tune ? ["--tune", JSON.stringify(tune)] : [])], { cwd: ROOT, stdio: "ignore" });
    return readFileSync(json, "utf8");
  };
  const plain = run("plain");
  const same = Object.fromEntries(["give.elbow", "give.back", "give.out", "give.from", "give.to", "give.rate", "seat.fit", "seat.palm"].map((n) => [n, paramOf(n).start]));
  assert.equal(run("same", same), plain);
  const rise = (text: string) => (JSON.parse(text) as { clip: string; measured: Record<string, number> }[]).find((r) => r.clip === "fist")!.measured["carried_rise_deg.stone_axe"];
  // With no shoulder turn back the elbow's bend tips the axe up (held-carry j5: 28.3 degrees).
  assert.ok(rise(run("back", { "give.back": 0 })) > rise(plain) + 15, `${rise(plain)}`);
  // A tuned pose at its committed numbers measures as the baked one, to a tenth of a degree.
  const posed = rise(run("pose", { "pose.Carry_Tool.roll.hand_r": sets.Carry_Tool.hold![0].roll!.hand_r }));
  assert.ok(Math.abs(posed - rise(plain)) < 0.1, `${posed} against ${rise(plain)}`);
});
