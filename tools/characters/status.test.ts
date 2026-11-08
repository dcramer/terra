// node --test tools/characters/status.test.ts: the status of character work (D625), from records
// and criteria made up here, with git answered by hand.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { type Criteria, criteria, format, freshness, live, offered, type Git, rows, where } from "./status.ts";
import { record, type Record, records } from "./record.ts";
import { type Wardrobe, wardrobeData } from "./build_steps.ts";
import type { Pairing } from "./pairings.ts";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const NOW = Date.parse("2026-11-05T12:00:00Z");
const run = (commit: string, extra: Partial<Record> = {}): Record => ({
  check: "wardrobe",
  time: "2026-11-05T10:00:00Z",
  commit,
  dirty: [],
  worktree: "/w",
  pass: true,
  stages: [
    { name: "vendored", ok: true, said: "161 files of theirs" },
    { name: "pairings", ok: false, said: "3 faults: male_a|male_b has no row" },
  ],
  failures: ["pairings: male_a|male_b has no row"],
  log: "target/check/wardrobe.log",
  ...extra,
});
const C: Criteria = {
  checks: { wardrobe: { command: "node tools/characters/wardrobe.ts", takes: "80 s", owns: ["assets/models/character/", "tools/characters/*.py"] } },
  criteria: [
    { id: "guard.vendored", check: "wardrobe", stage: "vendored", bar: "every file" },
    { id: "pairings", check: "wardrobe", stage: "pairings", bar: "a row each" },
    { id: "intact", check: "wardrobe", stage: "intact", bar: "as made" },
  ],
  tasks: [
    { what: "the sheet", files: ["tools/characters/status.ts#where"], check: "the desk" },
    { what: "rows for every pairing", files: ["tools/characters/pairings.ts"], check: "node tools/characters/pairings.ts", criterion: "pairings" },
  ],
};
// A git that holds commit "old" and "new" (HEAD), with `touched` commits to the owned files since "old".
const gitWith =
  (touched: number): Git =>
  (args) => {
    if (args[0] === "merge-base") return ["old", "new"].includes(args[2]) ? "" : "\u0000";
    if (args[0] === "rev-list") return args[2] === "new..HEAD" ? "0" : String(touched);
    return "";
  };

test("a record is fresh when nothing it reads changed since, and says why when it is stale", () => {
  const ch = C.checks.wardrobe;
  assert.equal(freshness(ch, [run("new")], gitWith(0), [], NOW).stale, null);
  assert.match(freshness(ch, [run("old")], gitWith(2), [], NOW).stale ?? "", /2 commits to its files since/);
  assert.match(freshness(ch, [run("old")], gitWith(0), ["tools/characters/fit_gear.py"], NOW).stale ?? "", /fit_gear\.py changed here/, "a glob in owns");
  assert.equal(freshness(ch, [run("old")], gitWith(0), ["docs/README.md"], NOW).stale, null, "a file it does not read");
  assert.match(freshness(ch, [run("new", { dirty: ["assets/models/character/wardrobe.json"] })], gitWith(0), [], NOW).stale ?? "", /ran on uncommitted/);
  // An unlanded run newer than a landed one: the landed one stands.
  const f = freshness(ch, [run("elsewhere"), run("new")], gitWith(0), [], NOW);
  assert.equal(f.record?.commit, "new");
  assert.match(freshness(ch, [run("elsewhere")], gitWith(0), [], NOW).stale ?? "", /does not hold/);
  assert.equal(freshness(ch, [], gitWith(0), [], NOW).record, null);
  assert.equal(freshness(ch, [run("new")], gitWith(0), [], NOW).age, "2 h ago");
});

test("each criterion reads its stage: passing, failing, stale or never reached", () => {
  const fresh = { wardrobe: freshness(C.checks.wardrobe, [run("new")], gitWith(0), [], NOW) };
  assert.deepEqual(
    rows(C, fresh, null).map((r) => [r.id, r.mark]),
    [["guard.vendored", "✓"], ["pairings", "✗"], ["intact", "?"]],
  );
  const stale = { wardrobe: freshness(C.checks.wardrobe, [run("old")], gitWith(1), [], NOW) };
  assert.deepEqual(rows(C, stale, null).map((r) => r.mark), ["?", "?", "?"], "a stale record proves nothing");
});

test("the build passes on its known faults alone", () => {
  const c: Criteria = { ...C, checks: { build: { ...C.checks.wardrobe } }, criteria: [{ id: "build", check: "build", bar: "no failure but the known", known: [{ match: "Sprint_Loop", why: "known" }] }] };
  const build = (failures: string[]) => ({ build: freshness(c.checks.build, [run("new", { check: "build", pass: false, stages: [], failures })], gitWith(0), [], NOW) });
  assert.equal(rows(c, build(["✗ male: Sprint_Loop foot slides 2.7 cm"]), null)[0].mark, "✓");
  const r = rows(c, build(["✗ male: Sprint_Loop foot slides 2.7 cm", "✗ rig: hand_l rolls 40°"]), null)[0];
  assert.equal(r.mark, "✗");
  assert.match(r.said, /2 failures, 1 known: ✗ rig: hand_l/);
});

