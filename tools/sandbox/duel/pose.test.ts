// The composed-pose checks (pose.ts) on made-up poses whose answers are known by construction:
// the base body at rest with its head turned a known way, a board hung a known way off a known
// forearm. node --test tools/sandbox/duel/pose.test.ts
import assert from "node:assert/strict";
import { resolve } from "node:path";
import { test } from "node:test";
import type { Board } from "../../characters/clash.ts";
import { Glb, type Q, posed, qmul } from "../../characters/glb.ts";
import { ARMS_CM, type Eight, type PoseFrame, type Posed, armsIn, headOff, intoModel, onForearm, poseChecks, shieldHigh } from "./pose.ts";

const body = new Glb(resolve(import.meta.dirname, "../../../assets/models/character/base.glb"));
const atRest = posed(body, undefined, 0);
/// A turn about the up (+Y), degrees.
const aboutUp = (deg: number): Q => {
  const h = (deg * Math.PI) / 360;
  return [0, Math.sin(h), 0, Math.cos(h)];
};
const e = (t: number[], r: Q = [0, 0, 0, 1], s = 1): Eight => [...t, ...r, s];

/// The base body at rest, its root turned `facing` degrees, its head `head` degrees about the up.
function restPose(head = 0, facing = 0): Posed {
  const bones: Record<string, Eight> = {};
  body.gltf.nodes.forEach((n, i) => {
    if (!n.name) return;
    const p = atRest[i];
    const r = n.name === "Head" ? qmul(aboutUp(head), p.r) : p.r;
    bones[n.name] = e(p.t, r, p.s[0]);
  });
  return { root: e([0, 0, 0], aboutUp(facing)), bones, held: [] };
}

/// A round board 0.3 m across its face, 2 cm thick, centred on its origin, its face +Z.
const BOARD: Board = { centre: [0, 0, 0], normal: [0, 0, 1], radius: 0.3, half: 0.01 };
/// The forearm along +Z at chest height, the fist 5 cm past the wrist, the hips at 1 m; the
/// board hung `at`, turned by `r`.
function strapped(at: number[], r: Q): Posed {
  const pose = restPose();
  Object.assign(pose.bones, {
    pelvis: e([0, 1, 0]),
    lowerarm_l: e([0.2, 1.2, 0]),
    hand_l: e([0.2, 1.2, 0.25]),
    "socket.hand.L": e([0.2, 1.2, 0.3]),
    index_01_l: e([0.2, 1.24, 0.3]),
    pinky_01_l: e([0.2, 1.16, 0.3]),
  });
  pose.held = [{ look: "board", on: "lowerarm_l", strapped: true, at: e(at, r) }];
  return pose;
}
const near = (got: number | undefined, want: number, within = 0.05) => assert.ok(got !== undefined && Math.abs(got - want) <= within, `${got} not ${want}`);

test("a head turned 45 degrees left is 45 degrees off an aim straight ahead, and off the hips", () => {
  const off = headOff(restPose(45), [0, 0, 1]);
  near(off?.aim, 45);
  near(off?.hips, 45);
  near(headOff(restPose(-30), [0, 0, 1])?.aim, -30);
  near(headOff(restPose(0), [0, 0, 1])?.aim, 0);
});

test("the aim is read in the body's own frame, however the body faces", () => {
  // Facing 90 degrees about the up, its aim along its own ahead: the head straight on it.
  const pose = restPose(0, 90);
  const ahead = [1, 0, 0];
  near(headOff(pose, intoModel(pose, ahead))?.aim, 0);
  near(headOff(pose, intoModel(pose, [0, 0, 1]))?.aim, 90);
});

test("a board lying along the forearm is square to it, a forearm's half and a strap off, its strap where it is hung", () => {
  // Its face turned to +X (a quarter turn about the up): the forearm, along +Z, in its plane.
  const fit = onForearm(strapped([0.25, 1.2, 0.12], aboutUp(90)), BOARD);
  near(fit?.deg, 0, 0.01);
  // The forearm's middle 5 cm off the board's middle plane, 1 cm of it the board.
  near(fit?.off, 4, 0.01);
  // The hand strap (the origin) 18 cm short of the fist and 5 cm out: sqrt(5² + 18²) cm.
  near(fit?.strap, Math.hypot(5, 18), 0.01);
});

test("a board tipped off the forearm measures the tip", () => {
  near(onForearm(strapped([0.25, 1.2, 0.12], aboutUp(70)), BOARD)?.deg, 20, 0.01);
});

test("a board's middle is measured over the hips", () => {
  near(shieldHigh(strapped([0.25, 1.35, 0.12], aboutUp(90)), BOARD)?.cm, 35, 0.01);
  near(shieldHigh(strapped([0.25, 0.95, 0.12], aboutUp(90)), BOARD)?.cm, -5, 0.01);
});

test("the checks read the player's blows, and leave the guard standing and the block out of the head's", () => {
  const frame = (n: number, move: string | null, head: number, guard = false) =>
    ({
      frame: n,
      step: null,
      asked: [],
      contacts: [],
      fighters: [{ who: "you", feet: [0, 0, 0], facing: [0, 0, 1], move, guard, pose: restPose(head) }],
    }) as unknown as PoseFrame;
  const frames = [frame(1, null, 5), frame(2, "slash", 40), frame(3, "slash", 44), frame(4, null, 3, true)];
  const head = poseChecks(frames, false).find((c) => c.name === "head yaw in blows");
  near(head?.value, 44, 0.1);
  assert.equal(head?.faults, 2);
});

test("arms held out from the body are clear of it, a hand laid inside the chest is in it", () => {
  // The base body at rest holds its arms out (the A pose): every arm point outside the skin.
  assert.ok(armsIn(restPose()).cm < 0, `${armsIn(restPose()).cm}`);
  // The right hand and its fingers moved to the chest's middle (spine_03's place, inside the
  // torso): deeper than the bar, in the torso.
  const pose = restPose();
  const chest = pose.bones.spine_03.slice(0, 3);
  for (const bone of ["hand_r", "middle_01_r", "middle_02_r", "index_02_r", "thumb_02_r"]) pose.bones[bone] = e(chest);
  const inside = armsIn(pose);
  assert.ok(inside.cm > ARMS_CM, `${inside.cm}`);
  assert.match(inside.where, /^hand_r in spine/);
});
