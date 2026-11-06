// The pipeline's tests, with no Tripo and no GPU: a small walker built here, whose answers are
// known by construction (its feet plant and slide back at 1.2 m/s, it faces +Z, it is 1.8 m
// tall), measured, checked, turned and scaled; the ledger, the costs, the prompt and refusals.

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { check, measure, skeleton } from "./check.ts";
import { type Glb, clips, pixelSize, readGlb, sample, writeGlb } from "./gltf.ts";
import { type Ctx, ROOT, conceptPrompt, main, parseOutput } from "./iso-art.ts";
import { budget, clipsFor } from "./kinds.ts";
import { readAsset, readmeCost, statusTable, writeAsset } from "./ledger.ts";
import { centre, cut, inPlace, loop, place, renameClips } from "./normalize.ts";

const README = readFileSync(join(ROOT, "tools", "tripo", "README.md"), "utf8");
const GUIDE = readFileSync(join(ROOT, "docs", "art-style.md"), "utf8");

/** A GLB from plain arrays: a hips joint 1 m up with two feet and a head, one point skinned
 * to each, and the clips given as [name, [[node, path, times, values]]]. `rooted` puts it as
 * Tripo's V3 rig has it: a `root` joint (node 6) on the ground above the hips, turned Z-up (a
 * quarter about X), so the hips' and feet's keys are in its frame (`zUp`). */
function walker(anims: [string, [number, string, number[], number[][]][]][], sceneRotation?: number[], rooted = false): Glb {
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
  const ibm = add([...rest, [0, 0, 0]].map(([x, y, z]) => [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, -x, -y, -z, 1]), "MAT4");
  const at = (p: number[]) => (rooted ? zUp(p) : p);
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
      { name: "armature", children: [rooted ? 6 : 1], ...(sceneRotation ? { rotation: sceneRotation } : {}) },
      { name: "hips", translation: at([0, 1, 0]), children: [2, 3, 4] },
      { name: "foot_l", translation: at([0.1, -1, 0]) },
      { name: "foot_r", translation: at([-0.1, -1, 0]) },
      { name: "head", translation: at([0, 0.8, 0]) },
      { name: "body", mesh: 0, skin: 0 },
      ...(rooted ? [{ name: "root", rotation: [-Math.SQRT1_2, 0, 0, Math.SQRT1_2], children: [1] }] : []),
    ],
    meshes: [{ primitives: [{ attributes: { POSITION: pos, JOINTS_0: joints, WEIGHTS_0: weights }, indices }] }],
    skins: [{ joints: rooted ? [1, 2, 3, 4, 6] : [1, 2, 3, 4], inverseBindMatrices: ibm }],
    animations,
    accessors,
    bufferViews: views,
    buffers: [{ byteLength: offset }],
  };
  return readGlb(writeGlb({ doc, bin: Buffer.concat(chunks) }));
}

/** A point in the world as a key in the Z-up root's frame: (x, y, z) is (x, -z, y) there. */
const zUp = ([x, y, z]: number[]) => [x, -z, y];

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
  assert.deepEqual(renameClips(glb, clipsFor("biped")), ["run", "attack"]);
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

// Tripo's V3 rig as the knight came (2026-11-05): a Z-up `root` joint on the ground over the
// pelvis, kept still by Tripo's in-place export while the pelvis walks forward 1.2 m/s.
const travelling = (k: number[][], t: number[]) => k.map((p, i) => zUp([p[0], p[1], p[2] + 1.2 * t[i]]));
const ROOTED_WALK: [string, any[]] = ["walk", [
  [1, "translation", [0, 0.25, 0.5, 0.75, 1], travelling([[0, 1, 0], [0, 1.03, 0], [0, 1, 0], [0, 1.03, 0], [0, 1, 0]], [0, 0.25, 0.5, 0.75, 1])],
  [2, "translation", footL[2], footL[3].map(zUp)],
  [3, "translation", footR[2], footR[3].map(zUp)],
]];

test("the root joint on the ground over the pelvis is no foot", () => {
  const glb = walker([IDLE, ROOTED_WALK], undefined, true);
  const sk = skeleton(glb)!;
  assert.deepEqual(sk.feet.map((j) => glb.doc.nodes[j].name).sort(), ["foot_l", "foot_r"]);
  assert.equal(glb.doc.nodes[sk.body].name, "hips");
});

