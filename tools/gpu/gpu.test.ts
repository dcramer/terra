// The scheduler's stall rule on real processes (D319): a job that writes nothing but works is
// left running, one that does nothing is stopped at its limit. Each run has its own state
// directory and lock, and a stand-in nvidia-smi that fails, so the live queue and card are never
// touched: the card is counted as TERRA_GPU_VRAM_MB and only requests are counted.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

const GPU = path.join(import.meta.dirname, "gpu.ts");

function run(stallMin: number, cmd: string[]): Promise<{ code: number | null; err: string; seconds: number }> {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "terra-gpu-test-"));
  const bin = path.join(dir, "bin");
  fs.mkdirSync(bin);
  fs.writeFileSync(path.join(bin, "nvidia-smi"), "#!/bin/sh\nexit 1\n", { mode: 0o755 });
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    PATH: `${bin}:${process.env.PATH}`,
    TERRA_GPU_DIR: path.join(dir, "gpu"),
    TERRA_GPU_LOCK: path.join(dir, "gpu.lock"),
    TERRA_GPU_VRAM_MB: "10240",
  };
  delete env.TERRA_GPU_JOB;
  const started = Date.now();
  const child = spawn(process.execPath, [GPU, "still", "--stall", String(stallMin), "--", ...cmd], { env, stdio: ["ignore", "pipe", "pipe"] });
  let err = "";
  child.stderr.on("data", (chunk) => (err += chunk));
  return new Promise((done) =>
    child.on("close", (code) => {
      fs.rmSync(dir, { recursive: true, force: true });
      done({ code, err, seconds: (Date.now() - started) / 1000 });
    }),
  );
}

test("a silent job that burns CPU runs to its end; a silent sleep is stopped at the limit", async () => {
  // A 3 s limit: the busy loop is silent for twice that, the sleep would last far longer.
  const busy = run(0.05, [process.execPath, "-e", "const end = Date.now() + 6000; while (Date.now() < end);"]);
  const idle = run(0.05, ["sleep", "60"]);
  const [b, i] = await Promise.all([busy, idle]);
  assert.equal(b.code, 0, b.err);
  assert.ok(b.seconds >= 6, `the busy job ran ${b.seconds} s`);
  assert.equal(i.code, 124, i.err);
  assert.match(i.err, /no progress for 0.05 min/);
  assert.ok(i.seconds < 20, `the sleep was stopped after ${i.seconds} s`);
});
