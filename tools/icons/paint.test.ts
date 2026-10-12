import assert from "node:assert/strict";
import { test } from "node:test";
import { aspect, fingerprint, formatManifest, parseManifest, parseNotes } from "./paint.ts";

test("fingerprints a file as the sandbox does (FNV-1a, 64 bits)", () => {
  assert.equal(fingerprint(new Uint8Array()), "cbf29ce484222325");
  assert.equal(fingerprint(new TextEncoder().encode("a")), "af63dc4c8601ec8c");
});

test("reads and writes the manifest the sandbox writes, sorted", () => {
  const text = "stone painted - - -\nbow rendered models/held/bow.glb 5158e7b9be852413 1\n";
  const lines = parseManifest(text);
  assert.deepEqual(lines[0], { item: "stone", source: "painted", look: "-", fingerprint: "-", style: "-" });
  assert.equal(formatManifest(lines), "bow rendered models/held/bow.glb 5158e7b9be852413 1\nstone painted - - -\n");
});

test("reads the notes: the style, the anchors and what each item is", () => {
  const notes = parseNotes("# a comment\n@style style/x.jpg\n@anchors a b\nstone_axe: a stone axe, with: a colon\n");
  assert.equal(notes.style, "style/x.jpg");
  assert.deepEqual(notes.anchors, ["a", "b"]);
  assert.equal(notes.items.get("stone_axe"), "a stone axe, with: a colon");
});

test("asks for the aspect nearest the item's cells", () => {
  assert.equal(aspect(2, 1), "16:9");
  assert.equal(aspect(1, 2), "9:16");
  assert.equal(aspect(2, 2), "1:1");
});
