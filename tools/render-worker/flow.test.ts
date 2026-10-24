// The whole path on one machine: a client queues into a folder store, the worker takes the job
// in a checkout of a scratch repository's origin, runs it and uploads its files, and the client
// downloads them. No GitHub, no GPU; a few seconds.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { queue, wait } from "./client.ts";
import { Folder } from "./store.ts";
import { takeOne, type Worker } from "./worker.ts";

const tmp = mkdtempSync(join(tmpdir(), "render-flow-"));
after(() => rmSync(tmp, { recursive: true, force: true }));
const sh = (cwd: string, ...args: string[]) => execFileSync("git", ["-c", "user.name=t", "-c", "user.email=t@t", ...args], { cwd, encoding: "utf8" }).trim();

const origin = join(tmp, "origin");
mkdirSync(origin);
sh(origin, "init", "-q", "-b", "main");
writeFileSync(join(origin, "a.txt"), "a\n");
sh(origin, "add", ".");
sh(origin, "commit", "-q", "-m", "a");
const pushed = sh(origin, "rev-parse", "HEAD");
const root = join(tmp, "root");
sh(tmp, "clone", "-q", origin, root);
writeFileSync(join(root, "b.txt"), "b\n");
sh(root, "add", ".");
sh(root, "commit", "-q", "-m", "local only");
const unpushed = sh(root, "rev-parse", "HEAD");

const home = join(tmp, "home");
const review = join(home, ".cache/terra-review");
mkdirSync(join(review, "old"), { recursive: true });
writeFileSync(join(review, "old/before.png"), "x");
utimesSync(join(review, "old/before.png"), 1, 1);

const lines: string[] = [];
const w: Worker = { root, state: join(tmp, "state"), home, policy: { afterCi: true }, ciLock: join(tmp, "ci-lock"), name: "test-box", days: 7, say: (l) => lines.push(l) };
const store = new Folder(join(tmp, "store"));
const noPause = () => {};

// Writes a film under ~/.cache/terra-review, its frames beside it, a shot in the checkout, and
// whether it could see a token.
const script = `
const fs = require("fs"), p = require("path");
const d = p.join(process.env.HOME, ".cache/terra-review/film");
fs.mkdirSync(p.join(d, "frames"), { recursive: true });
fs.writeFileSync(p.join(d, "cut.mp4"), "film");
fs.writeFileSync(p.join(d, "frames/f-001.png"), "frame");
fs.writeFileSync("shot.png", fs.readFileSync("a.txt"));
console.log("token:", process.env.GH_TOKEN ?? "none");
`;

test("a job runs at its pushed commit and hands back what it wrote, and only that", () => {
  process.env.GH_TOKEN = "secret-for-the-test";
  const rec = queue(store, { rev: pushed, command: ["node", "-e", script], collect: [], cook: false, minutes: 1 });
  mkdirSync(w.ciLock!);
  assert.deepEqual(takeOne(store, w).why, "nightly is running");
  rmSync(w.ciLock!, { recursive: true });
  assert.equal(takeOne(store, w).did, "ran");
  delete process.env.GH_TOKEN;
  const out = join(tmp, "out1");
  assert.equal(wait(store, rec.job.id, out, noPause), 0);
  assert.equal(readFileSync(join(out, "film/cut.mp4"), "utf8"), "film");
  assert.ok(!existsSync(join(out, "film/frames")), "frames are not collected");
  assert.ok(!existsSync(join(out, "old")), "files from before the job are not collected");
  assert.ok(!existsSync(join(out, "shot.png")), "the checkout is collected only when asked");
  const log = readFileSync(join(out, "job.log"), "utf8");
  assert.match(log, /token: none/);
  assert.match(log, /render-worker: fetch origin's branches/, "the worker's own lines are kept beside the command's");
});

test("--collect takes a path in the checkout", () => {
  const rec = queue(store, { rev: pushed, command: ["node", "-e", script], collect: ["shot.png"], cook: false, minutes: 1 });
  takeOne(store, w);
  const out = join(tmp, "out2");
  assert.equal(wait(store, rec.job.id, out, noPause), 0);
  assert.equal(readFileSync(join(out, "shot.png"), "utf8"), "a\n");
});

test("a commit on no branch of origin is refused, and a failing command fails the job with its code", () => {
  const refused = queue(store, { rev: unpushed, command: ["true"], collect: [], cook: false, minutes: 1 });
  takeOne(store, w);
  assert.equal(store.list().find((r) => r.job.id === refused.job.id)?.job.state, "failed");
  assert.match(store.list().find((r) => r.job.id === refused.job.id)?.job.note ?? "", /push it first/);
  const failing = queue(store, { rev: pushed, command: ["sh", "-c", "exit 3"], collect: [], cook: false, minutes: 1 });
  takeOne(store, w);
  assert.equal(wait(store, failing.job.id, join(tmp, "out3"), noPause), 3);
});
