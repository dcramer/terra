import assert from "node:assert/strict";
import { join } from "node:path";
import { test } from "node:test";
import { type Look, ROOT, readLook } from "./attach_contract.ts";
import { Glb, type Pose, type Q, type V3, posed, qmul, rot } from "./glb.ts";
import { holdPose, wristRest } from "./hold_pose.ts";
import { type Measure, type Row, fistOf, fixFor, judged, lineFor, shapeOf } from "./holds.ts";

// The planted faults (D632): each bar of the hold report is shown to catch what it is for, on
// the committed stone axe changed one way at a time; the axe as committed passes them all.
const body = new Glb(join(ROOT, "assets", "models", "character", "base.glb"));
const fist = fistOf(body);
const axe = readLook(join(ROOT, "assets", "models", "held", "stone_axe.glb"), "held/stone_axe");
const changed = (f: (p: V3) => V3): Look => ({ ...axe, points: axe.points.map(f) });
const failing = (ms: Measure[]) => ms.filter((m) => m.ok === false).map((m) => m.measure);
const shape = (look: Look) => shapeOf(look, "hafted", "in hand", fist);

test("the committed axe's shape passes, and each planted fault of shape fails naming its measure", () => {
  assert.deepEqual(failing(shape(axe)), []);
  // Three times life size (the axe is a 0.42 m hatchet, D653: twice is still under a hafted's
  // metre): its length, and its handle's radius with it; its head reads as many fists.
  const big = shape(changed((p) => [p[0] * 3, p[1] * 3, p[2] * 3]));
  assert.ok(failing(big).includes("size.length"), JSON.stringify(failing(big)));
  // An oversized head alone (the far quarter three times as wide) is read, not barred: no source
  // gives a bar for a head in fists.
  const top = Math.max(...axe.points.map((p) => p[1]));
  const head = (ms: Measure[]) => ms.find((m) => m.measure === "size.head_fists")!;
  const wide = shape(changed((p) => (p[1] > top * 0.75 ? [p[0], p[1], p[2] * 3] : p)));
  assert.ok(head(wide).value > 2.5 * head(shape(axe)).value);
  assert.equal(head(wide).ok, null);
  // The handle standing 20 cm further out of the fist past the little finger.
  const out = shape(changed((p) => (p[1] < 0 ? [p[0], p[1] - 0.2, p[2]] : p)));
  assert.deepEqual(failing(out), ["grip.butt"]);
  assert.ok(out.find((m) => m.measure === "grip.past_little")!.value > 20);
  // A handle three times as thick, and a grip 3 cm off the handle's line.
  assert.ok(failing(shape(changed((p) => [p[0] * 3, p[1], p[2] * 3]))).includes("grip.radius"));
  assert.ok(failing(shape(changed((p) => [p[0] + 0.03, p[1], p[2]]))).includes("grip.offset"));
});

test("a wrist past its limit and a look under the ground fail as carried, naming the measure, body, clip and time", () => {
  const at = (n: string) => body.named(n);
  const rest = posed(body, undefined, 0);
  const wrist = wristRest(rest, at, "r");
  const socket = rest[at("socket.hand.R")];
  const measure = (pose: Pose[], points: V3[]): Row[] => {
    const m = holdPose(pose, at, "r", wrist, socket, points);
    const keys = Object.entries(m).map(([k, v]) => [`hold_${k}.axe`, v] as const);
    return [{ clip: "Idle_Loop", measured: Object.fromEntries(keys), where: Object.fromEntries(keys.map(([k]) => [k, "Idle_Loop at 0.40 s, male"])) }];
  };
  const run = (pose: Pose[], points: V3[]) => judged("axe", "male", measure(pose, points), ["Idle_Loop"]);
  // At rest, holding nothing longer than the fist: every bar holds, the bend reads nothing.
  assert.deepEqual(failing(run(rest, [[0, 0, 0]])), []);
  // The hand turned 50° about its palm's line (toward the little finger or the thumb): past
  // either deviation's limit (35° and 20°), and the measure reads the 50°.
  const palm = rot(rest[at("hand_r")].r, wrist.axes.palm);
  const s = Math.sin((50 * Math.PI) / 360);
  for (const sign of [1, -1]) {
    const q: Q = [palm[0] * s * sign, palm[1] * s * sign, palm[2] * s * sign, Math.cos((50 * Math.PI) / 360)];
    const bent = rest.map((p, i) => (i === at("hand_r") ? { ...p, r: qmul(q, p.r) } : p));
    const bad = run(bent, [[0, 0, 0]]).filter((m) => m.ok === false);
    assert.equal(bad.length, 1);
    assert.match(bad[0].measure, /^carry\.wrist_(ulnar|radial)$/);
    assert.ok(Math.abs(bad[0].value - 50) < 1, `${bad[0].value}`);
    assert.deepEqual([bad[0].body, bad[0].clip, bad[0].time], ["male", "Idle_Loop", 0.4]);
  }
  // A look reaching 1.5 m down its handle's line from a hand at the thigh goes under the feet.
  const sunk = run(rest, [[0, 0, 0], ...[-1.5, 1.5].map((y): V3 => [0, y, 0]), [1.5, 0, 0], [0, 0, 1.5], [-1.5, 0, 0], [0, 0, -1.5]]);
  assert.deepEqual(failing(sunk), ["carry.ground"]);
  // The same in a clip the game uses the look in is read, not barred (a pickaxe strikes the ground).
  assert.deepEqual(failing(judged("axe", "male", measure(rest, [[0, -1.5, 0], [0, 1.5, 0], [1.5, 0, 0], [0, 0, 1.5], [-1.5, 0, 0], [0, 0, -1.5]]), [])), []);
});

