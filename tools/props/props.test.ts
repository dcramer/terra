import assert from "node:assert/strict";
import { test } from "node:test";
import { readProp, refineArgs } from "./props.ts";

const forge = `{"subject": "forge", "file": "Forge", "size": 2.6, "budget": 2500, "roles": ["Rock", "Fire"]}`;

test("reads a prop.json", () => {
  const prop = readProp(forge, "forge");
  assert.equal(prop.file, "Forge");
  assert.deepEqual(prop.roles, ["Rock", "Fire"]);
});

test("refuses a prop.json missing what refine needs", () => {
  assert.throws(() => readProp(`{"subject": "forge", "file": "Forge", "size": 2.6, "roles": ["Rock"]}`, "forge"), /budget/);
  assert.throws(() => readProp(`{"subject": "forge", "file": "Forge", "size": 2.6, "budget": 9, "roles": []}`, "forge"), /roles/);
});

test("passes refine.py its arguments in order, turned 0 if not given", () => {
  const args = refineArgs(readProp(forge, "forge"), "m.glb", "o.glb", "s.png");
  assert.deepEqual(args, ["m.glb", "o.glb", "s.png", "2.6", "2500", "0", "Rock", "Fire"]);
});

test("tells refine.py a flat thing is flat", () => {
  const prop = readProp(forge, "forge");
  prop.flat = true;
  assert.equal(refineArgs(prop, "m", "o", "s").at(-1), "--flat");
});

test("gives refine.py the texture's colour for a role matched by another", () => {
  const prop = readProp(forge, "forge");
  prop.match = { Fire: [0.68, 0.24, 0.17] };
  assert.deepEqual(refineArgs(prop, "m", "o", "s").slice(6), ["Rock", "Fire=0.68,0.24,0.17"]);
});
