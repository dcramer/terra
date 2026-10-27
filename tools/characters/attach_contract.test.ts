// The attachment contract (docs/attachment.md, D546) on the committed looks and the base body,
// and on copies of them broken on purpose, so each rule is seen to catch what it is for.
// node --test tools/characters/attach_contract.test.ts

import assert from "node:assert/strict";
import { test } from "node:test";
import { join } from "node:path";
import { type V3 } from "./glb.ts";
import { type Look, ROOT, against, audit, dataOf, judge, judgeSockets, kindOf, knownFaults, readBody, readLook, sourceSha } from "./attach_contract.ts";

const body = readBody();
const read = new Map<string, Look>();
const look = (name: string) => read.get(name) ?? read.set(name, readLook(join(ROOT, "assets", "models", `${name}.glb`), name)).get(name)!;
const rules = (l: Look) => judge(l, body).map((f) => f.rule);
/** A copy of a look with its vertices moved by `f`, and its normals turned as they are; a
 * mirror leaves the winding as it was, so the copy is inside out, as a script's would be. */
const moved = (l: Look, f: (p: V3) => V3): Look => {
  const o = f([0, 0, 0]);
  return { ...l, points: l.points.map(f), normals: l.normals?.map((n) => n && (f(n).map((v, k) => v - o[k]) as V3)) };
};
/** A copy mirrored across `axis` as a modeller does it: the winding flipped with the vertices,
 * so it is the right way out, only its other hand. */
const mirror = (l: Look, axis: 0 | 1 | 2): Look => {
  const m = moved(l, (p) => p.map((v, k) => (k === axis ? -v : v)) as V3);
  const tris = [...l.tris];
  for (let t = 0; t + 2 < tris.length; t += 3) [tris[t + 1], tris[t + 2]] = [tris[t + 2], tris[t + 1]];
  return { ...m, tris };
};
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
  assert.deepEqual(rules(mirror(sword, 1)), ["along +Y", "grip along"]);
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
  assert.deepEqual(rules(mirror(marked, 2)), ["hangs", "hangs"]);
});

test("a strapped shield passes, and fails with its board behind its grip or a strap on no bone", () => {
  const shield = look("held/round_shield");
  assert.deepEqual(rules(shield), []);
  assert.deepEqual(rules(mirror(shield, 2)), ["board"]);
  const strap = { ...(shield.extras.strap as object), bone: "lowerarm_x" };
  assert.deepEqual(rules({ ...shield, extras: { ...shield.extras, strap } }), ["marker", "strap"]);
});

