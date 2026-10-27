import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { COMBAT_FILMS, CONTROLLER_FILMS, filmedThrough, holds, keep, type Kind, mainFilm } from "./films.ts";

const ROOT = resolve(import.meta.dirname, "../..");
const git = (...args: string[]) => spawnSync("git", args, { cwd: ROOT, encoding: "utf8" }).stdout.trim();

test("a film holds after changes outside what it reads, not after one inside", () => {
  const reads = ["crates/terra_walker/src/", "assets/models/character/"];
  const notRead = CONTROLLER_FILMS.notRead;
  assert.ok(holds(["docs/sky.md", "crates/terra_world/src/sky.rs"], reads, notRead));
  assert.ok(!holds(["docs/sky.md", "assets/models/character/base.glb"], reads, notRead));
  assert.ok(holds([], reads, notRead));
  // A raise or a lowering of the baseline is not what the film reads.
  assert.ok(holds(["tools/controller/baseline.tsv"], [...reads, "tools/controller/"], notRead));
  assert.ok(!holds(["tools/controller/yard.ts"], [...reads, "tools/controller/"], notRead));
  assert.ok(holds(["tools/combat/baseline.tsv"], ["tools/combat/"], COMBAT_FILMS.notRead));
});

test("a film is clean only if the checkout was clean and at one commit from start to end", () => {
  const at = (commit: string, clean: boolean) => ({ commit, clean, when: "" });
  assert.ok(filmedThrough(at("a", true), at("a", true)).clean);
  assert.ok(!filmedThrough(at("a", true), at("b", true)).clean);
  assert.ok(!filmedThrough(at("a", true), at("a", false)).clean);
  assert.ok(!filmedThrough(at("a", false), at("a", true)).clean);
});

// Each kind by its own scope list: the commit before the last change to its scorecard's own
// tools (its baseline aside) is a film those changes made stale; HEAD itself is its own film.
for (const [kind, tools] of [
  [CONTROLLER_FILMS, "tools/controller/"],
  [COMBAT_FILMS, "tools/combat/"],
] as [Kind, string][]) {
  test(`${kind.scope}: only a clean checkout's film is kept, and main's film is the nearest one nothing it reads changed since`, () => {
    const films = mkdtempSync(join(tmpdir(), "films-"));
    const out = mkdtempSync(join(tmpdir(), "film-out-"));
    const at = { ...kind, dir: films };
    try {
      writeFileSync(join(out, "checks.json"), "[]\n");
      const head = git("rev-parse", "HEAD");
      const lastTools = git("log", "-1", "--format=%H", "--", tools, `:!${kind.notRead[0]}`);
      const before = git("rev-parse", `${lastTools}~1`);
      assert.match(keep(out, { commit: before, clean: false, when: "" }, at), /not kept/);
      assert.ok(!existsSync(join(films, before)));
      keep(out, { commit: before, clean: true, when: "" }, at);
      const stale = mainFilm(at, head);
      assert.ok("why" in stale && stale.why.includes(before.slice(0, 10)) && stale.why.includes(kind.film), JSON.stringify(stale));
      keep(out, { commit: head, clean: true, when: "" }, at);
      const found = mainFilm(at, head);
      assert.ok(!("why" in found) && found.commit === head && found.behind === 0, JSON.stringify(found));
      // A commit main never had is never main's film.
      keep(out, { commit: "0".repeat(40), clean: true, when: "" }, at);
      const still = mainFilm(at, head);
      assert.ok(!("why" in still) && still.commit === head);
    } finally {
      rmSync(films, { recursive: true, force: true });
      rmSync(out, { recursive: true, force: true });
    }
  });
}
