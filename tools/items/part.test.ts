import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { boxHead, collar, collarMesh, haft, head } from "./fixtures.ts";
import { type Mesh, normal, readMesh, triangles } from "./mesh.ts";
import { checkPart, type Ctx, loadCtx, rigid } from "./part.ts";

// The real types, kinds and mounts, with only the made-up parts.
function ctx(parts: any[], meshes: Record<string, Mesh>): Ctx {
  const real = loadCtx();
  return { ...real, parts: Object.fromEntries(parts.map((p) => [p.part, p])), mesh: (p) => meshes[p.part] };
}
const faults = (part: any, c: Ctx, stage?: string) => checkPart(part, c).filter((s) => !stage || s.stage === stage).flatMap((s) => s.faults);
const good = () => ctx([head(), haft(), collar()], { test_box_head: boxHead(), test_collar: collarMesh() });

test("the made-up head is what its numbers say: its top faces up, its eye round the line", () => {
  const top = triangles(boxHead()).filter((t) => t.every((p) => p[1] === 0.06));
  assert.ok(top.length > 0 && top.every((t) => normal(t)[1] > 0.99));
});

test("a good head, haft and collar pass every stage", () => {
  const c = good();
  for (const p of Object.values(c.parts)) assert.deepEqual(checkPart(p, c).filter((s) => s.faults.length), [], p.part);
});

test("a point off the line is refused at its field", () => {
  const h = head();
  h.points[1].at = [0.005, 0.06, 0];
  const f = faults(h, ctx([h, haft()], { test_box_head: boxHead() }), "points");
  assert.ok(f.some((s) => /eye\.upper\.at is 5\.0 mm off the line/.test(s)), f.join("\n"));
});

test("a point of the wrong type, and a missing one, are refused", () => {
  const h = head();
  h.points[0].type = "socket";
  h.points.pop();
  const f = faults(h, ctx([h], { test_box_head: boxHead() }), "points");
  assert.ok(f.some((s) => /eye\.lower is a socket/.test(s)) && f.some((s) => /no edge\.upper/.test(s)), f.join("\n"));
});

test("an eye wider than its points say is a gap round the haft", () => {
  const f = faults(head(), ctx([head(), haft()], { test_box_head: boxHead(0.04) }), "openings");
  assert.ok(f.some((s) => /2\.5 mm gap round the handle/.test(s)), f.join("\n"));
});

test("an eye narrower than its points say buries the haft", () => {
  const f = faults(head(), ctx([head(), haft()], { test_box_head: boxHead(0.03) }), "openings");
  assert.ok(f.some((s) => /buried 2\.5 mm/.test(s)), f.join("\n"));
});

test("a scaled part is refused, read from its glb", () => {
  const dir = mkdtempSync(join(tmpdir(), "part-"));
  const file = join(dir, "head.glb");
  writeFileSync(file, glbOf(boxHead(), [1.2, 1.2, 1.2]));
  const mesh = readMesh(file);
  assert.deepEqual(mesh.scaled, ["head"]);
  const f = faults(head(), ctx([head()], { test_box_head: mesh }), "scale");
  assert.match(f[0], /parts are never scaled \(D566\)/);
});

