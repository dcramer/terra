// The LFS pointer checks on a scratch repository with no git-lfs filter set, as on a machine
// without git-lfs: a whole binary under an LFS pattern is caught, a pointer and a file outside
// the patterns are not, and a commit's pointers name the objects the render worker needs.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { commitWhole, objects, pointerOid, pushedCommits, stagedWhole } from "./lfs.ts";

// No global or system config: no LFS filter turns a whole file into a pointer on `git add`.
process.env.GIT_CONFIG_GLOBAL = "/dev/null";
process.env.GIT_CONFIG_NOSYSTEM = "1";

const OID = "4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393";
const POINTER = `version https://git-lfs.github.com/spec/v1\noid sha256:${OID}\nsize 12345\n`;

function repo() {
  const dir = mkdtempSync(join(tmpdir(), "lfs-"));
  const git = (...args: string[]) => execFileSync("git", args, { cwd: dir, encoding: "utf8" }).trim();
  git("init", "-q", "-b", "main");
  git("config", "user.email", "test@example.com");
  git("config", "user.name", "test");
  writeFileSync(join(dir, ".gitattributes"), "*.png filter=lfs diff=lfs merge=lfs -text\n");
  return { dir, git, done: () => rmSync(dir, { recursive: true, force: true }) };
}

test("a pointer is read and other text is not", () => {
  assert.equal(pointerOid(POINTER), OID);
  assert.equal(pointerOid(`${POINTER.replace("v1", "v2")}`), undefined);
  assert.equal(pointerOid("\x89PNG\r\n\x1a\n"), undefined);
});

test("a whole binary staged under an LFS pattern is caught; a pointer and other files pass", () => {
  const r = repo();
  writeFileSync(join(r.dir, "raw.png"), Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 1, 2]));
  writeFileSync(join(r.dir, "big.png"), Buffer.alloc(5000, 7));
  writeFileSync(join(r.dir, "ok.png"), POINTER);
  writeFileSync(join(r.dir, "notes.txt"), "plain\n");
  r.git("add", "-A");
  assert.deepEqual(stagedWhole(r.dir).sort(), ["big.png", "raw.png"]);
  r.git("rm", "-q", "--cached", "raw.png", "big.png");
  assert.deepEqual(stagedWhole(r.dir), []);
  r.done();
});

test("the commits a push sends are checked, and a commit's pointers name its objects", () => {
  const r = repo();
  writeFileSync(join(r.dir, "ok.png"), POINTER);
  r.git("add", "-A");
  r.git("commit", "-q", "-m", "pointer");
  const good = r.git("rev-parse", "HEAD");
  writeFileSync(join(r.dir, "raw.png"), Buffer.alloc(300, 1));
  r.git("add", "-A");
  r.git("commit", "-q", "-m", "whole");
  const bad = r.git("rev-parse", "HEAD");
  const stdin = `refs/heads/main ${bad} refs/heads/main ${"0".repeat(40)}\n`;
  assert.deepEqual(pushedCommits(r.dir, stdin), [bad, good]);
  assert.deepEqual(commitWhole(r.dir, good), []);
  assert.deepEqual(commitWhole(r.dir, bad), [`${bad.slice(0, 9)} raw.png`]);
  assert.deepEqual(objects(r.dir, good), [OID]);
  r.done();
});
