import assert from "node:assert/strict";
import { test } from "node:test";
import { blend } from "./blend.ts";
import { Glb } from "./glb.ts";

test("a loop blends into itself from every moment, and into another clip from further off", () => {
  const g = new Glb("assets/models/character/mannequin.glb");
  const same = blend(g, "Walk_Loop", g, "Walk_Loop");
  assert.ok(same.first < 1e-6 && same.worst < 1e-6, JSON.stringify(same));
  const other = blend(g, "Walk_Loop", g, "Crouch_Fwd_Loop");
  assert.ok(other.first > 1 && other.mean <= other.worst, JSON.stringify(other));
});
