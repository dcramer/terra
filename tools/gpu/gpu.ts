// The GPU scheduler (D319): `cargo xtask gpu <class> -- <command…>` waits for the GPU's memory
// to fit the job, runs it beside others, stops it if it stalls and retries it once if the
// device is lost or out of memory; `cargo xtask gpu --status` lists running and waiting jobs.
// The decisions are plan.ts's; this file reads the machine, keeps the state and runs the job.
//
// State lives in ~/.cache/terra/gpu (TERRA_GPU_DIR): a file a job in jobs/ (its scheduler's
// pid, and its command's once running), a mkdir mutex around each admission, and history.jsonl,
// a line a finished run that the estimates are learned from. A job whose processes are all
// gone is removed by whoever reads next, so a crash leaves nothing held.
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import * as plan from "./plan.ts";

const CACHE = path.join(process.env.XDG_CACHE_HOME || path.join(os.homedir(), ".cache"), "terra");
const DIR = process.env.TERRA_GPU_DIR || path.join(CACHE, "gpu");
const JOBS = path.join(DIR, "jobs");
const HISTORY = path.join(DIR, "history.jsonl");
const MUTEX = path.join(DIR, "admit.lock");
/// The game's own turn (terra_world::gpu_turn): a job runs under it, shared or alone for a bench.
const LOCK = process.env.TERRA_GPU_LOCK || path.join(CACHE, "gpu.lock");
/// The old serial queue: whoever holds it is a film outside the scheduler.
const QUEUE = path.join(path.dirname(LOCK), "gpu.queue");
const POLL_MS = 2000;
const SAY_EVERY_MS = 5 * 60000;

/// A job's file: the plan's view of it and what is needed to find and describe it.
interface Stored extends plan.Job {
  state: "queued" | "running";
  pid: number;
  child?: number;
  cmd: string[];
  cwd: string;
  attempt: number;
  output?: number;
}

const GB = (mb: number) => `${(mb / 1024).toFixed(1)} GiB`;
const say = (line: string) => process.stderr.write(`gpu: ${line}\n`);
const sleep = (ms: number) => new Promise((done) => setTimeout(done, ms));

function alive(pid: number | undefined): boolean {
  if (!pid) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === "EPERM";
  }
}

function readText(file: string): string {
  try {
    return fs.readFileSync(file, "utf8");
  } catch {
    return "";
  }
}

// ---- the machine: processes, the card, the locks ----------------------------------------------

/// Every process's parent, from /proc/<pid>/stat (its fourth field, after the name in brackets).
function parents(): Map<number, number> {
  const found = new Map<number, number>();
  for (const name of fs.readdirSync("/proc")) {
    if (!/^\d+$/.test(name)) continue;
    const after = readText(`/proc/${name}/stat`).split(")").pop() ?? "";
    const parent = Number(after.trim().split(/\s+/)[1]);
    if (Number.isInteger(parent)) found.set(Number(name), parent);
  }
  return found;
}

/// When a process started, ms since the epoch: its start in clock ticks after boot (/proc).
function startOf(pid: number): number {
  const ticks = Number((readText(`/proc/${pid}/stat`).split(")").pop() ?? "").trim().split(/\s+/)[19]);
  const boot = Number(/^btime (\d+)/m.exec(readText("/proc/stat"))?.[1]);
  return Number.isFinite(ticks) && Number.isFinite(boot) ? (boot + ticks / 100) * 1000 : Date.now();
}

const commandOf = (pid: number) => readText(`/proc/${pid}/cmdline`).split("\0").join(" ").replace(/\s+/g, " ").trim();

function nvidia(query: string): string {
  const out = spawnSync("nvidia-smi", [query, "--format=csv,noheader,nounits"], { encoding: "utf8", timeout: 15000 });
  return out.status === 0 ? out.stdout : "";
}

function holders(file: string): plan.Holder[] {
  try {
    return plan.parseLocks(readText("/proc/locks"), fs.statSync(file).ino);
  } catch {
    return [];
  }
}

/// One reading of the machine, taken outside the mutex: jobs are read inside it.
interface Machine {
  card: { totalMB: number; usedMB: number } | null;
  apps: Map<number, number>;
  parents: Map<number, number>;
  lock: plan.Holder[];
  queue: plan.Holder[];
}

