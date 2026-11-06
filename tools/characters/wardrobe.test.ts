// The wardrobe check's files and bound stages, each on the catalogue as it is and with a
// planted fault it must catch (D632).
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { wardrobeData } from "./build_steps.ts";
import { Glb } from "./glb.ts";
import { bound, files } from "./wardrobe.ts";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const w = wardrobeData(ROOT);
const PIECE = "male_ranger_body";
const at = (piece: string) => `assets/${w.bodies[w.pieces[piece].body].pieces}/${piece}.gltf`;
const there = (f: string) => existsSync(join(ROOT, f));
const read = (f: string) => new Glb(join(ROOT, f));

/** A piece's file as it is, with `change` made to its JSON and its buffer first. */
function changed(piece: string, change: (g: any, bin: Buffer) => void): Glb {
  const path = join(ROOT, at(piece));
  const g = JSON.parse(readFileSync(path, "utf8"));
  const bin = Buffer.from(readFileSync(join(dirname(path), decodeURIComponent(g.buffers[0].uri))));
  change(g, bin);
  return new Glb(Buffer.from(JSON.stringify(g)), bin);
}

test("files: the catalogue's files are all there", () => {
  const s = files(w);
  assert.deepEqual(s.faults, []);
  assert.match(s.said, /^\d+ files$/);
});

test("files: a piece's file gone is named, with the command that builds it", () => {
  const s = files(w, (f) => f !== at(PIECE) && there(f));
  assert.equal(s.faults.length, 1);
  assert.match(s.faults[0], new RegExp(`${at(PIECE)} is missing: build it, tools/characters/build_bodies.sh --step wardrobe:`));
});

test("files: a texture a piece names, or the body's file, gone is named", () => {
  const texture = `${dirname(at(PIECE))}/${decodeURIComponent(JSON.parse(readFileSync(join(ROOT, at(PIECE)), "utf8")).images[0].uri)}`;
  const body = `assets/${w.bodies.male.model}`;
  const s = files(w, (f) => f !== texture && f !== body && there(f));
  assert.deepEqual(s.faults.map((f) => f.split(" ")[0]).sort(), [body, texture].sort());
});

test("bound: every piece on its body's skeleton", () => {
  const s = bound(w, read);
  assert.deepEqual(s.faults, []);
  assert.ok(Number(s.said.split(" ")[0]) > 100, s.said);
});

test("bound: a bind pose 1 mm off its joint's rest fails, naming the piece and how far", () => {
  const off = changed(PIECE, (g, bin) => {
    const acc = g.accessors[g.skins[0].inverseBindMatrices];
    const view = g.bufferViews[acc.bufferView];
    const x = (view.byteOffset ?? 0) + (acc.byteOffset ?? 0) + 3 * 64 + 12 * 4; // joint 3's translation
    bin.writeFloatLE(bin.readFloatLE(x) + 0.001, x);
  });
  const s = bound(w, (f) => (f === at(PIECE) ? off : read(f)));
  assert.equal(s.faults.length, 1, s.faults.join("; "));
  assert.match(s.faults[0], new RegExp(`^${PIECE}: its bind poses are 1\\.0e-3 off male's skeleton`));
});

test("bound: a joint the body has not fails, naming it", () => {
  const off = changed(PIECE, (g) => {
    g.nodes[g.skins[0].joints[2]].name = "spine_04";
  });
  const s = bound(w, (f) => (f === at(PIECE) ? off : read(f)));
  assert.deepEqual(s.faults, [`${PIECE}: joint spine_04 is not on male's skeleton`]);
});
