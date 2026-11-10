import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { ROOT } from "./attach_contract.ts";
import { Glb } from "./glb.ts";
import { FINGER_M, FIST_CM, FIT_M, PALM_M, type Handle, handRest, palmed, restOff, seatOf, seated } from "./grip.ts";
import { BODY, skinOf } from "./skin.ts";

// The fist on thin, middle and thick handles (D562): every template's own hand, closed at rest
// on a handle seated as the game seats it, and the planted fault (D632) the seat was made for.
const body = new Glb(join(ROOT, "assets", "models", "character", "base.glb"));
const { shapes } = skinOf(body, join(ROOT, "assets", "models", "character", "templates.json"), [], BODY);

/** The bone furthest off a handle `r` round whose line crosses the grip at `at`, cm, and on whom:
 * the right hand's, which holds every thin look. Not the left's: its socket's +X runs out of
 * the palm (docs/attachment.md), so the seat's `PALM_M - r` up +X lays a handle thinner than
 * `PALM_M` off its palm (2.0 cm off at 1.2 cm round, seated mid palm or at the fingers' roots;
 * 1.0 with the seat's X mirrored); only the bow, 2.6 cm round, is held there. */
function worst(r: number, at: [number, number, number]): { cm: number; where: string } {
  let most = { cm: 0, where: "" };
  for (const shape of [{ name: "base", moved: undefined }, ...shapes]) {
    const { locals, socket } = handRest(body, "r", shape.moved);
    for (const [bone, cm] of restOff(locals, socket, { at, way: [0, 1, 0], r }, "power", "r")) if (Math.abs(cm) > Math.abs(most.cm)) most = { cm, where: `${bone}, ${shape.name}` };
  }
  return most;
}

test("a thin, a middle and a thick handle each leave every finger of every body within a centimetre", () => {
  assert.ok(shapes.length >= 5, `${shapes.length} templates`);
  for (const r of [0.012, 0.016, 0.021]) {
    const off = worst(r, palmed(r));
    assert.ok(Math.abs(off.cm) <= FIST_CM, `a handle ${r * 100} cm round: ${off.cm.toFixed(2)} cm off at ${off.where} (bar ${FIST_CM})`);
  }
});

test("the planted fault: a thin handle seated mid palm, as every handle was, leaves a knuckle bone short of it", () => {
  const off = worst(0.012, palmed(0.012, 0));
  assert.ok(off.cm > FIST_CM, `${off.cm.toFixed(2)} cm at ${off.where}`);
  assert.match(off.where, /_01_/, "a first bone, the knuckle's");
});

test("a seated look's handle crosses the grip on the palm, a thinner one nearer the fingers by what it lacks", () => {
  const h: Handle = { at: [0.004, 0, -0.003], way: [0, 1, 0], r: 0.012 };
  const seat = seatOf(h);
  assert.deepEqual(seat.map((x, i) => x + h.at[i]), seated(h).at);
  assert.deepEqual(seated(h).at, [PALM_M - 0.012, 0, -(FIT_M - 0.012)]);
  assert.deepEqual(palmed(0.016), [PALM_M - 0.016, 0, -(FIT_M - 0.016)]);
  // As thick as the fist is placed for, or thicker: mid palm.
  assert.equal(palmed(FIT_M)[2], -0);
  assert.equal(palmed(0.025)[2], -0);
});

test("the twin's numbers are the game's", () => {
  const rs = readFileSync(join(ROOT, "crates", "terra_walker", "src", "grip.rs"), "utf8");
  const of = (name: string) => Number(rs.match(new RegExp(`pub const ${name}: f32 = ([0-9.]+);`))![1]);
  assert.deepEqual([of("FINGER_M"), of("PALM_M"), of("FIT_M")], [FINGER_M, PALM_M, FIT_M]);
});
