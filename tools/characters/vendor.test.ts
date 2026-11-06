// node --test tools/characters/vendor.test.ts: what vendoring their pieces would copy (D599),
// from a private manifest made up here.
import assert from "node:assert/strict";
import { test } from "node:test";
import { plan, withLines } from "./vendor.ts";

const H = (c: string) => c.repeat(64);
const PARTS = "quaternius/modular-character-outfits-fantasy/Exports/glTF (Godot-Unreal)/Modular Parts";
const WHOLE = "quaternius/modular-character-outfits-fantasy/Exports/glTF (Godot-Unreal)/Outfits";
const priv = new Map([
  [`${PARTS}/Male_Bard_Body.gltf`, H("a")],
  [`${PARTS}/Male_Bard_Body.bin`, H("b")],
  [`${PARTS}/Male_Bard_Legs.gltf`, H("c")],
  [`${PARTS}/Male_Bard_Legs.bin`, H("d")],
  [`${PARTS}/T_Bard_BaseColor.png`, H("e")],
  [`${WHOLE}/T_Bard_BaseColor.png`, H("e")],
  [`${WHOLE}/Male_Bard.gltf`, H("f")],
  ["quaternius/modular-character-outfits-fantasy/Textures/Bard/T_Bard_2_BaseColor.png", H("1")],
  ["quaternius/bestiary-dungeon-monsters/Exports/glTF/Male_Bard_Imp.gltf", H("9")],
  [`${PARTS}/Male_Odd_Body.gltf`, H("2")],
  [`${WHOLE}/Male_Odd_Body.gltf`, H("3")],
]);
const gltf = (bin: string) => JSON.stringify({ buffers: [{ uri: bin }], images: [{ uri: "T_Bard_BaseColor.png" }] });
const read = (p: string) => (p.endsWith("Body.gltf") ? gltf("Male_Bard_Body.bin") : gltf("Male_Bard_Legs.bin"));
const V = "art/characters/quaternius/modular-outfits-fantasy";

test("a prefix vendors each piece of an outfit with what it names, never the whole outfit's file or a pack not CC0", () => {
  const p = plan(["Male_Bard_"], priv, new Map(), read, true);
  assert.deepEqual(p.faults, []);
  assert.deepEqual(p.pieces, ["Male_Bard_Body", "Male_Bard_Legs"]);
  assert.deepEqual(
    p.copy.map((c) => c.to.slice(V.length + 1)).sort(),
    ["Male_Bard_Body.bin", "Male_Bard_Body.gltf", "Male_Bard_Legs.bin", "Male_Bard_Legs.gltf", "T_Bard_2_BaseColor.png", "T_Bard_BaseColor.png"],
  );
  assert.ok(p.copy.every((c) => c.from.startsWith("quaternius/modular-character-outfits-fantasy/")), "never the Bestiary's Male_Bard_Imp");
  assert.equal(p.copy.find((c) => c.to.endsWith("T_Bard_BaseColor.png"))?.from, `${PARTS}/T_Bard_BaseColor.png`, "a texture from beside its glTF");
});

test("what is vendored already is skipped; other bytes under its name, or two pieces of one name, are refused", () => {
  const sums = new Map([[`${V}/Male_Bard_Body.gltf`, H("a")], [`${V}/Male_Bard_Body.bin`, H("b")], [`${V}/T_Bard_BaseColor.png`, H("e")]]);
  const p = plan(["Male_Bard_Body"], priv, sums, read);
  assert.deepEqual([p.copy.length, p.have.length, p.faults], [0, 3, []]);
  const changed = plan(["Male_Bard_Body"], priv, new Map([[`${V}/Male_Bard_Body.gltf`, H("0")]]), read);
  assert.match(changed.faults[0], /vendored with other bytes/);
  assert.match(plan(["Male_Odd_Body"], priv, new Map(), () => "{}").faults.join(), /^$/, "the parts folder wins over the whole outfits'");
  assert.match(plan(["Male_Nobody"], priv, new Map(), read).faults[0], /no glTF piece of that name/);
});

test("SHA256SUMS gains the new lines among its own, every line in path order", () => {
  const text = `${H("a")}  art/b.gltf\n${H("b")}  art/d.gltf\n`;
  const out = withLines(text, [{ to: "art/c.gltf", hash: H("c") }, { to: "art/a.gltf", hash: H("1") }, { to: "art/b.gltf", hash: H("9") }]);
  assert.equal(out, `${H("1")}  art/a.gltf\n${H("a")}  art/b.gltf\n${H("c")}  art/c.gltf\n${H("b")}  art/d.gltf\n`, "sorted, and a line already there keeps its hash");
  // An unsorted file comes out sorted: the order is the file's, not the order it was vendored in.
  assert.equal(withLines(`${H("b")}  art/d.gltf\n${H("a")}  art/b.gltf\n`, []), `${H("a")}  art/b.gltf\n${H("b")}  art/d.gltf\n`);
});