test("a look in the shin or sunk in the thigh fails from moves.ts's numbers, the line naming its point and part", () => {
  // moves.ts's own measure is planted in moves.test.ts (an axe moved into the shin); here its
  // rows are judged: 1 cm is the bar, the calf the legs', the thigh the trunk's.
  const rows: Row[] = [
    { clip: "Walk_Loop", measured: { "held_cm.axe": 4.2, "held_legs_cm.axe": 0.4 }, where: { "held_cm.axe": "its point (-2, 9, 0) cm in thigh_r in Walk_Loop at 0.53 s, heavy" } },
    { clip: "Jump_Land", measured: { "held_cm.axe": -3, "held_legs_cm.axe": 2.5 }, where: { "held_legs_cm.axe": "its point (-2, 48, 8) cm in calf_r in Jump_Land at 0.07 s, heavy" } },
    { move: "slash", clip: "Open_Slash", measured: { "held_cm.axe": 1.4, "held_cm.other": 9 } },
  ];
  const ms = judged("axe", "heavy", rows, ["Walk_Loop", "Jump_Land"]);
  assert.deepEqual(failing(ms), ["carry.trunk", "carry.legs", "use.trunk"]);
  const { ok, line, failed } = lineFor("axe", ms, "Carry_Tool");
  assert.equal(ok, false);
  assert.equal(
    line,
    "✗ axe (Carry_Tool): carry.legs 2.5 cm (bar 1, heavy, Jump_Land 0.07 s, its point (-2, 48, 8) in calf_r, 1 rows on 1 bodies); " +
      "carry.trunk 4.2 cm (bar 1, heavy, Walk_Loop 0.53 s, its point (-2, 9, 0) in thigh_r, 1 rows on 1 bodies); use.trunk 1.4 cm (bar 1, heavy, slash, 1 rows on 1 bodies)",
  );
  for (const m of failed) assert.match(fixFor(m), /fit\.ts ITEM/);
  // With every row clear it is one line that passes.
  const clear = lineFor("axe", judged("axe", "heavy", [{ clip: "Walk_Loop", measured: { "held_cm.axe": -6.7 } }], ["Walk_Loop"]));
  assert.deepEqual([clear.ok, clear.line], [true, "✓ axe: carry.trunk -6.7 cm; 1 bodies"]);
});

// The ground is where the game stands the body, the model's y = 0 (held-carry j3): planted,
// both feet 10 cm up in a stride's flight leave it there, and a foot 3 cm under takes it down.
test("the ground under a pose is the model's zero, lower only by a foot under it, never higher", async () => {
  const { groundUnder } = await import("./hold_pose.ts");
  const at = (...ys: number[]) => ys.map((y) => ({ t: [0, y, 0], r: [0, 0, 0, 1], s: [1, 1, 1] }) as Pose);
  const feet: [number, number][] = [[0, 0.1], [1, 0.1]];
  assert.equal(groundUnder(at(0.1, 0.1), feet), 0);
  assert.equal(groundUnder(at(0.2, 0.2), feet), 0, "both feet in the air raised the ground");
  assert.ok(Math.abs(groundUnder(at(0.07, 0.3), feet) + 0.03) < 1e-9);
});
