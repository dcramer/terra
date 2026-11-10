import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { type Look, ROOT, readLook } from "./attach_contract.ts";
import { Glb, type Pose, type Q, type V3, posed, qmul, rot } from "./glb.ts";
import { holdPose, stancePose, wristRest } from "./hold_pose.ts";
import { ARM_OUT_DEG, type Measure, RISE_DEG, STANDING, type Row, fistOf, fixFor, judged, lineFor, shapeOf } from "./holds.ts";

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

// The axe level in a crouch and the fingers off a thin haft (held-carry j5, the developer's
// still of 2026-10-28): moves.ts's row "fist" as it writes it today, the committed stone axe
// through the crouch on one body, passes both and says where; the same row with the numbers
// main had on 2026-11-09 planted (the give's elbow bent 40 degrees with no turn back at the
// shoulder tipped every tool 28.3 degrees above level in Crouch_Idle_Loop; the iron axe's
// 1.75 cm haft left the ring finger's knuckle bone 1.06 cm off it) fails each by name.
/** moves.ts's row "fist" for a committed look carried in `hand` through the crouch on one body. */
function fistRow(look: string, hand = "R"): Row {
  const dir = mkdtempSync(join(tmpdir(), "holds-fist-"));
  const json = join(dir, "rows.json");
  const args = ["--set", "none", "--also", "Crouch_Idle_Loop", "--template", "male", "--hold", `socket.hand.${hand}=assets/models/held/${look}.glb`, "--carry", "--arms", "--json", json];
  const run = spawnSync(process.execPath, [join(ROOT, "tools", "characters", "moves.ts"), ...args], { cwd: ROOT, encoding: "utf8" });
  const rows = JSON.parse(readFileSync(json, "utf8")) as Row[];
  rmSync(dir, { recursive: true, force: true });
  const row = rows.find((r) => r.clip === "fist");
  assert.ok(row, run.stderr);
  return row;
}

// A pose that stands up is not a tool tipped up (held-report j2): on main of 2026-11-09 the
// torch (70.6 degrees), the spear (89.8) and the bow (41) each failed carry.rise, the bar of a
// tool carried head down, though their carry poses are marked upright. moves.ts writes such a
// look no rise but how far below level its working end falls, which the report reads with no
// bar; the same row with the rise main wrote planted fails as it did, and a hanging tool's row
// carries a rise and no fall.
test("an upright pose is not judged on rising and reads how far it falls; a hanging one is", () => {
  const row = fistRow("torch");
  assert.equal(row.measured["carried_rise_deg.torch"], undefined, "moves.ts wrote an upright look a rise");
  const of = (r: Row) => judged("torch", "male", [r], ["Crouch_Idle_Loop"]).filter((m) => m.measure.startsWith("carry."));
  const real = of(row);
  assert.deepEqual(real.map((m) => [m.measure, m.ok, m.bar, m.clip, m.unit]), [["carry.fall", null, null, "Crouch_Idle_Loop", "deg"]]);
  // The flame stays well above level through the crouch (its pose stands it up).
  assert.ok(real[0].value < -30 && real[0].value > -90, `${real[0].value} degrees`);
  const { ok, line } = lineFor("torch", real, "Carry_Torch");
  assert.equal(ok, true);
  assert.match(line, /^✓ torch \(Carry_Torch\): carry\.fall -[\d.]+ deg; 1 bodies$/);
  const planted: Row = { ...row, measured: { ...row.measured, "carried_rise_deg.torch": 70.6 } };
  assert.deepEqual(failing(of(planted)), ["carry.rise"]);
  // The reading is a reading whatever it says (no bar is proven, D632): a flame under level
  // does not fail, and shows.
  const fallen: Row = { ...row, measured: { ...row.measured, "carried_fall_deg.torch": 12 } };
  assert.deepEqual(failing(of(fallen)), []);
  assert.match(lineFor("torch", of(fallen)).line, /carry\.fall 12 deg/);
});