test("a head outside its kind's envelope is refused with the number and the limit", () => {
  const h = head({ mass_kg: { value: 1.5, source: "made up for the part checker's tests" } });
  const f = faults(h, ctx([h], { test_box_head: boxHead() }), "slots");
  assert.ok(f.some((s) => /axe: its mass 1\.5 kg is outside the axe's head mass \[0\.656,1\.28\]/.test(s)), f.join("\n"));
});

test("a haft too long for its kind, or its grip off its mount's range, is refused from its numbers", () => {
  const long = haft({ length_m: { value: 0.8, source: "made up for the part checker's tests" } });
  assert.ok(faults(long, ctx([long], {}), "slots").some((s) => /axe: 0\.8 m long, past the axe's longest item 0\.748 m/.test(s)));
  const off = haft({ grip_from_end_m: { value: 0.2, source: "made up for the part checker's tests" } });
  assert.ok(faults(off, ctx([off], {}), "slots").some((s) => /grip 0\.2 m from the butt is outside a hafted look's \[0\.04,0\.12\]/.test(s)));
});

test("a haft that fits many kinds must name the ones it is made for", () => {
  const h = haft();
  delete h.kinds;
  assert.match(faults(h, ctx([h], {}), "data")[0], /\/kinds is missing: a haft fits axe, hammer/);
});

test("a collar whose model stops short of its cover leaves a gap under the head", () => {
  const f = faults(collar(), ctx([head(), collar()], { test_box_head: boxHead(), test_collar: collarMesh(-0.003) }), "joins");
  assert.ok(f.some((s) => /axe, with test_box_head: a 3\.0 mm gap where they meet/.test(s)), f.join("\n"));
});

test("a collar pushed up into the head is buried in it", () => {
  const f = faults(collar(), ctx([head(), collar()], { test_box_head: boxHead(), test_collar: collarMesh(0.004) }), "joins");
  assert.ok(f.some((s) => /buried 4\.0 mm/.test(s)), f.join("\n"));
});

test("an eye.upper below the head's top puts the haft's end in the head's top face", () => {
  const h = head();
  h.points[1].at = [0, 0.056, 0];
  const f = faults(h, ctx([h, haft()], { test_box_head: boxHead() }), "joins");
  assert.ok(f.some((s) => /faces of the head lie in the handle's top plane/.test(s)), f.join("\n"));
});

test("a material not named by its role is refused", () => {
  const m = { ...boxHead(), materials: ["Material.001"] };
  assert.match(faults(head(), ctx([head()], { test_box_head: m }), "roles")[0], /"Material" is not a role/);
});

test("a full point's turn takes its direction and roll onto its partner's", () => {
  const r = rigid({ dir: [0, 0, -1], up: [0, 1, 0] }, { dir: [1, 0, 0], up: [0, 1, 0] });
  const turn = (p: number[]) => r.map((row) => row[0] * p[0] + row[1] * p[1] + row[2] * p[2]);
  assert.deepEqual(turn([0, 0, -1]).map(Math.round), [1, 0, 0]);
  assert.deepEqual(turn([0, 1, 0]).map(Math.round), [0, 1, 0]);
});

// A glb of one mesh on one node named head, with the node's scale.
function glbOf(m: Mesh, scale: number[]): Buffer {
  const pos = Buffer.from(new Float32Array(m.positions).buffer);
  const idx = Buffer.from(new Uint32Array(m.indices).buffer);
  const lo = [0, 1, 2].map((k) => Math.min(...m.positions.filter((_, i) => i % 3 === k)));
  const hi = [0, 1, 2].map((k) => Math.max(...m.positions.filter((_, i) => i % 3 === k)));
  const gltf = {
    asset: { version: "2.0" },
    scenes: [{ nodes: [0] }],
    nodes: [{ name: "head", mesh: 0, scale }],
    meshes: [{ primitives: [{ attributes: { POSITION: 0 }, indices: 1, material: 0 }] }],
    materials: [{ name: m.materials[0] }],
    buffers: [{ byteLength: pos.length + idx.length }],
    bufferViews: [{ buffer: 0, byteOffset: 0, byteLength: pos.length }, { buffer: 0, byteOffset: pos.length, byteLength: idx.length }],
    accessors: [
      { bufferView: 0, componentType: 5126, count: m.positions.length / 3, type: "VEC3", min: lo, max: hi },
      { bufferView: 1, componentType: 5125, count: m.indices.length, type: "SCALAR" },
    ],
  };
  let json = Buffer.from(JSON.stringify(gltf));
  json = Buffer.concat([json, Buffer.alloc((4 - (json.length % 4)) % 4, 0x20)]);
  const bin = Buffer.concat([pos, idx]);
  const head = Buffer.alloc(12);
  head.writeUInt32LE(0x46546c67, 0);
  head.writeUInt32LE(2, 4);
  head.writeUInt32LE(12 + 8 + json.length + 8 + bin.length, 8);
  const chunk = (b: Buffer, t: number) => {
    const h = Buffer.alloc(8);
    h.writeUInt32LE(b.length, 0);
    h.writeUInt32LE(t, 4);
    return Buffer.concat([h, b]);
  };
  return Buffer.concat([head, chunk(json, 0x4e4f534a), chunk(bin, 0x004e4942)]);
}
