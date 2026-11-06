import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { Glb } from "./glb.ts";
import { check, pieces, type Piece, said, sources, THEIRS } from "./intact.ts";

const srcs = sources();
const LEGS = join(THEIRS, "modular-outfits-fantasy/Male_Ranger_Legs.gltf");

/** Their legs, read afresh, so a test may change them. */
const legs = (): Piece[] => pieces(new Glb(LEGS));

/** The check's lines for `out`, which must fail, joined. */
function fails(out: Glb | Piece[]): string {
  const r = check(out, srcs);
  assert.equal(r.ok, false, "the check passed a changed piece");
  return r.lines.join("\n");
}

test("every shipped file is its own pieces as made", () => {
  for (const dir of readdirSync(THEIRS, { withFileTypes: true }).filter((d) => d.isDirectory())) {
    for (const f of readdirSync(join(THEIRS, dir.name)).filter((f) => f.endsWith(".gltf"))) {
      const glb = new Glb(join(THEIRS, dir.name, f));
      const r = check(glb, srcs);
      assert.ok(r.ok, `${f}: ${r.lines.join("; ")}`);
      assert.equal(r.theirs.length, pieces(glb).length, `${f}: every piece is theirs`);
    }
  }
});

test("one vertex moved a millimetre in the file fails on position", () => {
  const json = readFileSync(LEGS);
  const g = JSON.parse(json.toString());
  const bin = Buffer.from(readFileSync(join(THEIRS, "modular-outfits-fantasy", g.buffers[0].uri)));
  const acc = g.accessors[g.meshes[0].primitives[0].attributes.POSITION];
  const view = g.bufferViews[acc.bufferView];
  const at = (view.byteOffset ?? 0) + (acc.byteOffset ?? 0) + 100 * (view.byteStride ?? 12);
  bin.writeFloatLE(bin.readFloatLE(at) + 0.001, at);
  const lines = fails(new Glb(json, bin));
  assert.match(lines, /position: 1 of/);
  assert.match(lines, /1\.00 mm away/);
});

test("one UV moved fails on that UV set", () => {
  const p = legs();
  p[0].uvs[0][2 * 7] += 0.01;
  assert.match(fails(p), /TEXCOORD_0: 1 of/);
});

test("one weight changed fails on weights, naming the bone", () => {
  const p = legs();
  const [bone, w] = [...p[0].weights[42]].reduce((a, b) => (b[1] > a[1] ? b : a));
  p[0].weights[42].set(bone, w - 0.05);
  assert.match(fails(p), new RegExp(`weights: 1 of .*'${bone}'`));
});

test("one turned normal fails on normal", () => {
  const p = legs();
  const n = p[0].nrm!;
  [n[30], n[31], n[32]] = [n[31], n[32], n[30]];
  assert.match(fails(p), /normal: 1 of/);
});

test("one triangle wound the other way fails on indices", () => {
  const p = legs();
  [p[0].tris[1], p[0].tris[2]] = [p[0].tris[2], p[0].tris[1]];
  assert.match(fails(p), /indices: 1 of their .* triangles missing, 1 of .* here not theirs/);
});

test("a bone's bind moved fails on bind", () => {
  const p = legs();
  const bone = [...p[0].weights[0].keys()][0];
  p[0].bind.get(bone)![13] += 0.01;
  assert.match(fails(p), new RegExp(`bind: 1 of .*'${bone}'`));
});

test("their piece renamed fails as theirs under another name", () => {
  const p = legs();
  p[0].node = p[0].mesh = "Our_Trousers";
  assert.match(fails(p), /Our_Trousers .* on their Male_Ranger_Legs/);
});

