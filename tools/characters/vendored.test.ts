import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { Glb } from "./glb.ts";
import { check, sources } from "./intact.ts";
import { manifest, matchesPrivate, privateRoot, reshapingSteps, VENDORED, vendored } from "./vendored.ts";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SUMS = manifest(readFileSync(join(ROOT, VENDORED, "SHA256SUMS"), "utf8"));
const LEGS = `${VENDORED}/modular-outfits-fantasy/Male_Ranger_Legs.gltf`;
const LICENCE = `${VENDORED}/modular-outfits-fantasy/License_Source.txt`;

/** A repository of two of their files and their SHA256SUMS lines, to change in a test. */
function copy(): string {
  const at = mkdtempSync(join(tmpdir(), "vendored-"));
  mkdirSync(join(at, VENDORED, "modular-outfits-fantasy"), { recursive: true });
  for (const f of [LEGS, LICENCE, `${VENDORED}/.gitattributes`]) cpSync(join(ROOT, f), join(at, f));
  writeFileSync(join(at, VENDORED, "SHA256SUMS"), [LEGS, LICENCE].map((f) => `${SUMS.get(f)}  ${f}\n`).join(""));
  return at;
}

test("their files as shipped pass", () => {
  const at = copy();
  assert.deepEqual(vendored(at).faults, []);
  rmSync(at, { recursive: true });
});

test("one byte of a file of theirs changed fails, naming it and what to do", () => {
  const at = copy();
  const text = readFileSync(join(at, LEGS));
  text[200] ^= 1;
  writeFileSync(join(at, LEGS), text);
  const faults = vendored(at).faults.join("\n");
  assert.match(faults, /Male_Ranger_Legs\.gltf is not as shipped/);
  assert.match(faults, /git checkout --/);
  rmSync(at, { recursive: true });
});

test("a file not in SHA256SUMS, or one listed and gone, fails", () => {
  const at = copy();
  writeFileSync(join(at, VENDORED, "modular-outfits-fantasy/Male_Ranger_Legs_Fixed.gltf"), "{}");
  rmSync(join(at, LICENCE));
  const faults = vendored(at).faults.join("\n");
  assert.match(faults, /Legs_Fixed\.gltf is not in .*SHA256SUMS/);
  assert.match(faults, /License_Source\.txt is in SHA256SUMS but not there/);
  rmSync(at, { recursive: true });
});

test("their folder losing its line-ending guard fails", () => {
  const at = copy();
  writeFileSync(join(at, VENDORED, ".gitattributes"), "");
  assert.match(vendored(at).faults.join("\n"), /lost its "\* -text"/);
  rmSync(at, { recursive: true });
});

test("a pack that is not CC0 is refused", () => {
  const at = copy();
  mkdirSync(join(at, VENDORED, "bestiary-dungeon-monsters"));
  writeFileSync(join(at, VENDORED, "bestiary-dungeon-monsters/Imp.gltf"), "{}");
  assert.match(vendored(at).faults.join("\n"), /bestiary-dungeon-monsters is not a CC0 pack/);
  rmSync(at, { recursive: true });
});

test("SHA256SUMS rewritten to bless a changed file fails against the private manifest", () => {
  const priv = new Map([
    ["quaternius/modular-character-outfits-fantasy/Exports/glTF (Godot-Unreal)/Modular Parts/Male_Ranger_Legs.gltf", SUMS.get(LEGS)!],
  ]);
  assert.deepEqual(matchesPrivate(new Map([[LEGS, SUMS.get(LEGS)!]]), priv), []);
  const blessed = new Map([[LEGS, "0".repeat(64)]]);
  assert.match(matchesPrivate(blessed, priv).join("\n"), /no Male_Ranger_Legs\.gltf the private repository has/);
  assert.match(matchesPrivate(new Map([[`${VENDORED}/modular-outfits-fantasy/Ours.gltf`, "0".repeat(64)]]), priv).join("\n"), /no Ours\.gltf in the private/);
});

test("every vendored file is the private repository's own, where this machine has it", (t) => {
  const at = privateRoot();
  if (!at) return t.skip("no private repository here");
  assert.deepEqual(matchesPrivate(SUMS, manifest(readFileSync(join(at, "SHA256SUMS"), "utf8"))), []);
});