function machine(): Machine {
  return {
    card: plan.parseCard(nvidia("--query-gpu=memory.total,memory.used")),
    apps: plan.parseApps(nvidia("--query-compute-apps=pid,used_memory")),
    parents: parents(),
    lock: holders(LOCK),
    queue: holders(QUEUE),
  };
}

// ---- the state: a file a job ------------------------------------------------------------------

function save(job: Stored) {
  fs.mkdirSync(JOBS, { recursive: true });
  const file = path.join(JOBS, `${job.id}.json`);
  fs.writeFileSync(`${file}.tmp`, JSON.stringify(job));
  fs.renameSync(`${file}.tmp`, file);
}

const forget = (job: Stored) => fs.rmSync(path.join(JOBS, `${job.id}.json`), { force: true });

/// Every job on file; those whose scheduler and command are both gone are removed.
function jobs(): Stored[] {
  const found: Stored[] = [];
  for (const name of fs.existsSync(JOBS) ? fs.readdirSync(JOBS) : []) {
    if (!name.endsWith(".json")) continue;
    const file = path.join(JOBS, name);
    let job: Stored;
    try {
      job = JSON.parse(fs.readFileSync(file, "utf8"));
    } catch {
      continue;
    }
    if (alive(job.pid) || alive(job.child)) found.push(job);
    else fs.rmSync(file, { force: true });
  }
  return found;
}

/// Runs a step alone among every scheduler on the machine. The mutex is a directory, made
/// atomically; one whose maker died, or older than 30 s, is broken.
async function alone<T>(step: () => T): Promise<T> {
  fs.mkdirSync(DIR, { recursive: true });
  for (;;) {
    try {
      fs.mkdirSync(MUTEX);
    } catch {
      const holder = Number(readText(path.join(MUTEX, "pid")));
      const old = Date.now() - (fs.statSync(MUTEX, { throwIfNoEntry: false })?.mtimeMs ?? 0) > 30000;
      if ((holder && !alive(holder)) || old) fs.rmSync(MUTEX, { recursive: true, force: true });
      await sleep(50);
      continue;
    }
    try {
      fs.writeFileSync(path.join(MUTEX, "pid"), String(process.pid));
      return step();
    } finally {
      fs.rmSync(MUTEX, { recursive: true, force: true });
    }
  }
}

function history(): plan.Run[] {
  const text = readText(HISTORY);
  return text
    .slice(-512 * 1024)
    .split("\n")
    .flatMap((line) => {
      try {
        const run = JSON.parse(line);
        return plan.isClass(run.class) ? [run as plan.Run] : [];
      } catch {
        return [];
      }
    });
}

// ---- the view: our jobs and those outside the scheduler, as the plan sees them ---------------

interface View {
  queued: plan.Job[];
  running: plan.Job[];
  card: plan.Card;
  usedMB: number;
}

/// Our jobs with what their processes hold, and stand-ins for runs outside the scheduler: a
/// benchmark holding or waiting for gpu.lock alone, an old caller holding gpu.queue (a film),
/// an unattended game holding gpu.lock shared (a still).
function view(stored: Stored[], m: Machine, runs: plan.Run[]): View {
  const ours = new Set<number>();
  const usage = (root: number) => {
    const tree = plan.descendants(root, m.parents);
    for (const pid of tree) ours.add(pid);
    return [...tree].reduce((sum, pid) => sum + (m.apps.get(pid) ?? 0), 0);
  };
  const queued: plan.Job[] = stored.filter((job) => job.state === "queued");
  const running: plan.Job[] = stored
    .filter((job) => job.state === "running")
    .map((job) => ({ ...job, usedMB: usage(job.child ?? job.pid) }));
  const stand = (holder: plan.Holder, cls: plan.Class, what: string): plan.Job => ({
    id: `outside-${holder.pid}`,
    class: cls,
    vramMB: cls === "bench" ? 0 : plan.estimate(cls, runs),
    enqueued: startOf(holder.pid),
    started: startOf(holder.pid),
    foreign: `${what}: ${commandOf(holder.pid).slice(0, 120)}`,
  });
  for (const holder of m.queue.filter((h) => !h.waiting && !ours.has(h.pid))) {
    const job = stand(holder, "film", "holds gpu.queue");
    running.push({ ...job, usedMB: usage(holder.pid) });
  }
  for (const holder of m.lock.filter((h) => !ours.has(h.pid))) {
    if (holder.waiting && holder.write) queued.push(stand(holder, "bench", "waits for gpu.lock alone"));
    else if (!holder.waiting) {
      const job = stand(holder, holder.write ? "bench" : "still", holder.write ? "holds gpu.lock alone" : "holds gpu.lock");
      running.push({ ...job, usedMB: usage(holder.pid) });
    }
  }
  const totalMB = m.card?.totalMB ?? (Number(process.env.TERRA_GPU_VRAM_MB) || 10240);
  const usedMB = m.card?.usedMB ?? 0;
  const counted = running.reduce((sum, job) => sum + (job.usedMB ?? 0), 0);
  return { queued, running, card: { totalMB, otherMB: Math.max(0, usedMB - counted) }, usedMB };
}

