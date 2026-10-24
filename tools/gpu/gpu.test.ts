// The wrapper on real processes (D319): shared runs start at once, a bench waits for them and
// holds back new ones while it waits, and a job that makes no progress is stopped at its limit.
// Each test has its own state directory and lock, and a stand-in nvidia-smi that fails, so the
// live runs, their lock and the card are never touched.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

const GPU = path.join(import.meta.dirname, "gpu.ts");

/// A private machine: state, lock and a failing nvidia-smi in a temporary folder.
function machine() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "terra-gpu-test-"));
  const bin = path.join(dir, "bin");
  fs.mkdirSync(bin);
  fs.writeFileSync(path.join(bin, "nvidia-smi"), "#!/bin/sh\nexit 1\n", { mode: 0o755 });
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    PATH: `${bin}:${process.env.PATH}`,
    TERRA_GPU_DIR: path.join(dir, "gpu"),
    TERRA_GPU_LOCK: path.join(dir, "gpu.lock"),
  };
  delete env.TERRA_GPU_JOB;
  const log = path.join(dir, "order.log");
  const run = (args: string[]): Promise<{ code: number | null; err: string; seconds: number }> => {
    const started = Date.now();
    const child = spawn(process.execPath, [GPU, ...args], { env, stdio: ["ignore", "pipe", "pipe"] });
    let err = "";
    child.stderr.on("data", (chunk) => (err += chunk));
    return new Promise((done) => child.on("close", (code) => done({ code, err, seconds: (Date.now() - started) / 1000 })));
  };
  /// A command that logs its name and the time when it starts and ends, sleeping between.
  const step = (name: string, seconds: number) => ["sh", "-c", `echo "${name} start $(date +%s%N)" >> ${log}; sleep ${seconds}; echo "${name} end $(date +%s%N)" >> ${log}`];
  const lines = () =>
    fs.readFileSync(log, "utf8").trim().split("\n").map((line) => {
      const [name, at, ns] = line.split(" ");
      return { name, at, ms: Number(BigInt(ns) / 1_000_000n) };
    });
  return { run, step, lines, done: () => fs.rmSync(dir, { recursive: true, force: true }) };
}

const pause = (ms: number) => new Promise((done) => setTimeout(done, ms));

test("two stills start at once, side by side", async () => {
  const m = machine();
  const launched = Date.now();
  const [a, b] = await Promise.all([m.run(["still", "--", ...m.step("a", 2)]), m.run(["still", "--", ...m.step("b", 2)])]);
  assert.equal(a.code, 0, a.err);
  assert.equal(b.code, 0, b.err);
  const starts = m.lines().filter((l) => l.at === "start");
  assert.equal(starts.length, 2);
  for (const s of starts) assert.ok(s.ms - launched < 1500, `${s.name} started ${s.ms - launched} ms after launch`);
  const ends = m.lines().filter((l) => l.at === "end");
  assert.ok(Math.max(...starts.map((s) => s.ms)) < Math.min(...ends.map((e) => e.ms)), "both ran at the same time");
  m.done();
});

test("a bench waits for the running still, and a still asked for meanwhile waits for the bench", async () => {
  const m = machine();
  // The bench's stall limit (3 s) is shorter than its wait: waiting for the lock is not a stall.
  const first = m.run(["still", "--", ...m.step("first", 4)]);
  await pause(500);
  const bench = m.run(["bench", "--stall", "0.05", "--", ...m.step("bench", 1)]);
  await pause(1500);
  const later = m.run(["still", "--", ...m.step("later", 1)]);
  const [f, b, l] = await Promise.all([first, bench, later]);
  for (const r of [f, b, l]) assert.equal(r.code, 0, r.err);
  assert.deepEqual(
    m.lines().map((x) => `${x.name} ${x.at}`),
    ["first start", "first end", "bench start", "bench end", "later start", "later end"],
  );
  assert.match(l.err, /waits for a benchmark/);
  m.done();
});

test("a silent job that burns CPU runs to its end; a silent sleep is stopped at the limit", async () => {
  // A 3 s limit: the busy loop is silent for twice that, the sleep would last far longer.
  const m = machine();
  const busy = m.run(["still", "--stall", "0.05", "--", process.execPath, "-e", "const end = Date.now() + 6000; while (Date.now() < end);"]);
  const idle = m.run(["still", "--stall", "0.05", "--", "sleep", "60"]);
  const [b, i] = await Promise.all([busy, idle]);
  assert.equal(b.code, 0, b.err);
  assert.ok(b.seconds >= 6, `the busy job ran ${b.seconds} s`);
  assert.equal(i.code, 124, i.err);
  assert.match(i.err, /no progress for 0.05 min/);
  assert.ok(i.seconds < 20, `the sleep was stopped after ${i.seconds} s`);
  m.done();
});
