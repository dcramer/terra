// The board's place against the chest (shield_place.ts, D554): the measure, the chest's frame,
// and the captured targets against the assets they were captured from.

import { rmSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import assert from "node:assert/strict";
import { type Pose, type Q, type V3, qmul, rot, turn } from "./glb.ts";
import type { Held } from "./held.ts";
import { type Place, atCommit, boardPlace, placeOff, readBoard, readTargets, reader, strappedBoard, worstOff } from "./shield_place.ts";

const near = (a: number, b: number, eps = 1e-6) => assert.ok(Math.abs(a - b) < eps, `${a} is not ${b}`);

test("a place is off another by its centre's distance and its board's one turn", () => {
  const a: Place = { centre: [0.4, -0.1, 0.26], face: [1, 0, 0], up: [0, 1, 0] };
  near(placeOff(a, a).cm, 0);
  near(placeOff(a, a).deg, 0, 1e-4);
  near(placeOff(a, { ...a, centre: [0.4, -0.13, 0.26] }).cm, 3);
  // Turned 10 degrees about its own face: the face the same, the up 10 degrees off.
  const q = turn([10, 0, 0]);
  near(placeOff(a, { ...a, face: rot(q, a.face), up: rot(q, a.up) }).deg, 10, 1e-4);
  // Turned about a slanting axis, every axis moves: the one turn, not the worst axis's.
  const s = turn([3, 4, 0]);
  const off = placeOff(a, { ...a, face: rot(s, a.face), up: rot(s, a.up) }).deg;
  assert.ok(off > 4 && off < 7, `${off}`);
});

// Two nodes: the chest (0) and the shield's bone (1), a board 30 cm along the bone.
const shield = { board: { centre: [0, 0.3, 0] as V3 }, front: [0, 0, -1] as V3, up: [-1, 0, 0] as V3 } as unknown as Held;
const pose = (chestR: Q, boneR: Q, shift: V3 = [0, 0, 0]): Pose[] => [
  { t: [0.1, 1.4, 0.0].map((x, i) => x + shift[i]) as V3, r: chestR, s: [1, 1, 1] },
  { t: [0.3, 1.3, 0.1].map((x, i) => x + shift[i]) as V3, r: boneR, s: [1, 1, 1] },
];
const whole = (q: Q, p: Pose[]): Pose[] => p.map((n) => ({ t: rot(q, n.t), r: qmul(q, n.r), s: n.s }));

test("the board's place in the chest's frame is the same however the whole body turns or stands", () => {
  const rest = pose(turn([0, 0, 0]), turn([0, 0, 0]));
  const p = pose(turn([5, 20, 0]), turn([40, -10, 30]));
  const here = boardPlace(p, rest, 0, p[1], shield);
  const w = whole(turn([0, 90, 0]), pose(turn([5, 20, 0]), turn([40, -10, 30]), [2, 0, -3]));
  const turned = boardPlace(w, rest, 0, w[1], shield);
  near(placeOff(here, turned).cm, 0, 1e-6);
  near(placeOff(here, turned).deg, 0, 1e-3);
});

test("the chest's frame does not depend on which way the chest bone's own axes run (D542)", () => {
  // The same body with spine_03's own axes turned (a rebuild that lays bones along their limbs):
  // its rest and posed rotations both carry the turn on the right.
  const k = turn([0, 0, 90]);
  const rest = pose(turn([0, 0, 0]), turn([0, 0, 0]));
  const p = pose(turn([5, 20, 0]), turn([40, -10, 30]));
  const rebuilt = (x: Pose[]): Pose[] => [{ ...x[0], r: qmul(x[0].r, k) }, x[1]];
  const off = placeOff(boardPlace(p, rest, 0, p[1], shield), boardPlace(rebuilt(p), rebuilt(rest), 0, p[1], shield));
  near(off.cm, 0, 1e-6);
  near(off.deg, 0, 1e-3);
});

test("the strapped board hold.py solves with is the shield's own, as it is strapped now", () => {
  const kept = readBoard();
  assert.ok(kept, "shield_targets.json has the strapped board (shield_place.ts --board)");
  const now = strappedBoard({ body: "assets/models/character/base.glb", shield: kept.shield });
  const off = placeOff(kept, now);
  assert.ok(off.cm < 0.01 && off.deg < 0.01, `the kept board is ${off.cm.toFixed(3)} cm, ${off.deg.toFixed(3)}° off ${kept.shield}'s: run shield_place.ts --board and re-bake`);
  // It sits beyond the fist toward the elbow and out of the forearm's back, face out: a heater
  // strapped on a forearm, not a board in the hand.
  assert.ok(Math.hypot(...now.centre) > 0.05 && Math.hypot(...now.centre) < 0.4, `${now.centre}`);
});

// The bar a captured target is held to against the assets it was captured from.
const CAPTURE_CM = 0.5;
const CAPTURE_DEG = 1;

test("each captured target reproduces the board of the commit and template it was captured from", () => {
  const targets = readTargets();
  assert.ok(targets.Low_Guard, "Low_Guard, the developer-approved guard (D531), has a target");
  for (const [pose, target] of Object.entries(targets)) {
    // A target authored from the geometry (Raised_Block) was captured from no commit.
    if (!target.from) continue;
    const files = ["assets/models/character/base.glb", "assets/models/character/fight.glb", "assets/models/character/templates.json", "assets/models/held/heater_shield.glb"];
    const dir = atCommit(target.from, files);
    const r = reader({ body: join(dir, files[0]), clips: join(dir, files[1]), templates: join(dir, files[2]), shield: join(dir, files[3]), only: [target.template] });
    rmSync(dir, { recursive: true });
    const off = worstOff(r, r.clipOf(target.clip ?? pose), target);
    assert.ok(off.cm <= CAPTURE_CM, `${pose}: the board ${off.cm.toFixed(2)} cm off its target (${off.cmWhere}) at ${target.from}`);
    assert.ok(off.deg <= CAPTURE_DEG, `${pose}: the board ${off.deg.toFixed(2)}° off its target (${off.degWhere}) at ${target.from}`);
  }
});