test("a rigid quiver passes in its socket's space, and fails saved in the body's or without its morphs", () => {
  const quiver = look("gear/quiver");
  assert.deepEqual(rules(quiver), []);
  assert.deepEqual(rules(moved(quiver, ([x, y, z]) => [x - 0.15, y + 1.07, z])), ["socket space", "stale"]);
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

test("a look's kind is its data's, and a garment's its skin with no data", () => {
  assert.equal(kindOf(look("held/stone_axe"), dataOf("held/stone_axe")), "hafted");
  assert.equal(kindOf(look("gear/quiver"), dataOf("gear/quiver")), "belt");
  assert.equal(kindOf(look("gear/cap"), dataOf("gear/cap")), "garment");
  const sword = look("held/arming_sword");
  const data = dataOf("held/arming_sword")!;
  const { kind: _, ...unnamed } = data;
  assert.deepEqual(judge(sword, body, unnamed).map((f) => f.rule), ["kind"]);
  assert.deepEqual(judge(sword, body, { ...data, kind: "sabre" }).map((f) => f.rule), ["kind"]);
  // A blade named a shield: it goes on a strap, which its data does not give.
  assert.ok(judge(sword, body, { ...data, kind: "shield" }).some((f) => f.rule === "kind"));
});

test("a look is its kind's life size, its grip where its kind's falls", () => {
  const knife = look("held/knife");
  assert.deepEqual(rules(knife), ["facing"]);
  // Six times its size, as a model left at its source's scale: a 1.56 m knife, gripped 30 cm up.
  assert.deepEqual(rules(moved(knife, ([x, y, z]) => [6 * x, 6 * y, 6 * z])).sort(), ["facing", "grip along", "size", "stale", "stale"]);
  // Gripped at its middle, not near its pommel: the length fit's grip_from_end mistyped.
  assert.deepEqual(rules(moved(knife, ([x, y, z]) => [x, y - 0.08, z])), ["facing", "grip along", "stale"]);
  const bow = look("held/bow");
  assert.deepEqual(rules(bow).sort(), ["facing", "grip", "second"]);
  assert.ok(rules(moved(bow, ([x, y, z]) => [x, y + 0.3, z])).includes("grip along"), "held near a limb's tip");
  const shield = look("held/round_shield");
  assert.deepEqual(rules(moved(shield, ([x, y, z]) => [2 * x, 2 * y, 2 * z])), ["size"]);
});

test("a kind held in two says where its other hand goes, on its handle, in its data and its extras", () => {
  const spear = look("held/spear");
  const data = dataOf("held/spear")!;
  const second = (d: Record<string, unknown>, l = spear) => judge(l, body, d).filter((f) => f.rule === "second" || f.rule === "marker").map((f) => f.rule);
  assert.deepEqual(second(data), ["second"], "none given");
  assert.deepEqual(second({ ...data, second: -0.5 }), ["marker"], "given, but its look not made again");
  const made = { ...spear, extras: { ...spear.extras, second: -0.5 } };
  assert.deepEqual(second({ ...data, second: -0.5 }, made), [], "50 cm behind the lead hand, on its shaft");
  assert.deepEqual(second({ ...data, second: 0.9 }, { ...spear, extras: { ...spear.extras, second: 0.9 } }), ["second"], "on its blade");
  const bow = look("held/bow");
  const nock = [0, 0, -0.15];
  assert.deepEqual(judge({ ...bow, extras: { ...bow.extras, nock } }, body, { ...dataOf("held/bow")!, nock }).filter((f) => f.rule === "second"), []);
  // One-handed kinds need none.
  assert.ok(!rules(look("held/arming_sword")).includes("second"));
});

test("a look mirrored in its vertices alone is inside out; a garment mirrored whole has its sides swapped", () => {
  const sword = look("held/arming_sword");
  assert.deepEqual(rules(moved(sword, ([x, y, z]) => [-x, y, z])), ["inside out"]);
  assert.deepEqual(rules(mirror(sword, 0)), [], "the right way out, only its other hand: a two-edged blade is the same");
  assert.deepEqual(rules(moved(sword, aboutY(180))).filter((r) => r === "inside out"), [], "a turn is no mirror");
  const coat = look("gear/leather_coat");
  assert.deepEqual(rules(coat), []);
  assert.deepEqual(rules(mirror(coat, 0)), ["on the body", "mirrored"], "its left sleeve weighted to the right arm");
});

test("a skinned look is made on the base, at its size, and a pack's bag is behind the back", () => {
  const cap = look("gear/cap");
  assert.deepEqual(rules(moved(cap, ([x, y, z]) => [100 * x, 100 * y, 100 * z])), ["on the body"], "in centimetres");
  assert.deepEqual(rules(moved(cap, ([x, y, z]) => [x, y + 1, z])), ["on the body"], "a metre over the head");
  const pack = look("gear/small_pack");
  assert.deepEqual(rules(pack), []);
  assert.deepEqual(rules(mirror(pack, 2)), ["facing"], "Tripo's front kept: the bag on the chest");
  assert.deepEqual(rules(look("gear/skirt")), [], "a loose hem hangs free of the hips");
});

test("a look is made from its data as it stands, and its data's source is there", () => {
  const stale = (name: string, change: Record<string, unknown>) => judge(look(name), body, { ...dataOf(name)!, ...change }).map((f) => f.rule);
  assert.deepEqual(stale("held/stone_axe", { length: 0.8 }).filter((r) => r === "stale"), ["stale"], "its length changed, not made again");
  assert.deepEqual(stale("held/stone_axe", { grip_from_end: 0.12 }).filter((r) => r === "stale"), ["stale"]);
  assert.deepEqual(stale("held/basket", { out: 0.1 }).filter((r) => r === "stale"), ["stale"]);
  assert.deepEqual(stale("gear/quiver", { size: 0.4 }), ["stale"]);
  assert.deepEqual(stale("gear/quiver", { hangs_at: 0.6 }), ["stale"]);
  assert.deepEqual(stale("gear/quiver", { swing: { stiffness: 0.5, drag: 0.1, gravity: 0.6 } }), ["stale"]);
  assert.deepEqual(stale("held/arming_sword", { source: "tools/Nope.glb" }), ["file"]);
  // Its source's hash in its extras (the fitter's): a points fit's source changed is found.
  const sword = look("held/arming_sword");
  const made = sourceSha(join(ROOT, "assets", "models", dataOf("held/arming_sword")!.source as string));
  assert.deepEqual(rules({ ...sword, extras: { ...sword.extras, source_sha: made } }), []);
  assert.deepEqual(rules({ ...sword, extras: { ...sword.extras, source_sha: "0123456789ab" } }), ["stale"]);
  const quiver = look("gear/quiver");
  assert.deepEqual(rules({ ...quiver, extras: { ...quiver.extras, source_sha: sourceSha(join(ROOT, "art", "gear", "quiver", "model.glb")) } }), []);
});

test("an item's hold in items.ron agrees with its look's kind and length", () => {
  const sword = look("held/arming_sword");
  const data = dataOf("held/arming_sword")!;
  const hold = { class: "sword", grip: "Main", length: 0.95 };
  const of = (h: typeof hold, d = data, l = sword) => judge(l, body, d, h).map((f) => f.rule);
  assert.deepEqual(of(hold), []);
  assert.deepEqual(of({ ...hold, class: "axe" }), ["hold"], "a blade named for an axe's move set");
  assert.deepEqual(judge(sword, body, { ...data, kind: "hafted" }, hold).map((f) => f.rule), ["hold"], "a sword named hafted");
  assert.deepEqual(of({ ...hold, length: 1.2 }), ["hold"], "its look made at another length");
  assert.deepEqual(of({ ...hold, grip: "Two" }), ["second"], "a greatsword with no second grip");
  assert.deepEqual(of({ ...hold, grip: "Two" }, { ...data, second: -0.2 }), ["marker"], "given, not made again");
  assert.deepEqual(of({ ...hold, grip: "Two" }, { ...data, second: -0.2 }, { ...sword, extras: { ...sword.extras, second: -0.2 } }), []);
});

test("a fault listed but no longer found is reported, so a fix removes its line", () => {
  const { fixed } = against([], { "held/hammer": { grip: "why" } });
  assert.deepEqual(fixed, ["held/hammer grip"]);
});