const runMinutes = (runs: plan.Run[]) => (job: plan.Job) => plan.usualMinutes(job.class, runs);

/// Where a job stands in the queue: its place, the expected wait, what it is behind.
function standing(job: plan.Job, v: View, runs: plan.Run[], now: number): string {
  const queue = plan.order(v.queued, now);
  const place = queue.findIndex((q) => q.id === job.id) + 1;
  const start = plan.expectedStarts(v.queued, v.running, v.card, now, runMinutes(runs)).get(job.id) ?? Infinity;
  const wait = Number.isFinite(start) ? `about ${plan.age(start - now)}` : "no end in sight (asks for more than the card has)";
  const behind = v.running.map((r) => `${r.foreign ? "outside " : ""}${r.class} ${plan.age(now - (r.started ?? now))}`).join(", ");
  const free = Math.max(0, plan.available(v.card, v.running));
  return `${place} of ${queue.length} in the queue, ${wait}; asks ${GB(job.vramMB)}, ${GB(free)} free; running: ${behind || "nothing"}`;
}

// ---- running a job ---------------------------------------------------------------------------

/// Waits until the plan admits this job, saying where it stands at first and every 5 minutes.
async function turn(job: Stored, runs: plan.Run[]): Promise<void> {
  let said = 0;
  for (;;) {
    const m = machine();
    const now = Date.now();
    const go = await alone(() => {
      const v = view(jobs(), m, runs);
      if (!plan.admit(v.queued, v.running, v.card, now).some((j) => j.id === job.id)) return standing(job, v, runs, now);
      Object.assign(job, { state: "running", started: now });
      save(job);
      return null;
    });
    if (go === null) {
      const waited = now - job.enqueued;
      say(`${job.class} starts${waited > 5000 ? ` after waiting ${plan.age(waited)}` : " now"}, asking ${GB(job.vramMB)}`);
      return;
    }
    if (now - said >= SAY_EVERY_MS) {
      say(`${job.class} waits, ${said ? `${plan.age(now - job.enqueued)} so far, ` : ""}${go}`);
      said = now;
    }
    await sleep(POLL_MS);
  }
}

function kill(root: number, signal: NodeJS.Signals) {
  for (const pid of plan.descendants(root, parents())) {
    try {
      process.kill(pid, signal);
    } catch {}
  }
}

interface Outcome {
  code: number;
  peakMB: number;
  why: string | null;
  retry: string | null;
}

