// The catalogue's rows drafted for newly vendored pieces (draft_rows.ts): a row a new piece in
// its neighbours' shape, a look for an outfit drafted whole, nothing for a piece a row names,
// an accessory or a body the catalogue lacks; the text kept as written round them.
//
//   node --test tools/characters/draft_rows.test.ts

import assert from "node:assert/strict";
import { test } from "node:test";
import type { Wardrobe } from "./build_steps.ts";
import { draft, OUTFITS, withDraft } from "./draft_rows.ts";

const TEXT = `{
  "slots": ["body", "arms", "legs", "feet"],
  "bodies": {
    "male": { "head": "male_head" }
  },
  "pieces": {
    "male_ranger_body": { "body": "male", "source": "${OUTFITS}/Male_Ranger_Body.gltf", "slot": "body" }
  },
  "looks": {
    "ranger": { "body": "male" }
  },
  "pairings": []
}
`;
const w = JSON.parse(TEXT) as Wardrobe;
const FILES = [
  "Male_Ranger_Body.gltf",
  "Male_Ranger_Body.bin",
  ...["Body", "Arms", "Legs", "Feet", "Head_Hood"].map((p) => `Male_Monk_${p}.gltf`),
  "T_Monk_2_BaseColor.png",
  "Male_Monk_Acc_Belt.gltf",
  "Female_Monk_Body.gltf",
  "Male_Smith_Body.gltf",
];

test("a row a new piece, in its neighbours' shape, and a look for an outfit drafted whole", () => {
  const d = draft(w, FILES);
  assert.deepEqual(d.pieces.map((p) => p.id), ["male_monk_arms", "male_monk_body", "male_monk_feet", "male_monk_hood", "male_monk_legs", "male_smith_body"]);
  const body = d.pieces.find((p) => p.id === "male_monk_body")!.row;
  assert.equal(body.source, `${OUTFITS}/Male_Monk_Body.gltf`);
  assert.deepEqual(body.colourways, ["shipped", "2"]);
  assert.equal(d.pieces.find((p) => p.id === "male_monk_hood")!.row.slot, "headwear");
  // The smith has a body alone: no look. The monk's look names a piece a slot.
  assert.deepEqual(d.looks.map((l) => l.id), ["monk"]);
  assert.deepEqual(d.looks[0].row.pieces, { body: "male_monk_body", arms: "male_monk_arms", legs: "male_monk_legs", feet: "male_monk_feet" });
});

test("nothing drafted for a piece a row names, an accessory or a body the catalogue lacks, each said", () => {
  const d = draft(w, FILES);
  assert.ok(!d.pieces.some((p) => p.id === "male_ranger_body"));
  assert.ok(d.skipped.some((s) => s.startsWith("Male_Monk_Acc_Belt.gltf: an accessory")));
  assert.ok(d.skipped.some((s) => s.startsWith("Female_Monk_Body.gltf: no female body")));
  assert.deepEqual(draft(w, ["Male_Ranger_Body.gltf"]), { pieces: [], looks: [], skipped: [] });
});

test("the drafted rows go at the end of pieces and looks, the rest of the text as written", () => {
  const d = draft(w, FILES);
  const out = withDraft(TEXT, d);
  const parsed = JSON.parse(out);
  assert.deepEqual(Object.keys(parsed.pieces), ["male_ranger_body", ...d.pieces.map((p) => p.id)]);
  assert.deepEqual(Object.keys(parsed.looks), ["ranger", "monk"]);
  assert.ok(out.startsWith(TEXT.slice(0, TEXT.indexOf("\n  },\n  \"looks\""))), "the text before the new rows changed");
  assert.ok(out.endsWith('\n  },\n  "pairings": []\n}\n'));
  assert.equal(withDraft(TEXT, { pieces: [], looks: [], skipped: [] }), TEXT);
});