test("a carried look tipped above level and fingers off the handle fail from moves.ts's fist row", () => {
  const row = fistRow("stone_axe");
  assert.equal(row.measured["carried_fall_deg.stone_axe"], undefined, "moves.ts wrote a hanging look a fall");
  const of = (rows: Row[]) => judged("stone_axe", "male", rows, ["Crouch_Idle_Loop"]).filter((m) => m.measure === "carry.rise" || m.measure === "grip.fist_off");
  const real = of([row]);
  assert.deepEqual(real.map((m) => [m.measure, m.ok, m.clip, m.unit]), [["carry.rise", true, "Crouch_Idle_Loop", "deg"], ["grip.fist_off", true, "Crouch_Idle_Loop", "cm"]]);
  assert.ok(real.every((m) => m.time !== null && /, male$/.test(m.where)), JSON.stringify(real));
  // The crouch does tip it (so the measure sees the give), by less than the bar.
  const standing = -20;
  assert.ok(real[0].value > standing + 5 && real[0].value <= RISE_DEG, `${real[0].value} degrees`);
  const planted: Row = { clip: "fist", measured: { ...row.measured, "carried_rise_deg.stone_axe": 28.3, "fist_cm.stone_axe": 1.06 }, where: row.where };
  const ms = of([planted]);
  assert.deepEqual(failing(ms), ["carry.rise", "grip.fist_off"]);
  const { ok, line, failed } = lineFor("stone_axe", ms, "Carry_Tool");
  assert.equal(ok, false);
  assert.match(line, /^✗ stone_axe \(Carry_Tool\): carry\.rise 28\.3 deg \(bar 10, male, Crouch_Idle_Loop [\d.]+ s, 1 rows on 1 bodies\); grip\.fist_off 1\.1 cm \(bar 1, male, Crouch_Idle_Loop [\d.]+ s, 1 rows on 1 bodies\)$/);
  assert.match(fixFor(failed[0]), /carry_arm/);
  assert.match(fixFor(failed[1]), /grip::seat/);
  // A set's move named "fist" is not the fist row, and a look with no number there has none.
  assert.deepEqual(of([{ ...planted, move: "fist" }]), []);
  assert.deepEqual(judged("other", "male", [planted], []).length, 0);
});