/// Runs the command under the game's turn (gpu.lock, shared or alone for a bench), passing its
/// output on, measuring its memory on the card, and stopping it once it writes nothing for its
/// stall limit (its own output, or growth of a watched file).
function execute(job: Stored, stallMin: number, watch: string[]): Promise<Outcome> {
  const flock = spawnSync("flock", ["--version"]).status === 0;
  fs.mkdirSync(path.dirname(LOCK), { recursive: true });
  const argv = flock ? ["flock", job.class === "bench" ? "-x" : "-s", LOCK, ...job.cmd] : job.cmd;
  const env = { ...process.env, TERRA_GPU_JOB: job.id };
  const child = spawn(argv[0], argv.slice(1), { stdio: ["inherit", "pipe", "pipe"], cwd: job.cwd, env });
  let last = Date.now();
  let tail = "";
  let peakMB = 0;
  let why: string | null = null;
  const sizes = new Map<string, number>();
  const pass = (to: NodeJS.WriteStream) => (chunk: Buffer) => {
    to.write(chunk);
    last = Date.now();
    tail = (tail + chunk.toString("utf8")).slice(-16384);
  };
  child.stdout.on("data", pass(process.stdout));
  child.stderr.on("data", pass(process.stderr));
  job.child = child.pid;
  save(job);
  const sample = setInterval(() => {
    for (const file of watch) {
      const size = fs.statSync(file, { throwIfNoEntry: false })?.size ?? 0;
      if (size !== sizes.get(file)) (sizes.set(file, size), (last = Date.now()));
    }
    const m = machine();
    const tree = plan.descendants(child.pid ?? 0, m.parents);
    job.usedMB = [...tree].reduce((sum, pid) => sum + (m.apps.get(pid) ?? 0), 0);
    peakMB = Math.max(peakMB, job.usedMB);
    job.output = last;
    save(job);
    if (!why && Date.now() - last > stallMin * 60000) {
      why = `no output for ${stallMin} min, the ${job.class} stall limit (--stall to change)`;
      kill(child.pid ?? 0, "SIGTERM");
      setTimeout(() => kill(child.pid ?? 0, "SIGKILL"), 10000).unref();
    }
  }, POLL_MS);
  return new Promise((done) => {
    child.on("close", (code, signal) => {
      clearInterval(sample);
      const exit = why ? 124 : code ?? 128 + (signal ? os.constants.signals[signal] : 1);
      done({ code: exit, peakMB, why, retry: exit !== 0 && !why ? plan.retryable(tail) : null });
    });
    child.on("error", (error) => {
      clearInterval(sample);
      done({ code: 127, peakMB, why: `could not start ${argv[0]}: ${error.message}`, retry: null });
    });
  });
}

function record(job: Stored, outcome: Outcome) {
  const ended = Date.now();
  const line = {
    class: job.class,
    cmd: job.cmd.join(" ").slice(0, 300),
    cwd: job.cwd,
    enqueued: job.enqueued,
    started: job.started,
    ended,
    waitS: Math.round(((job.started ?? ended) - job.enqueued) / 1000),
    runS: Math.round((ended - (job.started ?? ended)) / 1000),
    peakMB: outcome.peakMB,
    code: outcome.code,
    why: outcome.why ?? outcome.retry,
    attempt: job.attempt,
  };
  fs.appendFileSync(HISTORY, `${JSON.stringify(line)}\n`);
}

interface Options {
  cls: plan.Class;
  stallMin: number;
  vramMB?: number;
  watch: string[];
  cmd: string[];
}

/// The job this scheduler holds, so a signal to it stops the job's processes and its file.
let current: Stored | null = null;
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"] as const)
  process.on(signal, () => {
    if (current?.child) kill(current.child, signal);
    if (current) forget(current);
    process.exit(128 + os.constants.signals[signal]);
  });

async function schedule(o: Options): Promise<number> {
  // A job started inside a job already has its turn: run it, as the game does under a held turn.
  if (process.env.TERRA_GPU_JOB) return spawnSync(o.cmd[0], o.cmd.slice(1), { stdio: "inherit" }).status ?? 1;
  const runs = history();
  let vramMB = o.vramMB ?? plan.estimate(o.cls, runs);
  const enqueued = Date.now();
  for (let attempt = 1; ; attempt++) {
    const job: Stored = { id: `${enqueued}-${process.pid}-${attempt}`, class: o.cls, vramMB, enqueued, state: "queued", pid: process.pid, cmd: o.cmd, cwd: process.cwd(), attempt };
    current = job;
    save(job);
    await turn(job, runs);
    const outcome = await execute(job, o.stallMin, o.watch);
    record(job, outcome);
    forget(job);
    const took = plan.age(Date.now() - (job.started ?? Date.now()));
    if (outcome.retry && attempt === 1) {
      vramMB = Math.ceil((Math.max(vramMB, outcome.peakMB) * 1.25) / 100) * 100;
      say(`✗ ${o.cls} failed after ${took}: ${outcome.retry}; trying once more when ${GB(vramMB)} is free`);
      continue;
    }
    const peak = outcome.peakMB ? `, peak ${GB(outcome.peakMB)}` : "";
    if (outcome.code === 0) say(`${o.cls} done in ${took}${peak}`);
    else say(`✗ ${o.cls} exit ${outcome.code} after ${took}${peak}${outcome.why || outcome.retry ? `: ${outcome.why ?? outcome.retry}` : ""}`);
    return outcome.code;
  }
}