const theirs = (script: string, ...args: string[]) => spawnSync("python3", [join(ROOT, "tools/characters/theirs.py"), join(ROOT, script), ...args], { cwd: ROOT, encoding: "utf8" });

test("a script that reshapes meshes is refused their files, in any spelling, saying what to do", () => {
  for (const arg of [`${VENDORED}/universal-base-characters/Regular_Male_FullBody.gltf`, `x.gltf+${ROOT}${LEGS}`, "--in=assets/models/character/male/male_ranger_body.gltf", "tools/characters/out/quaternius-male-bare-legs.glb"]) {
    const r = theirs("tools/characters/convert_rig.py", arg, "x", "1.8");
    assert.equal(r.status, 2, `convert_rig.py was let through ${arg}`);
    assert.match(r.stderr, /their parts stay as made \(D599\)/i);
    assert.match(r.stderr, /repaint it/);
  }
  assert.equal(theirs("tools/props/refine.py", LEGS).status, 2, "a prop tool that decimates was let through");
});

test("a script that only moves ours, or reshapes nothing, is let through", () => {
  assert.equal(theirs("tools/characters/export_pieces.py", "male").status, 0);
  assert.equal(theirs("tools/characters/fit_gear.py", "tools/characters/out/quaternius-male-ranger.glb", "small_pack").status, 0);
  assert.equal(theirs("tools/characters/convert_rig.py", "art/characters/tripo/body.glb", "base", "1.8").status, 0);
});

test("the stretch refuses when its script was given their files", () => {
  const code = "import sys; sys.argv=['blender','--','art/characters/quaternius/x.gltf']; import stretch; stretch.apply({}, [[0,0,0]])";
  const r = spawnSync("python3", ["-c", code], { cwd: join(ROOT, "tools/characters"), encoding: "utf8" });
  if (r.stderr.includes("No module named 'numpy'")) return;
  assert.equal(r.status, 2);
  assert.match(r.stderr, /stretch\.py: refused/);
});

test("theirs.py finds the stretch and the clipping fixer's users, and spares none it should not", () => {
  const out = execFileSync("python3", [join(ROOT, "tools/characters/theirs.py"), "--list"], { encoding: "utf8" });
  for (const s of ["convert_rig.py", "morph.py", "carry_gear.py", "shell_garment.py", "hung_garment.py"]) assert.match(out, new RegExp(`RESHAPES tools/characters/${s}`));
  assert.doesNotMatch(out, /STALE/);
});

test("a build step that reads their files and runs a reshaper is named", () => {
  const steps = [
    { name: "wardrobe:male", inputs: [LEGS], run: "$blender export_pieces.py male" },
    { name: "bad", inputs: [LEGS], run: "$blender convert_rig.py x" },
    { name: "base", inputs: ["art/characters/tripo/body.glb"], run: "$blender convert_rig.py x" },
  ];
  const faults = reshapingSteps(steps, ["tools/characters/convert_rig.py"], (f) => f.startsWith(VENDORED));
  assert.equal(faults.length, 1);
  assert.match(faults[0], /build step bad reads their files and runs convert_rig\.py/);
});

test("a game piece with one vertex moved is no longer theirs", () => {
  const file = join(ROOT, "assets/models/character/male/male_ranger_legs.gltf");
  const json = readFileSync(file);
  const g = JSON.parse(json.toString());
  const bin = Buffer.from(readFileSync(join(ROOT, "assets/models/character/male", g.buffers[0].uri)));
  assert.equal(check(new Glb(json, bin), sources()).ok, true, "the piece as built is theirs");
  const acc = g.accessors[g.meshes[0].primitives[0].attributes.POSITION];
  const view = g.bufferViews[acc.bufferView];
  const at = (view.byteOffset ?? 0) + (acc.byteOffset ?? 0) + 50 * (view.byteStride ?? 12);
  bin.writeFloatLE(bin.readFloatLE(at) + 0.001, at);
  assert.equal(check(new Glb(json, bin), sources()).ok, false, "a moved vertex passed");
});