test("export takes the pelvis's travel out of a walk Tripo kept in place only at its root", () => {
  const glb = walker([IDLE, ROOTED_WALK], undefined, true);
  const before = measure(glb).clips.walk.motion!;
  assert.ok(before.travel > 1, `Tripo's walk travels ${before.travel} m`);
  assert.deepEqual(inPlace(glb, clipsFor("biped")), ["walk"]);
  const after = measure(readGlb(writeGlb(glb))).clips.walk.motion!;
  assert.ok(after.travel < 0.01, `in place: ${after.travel} m`);
  assert.ok(after.gap < 0.01, `the loop closes: ${after.gap}`);
  assert.ok(Math.abs(after.speed - 1.2) < 0.05, `the planted foot still says 1.2 m/s: ${after.speed}`);
  assert.ok(Math.abs(after.heading) < 2, `toward +Z: ${after.heading}`);
  const hips = clips(readGlb(writeGlb(glb))).find((c) => c.name === "walk")!.channels[0];
  assert.ok(Math.abs(hips.values[1][2] - 1.03) < 1e-5, "the bob stays, up being the root frame's z");
});

test("export cuts a clip to its window, keeping the poses in it", () => {
  const swing: [string, any[]] = ["attack", [[4, "translation", [0, 1, 2, 3], [[0, 0.8, 0], [0, 0.8, 0.2], [0, 0.8, 0.6], [0, 0.8, 0]]]]];
  const glb = walker([IDLE, swing]);
  const original = clips(glb).find((c) => c.name === "attack")!;
  assert.deepEqual(cut(glb, [{ name: "attack", preset: "slash", loop: false, seconds: [0.4, 4], window: [0.5, 2.5] }]), ["attack"]);
  const cutClip = clips(readGlb(writeGlb(glb))).find((c) => c.name === "attack")!;
  assert.equal(cutClip.duration, 2);
  for (const t of [0, 0.7, 1.5, 2]) assert.deepEqual(sample(cutClip.channels[0], t).map((v) => v.toFixed(5)), sample(original.channels[0], t + 0.5).map((v) => v.toFixed(5)), `at ${t} s`);
});

// A run as Tripo's came (2026-11-05): 0.8 s holding one and a half strides of 0.5 s, so its
// ends are a half stride apart, each foot where the other was.
const STRIDE = 0.5;
const keys = Array.from({ length: 49 }, (_, k) => k / 60);
const foot = (x: number, phase: number) => keys.map((t) => {
  const a = (2 * Math.PI * t) / STRIDE + phase;
  return [x, -1 + 0.15 * Math.max(0, Math.sin(a)), 0.4 * Math.cos(a)];
});
const RUN: [string, any[]] = ["run", [[2, "translation", keys, foot(0.1, 0)], [3, "translation", keys, foot(-0.1, Math.PI)]]];

test("export cuts a run to the stride whose ends meet and closes it, and keeps a clip that loops whole", () => {
  const turn = (deg: number) => [0, Math.sin((deg * Math.PI) / 360), 0, Math.cos((deg * Math.PI) / 360)];
  // The head turns 10 degrees over the walk and does not come back: no joint moves for it, so
  // the walk is one cycle, but its last key is not its first.
  const glb = walker([IDLE, ["walk", [footL, footR, [4, "rotation", [0, 1], [turn(0), turn(10)]]]], RUN]);
  const before = measure(glb).clips.run.motion!;
  assert.ok(before.gap > 0.3, `the run's ends are a half stride apart: ${before.gap}`);
  const looped = Object.fromEntries(loop(glb, clipsFor("biped")).map((l) => [l.name, l]));
  assert.ok(Math.abs(looped.run.to - looped.run.from - STRIDE) < 0.01, `one stride: ${looped.run.from}-${looped.run.to}`);
  assert.ok(looped.run.gap < 0.01, `its ends meet: ${looped.run.gap}`);
  assert.deepEqual([looped.walk.from, looped.walk.to], [0, 1], "the walk stays whole");
  const after = readGlb(writeGlb(glb));
  const m = measure(after);
  assert.ok(Math.abs(m.clips.run.seconds - STRIDE) < 0.01 && m.clips.run.motion!.gap < 0.001, `run ${m.clips.run.seconds} s, gap ${m.clips.run.motion!.gap}`);
  const head = clips(after).find((c) => c.name === "walk")!.channels[2];
  assert.ok(head.values.at(-1)!.every((v, c) => Math.abs(v - head.values[0][c]) < 1e-6), `the head turns back: ${head.values.at(-1)}`);
  // A clip with no cycle in it, the head rising 1 m a second, is left for `check` to fail.
  const rising = walker([["run", [[4, "translation", [0, 1], [[0, 0.8, 0], [0, 1.8, 0]]]]]]);
  assert.deepEqual(loop(rising, clipsFor("biped")), []);
});

