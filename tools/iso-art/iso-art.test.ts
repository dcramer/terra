// The pipeline's tests, with no Tripo and no GPU: a small walker built here, whose answers are
// known by construction (its feet plant and slide back at 1.2 m/s, it faces +Z, it is 1.8 m
// tall), measured, checked, turned and scaled; the ledger, the costs, the prompt and refusals.

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { check, measure } from "./check.ts";
import { type Glb, pixelSize, readGlb, writeGlb } from "./gltf.ts";
import { type Ctx, ROOT, conceptPrompt, main, parseOutput } from "./iso-art.ts";
import { clipsFor } from "./kinds.ts";
import { readAsset, readmeCost, statusTable, writeAsset } from "./ledger.ts";
import { place, renameClips } from "./normalize.ts";

const README = readFileSync(join(ROOT, "tools", "tripo", "README.md"), "utf8");
const GUIDE = readFileSync(join(ROOT, "docs", "art-style.md"), "utf8");

/** A GLB from plain arrays: a hips joint 1 m up with two feet and a head, one point skinned
 * to each, and the clips given as [name, [[node, path, times, values]]]. */
function walker(anims: [string, [number, string, number[], number[][]][]][], sceneRotation?: number[]): Glb {
  const chunks: Buffer[] = [];
  const accessors: any[] = [];
  const views: any[] = [];
  let offset = 0;
  const add = (values: number[][], type: string, componentType = 5126) => {
    const flat = values.flat();
    const b = componentType === 5126 ? Buffer.from(new Float32Array(flat).buffer) : componentType === 5123 ? Buffer.from(new Uint16Array(flat).buffer) : Buffer.from(new Uint8Array(flat).buffer);
    const padded = Buffer.concat([b, Buffer.alloc((4 - (b.length % 4)) % 4)]);
    views.push({ buffer: 0, byteOffset: offset, byteLength: b.length });
    chunks.push(padded);
    offset += padded.length;
    accessors.push({ bufferView: views.length - 1, componentType, count: values.length, type });
    return accessors.length - 1;
  };
  const rest = [[0, 1, 0], [0.1, 0, 0], [-0.1, 0, 0], [0, 1.8, 0]];
  const pos = add(rest, "VEC3");
  const joints = add([[0, 0, 0, 0], [1, 0, 0, 0], [2, 0, 0, 0], [3, 0, 0, 0]], "VEC4", 5121);
  const weights = add(rest.map(() => [1, 0, 0, 0]), "VEC4");
  const indices = add([[0], [1], [2], [0], [2], [3]], "SCALAR", 5123);
  const ibm = add(rest.map(([x, y, z]) => [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, -x, -y, -z, 1]), "MAT4");
  const animations = anims.map(([name, chans]) => ({
    name,
    samplers: chans.map(([, path, times, values]) => ({ input: add(times.map((t) => [t]), "SCALAR"), output: add(values, path === "rotation" ? "VEC4" : "VEC3"), interpolation: "LINEAR" })),
    channels: chans.map(([node, path], i) => ({ sampler: i, target: { node, path } })),
  }));
  const doc = {
    asset: { version: "2.0" },
    scene: 0,
    scenes: [{ nodes: [0, 5] }],
    nodes: [
      { name: "armature", children: [1], ...(sceneRotation ? { rotation: sceneRotation } : {}) },
      { name: "hips", translation: [0, 1, 0], children: [2, 3, 4] },
      { name: "foot_l", translation: [0.1, -1, 0] },
      { name: "foot_r", translation: [-0.1, -1, 0] },
      { name: "head", translation: [0, 0.8, 0] },
      { name: "body", mesh: 0, skin: 0 },
    ],
    meshes: [{ primitives: [{ attributes: { POSITION: pos, JOINTS_0: joints, WEIGHTS_0: weights }, indices }] }],
    skins: [{ joints: [1, 2, 3, 4], inverseBindMatrices: ibm }],
    animations,
    accessors,
    bufferViews: views,
    buffers: [{ byteLength: offset }],
  };
  return readGlb(writeGlb({ doc, bin: Buffer.concat(chunks) }));
}

