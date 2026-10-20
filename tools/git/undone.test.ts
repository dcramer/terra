// The undo warning on a scratch repository: a stale copy over another's fresh commit is found;
// an edit of it, a move, and a commit that names what it undoes are not.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { undone } from "./undone.ts";

const BASE = ["fn shade() {", "    let colour = mix(colour, SNOW, snow);", "    let depth = weather.r;", "}"];
const FRESH = [
  "fn shade() {",
  "    let facing = smoothstep(0.2, 0.8, up);",
  "    let top = mix(SNOW, SNOW_TOP, facing);",
  "    colour = mix(colour, top, snow);",
  "    let depth = weather.r;",
  "}",
];

function repo() {
  const dir = mkdtempSync(join(tmpdir(), "undone-"));
  const git = (...args: string[]) => execFileSync("git", args, { cwd: dir, encoding: "utf8" }).trim();
  git("init", "-q", "-b", "main");
  git("config", "user.email", "test@example.com");
  git("config", "user.name", "test");
  const commit = (file: string, lines: string[], message: string) => {
    writeFileSync(join(dir, file), lines.join("\n") + "\n");
    git("add", "-A");
    git("commit", "-q", "-m", message);
    return git("rev-parse", "HEAD");
  };
  commit("foliage.wesl", BASE, "start");
  const fresh = commit("foliage.wesl", FRESH, "snow tops near white");
  return { dir, commit, fresh, done: () => rmSync(dir, { recursive: true, force: true }) };
}

test("a stale copy that takes out a fresh commit and puts back what it replaced is found", () => {
  const r = repo();
  r.commit("film.ron", ["hours: (16.4, 17.18),"], "the dusk film ends earlier");
  const stale = r.commit("foliage.wesl", BASE, "the dusk film ends earlier, film.ron");
  const found = undone(`${stale}~1..${stale}`, r.dir);
  r.done();
  assert.equal(found.length, 1);
  assert.equal(found[0].undid, r.fresh);
  assert.equal(found[0].file, "foliage.wesl");
  assert.equal(found[0].restored, 1);
});

test("an edit that keeps some of the fresh commit's lines is not an undo", () => {
  const r = repo();
  const edit = r.commit("foliage.wesl", [...FRESH.slice(0, 2), "    let top = mix(SNOW, SNOW_TOP, facing * 0.8);", ...FRESH.slice(3)], "softer tops");
  assert.deepEqual(undone(`${edit}~1..${edit}`, r.dir), []);
  r.done();
});

test("a commit that names the file or the commit it undoes is not warned of", () => {
  const r = repo();
  const named = r.commit("foliage.wesl", BASE, "foliage.wesl's snow tops go back to blue");
  assert.deepEqual(undone(`${named}~1..${named}`, r.dir), []);
  const r2 = repo();
  const reverted = r2.commit("foliage.wesl", BASE, `the snow tops go back to blue (reverts ${r2.fresh.slice(0, 8)})`);
  assert.deepEqual(undone(`${reverted}~1..${reverted}`, r2.dir), []);
  r.done();
  r2.done();
});

test("lines moved to another file are not an undo, nor a push's own earlier commit", () => {
  const r = repo();
  writeFileSync(join(r.dir, "snow.wesl"), FRESH.slice(1, 4).join("\n") + "\n");
  const moved = r.commit("foliage.wesl", BASE, "snow tops in their own module");
  assert.deepEqual(undone(`${moved}~1..${moved}`, r.dir), []);
  const r2 = repo();
  const stale = r2.commit("foliage.wesl", BASE, "start over");
  assert.deepEqual(undone(`${r2.fresh}~1..${stale}`, r2.dir), []);
  r.done();
  r2.done();
});
