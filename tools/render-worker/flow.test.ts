// The whole path on one machine, the box being a scratch folder reached without ssh (`local`):
// the client pushes a scratch repository's commit to the box's clone and queues it, the box's
// `wait` watches it while the daemon's job runner takes it, and the client fetches its files
// with rsync. No GPU, no network; a few seconds.

import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { after, test } from "node:test";
import { box, readJob } from "./box.ts";
import { finish, prepare, type Remote, submit } from "./client.ts";
import { next, runJob } from "./daemon.ts";

const tmp = mkdtempSync(join(tmpdir(), "render-flow-"));
after(() => rmSync(tmp, { recursive: true, force: true }));
const sh = (cwd: string, ...args: string[]) => execFileSync("git", ["-c", "user.name=t", "-c", "user.email=t@t", ...args], { cwd, encoding: "utf8" }).trim();

const agent = join(tmp, "agent");
mkdirSync(agent);
sh(agent, "init", "-q", "-b", "main");
writeFileSync(join(agent, "a.txt"), "a\n");
sh(agent, "add", ".");
sh(agent, "commit", "-q", "-m", "a");
const rev = sh(agent, "rev-parse", "HEAD");

const r: Remote = { host: "local", dir: join(tmp, "box") };
const b = box(r.dir);
const policy = { afterCi: true };
const quiet = () => {};

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
const job = (over: object = {}) => ({ rev, command: ["node", "-e", script], collect: [], cook: false, minutes: 1, ...over });

test("a box is set up from nothing, and a job hands back what it wrote, and only that", async () => {
  const log = console.log;
  console.log = quiet;
  try {
    assert.ok(prepare(r));
  } finally {
    console.log = log;
  }
  assert.ok(existsSync(join(b.repo, ".git")));
  const review = join(b.home, ".cache/terra-review");
  mkdirSync(join(review, "old"), { recursive: true });
  writeFileSync(join(review, "old/before.png"), "x");
  utimesSync(join(review, "old/before.png"), 1, 1);

  const sub = submit(r, agent, job());
  assert.ok("id" in sub, JSON.stringify(sub));
  const lock = join(tmp, "ci-lock");
  writeFileSync(lock, "");
  assert.equal(next(b, policy, lock).why, "the CI agent is running");
  rmSync(lock);
  const queued = next(b, policy, lock).job!;
  assert.equal(queued.id, sub.id);

  // The client waits while the job runs: the box's wait sees it start and end.
  process.env.GH_TOKEN = "secret-for-the-test";
  const out = join(tmp, "out1");
  const lines: string[] = [];
  const log2 = console.log;
  console.log = (l: string) => lines.push(l);
  let code: number;
  try {
    [code] = await Promise.all([finish(r, sub.id, out), runJob(b, queued)]);
  } finally {
    console.log = log2;
    delete process.env.GH_TOKEN;
  }
  assert.equal(code, 0, lines.join("\n"));
  assert.match(lines.at(-1)!, /✓ render .*: done/);
  assert.equal(readFileSync(join(out, "film/cut.mp4"), "utf8"), "film");
  assert.ok(!existsSync(join(out, "film/frames")), "frames are not collected");
  assert.ok(!existsSync(join(out, "old")), "files from before the job are not collected");
  assert.ok(!existsSync(join(out, "shot.png")), "the checkout is collected only when asked");
  const jobLog = readFileSync(join(out, "job.log"), "utf8");
  assert.match(jobLog, /token: none/);
  assert.match(jobLog, /render-worker: fetch the job's commit/, "the box's own lines are kept beside the command's");
});

test("--collect takes a path in the checkout; a failing command fails with its code", async () => {
  const quietly = async <T>(f: () => Promise<T>) => {
    const log = console.log;
    console.log = quiet;
    try {
      return await f();
    } finally {
      console.log = log;
    }
  };
  const shot = submit(r, agent, job({ collect: ["shot.png"] }));
  assert.ok("id" in shot);
  await runJob(b, next(b, policy, undefined).job!);
  assert.equal(await quietly(() => finish(r, shot.id, join(tmp, "out2"))), 0);
  assert.equal(readFileSync(join(tmp, "out2/shot.png"), "utf8"), "a\n");

  const failing = submit(r, agent, job({ command: ["sh", "-c", "exit 3"] }));
  assert.ok("id" in failing);
  await runJob(b, next(b, policy, undefined).job!);
  assert.equal(await quietly(() => finish(r, failing.id, join(tmp, "out3"))), 3);
});

test("a queued job is cancelled; a job whose commit was not pushed is refused", () => {
  const sub = submit(r, agent, job());
  assert.ok("id" in sub);
  const boxTs = resolve(import.meta.dirname, "box.ts");
  const cancel = spawnSync(process.execPath, [boxTs, "--dir", r.dir, "cancel", sub.id], { encoding: "utf8" });
  assert.equal(cancel.status, 0, cancel.stdout);
  assert.equal(readJob(b, sub.id)?.state, "cancelled");
  assert.equal(next(b, policy, undefined).job, undefined);

  const forged = { ...job(), id: "20261024-000000-dead", kind: "run", by: "x", queued: "", state: "queued" };
  const res = spawnSync(process.execPath, [boxTs, "--dir", r.dir, "submit"], { input: JSON.stringify(forged), encoding: "utf8" });
  assert.equal(res.status, 1);
  assert.match(res.stdout, /push it first/);
});