// In place: each foot planted for half the cycle, sliding back 0.6 m in 0.5 s (1.2 m/s), then
// lifted 0.2 m and swung forward.
const T = [0, 0.5, 0.75, 1];
const footL: [number, string, number[], number[][]] = [2, "translation", T, [[0.1, -1, 0.3], [0.1, -1, -0.3], [0.1, -0.8, 0], [0.1, -1, 0.3]]];
const footR: [number, string, number[], number[][]] = [3, "translation", [0, 0.25, 0.5, 1], [[-0.1, -1, -0.3], [-0.1, -0.8, 0], [-0.1, -1, 0.3], [-0.1, -1, -0.3]]];
const IDLE: [string, any[]] = ["preset:biped:idle", [[1, "translation", [0, 1, 2], [[0, 1, 0], [0, 0.98, 0], [0, 1, 0]]]]];
const WALK: [string, any[]] = ["preset:biped:walk", [footL, footR]];
// The body drops to 0.2 m while the feet stay on the floor.
const FALL: [string, any[]] = ["preset:biped:fall", [
  [1, "translation", [0, 1.5], [[0, 1, 0], [0, 0.2, 0]]],
  [2, "translation", [0, 1.5], [[0.1, -1, 0], [0.1, -0.2, 0]]],
  [3, "translation", [0, 1.5], [[-0.1, -1, 0], [-0.1, -0.2, 0]]],
]];

test("a walk's speed and heading come from its planted foot", () => {
  const m = measure(walker([IDLE, ["walk", [footL, footR]]]));
  const walk = m.clips.walk.motion!;
  assert.ok(Math.abs(walk.speed - 1.2) < 0.05, `speed ${walk.speed}`);
  assert.ok(Math.abs(walk.heading) < 2, `heading ${walk.heading}`);
  assert.ok(walk.travel < 0.01, "in place");
  assert.ok(walk.gap < 0.01, "the cycle closes");
  assert.ok(Math.abs(m.height - 1.8) < 1e-6);
  assert.ok(Math.abs(m.ground) < 1e-6);
  assert.equal(m.triangles, 2);
});

test("a model turned a quarter is seen walking toward +X, and export's root node turns it back and makes it life size", () => {
  const s = Math.SQRT1_2;
  const glb = walker([IDLE, WALK, FALL], [0, s, 0, s]); // 90 degrees about Y: its front to +X
  assert.deepEqual(renameClips(glb, clipsFor("biped")), ["run", "attack", "hit"]);
  const before = measure(glb).clips.walk.motion!;
  assert.ok(Math.abs(before.heading - 90) < 2, `heading ${before.heading}`);
  place(glb, "walker", 3.6, before.heading);
  const after = measure(readGlb(writeGlb(glb)));
  assert.ok(Math.abs(after.clips.walk.motion!.heading) < 2, `turned back: ${after.clips.walk.motion!.heading}`);
  assert.ok(Math.abs(after.height - 3.6) < 1e-4, `height ${after.height}`);
  assert.ok(Math.abs(after.clips.walk.motion!.speed - 2.4) < 0.1, "speed scales with size");
  assert.ok(Math.abs(after.ground) < 0.02 * 3.6, "on the ground");
  assert.ok(after.clips.death.motion!.endHeight < 0.3, "the fall ends down");
});

test("the checks name what is missing and pass what is right", () => {
  const glb = walker([IDLE, WALK, FALL]);
  renameClips(glb, clipsFor("biped"));
  const results = check(measure(glb), "hero", "biped", 1.8);
  const by = Object.fromEntries(results.map((r) => [r.check, r]));
  for (const ok of ["clip idle", "clip walk", "walk in place", "walk speed", "walk facing", "walk loops", "death ends down", "height", "on the ground", "triangles"]) assert.ok(by[ok].ok, `${ok}: ${by[ok].value}`);
  for (const bad of ["clip run", "clip attack", "clip hit", "textures"]) assert.equal(by[bad].ok, false, bad);
  // A quadruped gets the one walk Tripo has for it.
  assert.deepEqual(clipsFor("quadruped").map((c) => c.preset), ["walk"]);
  assert.deepEqual(clipsFor("serpentine").map((c) => c.preset), ["march"]);
});

