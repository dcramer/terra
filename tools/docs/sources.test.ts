import assert from "node:assert/strict";
import { test } from "node:test";
import { priorArt, sourceFaults } from "./sources.ts";

const none = () => undefined;
const held = { notes: [], docs: [], not: {} };
const check = (file: string, text: string, read: (p: string) => string | undefined = none, h = held) => sourceFaults(file, text, read, h);

const GOOD_NOTE = [
  "# How shipped games step a character",
  "",
  "Grades: **[P]** primary, **[S]** secondary, **[C]** community, **[M]** from memory.",
  "",
  "- Jolt's CharacterVirtual sweeps, then slides [P]: [source](https://github.com/jrouwe/JoltPhysics).",
  "- Godot's CharacterBody3D moves and slides [P]: [docs](https://docs.godotengine.org/x).",
  "- A forum thread says Unreal does the same [C]: https://forums.example.com/t/1",
  "- The step height is 0.4 m in most of them [M].",
  "",
  "```",
  "curl https://example.com/in-a-fence-is-no-source",
  "```",
].join("\n");

test("a known-good note passes: every source graded, memory marked", () => {
  assert.deepEqual(check("research/2026-11-10-stepping.md", GOOD_NOTE), []);
});

test("a planted fault: a link with no grade fails, and says not to grade it from memory", () => {
  const bad = GOOD_NOTE.replace("slides [P]: [source]", "slides: [source]");
  const faults = check("research/2026-11-10-stepping.md", bad);
  assert.equal(faults.length, 1);
  assert.match(faults[0], /a source with no grade, at "- Jolt's CharacterVirtual.*D654.*do not mark it \[P\] from what you remember/);
});

test("a planted fault: a claim from memory with no [M] fails", () => {
  const bad = GOOD_NOTE.replace("in most of them [M].", "in most of them, from memory.");
  assert.match(check("research/2026-11-10-stepping.md", bad)[0], /from memory not marked \[M\].*never a ground/);
  const recalled = GOOD_NOTE.replace("in most of them [M].", "in most of them (recalled).");
  assert.equal(check("research/2026-11-10-stepping.md", recalled).length, 1);
});

test("a note from before the rule, an undated one and one held are left as written", () => {
  const bad = "See https://example.com, from memory.";
  assert.deepEqual(check("research/2026-11-08-old.md", bad), []);
  assert.deepEqual(check("research/repo-layout.md", bad), []);
  assert.deepEqual(check("research/2026-11-09-held.md", bad, none, { ...held, notes: ["research/2026-11-09-held.md"] }), []);
  assert.equal(check("research/2026-11-09-new.md", bad).length, 1);
});

const DESIGN = (art: string) => ["# Stepping", "", "What it is.", "", "## Prior art", "", art, "", "## Design", "", "Ours: https://example.com/not-in-the-section"].join("\n");
const TWO = "- Jolt [P]: https://github.com/jrouwe/JoltPhysics\n- Godot [P]: https://docs.godotengine.org/x";

test("a known-good design doc passes: two primary sources under Prior art", () => {
  assert.deepEqual(check("docs/stepping.md", DESIGN(TWO)), []);
});

test("the Prior art section ends at the next heading of its level", () => {
  assert.equal(priorArt(DESIGN(TWO)), `\n${TWO}\n`);
  assert.equal(priorArt("# A\n\n### Prior art\n\nx\n\n#### deeper\n\ny\n\n## Next\n\nz"), "\nx\n\n#### deeper\n\ny\n");
  assert.equal(priorArt("# A\n\nNo such section."), undefined);
});

test("a planted fault: a design doc with no Prior art fails, and says where a page that is no design goes", () => {
  const faults = check("docs/stepping.md", "# Stepping\n\nWe sweep a capsule.");
  assert.equal(faults.length, 1);
  assert.match(faults[0], /no Prior art section.*D654.*NOT_A_SYSTEM.*do not add it to BEFORE_D654/);
});

test("a planted fault: one primary source, or two from memory, is not two read", () => {
  assert.match(check("docs/stepping.md", DESIGN("- Jolt [P]: https://github.com/jrouwe/JoltPhysics"))[0], /has 1 primary source\(s\).*read a second one/);
  const memory = "- Jolt [P]: https://github.com/jrouwe/JoltPhysics\n- Godot does the same [M]: https://docs.godotengine.org/x";
  assert.match(check("docs/stepping.md", DESIGN(memory))[0], /has 1 primary source\(s\)/);
  assert.match(check("docs/stepping.md", DESIGN("- Jolt and Godot both sweep [P]."))[0], /has 0 primary source\(s\)/, "a grade with no link names no source");
});

test("a planted fault: an ungraded source under Prior art fails beside the count", () => {
  const faults = check("docs/stepping.md", DESIGN(`${TWO}\n- Unreal: https://dev.epicgames.com/x`));
  assert.equal(faults.length, 1);
  assert.match(faults[0], /a source with no grade/);
});

test("Prior art may be a research note it links, which must itself hold two primary sources", () => {
  const art = "The survey is [the stepping note](../research/2026-11-10-stepping.md).";
  const read = (p: string) => (p === "research/2026-11-10-stepping.md" ? GOOD_NOTE : undefined);
  assert.deepEqual(check("docs/stepping.md", DESIGN(art), read), []);
  const thin = (p: string) => (p === "research/2026-11-10-stepping.md" ? "# Stepping\n\nJolt sweeps, as I remember." : undefined);
  assert.match(check("docs/stepping.md", DESIGN(art), thin)[0], /no research note it links has two \(research\/2026-11-10-stepping\.md\)/);
});

test("pages that design no system are not asked for prior art", () => {
  for (const file of ["docs/README.md", "docs/development.md", "docs/v0-goals.md", "docs/decisions/D1.md", "AGENTS.md", "tools/docs/README.md"])
    assert.deepEqual(check(file, "# A page\n\nhttps://example.com"), [], file);
  assert.deepEqual(check("docs/naming.md", "# Naming", none, { ...held, not: { "docs/naming.md": "a guide" } }), []);
});

test("a doc from before the rule is held until its Prior art passes, then must leave the list", () => {
  const before = { ...held, docs: ["docs/stepping.md"] };
  assert.deepEqual(check("docs/stepping.md", "# Stepping\n\nWe sweep a capsule.", none, before), []);
  assert.match(check("docs/stepping.md", DESIGN(TWO), none, before)[0], /passes now: take it off BEFORE_D654/);
});