test("a re-export's order and split are still theirs", () => {
  // Vertices reversed, every triangle renumbered to match, and one vertex split in two.
  const [p] = legs();
  const n = p.count;
  const rev = <T extends Float64Array>(a: T, w: number) => {
    const out = a.slice() as T;
    for (let i = 0; i < n; i++) for (let j = 0; j < w; j++) out[(n - 1 - i) * w + j] = a[i * w + j];
    return out;
  };
  const moved: Piece = { ...p, canon: undefined, pos: rev(p.pos, 3), nrm: rev(p.nrm!, 3), uvs: p.uvs.map((u) => rev(u, 2)), weights: [...p.weights].reverse(), tris: p.tris.map((i) => n - 1 - i) };
  const dup = moved.tris[0];
  moved.pos = Float64Array.from([...moved.pos, ...moved.pos.subarray(3 * dup, 3 * dup + 3)]);
  moved.nrm = Float64Array.from([...moved.nrm!, ...moved.nrm!.subarray(3 * dup, 3 * dup + 3)]);
  moved.uvs = moved.uvs.map((u) => Float64Array.from([...u, ...u.subarray(2 * dup, 2 * dup + 2)]));
  moved.weights = [...moved.weights, new Map(moved.weights[dup])];
  moved.count = n + 1;
  moved.tris[0] = n;
  const r = check([moved], srcs);
  assert.ok(r.ok, r.lines.join("; "));
});

test("a mesh of ours beside theirs is listed as ours", () => {
  const p = legs();
  const ours: Piece = { ...p[0], node: "Strap", mesh: "Strap", pos: p[0].pos.map((x, i) => (i % 3 === 2 ? x + 0.05 : x)), canon: undefined };
  const r = check([...p, ours], srcs);
  assert.ok(r.ok, r.lines.join("; "));
  assert.deepEqual(r.ours, ["Strap"]);
  assert.deepEqual(r.theirs, ["Male_Ranger_Legs"]);
});

test("an exact cut of theirs is named as one, with how many of their triangles it keeps", () => {
  const [p] = legs();
  const half = p.tris.length / 6;
  const cut: Piece = { ...p, node: "Bare_Shin", mesh: "Bare_Shin", tris: p.tris.slice(0, 3 * half), canon: undefined };
  const r = check([cut], srcs);
  assert.ok(r.ok, r.lines.join("; "));
  assert.equal(r.cuts.length, 1);
  assert.match(r.cuts[0], new RegExp(`^Bare_Shin \\(${half.toLocaleString("en")} of their 1,128 triangles of Male_Ranger_Legs`));
});

test("a cut with one vertex moved is not a cut but theirs changed", () => {
  const [p] = legs();
  const cut: Piece = { ...p, node: "Bare_Shin", mesh: "Bare_Shin", pos: p.pos.slice(), tris: p.tris.slice(0, 300), canon: undefined };
  cut.pos[3 * cut.tris[0]] += 0.001;
  assert.match(fails([cut]), /Bare_Shin .* on their Male_Ranger_Legs/);
});

test("their UpperBody under another name is a cut of their FullBody", () => {
  const up = pieces(new Glb(join(THEIRS, "universal-base-characters/Regular_Male_UpperBody.gltf"))).find((p) => p.node === "RegularMale")!;
  const r = check([{ ...up, node: "Bare_Upper", mesh: "Bare_Upper" }], srcs);
  assert.ok(r.ok, r.lines.join("; "));
  assert.match(r.cuts[0] ?? "", /^Bare_Upper \(8,056 of their 12,128 triangles of RegularMale, universal-base-characters\/Regular_Male_FullBody\.gltf\)/);
});

test("the terminal gets a line a file, a failure's first five faults, the rest in the log (D388)", () => {
  const ok = said("x.glb", check(legs(), srcs));
  assert.match(ok.line, /^✓ intact: x\.glb: 1 of their pieces as made$/);
  const p = legs();
  for (let i = 0; i < 4; i++) p[0].uvs[0][2 * (7 + i)] += 0.01;
  p[0].pos[3 * 20] += 0.01;
  p[0].weights[42].clear();
  const n = p[0].nrm!;
  [n[30], n[31], n[32]] = [n[31], n[32], n[30]];
  [p[0].tris[1], p[0].tris[2]] = [p[0].tris[2], p[0].tris[1]];
  const r = check(p, srcs);
  const bad = said("x.glb", r);
  assert.ok(r.lines.length > 5, `the planted faults make more than five lines: ${r.lines.join("; ")}`);
  assert.equal(bad.line.split("\n").length, 7, bad.line);
  assert.match(bad.line, /^✗ intact: x\.glb\n/);
  assert.match(bad.line, /more lines in the log$/);
  for (const l of r.lines) assert.ok(bad.log.includes(l), "every fault in the log");
});
