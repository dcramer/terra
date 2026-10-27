import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { holds, keep, mainFilm } from "./films.ts";

const ROOT = resolve(import.meta.dirname, "../..");
const git = (...args: string[]) => spawnSync("git", args, { cwd: ROOT, encoding: "utf8" }).stdout.trim();

test("a film holds after changes outside what it reads, not after one inside", () => {
  const reads = ["crates/terra_walker/src/", "assets/models/character/"];
  assert.ok(holds(["docs/sky.md", "crates/terra_world/src/sky.rs"], reads));
  assert.ok(!holds(["docs/sky.md", "assets/models/character/base.glb"], reads));
  assert.ok(holds([], reads));
});

test("only a clean checkout's film is kept, and main's film is the nearest one nothing it reads changed since", () => {
  const films = mkdtempSync(join(tmpdir(), "films-"));
  const out = mkdtempSync(join(tmpdir(), "film-out-"));
  try {
    writeFileSync(join(out, "checks.json"), "[]\n");
    const head = git("rev-parse", "HEAD");
    // Known from the history: the commit before the last change to the scorecard's own tools
    // is a film those changes made stale; HEAD itself is its own film.
    const lastTools = git("log", "-1", "--format=%H", "--", "tools/controller/");
    const before = git("rev-parse", `${lastTools}~1`);
    assert.match(keep(out, { commit: before, clean: false, when: "" }, films), /not kept/);
    assert.ok(!existsSync(join(films, before)));
    keep(out, { commit: before, clean: true, when: "" }, films);
    const stale = mainFilm(head, films);
    assert.ok("why" in stale && stale.why.includes(before.slice(0, 10)), JSON.stringify(stale));
    keep(out, { commit: head, clean: true, when: "" }, films);
    const found = mainFilm(head, films);
    assert.ok(!("why" in found) && found.commit === head && found.behind === 0, JSON.stringify(found));
    // A commit main never had is never main's film.
    keep(out, { commit: "0".repeat(40), clean: true, when: "" }, films);
    const still = mainFilm(head, films);
    assert.ok(!("why" in still) && still.commit === head);
  } finally {
    rmSync(films, { recursive: true, force: true });
    rmSync(out, { recursive: true, force: true });
  }
});