test("export puts a run cut from mid-clip back over the origin", () => {
  // Tripo's run gathers speed from a stand, so its in-place cut, a straight line end to end,
  // leaves the pelvis ahead in the middle, where the loop's stride is cut from.
  const hips = keys.map((t) => [0, 1, 1.5 * t * t]);
  const glb = walker([IDLE, ["run", [[1, "translation", keys, hips], ...RUN[1]]]]);
  const specs = clipsFor("biped");
  inPlace(glb, specs);
  loop(glb, specs);
  const off = (g: Glb) => Object.fromEntries(check(measure(g), "hero", "biped", 1.8).map((r) => [r.check, r]))["run centred"];
  assert.equal(off(glb).ok, false, `before: ${off(glb).value}`);
  const moved = centre(glb, specs);
  assert.ok(moved[0].off > 0.03, `it was off: ${moved[0].off}`);
  const after = readGlb(writeGlb(glb));
  assert.ok(off(after).ok, `after: ${off(after).value}`);
  assert.ok(measure(after).clips.run.motion!.gap < 0.001, "still closed");
});

test("a loop whose pose meets at the seam but not its velocity fails its seam", () => {
  // The head rises and falls once a second: the last frame is the first, but it arrives going
  // down and leaves going up.
  const t = Array.from({ length: 31 }, (_, k) => k / 30);
  const bob: [number, string, number[], number[][]] = [4, "translation", t, t.map((s) => [0, 0.8 + 0.3 * Math.sin(Math.PI * s), 0])];
  const by = Object.fromEntries(check(measure(walker([["idle", [bob]]])), "hero", "biped", 1.8).map((r) => [r.check, r]));
  assert.ok(by["idle loops"].ok, `the pose meets: ${by["idle loops"].value}`);
  assert.equal(by["idle seam"].ok, false, `the velocity does not: ${by["idle seam"].value}`);
  // A last key snapped to the first pose: the gap is 0, the seam's step is not.
  const snap: [number, string, number[], number[][]] = [4, "translation", [0, 0.95, 1], [[0, 0.8, 0], [0, 0.8, 0.5], [0, 0.8, 0]]];
  const s = Object.fromEntries(check(measure(walker([["idle", [snap]]])), "hero", "biped", 1.8).map((r) => [r.check, r]));
  assert.ok(s["idle loops"].ok && !s["idle seam"].ok, `snap: ${s["idle loops"].value}, ${s["idle seam"].value}`);
});

test("the checks name what is missing and pass what is right", () => {
  const glb = walker([IDLE, WALK, FALL]);
  renameClips(glb, clipsFor("biped"));
  const results = check(measure(glb), "hero", "biped", 1.8);
  const by = Object.fromEntries(results.map((r) => [r.check, r]));
  for (const ok of ["clip idle", "clip walk", "walk in place", "walk speed", "walk facing", "walk loops", "walk seam", "walk centred", "idle seam", "death ends down", "height", "on the ground", "triangles"]) assert.ok(by[ok].ok, `${ok}: ${by[ok].value}`);
  for (const bad of ["clip run", "clip attack", "textures"]) assert.equal(by[bad].ok, false, bad);
  // A quadruped gets the one walk Tripo has for it.
  assert.deepEqual(clipsFor("quadruped").map((c) => c.preset), ["walk"]);
  assert.deepEqual(clipsFor("serpentine").map((c) => c.preset), ["march"]);
});

test("a budget is a triangle a pixel of the box at the closest camera, between a floor and the kind's cap", () => {
  // The hero is 1.85 m at 0.2 of 1080 pixels: 216 pixels, so a 1.85 m square is 216 squared.
  assert.equal(budget("prop", { height: 1.85, width: 1.85, depth: 0.1 }), 8000, "216 squared is over a prop's cap");
  assert.equal(budget("scenery", { height: 1.85, width: 1.85, depth: 0.1 }), 20_000, "216 squared is over scenery's cap");
  assert.equal(budget("hero", { height: 1.85, width: 0.5, depth: 0.4 }), Math.round((216 * 216 * 0.5) / 1.85 / 100) * 100, "a hero by its front");
  assert.equal(budget("creature", { height: 0.3, width: 0.6, depth: 1.2 }), Math.round((216 * 216 * 0.72) / 1.85 / 1.85 / 100) * 100, "a low long body by its top");
  assert.equal(budget("prop", { height: 0.17, width: 0.15, depth: 0.2 }), 1000, "a skull keeps the floor");
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