test("texture sizes are read from PNG and JPEG headers", () => {
  const png = Buffer.alloc(32);
  png.writeUInt32BE(0x89504e47, 0);
  png.writeUInt32BE(2048, 16);
  png.writeUInt32BE(1024, 20);
  assert.deepEqual(pixelSize(png), { width: 2048, height: 1024 });
  const jpg = Buffer.from([0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 17, 8, 4, 0, 2, 0, 0, 0, 0, 0, 0]);
  assert.deepEqual(pixelSize(jpg), { width: 512, height: 1024 });
});

test("credits come from tools/tripo/README.md's table", () => {
  assert.equal(readmeCost(README, "model"), 55);
  assert.equal(readmeCost(README, "rig"), 20);
  assert.equal(readmeCost(README, "animate"), 0);
  assert.equal(readmeCost(README, "export"), 0);
  assert.equal(readmeCost(README, "image"), 0);
});

test("the concept prompt ends with the style guide's paragraph, word for word", () => {
  const p = conceptPrompt("a hooded ranger in leather", "a dark-fantasy ruin at night", "hero", "biped", GUIDE);
  assert.ok(p.endsWith("on a plain light grey background, no ground, no text."));
  assert.match(p, /T-pose/);
  assert.match(p, /part of the one figure/);
  assert.doesNotMatch(conceptPrompt("a wolf", "a ruin", "creature", "other", GUIDE), /T-pose/);
});

function fakeCtx(credits: number, calls: string[][]): Ctx {
  const root = mkdtempSync(join(tmpdir(), "iso-art-"));
  return {
    root,
    readme: README,
    guide: GUIDE,
    print: () => {},
    tripo: (args) => {
      calls.push(args);
      if (args[0] === "balance") return { out: { credits }, minutes: 0 };
      if (args[0] === "show") return { out: parseOutput(JSON.stringify({ name: "ogre 3d model", flags: ["textured", "rigged"], rig: { type: "quadruped", status: "success" }, animations: [] }, null, 2)), minutes: 0.1 };
      return { out: { project: "p1", credits: 55, files: [] }, minutes: 2 };
    },
  };
}

test("a spending stage short of credits refuses before spending, and says what to do", () => {
  const calls: string[][] = [];
  const ctx = fakeCtx(5, calls);
  writeAsset(ctx.root, { name: "ranger", kind: "hero", theme: "ruin", description: "a ranger", body: "biped", height: 1.8, stages: {} });
  writeFileSync(join(ctx.root, "art", "iso", "ranger", "concept.png"), ""); // model needs its concept first
  assert.throws(() => main(["model", "ranger"], ctx), /costs 55 credits .* has 5: nothing was spent.*adopt/);
  assert.deepEqual(calls.map((c) => c[0]), ["balance"]);
});

test("adopt takes a project in at its step, and status shows its stages", () => {
  const calls: string[][] = [];
  const ctx = fakeCtx(5, calls);
  main(["adopt", "moose", "db7311a1-1e0d-4774-aef6-8c11882652a2", "--kind", "creature", "--height", "2.1"], ctx);
  const a = readAsset(ctx.root, "moose");
  assert.equal(a.rig, "quadruped");
  assert.equal(a.body, "other");
  assert.ok(a.stages.model?.adopted && a.stages.rig?.adopted);
  const table = statusTable([a]);
  assert.match(table, /moose\s+creature\s+db7311a1\s+-\s+adopted\s+adopted\s+-/);
  assert.throws(() => main(["adopt", "elk", "db73", "--kind", "creature"], ctx), /needs --height/);
});