test("the status puts a failing criterion's task first, resolves symbols to lines and names the next command, in 40 lines", () => {
  const fresh = { wardrobe: freshness(C.checks.wardrobe, [run("new")], gitWith(0), [], NOW) };
  const text = format(C, fresh, rows(C, fresh, null), "new");
  const lines = text.split("\n");
  assert.ok(lines.length <= 40);
  assert.match(lines[0], /^CHARACTERS 1 of 3 at their bar/);
  const first = lines.findIndex((l) => l.startsWith("1. "));
  assert.equal(lines[first], "1. rows for every pairing");
  assert.match(text, /status\.ts:\d+ where/);
  assert.match(text, /^Next: the task for pairings, or read the log target\/check\/wardrobe\.log/m);
  const staleText = format(C, { wardrobe: freshness(C.checks.wardrobe, [], gitWith(0), [], NOW) }, rows(C, {}, null), "new");
  assert.match(staleText, /^Next: node tools\/characters\/wardrobe\.ts {2}\(its record is missing\)/m);
});

test("where names a symbol's line, and says so when it has gone", () => {
  const line = readFileSync(join(ROOT, "tools/characters/status.ts"), "utf8").split("\n").findIndex((l) => l.startsWith("export function where")) + 1;
  assert.equal(where("tools/characters/status.ts#where"), `tools/characters/status.ts:${line} where`);
  assert.equal(where("tools/characters/status.ts#nothing_here"), "tools/characters/status.ts (no nothing_here)");
  assert.equal(where("tools/characters/status.ts"), "tools/characters/status.ts");
});

test("the committed criteria name only checks they define and files that are there", () => {
  const c = criteria();
  for (const k of c.criteria) if (k.check) assert.ok(c.checks[k.check], `${k.id}: no check ${k.check}`);
  for (const t of c.tasks) for (const f of t.files) assert.doesNotMatch(where(f), /\((missing|no .*)\)$/, `${t.what.slice(0, 40)}: ${f}`);
});

test("a record is kept per machine, newest first, the last 20", () => {
  process.env.TERRA_CHARACTER_RECORDS = mkdtempSync(join(tmpdir(), "records-"));
  for (let i = 0; i < 25; i++) record("demo", { pass: i % 2 === 0, stages: [], failures: [`f${i}`] });
  const r = records("demo");
  assert.equal(r.length, 20);
  assert.deepEqual(r[0].failures, ["f24"]);
  assert.match(r[0].commit, /^[0-9a-f]{40}$/);
  delete process.env.TERRA_CHARACTER_RECORDS;
});

// The live readings on the catalogue as it is, and each with a planted fault it must catch (D632).
type Look = Wardrobe["looks"][string];
const W = () => wardrobeData(ROOT) as Wardrobe & { pairings: Pairing[] };
const LIVE = criteria(ROOT).criteria.filter((k) => k.live);
const mark = (w: Wardrobe, id: string) => rows({ ...C, criteria: LIVE.filter((k) => k.id === id) }, {}, w)[0];

test("looks.hair: every look wears hair; one without, or with a beard in its hair slot, fails", () => {
  assert.equal(mark(W(), "looks.hair").mark, "✓");
  const faults = [(l: Look) => delete l.pieces.hair, (l: Look) => (l.pieces.hair = "male_beard_full")];
  for (const fault of faults) {
    const w = W();
    fault(w.looks.ranger);
    const n = Object.values(w.looks).filter((l) => offered().includes(l.body)).length;
    const r = mark(w, "looks.hair");
    assert.equal(r.mark, "✗");
    assert.ok(r.said.startsWith(`${n - 1} of ${n} looks wear hair`), r.said);
  }
});

test("a body not yet offered is said, not judged: its hairless look passes until the creator offers it", () => {
  const w = W();
  w.bodies.unoffered = w.bodies.male;
  w.looks.bald = { ...w.looks.ranger, body: "unoffered", pieces: { ...w.looks.ranger.pieces } };
  delete w.looks.bald.pieces.hair;
  assert.match(live("hair", w).said, /0 of \d+ on bodies not yet offered$/);
  assert.equal(live("hair", w).ok, true);
  assert.equal(live("hair", w, [...offered(), "unoffered"]).ok, false);
  assert.match(live("figures", w).said, /unoffered \(not yet offered\)/);
});

test("variants: the catalogue's figures within the bar; none (every pairing refused) or too many (nothing refused, the shoes thrice) fail", () => {
  assert.equal(mark(W(), "variants").mark, "✓");
  const none = W();
  for (const r of none.pairings) r.verdict = "refused";
  assert.equal(mark(none, "variants").mark, "✗");
  assert.match(mark(none, "variants").said, /^male 0 figures/);
  const many = W();
  many.pairings = many.pairings.filter((r) => r.verdict !== "refused");
  // Nothing refused, and every pair of shoes in three colours of their own: three times as many.
  for (const [name, p] of Object.entries(many.pieces)) for (let k = 1; k < 3 && p.slot === "feet"; k++) many.pieces[`${name}_${k}`] = p;
  const r = mark(many, "variants");
  assert.equal(r.mark, "✗");
  assert.ok(Number(r.said.split(" ")[1]) > 100000, r.said);
});
