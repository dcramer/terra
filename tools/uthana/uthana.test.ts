import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { BODY, glbOf, groundScale, scaled, withSkin } from "./uthana.ts";

const parts = (glb: Buffer) => {
  const jsonLen = glb.readUInt32LE(12);
  return { json: JSON.parse(glb.subarray(20, 20 + jsonLen).toString()), bin: glb.subarray(28 + jsonLen, 28 + jsonLen + glb.readUInt32LE(20 + jsonLen)) };
};
const matrices = (glb: Buffer) => {
  const { json, bin } = parts(glb);
  const acc = json.accessors[json.skins[0].inverseBindMatrices];
  const view = json.bufferViews[acc.bufferView];
  const from = (view.byteOffset ?? 0) + (acc.byteOffset ?? 0);
  return bin.subarray(from, from + acc.count * 64);
};

test("a clip with no skin is given the body's: the same joints by name and bind matrices", () => {
  const body = readFileSync(BODY);
  const { json, bin } = parts(body);
  // Uthana's download: the body's nodes, no skin, the nodes in another order.
  const order = json.nodes.map((_: unknown, i: number) => i).reverse();
  const nodes = order.map((i: number) => ({ ...json.nodes[i], children: json.nodes[i].children?.map((c: number) => order.indexOf(c)) }));
  const bare = glbOf({ ...json, nodes, skins: undefined, meshes: undefined, scenes: [{ nodes: json.scenes[0].nodes.map((n: number) => order.indexOf(n)) }] }, bin);
  const skinned = withSkin(bare, body);
  const out = parts(skinned).json;
  const name = (j: { nodes: { name: string }[] }, i: number) => j.nodes[i].name;
  assert.deepEqual(out.skins[0].joints.map((i: number) => name(out, i)), json.skins[0].joints.map((i: number) => name(json, i)));
  assert.ok(matrices(skinned).equals(matrices(body)), "the inverse bind matrices are the body's");
  assert.equal(skinned.readUInt32LE(8), skinned.length, "the GLB's length is its own");
});

test("a clip that has a skin is left as it is", () => {
  const body = readFileSync(BODY);
  assert.equal(withSkin(body, body), body);
});

test("the ground scale puts back translations made too small, and leaves a clip at its size alone", () => {
  const body = readFileSync(BODY);
  const { json, bin } = parts(body);
  // The library's idle as Uthana sends a clip: one animation, translation keyed on the root
  // and the pelvis only.
  const idle = json.animations.find((a: { name: string }) => a.name === "Idle_Loop");
  const keep = new Set(["root", "pelvis"]);
  idle.channels = idle.channels.filter((c: { target: { node: number; path: string } }) => c.target.path !== "translation" || keep.has(json.nodes[c.target.node].name));
  const dir = mkdtempSync(join(tmpdir(), "uthana-"));
  const whole = join(dir, "idle.glb");
  const small = join(dir, "small.glb");
  writeFileSync(whole, glbOf({ ...json, animations: [idle] }, bin));
  writeFileSync(small, scaled(readFileSync(whole), 1 / 13));
  const as = groundScale(whole);
  assert.ok(Math.abs(as - 1) < 0.03, `the library's idle stands on the ground as it is: ${as}`);
  assert.ok(Math.abs(groundScale(small) / as - 13) < 1e-3, `a clip 13 times too small is scaled 13 times: ${groundScale(small)}`);
  writeFileSync(small, scaled(readFileSync(small), groundScale(small)));
  assert.ok(Math.abs(groundScale(small) - 1) < 1e-3, "scaled by it, the clip stands on the ground");
});
