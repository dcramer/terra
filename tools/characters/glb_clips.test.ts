// glb_clips.ts on the committed mannequin: a clip put in from a copy of the file changes no
// value of the body or of any clip, a second time changes no byte, and a clip moved in the
// source arrives moved and nothing else does.
// node --test tools/characters/glb_clips.test.ts

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { Glb, posed } from "./glb.ts";
import { glbDiff } from "./glb_diff.ts";
import { joinClips, onlyClips, putClips } from "./glb_clips.ts";

const MANNEQUIN = join(import.meta.dirname, "..", "..", "assets/models/character/mannequin.glb");

/** A hash of every accessor's values, by what reads it, so files laid out differently compare. */
function values(bytes: Buffer): Map<string, string> {
  const g = new Glb(bytes);
  const memo = new Map<number, string>();
  const h = (i: number) => memo.get(i) ?? (memo.set(i, createHash("sha1").update(new Uint8Array(g.read(i).buffer)).digest("hex")), memo.get(i)!);
  const out = new Map<string, string>();
  g.gltf.meshes?.forEach((m, mi) => m.primitives.forEach((p, pi) => Object.entries(p.attributes).forEach(([k, a]) => out.set(`mesh ${mi}#${pi} ${k}`, h(a)))));
  g.gltf.animations?.forEach((a) => a.channels.forEach((c) => {
    const s = a.samplers[c.sampler];
    out.set(`${a.name} ${g.gltf.nodes[c.target.node].name} ${c.target.path}`, `${h(s.input)} ${h(s.output)}`);
  }));
  return out;
}

test("a clip put in as it is changes no value, and again no byte", () => {
  const m = readFileSync(MANNEQUIN);
  const clip = new Glb(m).clipNames()[0];
  const once = putClips(m, m, [clip]);
  assert.deepEqual(values(once), values(m));
  assert.ok(putClips(once, m, [clip]).equals(once), "the same clip written again gives the same bytes");
  assert.ok(putClips(once, once, [clip]).equals(once));
  // A clip's own copy of times it shared with another clip is all it may add.
  assert.ok(once.length <= m.length * 1.01, `the file does not grow: ${m.length} -> ${once.length}`);});

test("a clip changed in the source arrives changed, and nothing else moves", () => {
  const m = readFileSync(MANNEQUIN);
  const g = new Glb(m);
  const [a, b] = g.clipNames();
  // A source whose clip `a` is clip `b` renamed: put in, the target's `a` plays `b`.
  const json = JSON.parse(m.subarray(20, 20 + m.readUInt32LE(12)).toString());
  json.animations = json.animations.filter((x: { name: string }) => x.name !== a).map((x: { name: string }) => (x.name === b ? { ...x, name: a } : x));
  const src = Buffer.concat([m.subarray(0, 12), header(json), m.subarray(20 + m.readUInt32LE(12))]);
  const out = putClips(m, src, [a]);
  const [before, after] = [values(m), values(out)];
  for (const [k, v] of after) {
    if (k.startsWith(`${a} `)) assert.deepEqual(v, before.get(`${b} ${k.slice(a.length + 1)}`), k);
    else assert.deepEqual(v, before.get(k), k);
  }
  assert.ok(glbDiff(MANNEQUIN, MANNEQUIN).length === 0);
});

test("two clips joined play the first and then the second, at its own times", () => {
  const m = readFileSync(MANNEQUIN);
  const g = new Glb(m);
  const [a, b] = ["Jump_Start", "Jump_Loop"].map((n) => g.clip(n)!);
  const joined = new Glb(putClips(m, joinClips(m, "Joined", [a.name, b.name]), ["Joined"])).clip("Joined")!;
  assert.ok(Math.abs(joined.end - (a.end + b.end)) < 1e-4, `${joined.end} = ${a.end} + ${b.end}`);
  const pelvis = g.named("pelvis");
  const cases = [[a, 0.5 * a.end, 0.5 * a.end], [b, 0.3 * b.end, a.end + 0.3 * b.end], [b, b.end, a.end + b.end]] as const;
  for (const [clip, at, t] of cases) {
    const want = posed(g, clip, at)[pelvis];
    const got = posed(g, joined, t)[pelvis];
    for (const k of [0, 1, 2]) assert.ok(Math.abs(want.t[k] - got.t[k]) < 1e-4, `${clip.name} at ${at.toFixed(2)} s: ${want.t} against ${got.t}`);
  }
});

test("a pause before a part holds its first pose that long, and the part plays after it", () => {
  const m = readFileSync(MANNEQUIN);
  const g = new Glb(m);
  const a = g.clip("Jump_Start")!;
  const led = new Glb(putClips(m, joinClips(m, "Led", ["0.2", a.name]), ["Led"])).clip("Led")!;
  assert.ok(Math.abs(led.end - (0.2 + a.end)) < 1e-4, `${led.end} = 0.2 + ${a.end}`);
  const pelvis = g.named("pelvis");
  for (const [at, t] of [[0, 0], [0, 0.1], [0, 0.2], [0.5 * a.end, 0.2 + 0.5 * a.end]]) {
    const want = posed(g, a, at)[pelvis];
    const got = posed(g, led, t)[pelvis];
    for (const k of [0, 1, 2]) assert.ok(Math.abs(want.t[k] - got.t[k]) < 1e-4, `at ${t} s: ${got.t} against ${want.t}`);
  }
  assert.throws(() => joinClips(m, "Bad", [a.name, "0.1"]), /ends on a pause/);
});

test("a pack cut to two clips puts them in as the whole pack does, and holds nothing else", () => {
  const m = readFileSync(MANNEQUIN);
  const names = ["Jump_Start", "Jump_Loop"];
  const cut = onlyClips(m, names);
  const c = new Glb(cut);
  assert.deepEqual(c.clipNames().sort(), [...names].sort());
  assert.equal(c.gltf.meshes, undefined, "no mesh");
  assert.ok(cut.length < m.length / 4, `${m.length} -> ${cut.length} bytes`);
  const whole = putClips(m, joinClips(m, "Joined", names), ["Joined"]);
  const fromCut = putClips(m, joinClips(cut, "Joined", names), ["Joined"]);
  assert.deepEqual(values(fromCut), values(whole));
  assert.throws(() => onlyClips(m, ["No_Such_Clip"]), /No_Such_Clip/);
});

/** A JSON chunk's header and body, padded. */
function header(json: unknown): Buffer {
  let j = Buffer.from(JSON.stringify(json));
  if (j.length % 4) j = Buffer.concat([j, Buffer.alloc(4 - (j.length % 4), 0x20)]);
  const h = Buffer.alloc(8);
  h.writeUInt32LE(j.length, 0);
  h.writeUInt32LE(0x4e4f534a, 4);
  return Buffer.concat([h, j]);
}
