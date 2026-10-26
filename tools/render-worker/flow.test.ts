// The whole path on one machine, the box being a scratch folder reached without ssh (`local`):
// the client pushes a scratch repository's commit to the box's clone and queues it, the box's
// `wait` watches it while the daemon's job runner takes it, and the client fetches its files
// with rsync. No GPU, no network; a few seconds.

import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { after, test } from "node:test";
import { box, readJob, slotDir, slotHome } from "./box.ts";
import { finish, prepare, type Remote, sendLfs, submit } from "./client.ts";
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
const slots = { slots: 4, gpuSlots: 3 };
const first = (lock?: string) => next(b, policy, lock, slots).start[0]?.job;
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
  const review = join(slotHome(b, 1), ".cache/terra-review");
  mkdirSync(join(review, "old"), { recursive: true });
  writeFileSync(join(review, "old/before.png"), "x");
  utimesSync(join(review, "old/before.png"), 1, 1);

  const sub = submit(r, agent, job());
  assert.ok("id" in sub, JSON.stringify(sub));
  const lock = join(tmp, "ci-lock");
  writeFileSync(lock, "");
  assert.equal(next(b, policy, lock, slots).why, "the CI agent is running");
  rmSync(lock);
  const queued = first(lock)!;
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
  assert.match(jobLog, /render-worker: slot 1: [\s\S]*render-worker: check out the job's commit/, "the box's own lines are kept beside the command's");
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
  await runJob(b, first()!);
  assert.equal(await quietly(() => finish(r, shot.id, join(tmp, "out2"))), 0);
  assert.equal(readFileSync(join(tmp, "out2/shot.png"), "utf8"), "a\n");

  const failing = submit(r, agent, job({ command: ["sh", "-c", "exit 3"] }));
  assert.ok("id" in failing);
  await runJob(b, first()!);
  assert.equal(await quietly(() => finish(r, failing.id, join(tmp, "out3"))), 3);
});

test("a queued job is cancelled; a job whose commit was not pushed is refused", () => {
  const sub = submit(r, agent, job());
  assert.ok("id" in sub);
  const boxTs = resolve(import.meta.dirname, "box.ts");
  const cancel = spawnSync(process.execPath, [boxTs, "--dir", r.dir, "cancel", sub.id], { encoding: "utf8" });
  assert.equal(cancel.status, 0, cancel.stdout);
  assert.equal(readJob(b, sub.id)?.state, "cancelled");
  assert.equal(first(), undefined);

  const forged = { ...job(), id: "20261024-000000-dead", kind: "run", by: "x", queued: "", state: "queued" };
  const res = spawnSync(process.execPath, [boxTs, "--dir", r.dir, "submit"], { input: JSON.stringify(forged), encoding: "utf8" });
  assert.equal(res.status, 1);
  assert.match(res.stdout, /push it first/);
});

test("two jobs run at once, each in its own slot's checkout, build and HOME, and hand back only their own files", async () => {
  // Writes, under its HOME, a file named for its job holding where it ran and built, and lasts long enough to overlap.
  const write = `const fs = require("fs"), p = require("path");
const d = p.join(process.env.HOME, ".cache/terra-review/par");
fs.mkdirSync(d, { recursive: true });
fs.writeFileSync(p.join(d, process.env.TERRA_RENDER_JOB + ".txt"), process.cwd() + "\\n" + process.env.CARGO_TARGET_DIR);
setTimeout(() => {}, 300);`;
  const a = submit(r, agent, job({ command: ["node", "-e", write] }));
  const c = submit(r, agent, job({ command: ["node", "-e", write] }));
  assert.ok("id" in a && "id" in c);
  const starts = next(b, policy, undefined, slots).start;
  assert.deepEqual(starts.map((s) => s.slot), [1, 2]);
  const ended = await Promise.all(starts.map((s) => runJob(b, s.job, s.slot)));
  assert.deepEqual(ended.map((j) => j.state), ["done", "done"], ended.map((j) => j.note).join("; "));
  assert.ok(ended[0].started! < ended[1].ended! && ended[1].started! < ended[0].ended!, "they ran at once");
  for (const s of starts) {
    const files = readdirSync(join(b.jobs, s.job.id, "out/par"));
    assert.deepEqual(files, [`${s.job.id}.txt`], "a job collects only what it wrote");
    const [cwd, target] = readFileSync(join(b.jobs, s.job.id, "out/par", files[0]), "utf8").split("\n");
    assert.equal(cwd, slotDir(b, s.slot));
    assert.equal(target, join(slotDir(b, s.slot), "target"));
  }
});

test("a job's commit takes the LFS objects its pointers name to the box; one found nowhere fails", () => {
  const pointer = (oid: string) => `version https://git-lfs.github.com/spec/v1\noid sha256:${oid}\nsize 5\n`;
  const at = (root: string, oid: string) => join(root, ".git/lfs/objects", oid.slice(0, 2), oid.slice(2, 4), oid);
  const oid = createHash("sha256").update("model").digest("hex");
  mkdirSync(join(at(agent, oid), ".."), { recursive: true });
  writeFileSync(at(agent, oid), "model");
  writeFileSync(join(agent, "m.glb"), pointer(oid));
  sh(agent, "add", "m.glb");
  sh(agent, "commit", "-q", "-m", "a model in LFS");
  assert.equal(sendLfs(r, agent, sh(agent, "rev-parse", "HEAD")), undefined);
  assert.equal(readFileSync(at(b.repo, oid), "utf8"), "model");
  writeFileSync(join(agent, "lost.glb"), pointer("0".repeat(64)));
  sh(agent, "add", "lost.glb");
  sh(agent, "commit", "-q", "-m", "a pointer to nothing");
  assert.match(sendLfs(r, agent, sh(agent, "rev-parse", "HEAD")) ?? "", /1 LFS objects of [0-9a-f]{9} are neither here nor on origin/);
});
