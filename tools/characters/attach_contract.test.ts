// The attachment contract (docs/attachment.md, D546) on the committed looks and the base body,
// and on copies of them broken on purpose, so each rule is seen to catch what it is for.
// node --test tools/characters/attach_contract.test.ts

import assert from "node:assert/strict";
import { test } from "node:test";
import { join } from "node:path";
import { type V3 } from "./glb.ts";
import { type Look, ROOT, against, audit, judge, judgeSockets, knownFaults, readBody, readLook } from "./attach_contract.ts";

const body = readBody();
const look = (name: string) => readLook(join(ROOT, "assets", "models", `${name}.glb`), name);
const rules = (l: Look) => judge(l, body).map((f) => f.rule);
/** A copy of a look with its vertices moved by `f`. */
const moved = (l: Look, f: (p: V3) => V3): Look => ({ ...l, points: l.points.map(f) });
/** About +Y by `deg` degrees, as glTF turns: +X toward -Z for a positive turn. */
const aboutY = (deg: number) => {
  const [c, s] = [Math.cos((deg * Math.PI) / 180), Math.sin((deg * Math.PI) / 180)];
  return ([x, y, z]: V3): V3 => [x * c + z * s, y, -x * s + z * c];
};

test("every committed look keeps its kind's contract, but for the faults listed with why", () => {
  const { looks, faults } = audit();
  assert.ok(looks >= 20, `only ${looks} looks read`);
  const { unlisted, fixed } = against(faults, knownFaults());
  assert.deepEqual(unlisted.map((f) => `${f.look} ${f.rule}: ${f.detail}`), [], "faults not listed in attach_faults.json");
  assert.deepEqual(fixed, [], "listed in attach_faults.json but fixed: remove their lines");
});

test("the base body's sockets lie on their bones with the table's axes", () => {
  assert.deepEqual(judgeSockets(body), []);
  const turned = new Map(body.sockets);
  const hand = turned.get("socket.hand.R")!;
  turned.set("socket.hand.R", { ...hand, r: [0, 0, Math.SQRT1_2, Math.SQRT1_2] });
  assert.ok(judgeSockets({ ...body, sockets: turned }).some((f) => f.rule === "socket.hand.R"));
});

test("a sword in hand passes, and fails turned across X, off its grip, or without its tilt", () => {
  const sword = look("held/arming_sword");
  assert.deepEqual(rules(sword), []);
  assert.deepEqual(rules(moved(sword, aboutY(90))), ["facing"]);
  assert.deepEqual(rules(moved(sword, ([x, y, z]) => [x + 0.03, y, z])), ["grip"]);
  assert.deepEqual(rules(moved(sword, ([x, y, z]) => [x, -y, z])), ["along +Y"]);
  const { carry_tilt: _, ...untilted } = sword.extras;
  assert.deepEqual(rules({ ...sword, extras: untilted }), ["carry_tilt"]);
});

test("an axe turned so its bit faces -Z and its handle sits on the origin passes", () => {
  const axe = look("held/stone_axe");
  assert.deepEqual(rules(axe).sort(), ["facing", "grip"]);
  // Its handle is 4.7 cm toward +X and its bit toward -X: back onto the origin, then -X to -Z.
  const turned = moved(moved(axe, ([x, y, z]) => [x - 0.047, y, z]), aboutY(-90));
  assert.deepEqual(rules(turned), []);
  assert.deepEqual(rules(moved(turned, aboutY(180))), ["facing"], "its bit toward +Z");
});

test("a hung basket passes once it says so, and fails standing up", () => {
  const basket = look("held/basket");
  assert.deepEqual(rules(basket), ["marker"]);
  const marked = { ...basket, extras: { ...basket.extras, hold: "hung" } };
  assert.deepEqual(rules(marked), []);
  assert.deepEqual(rules(moved(marked, ([x, y, z]) => [x, y, -z])), ["hangs", "hangs"]);
});

test("a strapped shield passes, and fails with its board behind its grip or a strap on no bone", () => {
  const shield = look("held/round_shield");
  assert.deepEqual(rules(shield), []);
  assert.deepEqual(rules(moved(shield, ([x, y, z]) => [x, y, -z])), ["board"]);
  const strap = { ...(shield.extras.strap as object), bone: "lowerarm_x" };
  assert.deepEqual(rules({ ...shield, extras: { ...shield.extras, strap } }), ["marker", "strap"]);
});

test("a rigid quiver passes in its socket's space, and fails saved in the body's or without its morphs", () => {
  const quiver = look("gear/quiver");
  assert.deepEqual(rules(quiver), []);
  assert.deepEqual(rules(moved(quiver, ([x, y, z]) => [x - 0.15, y + 1.07, z])), ["socket space"]);
  const { morphs: _, ...still } = quiver.extras;
  assert.deepEqual(rules({ ...quiver, extras: still }), ["morphs"]);
  assert.deepEqual(rules({ ...quiver, extras: { ...quiver.extras, socket: "socket.hip" } }), ["socket"]);
});

test("a skinned cap passes, and fails on a joint or morph not the body's, or covering patch 1024", () => {
  const cap = look("gear/cap");
  assert.deepEqual(rules(cap), []);
  assert.deepEqual(rules({ ...cap, joints: [...cap.joints!, "head_x"] }), ["skin"]);
  assert.deepEqual(rules({ ...cap, targets: ["heavy"] }), ["morphs"]);
  assert.deepEqual(rules({ ...cap, extras: { ...cap.extras, covers: [3, 1024] } }), ["covers"]);
});

test("a fault listed but no longer found is reported, so a fix removes its line", () => {
  const { fixed } = against([], { "held/hammer": { grip: "why" } });
  assert.deepEqual(fixed, ["held/hammer grip"]);
});
