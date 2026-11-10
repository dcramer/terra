import assert from "node:assert/strict";
import { test } from "node:test";
import { Glb, len, posed, sub } from "./glb.ts";
import { blow } from "./blow.ts";
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
  const hips = at(guard, "pelvis");
  assert.ok(Math.abs(b.lands_ahead_m - (2.0 - hips[2])) < 0.08, `as far ahead of the hips' start as keyed, ${b.lands_ahead_m}`);
  assert.ok(b.foot_slide_m < 0.01, `planted feet do not slide, ${b.foot_slide_m}`);
  assert.ok(b.tell_s > 0.6 && b.tell_s < DOWN && b.strike_s > 0, `the strike begins after the top, ${b.tell_s}`);
});