// The stance (held-stance j1; the developers' still of 2026-11-09, the player idle with the
// hammer: "our stance with the hammer is still stupid", "sorry with all equipment"): the arm that
// carried a tool stood 26 to 28.5 degrees out from the trunk, 13 to 19 cm of air between forearm
// and body, and no number said so. The planted fault is that pose itself: main's Carry_Tool arm
// aims of that day, laid by --tune on the committed body, fail stand.arm_out and stand.arm_gap;
// the committed pose passes, and its free arm (the idle's own) passes either way.
test("the arm of a carried tool held out from the side fails the stance; the committed pose stands with it close", () => {
  const dir = mkdtempSync(join(tmpdir(), "holds-stand-"));
  const stand = (tune?: Record<string, number>) => {
    const json = join(dir, "moves.json");
    const args = ["--set", "none", "--also", STANDING, "--template", "male,female", "--hold", "socket.hand.R=assets/models/held/hammer.glb", "--carry", "--arms", "--json", json, ...(tune ? ["--tune", JSON.stringify(tune)] : [])];
    const run = spawnSync(process.execPath, [join(ROOT, "tools", "characters", "moves.ts"), ...args], { cwd: ROOT, encoding: "utf8" });
    const rows = (JSON.parse(readFileSync(json, "utf8")) as Row[]).filter((r) => r.clip === STANDING);
    assert.equal(rows.length, 1, run.stderr);
    return judged("hammer", "male,female", rows, [STANDING]).filter((m) => m.measure.startsWith("stand."));
  };
  const now = stand();
  assert.deepEqual(now.map((m) => m.measure).sort(), ["stand.arm_bend", "stand.arm_gap", "stand.arm_out", "stand.free_bend", "stand.free_gap", "stand.free_out"]);
  assert.deepEqual(failing(now), []);
  const was = stand({ "pose.Carry_Tool.aim.upperarm_r.up": -64.28, "pose.Carry_Tool.aim.upperarm_r.round": -100.64, "pose.Carry_Tool.aim.lowerarm_r.up": -60.83, "pose.Carry_Tool.aim.lowerarm_r.round": -36.92 });
  rmSync(dir, { recursive: true, force: true });
  assert.deepEqual(failing(was).sort(), ["stand.arm_gap", "stand.arm_out"]);
  const out = was.find((m) => m.measure === "stand.arm_out")!;
  assert.ok(out.value > 25 && out.value < 31 && out.bar === ARM_OUT_DEG && out.time !== null, JSON.stringify(out));
  const { ok, line, failed } = lineFor("hammer", was, "Carry_Tool");
  assert.equal(ok, false);
  assert.match(line, /stand\.arm_gap [\d.]+ cm \(bar 10, male,female, Idle_Loop/);
  assert.match(fixFor(failed[0]), /never raise ARM_OUT_DEG/);
  // An upright pose's own arm (a torch held up) is read, not barred; a free arm always is.
  const far: Row = { clip: STANDING, measured: { "stand_arm_out_deg.torch": 42, "stand_free_out_deg.torch": 42 } };
  assert.deepEqual(failing(judged("torch", "male", [far], [STANDING], false)), ["stand.free_out"]);
  assert.deepEqual(failing(judged("torch", "male", [far], [STANDING], true)), ["stand.arm_out", "stand.free_out"]);
});

// The stance's angles are known another way: an arm built 30 degrees out of a leaning trunk
// with its forearm bent 20 forward reads 30 out and 20 bent against that trunk.
test("an arm's stance is read against the trunk's own line, whichever way the body leans", () => {
  const names = ["thigh_l", "thigh_r", "upperarm_l", "upperarm_r", "lowerarm_l", "lowerarm_r", "hand_l", "hand_r", "middle_01_l", "middle_01_r"];
  const at = (n: string) => names.indexOf(n);
  const lean: Q = [Math.sin(0.2), 0, 0, Math.cos(0.2)];
  const d = (x: number) => (x * Math.PI) / 180;
  const along: V3 = [-Math.sin(d(30)), -Math.cos(d(30)), 0];
  const P: Record<string, V3> = { thigh_l: [0.1, 0, 0], thigh_r: [-0.1, 0, 0], upperarm_l: [0.2, 0.5, 0], upperarm_r: [-0.2, 0.5, 0], lowerarm_l: [0.2, 0.2, 0], hand_l: [0.2, -0.05, 0], middle_01_l: [0.2, -0.13, 0] };
  P.lowerarm_r = [-0.2 + along[0] * 0.3, 0.5 + along[1] * 0.3, 0];
  P.hand_r = [P.lowerarm_r[0] + along[0] * 0.25 * Math.cos(d(20)), P.lowerarm_r[1] + along[1] * 0.25 * Math.cos(d(20)), 0.25 * Math.sin(d(20))];
  P.middle_01_r = P.hand_r;
  const pose: Pose[] = names.map((n) => ({ t: rot(lean, P[n]), r: [0, 0, 0, 1], s: [1, 1, 1] }));
  const r = stancePose(pose, at, "r", () => -0.2);
  const l = stancePose(pose, at, "l", () => -0.2);
  const near = (a: number, b: number) => assert.ok(Math.abs(a - b) < 1e-6, `${a} is not ${b}`);
  near(r.out_deg, 30), near(r.fwd_deg, 0), near(r.bend_deg, 20), near(r.gap_cm, 16.5);
  near(l.out_deg, 0), near(l.fwd_deg, 0), near(l.bend_deg, 0);
});
