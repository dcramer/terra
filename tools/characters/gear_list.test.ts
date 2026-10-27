import assert from "node:assert/strict";
import { test } from "node:test";
import { type GearData, garmentLine, garmentOrder, gearData, rigidPieces } from "./gear_list.ts";

test("every committed worn item's data gives its maker's line, each garment after what it is worn over or under", () => {
  const items = gearData();
  const order = garmentOrder(items);
  assert.ok(order.length >= 12, `only ${order.length} garments`);
  for (const [i, g] of order.entries()) {
    const d = items.get(g)!;
    for (const n of [...(d.over ?? []), ...(d.under ?? [])]) assert.ok(order.indexOf(n) < i, `${g} made before ${n}`);
    assert.match(garmentLine(g, d), new RegExp(`^${g} (shell|hung) \\w+ \\w+ art/gear/[\\w-]+/model.glb`));
  }
  assert.deepEqual(rigidPieces(items), ["large_pack", "quiver", "small_pack"]);
});

test("a garment's flags come from its data, as build_bodies.sh's list once gave them", () => {
  const shirt: GearData = { kind: "garment", fit: "shell", model: "dressed-male-smith", role: "Linen", coverage: "chest_long_sleeves", over: ["trousers"], under: ["smith_apron"], plain: true };
  assert.equal(garmentLine("shirt", shirt), "shirt shell Linen chest_long_sleeves art/gear/dressed-male-smith/model.glb --over=trousers --under=smith_apron --plain");
  assert.throws(() => garmentLine("x", { ...shirt, fit: "back" }), /shell" or "hung/);
});

test("a garment worn over a missing piece, or pieces over each other in a circle, is refused", () => {
  const g = (over: string[]): GearData => ({ kind: "garment", fit: "shell", model: "m", role: "R", coverage: "c", over });
  assert.throws(() => garmentOrder(new Map([["a", g(["nothing"])]])), /no garment with data/);
  assert.throws(() => garmentOrder(new Map([["a", g(["b"])], ["b", g(["a"])]])), /in a circle/);
  assert.deepEqual(garmentOrder(new Map([["b", g(["a"])], ["a", g([])]])), ["a", "b"]);
});
