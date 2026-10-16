// How busy the machine is, for measuring (D185 as amended): the QA traces' side of what
// `cargo xtask bench` does (xtask/src/load.rs, the same thresholds). A trace shares the machine
// with builds, tests and other agents' runs, whose work makes frames slower and less even, so
// before each run it waits, a bounded time, for a quiet machine; during it, it samples the
// load; and it says which runs were taken on a busy one.
//
// The processors' busy share is read where it is whole: under WSL from Windows (`typeperf`),
// whose total counts WSL's own virtual machine and the Windows programs the runs are; on Linux
// from /proc/stat over a second; elsewhere from the load average against the cores. While a run
// goes, the sandbox's own share is taken out (its program, `terra_sandbox`, as Windows or /proc
// counts it), so what is left is everything else's. The GPU's, where `nvidia-smi` answers.
// Node 22.18 or later; no dependencies.

import { execFile } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { cpus, loadavg } from "node:os";
import { promisify } from "node:util";

const run = promisify(execFile);

/// Quiet: the processors less than this busy, and the GPU, before a run, less than this.
export const QUIET_CPU_PCT = 35;
export const QUIET_GPU_PCT = 15;
/// Busy during a run: everything but the run itself more than this busy on average, or more than
/// the second at once.
export const BUSY_DURING_CPU_PCT = 30;
export const BUSY_DURING_MOST_PCT = 70;
/// The program whose share a run's load leaves out.
const SANDBOX = "terra_sandbox";
/// The most to wait for a quiet machine before a run, and over all of a command's runs.
export const WAIT_S = 90;
export const MOST_WAITING_S = 300;

export type Load = { cpu: number; gpu?: number };
export type During = { mean: number; most: number };

const onWsl = (() => {
  try {
    return readFileSync("/proc/version", "utf8").toLowerCase().includes("microsoft");
  } catch {
    return false;
  }
})();

export const quiet = (load: Load | undefined) => !load || (load.cpu < QUIET_CPU_PCT && (load.gpu === undefined || load.gpu < QUIET_GPU_PCT));
export const busyDuring = (during: During | undefined) => !!during && (during.mean > BUSY_DURING_CPU_PCT || during.most > BUSY_DURING_MOST_PCT);
export const describe = (load: Load) => `processors ${load.cpu.toFixed(0)}% busy${load.gpu === undefined ? "" : `, GPU ${load.gpu.toFixed(0)}%`}`;

/// Reads typeperf's CSV, a header and then `"time","value",...`, as its values.
export function parseTypeperf(text: string): number[] {
  const line = text.split("\n").filter((each) => each.startsWith('"'))[1];
  return (line ?? "")
    .split(",")
    .slice(1)
    .map((cell) => Number(cell.trim().replace(/"/g, "")))
    .filter(Number.isFinite);
}

/// The first line of /proc/stat as busy and all jiffies: idle and waiting on the disk are not busy.
export function parseProcStat(text: string): [number, number] | undefined {
  const line = text.split("\n").find((each) => each.startsWith("cpu "));
  if (!line) return undefined;
  const numbers = line.trim().split(/\s+/).slice(1, 9).map(Number);
  const all = numbers.reduce((a, b) => a + b, 0);
  return [all - numbers[3] - (numbers[4] ?? 0), all];
}

async function output(program: string, args: string[]): Promise<string | undefined> {
  try {
    return (await run(program, args, { encoding: "utf8", timeout: 15000 })).stdout;
  } catch {
    return undefined;
  }
}

/// The jiffies every process called `terra_sandbox` has run, user and system.
function sandboxJiffies(): number {
  let sum = 0;
  for (const pid of readdirSync("/proc").filter((name) => /^\d+$/.test(name))) {
    try {
      if (!readFileSync(`/proc/${pid}/comm`, "utf8").trim().startsWith(SANDBOX)) continue;
      // After the name in brackets: its state, then fields 4 to 13, then utime and stime.
      const fields = readFileSync(`/proc/${pid}/stat`, "utf8").split(")").at(-1)!.trim().split(/\s+/);
      sum += Number(fields[11]) + Number(fields[12]);
    } catch {
      // Gone since it was listed.
    }
  }
  return sum;
}

/// The processors' busy share now, and the sandbox's own part of it, where it can be told.
async function cpuNow(): Promise<{ all: number; ours: number } | undefined> {
  if (onWsl) {
    const values = parseTypeperf((await output("typeperf.exe", ["\\Processor(_Total)\\% Processor Time", `\\Process(${SANDBOX}*)\\% Processor Time`, "-sc", "1"])) ?? "");
    // A process's time is counted a core at a time: 100% is one core.
    if (values.length) return { all: values[0], ours: values.slice(1).reduce((a, b) => a + b, 0) / cpus().length };
  }
  try {
    const a = parseProcStat(readFileSync("/proc/stat", "utf8"));
    const oursA = sandboxJiffies();
    await new Promise((done) => setTimeout(done, 1000));
    const b = parseProcStat(readFileSync("/proc/stat", "utf8"));
    const oursB = sandboxJiffies();
    if (a && b && b[1] > a[1]) return { all: (100 * (b[0] - a[0])) / (b[1] - a[1]), ours: (100 * (oursB - oursA)) / (b[1] - a[1]) };
  } catch {
    // Not Linux: the load average.
  }
  return { all: Math.min(100, (100 * loadavg()[0]) / cpus().length), ours: 0 };
}

async function gpuNow(): Promise<number | undefined> {
  const text = await output("nvidia-smi", ["--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"]);
  const values = (text ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map(Number)
    .filter(Number.isFinite);
  return values.length ? Math.max(...values) : undefined;
}

/// The load now.
export async function sample(): Promise<Load | undefined> {
  const [cpu, gpu] = await Promise.all([cpuNow(), gpuNow()]);
  return cpu === undefined ? undefined : { cpu: cpu.all, gpu };
}

/// Waits up to `most` seconds for a quiet machine, looking every five; the load it ran on and
/// how long it waited.
export async function waitForQuiet(most = WAIT_S): Promise<{ load?: Load; waited: number }> {
  const started = Date.now();
  for (;;) {
    const load = await sample();
    const waited = (Date.now() - started) / 1000;
    if (quiet(load) || waited + 5 > most) return { load, waited };
    await new Promise((done) => setTimeout(done, 5000));
  }
}

/// Samples the load every few seconds until stopped: its mean and most.
export function watch(): { stop: () => Promise<During | undefined> } {
  const samples: number[] = [];
  let going = true;
  const loop = (async () => {
    while (going) {
      // Everything but the run itself.
      const cpu = await cpuNow();
      if (cpu) samples.push(Math.max(0, cpu.all - cpu.ours));
      for (let i = 0; i < 10 && going; i++) await new Promise((done) => setTimeout(done, 300));
    }
  })();
  return {
    stop: async () => {
      going = false;
      await loop;
      return samples.length ? { mean: samples.reduce((a, b) => a + b, 0) / samples.length, most: Math.max(...samples) } : undefined;
    },
  };
}

/// The middle of some values, and their lowest and highest.
export function middle(values: number[]): { median: number; low: number; high: number; spreadPct: number } {
  const sorted = [...values].sort((a, b) => a - b);
  const median = sorted[Math.floor(sorted.length / 2)] ?? 0;
  const [low, high] = [sorted[0] ?? 0, sorted.at(-1) ?? 0];
  return { median, low, high, spreadPct: median > 0 ? (100 * (high - low)) / median : 0 };
}