// ---- status, for the coordinator --------------------------------------------------------------

async function status(): Promise<number> {
  const runs = history();
  const m = machine();
  const stored = await alone(() => jobs());
  const v = view(stored, m, runs);
  const now = Date.now();
  const byId = new Map(stored.map((job) => [job.id, job]));
  const where = (job: plan.Job) => {
    const s = byId.get(job.id);
    if (!s) return job.foreign ?? "";
    return `pid ${s.pid}  ${path.basename(s.cwd)}  ${s.cmd.join(" ").slice(0, 100)}`;
  };
  const card = m.card ? `card ${GB(v.card.totalMB)}, ${GB(v.usedMB)} used` : "no nvidia-smi: memory counted from requests";
  console.log(`gpu: ${card}, ${GB(Math.max(0, plan.available(v.card, v.running)))} free for jobs; ${v.running.length} running, ${v.queued.length} queued`);
  for (const job of v.running) {
    const s = byId.get(job.id);
    const out = s?.output ? `, output ${plan.age(now - s.output)} ago` : "";
    console.log(`  running ${job.foreign ? "outside " : ""}${job.class.padEnd(5)} ${plan.age(now - (job.started ?? now)).padStart(6)}  ${GB(job.usedMB ?? 0)}${job.vramMB ? ` of ${GB(job.vramMB)}` : ""}${out}  ${where(job)}`);
  }
  const starts = plan.expectedStarts(v.queued, v.running, v.card, now, runMinutes(runs));
  plan.order(v.queued, now).forEach((job, i) => {
    const start = starts.get(job.id) ?? Infinity;
    const wait = Number.isFinite(start) ? `starts in about ${plan.age(start - now)}` : "never fits";
    console.log(`  queued ${i + 1} ${job.class.padEnd(5)} waited ${plan.age(now - job.enqueued)}, ${wait}, asks ${GB(job.vramMB)}  ${where(job)}`);
  });
  return 0;
}

// ---- the command line ------------------------------------------------------------------------

const USAGE = `usage: cargo xtask gpu <${Object.keys(plan.CLASSES).join("|")}> [--stall MIN] [--watch FILE]... [--vram MB] -- <command…>
       cargo xtask gpu --status
${Object.entries(plan.CLASSES)
  .map(([name, c]) => `  ${name.padEnd(6)} ${c.does}; asks ${GB(c.vramMB)} until measured, stopped after ${c.stallMin} min silent`)
  .join("\n")}`;

function parse(args: string[]): Options | string {
  const dash = args.indexOf("--");
  const [cls, ...flags] = dash < 0 ? args : args.slice(0, dash);
  const cmd = dash < 0 ? [] : args.slice(dash + 1);
  if (!cls || !plan.isClass(cls) || cmd.length === 0) return USAGE;
  const o: Options = { cls, stallMin: plan.CLASSES[cls].stallMin, watch: [], cmd };
  for (let i = 0; i < flags.length; i += 2) {
    const value = flags[i + 1];
    if (flags[i] === "--stall" && Number(value) > 0) o.stallMin = Number(value);
    else if (flags[i] === "--vram" && Number(value) > 0) o.vramMB = Number(value);
    else if (flags[i] === "--watch" && value) o.watch.push(path.resolve(value));
    else return `gpu: unknown option ${flags[i]}\n${USAGE}`;
  }
  return o;
}

async function main(): Promise<number> {
  const args = process.argv.slice(2);
  if (args[0] === "--status") return status();
  const o = parse(args);
  if (typeof o === "string") {
    console.error(o);
    return 2;
  }
  return schedule(o);
}

process.exit(await main());
