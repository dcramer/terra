import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { Glb, posed, type Clip, type Q, type V3 } from "../characters/glb.ts";
import { against, ballHeight, bare, faults, glbOf, lift, lifted, withSkin } from "./carry.ts";
import { BODY } from "./uthana.ts";

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
const dir = mkdtempSync(join(tmpdir(), "uthana-"));
const saved = (name: string, glb: Buffer) => {
  const file = join(dir, name);
  writeFileSync(file, glb);
  return new Glb(file);
};
/** A clip keying `keys` (node name to a constant pose) over a second. */
const still = (keys: Record<string, { t?: V3; r?: Q }>): Clip => ({
  name: "still",
  end: 1,
  tracks: new Map(Object.entries(keys).map(([n, k]) => [n, { ...(k.t ? { t: () => k.t! } : {}), ...(k.r ? { r: () => k.r! } : {}) }])),
});

test("the body is uploaded without its animations, its skeleton and skin as they are", () => {
  const body = readFileSync(BODY);
  const { json } = parts(body);
  assert.ok(json.animations.length > 0, "the body carries clips");
  const out = parts(bare(body)).json;
  assert.equal(out.animations, undefined);
  assert.deepEqual(out.nodes, json.nodes);
  assert.deepEqual(out.skins, json.skins);
  assert.ok(matrices(bare(body)).equals(matrices(body)));
});

test("a clip with no skin is given the body's: the same joints by name and bind matrices", () => {
  const body = readFileSync(BODY);
  const { json, bin } = parts(body);
  // Uthana's download: the body's nodes, no skin, the nodes in another order.
  const order = json.nodes.map((_: unknown, i: number) => i).reverse();
  const nodes = order.map((i: number) => ({ ...json.nodes[i], children: json.nodes[i].children?.map((c: number) => order.indexOf(c)) }));
  const clip = glbOf({ ...json, nodes, skins: undefined, meshes: undefined, scenes: [{ nodes: json.scenes[0].nodes.map((n: number) => order.indexOf(n)) }] }, bin);
  const skinned = withSkin(clip, body);
  const out = parts(skinned).json;
  const name = (j: { nodes: { name: string }[] }, i: number) => j.nodes[i].name;
  assert.deepEqual(out.skins[0].joints.map((i: number) => name(out, i)), json.skins[0].joints.map((i: number) => name(json, i)));
  assert.ok(matrices(skinned).equals(matrices(body)), "the inverse bind matrices are the body's");
  assert.equal(skinned.readUInt32LE(8), skinned.length, "the GLB's length is its own");
  assert.equal(withSkin(body, body), body, "a clip that has a skin is left as it is");
});

test("a take is lifted by the world height asked, under the Rig's scale", () => {
  const body = readFileSync(BODY);
  const { json, bin } = parts(body);
  // The library's idle, the root's translation keyed as Uthana keys it.
  const idle = json.animations.find((a: { name: string }) => a.name === "Idle_Loop");
  const plain = saved("idle.glb", glbOf({ ...json, animations: [idle] }, bin));
  const up = saved("up.glb", lifted(readFileSync(plain.file), 0.05));
  const figure = new Glb(BODY);
  const root = figure.named("root");
  for (const t of [0, 0.5, 1]) {
    const a = posed(figure, plain.clip("Idle_Loop"), t)[root].t;
    const b = posed(figure, up.clip("Idle_Loop"), t)[root].t;
    assert.ok(Math.abs(b[1] - a[1] - 0.05) < 1e-5 && Math.abs(b[0] - a[0]) < 1e-5 && Math.abs(b[2] - a[2]) < 1e-5, `raised 5 cm straight up at ${t} s: ${a} -> ${b}`);
  }
});

test("the gate passes a take that stands as the reference does, and refuses today's broken download", () => {
  const figure = new Glb(BODY);
  const rest = still({});
  const right = against(figure, rest, figure, rest);
  assert.deepEqual(faults(right), [], `the body standing as itself stands: ${JSON.stringify(right)}`);
  // The trial idle (motion mQpwFrKxhYot) as Uthana sent it on base.glb uploaded with its clips,
  // 2026-10-26: the pelvis 6.3 cm over the root where it rests at 91.7, turned 68 degrees.
  const broken = still({ pelvis: { t: [0, 0, 0.063], r: [0.311, -0.061, -0.05, 0.947] } });
  const wrong = faults(against(figure, broken, figure, rest));
  assert.equal(wrong.length, 4, wrong.join("; "));
  assert.match(wrong.join("; "), /standing, the pelvis .* trunk .* feet .* into the ground/);
});

test("feet sunk to the reference's forefoot height are refused, and standing at our own height they pass", () => {
  const body = readFileSync(BODY);
  const { json, bin } = parts(body);
  // A reference whose forefeet stand 3 cm lower than ours at rest (Manny's at 0.8 cm, ours at 3.9).
  const rig = json.nodes.find((n: { name: string }) => n.name === "Rig");
  const low = structuredClone(json);
  low.nodes.find((n: { name: string }) => n.name === "Rig").translation = [0, -0.03, 0];
  const reference = saved("reference.glb", glbOf(low, bin));
  const figure = new Glb(BODY);
  const by = lift(figure, reference);
  assert.ok(Math.abs(by - 0.03) < 1e-6, `the lift is the rests' difference: ${by}`);
  assert.ok(Math.abs(ballHeight(figure) - by - ballHeight(reference)) < 1e-9);
  // Ours as Uthana sends it: standing at rest with its balls at the reference's height.
  const root = figure.gltf.nodes[figure.named("root")].translation!;
  const sunk = (down: number): V3 => [root[0], root[1] - down / rig.scale[1], root[2]];
  const rest = still({});
  const wrong = faults(against(figure, still({ root: { t: sunk(by) } }), reference, rest));
  assert.match(wrong.join("; "), /feet -3\.0 cm into the ground/);
  assert.deepEqual(faults(against(figure, still({ root: { t: sunk(0) } }), reference, rest)), []);
});
