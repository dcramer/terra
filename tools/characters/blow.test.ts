import assert from "node:assert/strict";
import { test } from "node:test";
import { Glb, len, posed, sub } from "./glb.ts";
import { blow, sweep } from "./blow.ts";
import { keyClip, takeBytes } from "./key.ts";

// A creature's own body (4 m, the armature scaled 2.2), keyed with two empty fists.
const BODY = "assets/models/characters/troll.glb";
const g = new Glb(BODY);
const at = (pose: ReturnType<typeof posed>, n: string) => pose[g.named(n)].t;
const guard = posed(g, g.clip("Troll_Idle"), 0);
// Both fists to the ground ahead, within the arms' reach of the bent body, then back.
const DOWN = 1.0;
const spec = {
  name: "Fists_Down",
  body: BODY,
  guard: "Troll_Idle",
  fists: true,
  fingers: 200,
  end: 2,
  keys: [
    { t: 0, guard: true },
    { t: 0.6, grip: { closed: 1 }, hand_r: { at: [0.2, 3.2, 0.4] }, hand_l: { at: [-0.2, 3.2, 0.4] } },
    { t: DOWN, pelvis: { move: [0, -0.86, 0.8], pitch: 55 }, chest: { pitch: 80 }, hand_r: { at: [0.2, 0.5, 2.0] }, hand_l: { at: [-0.2, 0.5, 2.0] } },
    { t: 1.4 },
    { t: 2, guard: true },
  ],
};
const keyed = keyClip(spec, g);
const take = new Glb(Buffer.from(takeBytes(g, keyed)));
const clip = take.clip("Fists_Down");

test("`fists` starts on the guard and puts each wrist where its key says, on a creature's own body", () => {
  const first = posed(take, clip, 0);
  for (const bone of ["hand_r", "hand_l", "Head", "ball_r", "index_02_r"]) assert.ok(len(sub(at(first, bone), at(guard, bone))) < 1e-3, `${bone} starts where the guard holds it`);
  const down = posed(take, clip, DOWN);
  // Keys are [right, up, forward]; the model's right is -X.
  assert.ok(len(sub(at(down, "hand_r"), [-0.2, 0.5, 2.0])) < 5e-3, `the right wrist at its key, ${at(down, "hand_r")}`);
  assert.ok(len(sub(at(down, "hand_l"), [0.2, 0.5, 2.0])) < 5e-3, `the left wrist at its key, ${at(down, "hand_l")}`);
  // A fist is closed: the fingertip nearer the wrist than in the guard's open hand.
  const tip = (p: ReturnType<typeof posed>) => len(sub(at(p, "middle_04_leaf_r"), at(p, "hand_r")));
  assert.ok(tip(down) < tip(guard) * 0.8, `the fingers close, ${tip(down).toFixed(2)} m from ${tip(guard).toFixed(2)}`);
  for (const side of ["r", "l"]) assert.ok(len(sub(at(down, `ball_${side}`), at(guard, `ball_${side}`))) < 1e-3, "a foot not told to move stays");
});

test("a blow's measured moments are the ones its keys wrote", () => {
  const b = blow(take, "Fists_Down");
  assert.ok(Math.abs(b.top_s - 0.6) < 0.05, `the top at its key, ${b.top_s}`);
  assert.ok(b.lands_s > DOWN - 0.08 && b.lands_s <= DOWN + 0.02, `the landing at its key, ${b.lands_s}`);
  assert.ok(Math.abs(b.lands_up_m - 0.5) < 0.08, `as high as keyed, ${b.lands_up_m}`);
  assert.ok(Math.abs(b.apart_m - 0.4) < 0.02, `as far apart as keyed, ${b.apart_m}`);
  assert.ok(Math.abs(b.lands_ahead_m - 2.0) < 0.08, `as far ahead of the model's origin as keyed, ${b.lands_ahead_m}`);
  assert.ok(b.foot_slide_m < 0.01, `planted feet do not slide, ${b.foot_slide_m}`);
  assert.ok(b.tell_s > 0.6 && b.tell_s < DOWN && b.strike_s > 0, `the strike begins after the top, ${b.tell_s}`);
});

test("a sideways blow's hand, way, crossing and arc are the ones its keys wrote", () => {
  // The right fist from out at its right, across the front at one height, to its left: the wrist
  // is straight ahead of the origin at CROSS, 1.0 m out and 2.2 m up.
  const CROSS = 1.0;
  const across = {
    ...spec,
    name: "Fist_Across",
    keys: [
      { t: 0, guard: true },
      { t: 0.7, grip: { closed: 1 }, chest: { yaw: -30 }, hand_r: { at: [1.3, 2.2, 0.3] } },
      { t: CROSS, chest: { yaw: 10 }, hand_r: { at: [0, 2.2, 1.0] } },
      { t: 1.2, chest: { yaw: 40 }, hand_r: { at: [-0.7, 2.2, 0.7] } },
      { t: 2, guard: true },
    ],
  };
  const made = new Glb(Buffer.from(takeBytes(g, keyClip(across, g))));
  const s = sweep(made, "Fist_Across");
  assert.equal(s.hand, "r");
  assert.equal(s.toward, "left");
  assert.ok(Math.abs(s.cross_s - CROSS) < 0.04, `the crossing at its key, ${s.cross_s}`);
  assert.ok(Math.abs(s.cross_up_m - 2.2) < 0.05, `as high as keyed, ${s.cross_up_m}`);
  assert.ok(Math.abs(s.cross_ahead_m - 1.0) < 0.05, `as far out as keyed, ${s.cross_ahead_m}`);
  assert.ok(s.tell_s >= 0.6 && s.tell_s < CROSS && s.tell_s + s.strike_s > CROSS, `the strike spans the crossing, ${s.tell_s} + ${s.strike_s}`);
  // From 77 degrees to its right to 45 to its left of straight ahead, as keyed.
  assert.ok(s.arc_deg > 90 && s.arc_deg < 135, `the arc ${s.arc_deg}`);
  assert.ok(s.arc_low_m > 2.1 && s.arc_high_m < 2.3, `flat, ${s.arc_low_m} to ${s.arc_high_m}`);
  assert.ok(s.foot_slide_m < 0.01, `planted feet do not slide, ${s.foot_slide_m}`);
  // The smash never crosses sideways at speed: its fists come straight down ahead.
  assert.ok(sweep(take, "Fists_Down").arc_deg < 30);
});
