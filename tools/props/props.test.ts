import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { conceptPrompt, hasPbr, readProp, refineArgs, stylePrompt } from "./props.ts";

const forge = `{"subject": "forge", "file": "Forge", "dims": [2.6, 2.6, 1.5], "budget": 2500, "roles": ["Rock", "Fire"]}`;

test("reads a prop.json", () => {
  const prop = readProp(forge, "forge");
  assert.equal(prop.file, "Forge");
  assert.deepEqual(prop.roles, ["Rock", "Fire"]);
});

test("refuses a prop.json missing what refine needs", () => {
  assert.throws(() => readProp(`{"subject": "forge", "file": "Forge", "dims": [1, 1, 1], "roles": ["Rock"]}`, "forge"), /budget/);
  assert.throws(() => readProp(`{"subject": "forge", "file": "Forge", "size": 2.6, "budget": 9, "palette": true, "roles": []}`, "forge"), /roles/);
  assert.doesNotThrow(() => readProp(`{"subject": "chest", "file": "Chest", "dims": [1, 0.6, 0.6], "budget": 9}`, "chest"));
  // The model contract: a real size, and a leaf's hinge and handle.
  assert.throws(() => readProp(`{"subject": "chest", "file": "Chest", "size": 1, "budget": 9}`, "chest"), /dims/);
  assert.throws(() => readProp(`{"subject": "door", "file": "Door", "dims": [0.9, 1.9, 0.08], "anchor": "hinge", "budget": 9}`, "door"), /hinge/);
});

test("passes refine.py its arguments in order, turned 0 if not given, textured by default", () => {
  const args = refineArgs(readProp(forge, "forge"), "m.glb", "o.glb", "s.png");
  assert.deepEqual(args, ["m.glb", "o.glb", "s.png", "2.6", "2500", "0", "Rock", "Fire", "--textured", "--dims=2.6,2.6,1.5"]);
});

test("a leaf is made to the contract: hinged at its origin, mirrored if hinged on its right, its sockets written", () => {
  const door = readProp(`{"subject": "door", "file": "Door", "dims": [0.9, 1.9, 0.08], "anchor": "hinge", "hinge": "right", "budget": 9, "sockets": {"hinge": [0, 0, 0], "handle": [0.8, 1, 0.04]}}`, "door");
  const args = refineArgs(door, "m", "o", "s");
  assert.ok(args.includes("--anchor=hinge"));
  assert.ok(args.includes("--mirror"));
  assert.ok(args.includes("--socket=hinge:0,0,0"));
  assert.ok(args.includes("--socket=handle:0.8,1,0.04"));
  assert.equal(args[3], "1.9");
});

test("a palette prop is painted in its roles, not textured", () => {
  const prop = readProp(forge, "forge");
  prop.palette = true;
  assert.ok(!refineArgs(prop, "m", "o", "s").includes("--textured"));
});

test("tells refine.py a flat thing is flat", () => {
  const prop = readProp(forge, "forge");
  prop.flat = true;
  assert.ok(refineArgs(prop, "m", "o", "s").includes("--flat"));
});

test("gives refine.py the texture's colour for a role matched by another", () => {
  const prop = readProp(forge, "forge");
  prop.match = { Fire: [0.68, 0.24, 0.17] };
  assert.deepEqual(refineArgs(prop, "m", "o", "s").slice(6, 8), ["Rock", "Fire=0.68,0.24,0.17"]);
});

test("knows a model with a normal map from one textured in colour only", () => {
  const glb = (json: object) => {
    const text = Buffer.from(JSON.stringify(json));
    const header = Buffer.alloc(20);
    header.writeUInt32LE(text.length, 12);
    return Buffer.concat([header, text]);
  };
  assert.equal(hasPbr(glb({ materials: [{ normalTexture: { index: 0 } }] })), true);
  assert.equal(hasPbr(glb({ materials: [{ pbrMetallicRoughness: { baseColorTexture: { index: 0 } } }] })), false);
});

test("a concept is the subject, then the style guide's paragraph word for word", () => {
  const guide = readFileSync(new URL("../../docs/art-style.md", import.meta.url), "utf8");
  const paragraph = stylePrompt(guide);
  assert.match(paragraph, /^Stylised realistic 3D game asset render/);
  assert.equal(conceptPrompt("oak tree", guide), `A single oak tree. ${paragraph}`);
  assert.throws(() => stylePrompt("# A guide with no concept prompt"), /concept prompt/);
});
