// The GPU wrapper (D319): `cargo xtask gpu <class> -- <command…>` runs the command at once under
// the game's turn, `~/.cache/terra/gpu.lock`, held shared; a `bench` holds it alone. Shots,
// films, sheets and bakes share the card side by side with no queue; only a benchmark waits, for
// the shared runs to end, and while it waits it leaves a want-file that holds back new shared
// runs, so it is never starved. A run that makes no progress for its class's limit is stopped.
// `cargo xtask gpu --status` lists what runs and waits.
//
// State lives in ~/.cache/terra/gpu/runs (TERRA_GPU_DIR): a file a run, its wrapper's pid and its
// command's, removed when it ends or by whoever reads next once its processes are gone. The
// want-files sit beside the lock, in gpu.want/, one a benchmark, named by its wrapper's pid.
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import * as plan from "./plan.ts";

const CACHE = path.join(process.env.XDG_CACHE_HOME || path.join(os.homedir(), ".cache"), "terra");
const RUNS = path.join(process.env.TERRA_GPU_DIR || path.join(CACHE, "gpu"), "runs");
/// The game's own turn (terra_world::gpu_turn): a run holds it shared, a bench alone.
const LOCK = process.env.TERRA_GPU_LOCK || path.join(CACHE, "gpu.lock");
/// A file a benchmark waiting for or holding the lock alone: new shared runs wait while one is here.
const WANT = path.join(path.dirname(LOCK), "gpu.want");
const POLL_MS = 1000;
const SAY_EVERY_MS = 5 * 60000;

/// A run's file: what is needed to find and describe it.
interface Run {
  class: plan.Class;
  pid: number;
  child?: number;
  cmd: string[];
  cwd: string;
  /// When it was asked for, and when it got the lock, ms since the epoch.
  asked: number;
  started?: number;
  /// When it last made progress (output, CPU or GPU use), ms since the epoch.
  progress?: number;
}

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

// ---- the machine: processes, the card, the lock -----------------------------------------------

/// Every process's parent and CPU ticks so far, from /proc/<pid>/stat (the parent is its fourth
/// field, after the name in brackets).
function processes(): { parents: Map<number, number>; cpu: Map<number, number> } {
  const parents = new Map<number, number>();
  const cpu = new Map<number, number>();
  for (const name of fs.readdirSync("/proc")) {
    if (!/^\d+$/.test(name)) continue;
    const stat = readText(`/proc/${name}/stat`);
    const parent = Number((stat.split(")").pop() ?? "").trim().split(/\s+/)[1]);
    if (!Number.isInteger(parent)) continue;
    parents.set(Number(name), parent);
    cpu.set(Number(name), plan.parseCpu(stat));
  }
  return { parents, cpu };
}

/// Clock ticks a second, the unit of /proc's CPU times.
const HZ = Number(spawnSync("getconf", ["CLK_TCK"], { encoding: "utf8" }).stdout) || 100;

const commandOf = (pid: number) => readText(`/proc/${pid}/cmdline`).split("\0").join(" ").replace(/\s+/g, " ").trim();

function nvidia(args: string[]): string {
  const out = spawnSync("nvidia-smi", args, { encoding: "utf8", timeout: 15000 });
  return out.status === 0 ? out.stdout : "";
}

/// Who holds or waits for the lock now (/proc/locks).
function holders(): plan.Holder[] {
  try {
    return plan.parseLocks(readText("/proc/locks"), fs.statSync(LOCK).ino);
  } catch {
    return [];
  }
}

/// The benchmarks that want the card: a want-file whose wrapper lives (a dead one's is removed),
/// and anything else waiting for the lock alone (the game's own bench, a `flock -x` by hand).
function benches(): number[] {
  const wanting: number[] = [];
  // The `flock` each wrapper runs its bench under, written into its want-file once started.
  const theirs = new Set<number>();
  for (const name of fs.existsSync(WANT) ? fs.readdirSync(WANT) : []) {
    const pid = Number(name);
    if (pid === process.pid) continue;
    if (alive(pid)) wanting.push(pid), theirs.add(Number(readText(path.join(WANT, name))));
    else fs.rmSync(path.join(WANT, name), { force: true });
  }
  for (const h of holders()) if (h.write && h.waiting && !theirs.has(h.pid)) wanting.push(h.pid);
  return wanting;
}

// ---- the state: a file a run ------------------------------------------------------------------

const fileOf = (run: Run) => path.join(RUNS, `${run.pid}.json`);

function save(run: Run) {
  fs.mkdirSync(RUNS, { recursive: true });
  fs.writeFileSync(`${fileOf(run)}.tmp`, JSON.stringify(run));
  fs.renameSync(`${fileOf(run)}.tmp`, fileOf(run));
}

/// Every run on file; those whose wrapper and command are both gone are removed.
function runs(): Run[] {
  const found: Run[] = [];
  for (const name of fs.existsSync(RUNS) ? fs.readdirSync(RUNS) : []) {
    if (!name.endsWith(".json")) continue;
    const file = path.join(RUNS, name);
    let run: Run;
    try {
      run = JSON.parse(fs.readFileSync(file, "utf8"));
    } catch {
      continue;
    }
    if (alive(run.pid) || alive(run.child)) found.push(run);
    else fs.rmSync(file, { force: true });
  }
  return found;
}

// ---- running a job ---------------------------------------------------------------------------

/// A shared run waits only while a benchmark wants the card, saying so at first and every 5 minutes.
async function clear(run: Run) {
  let said = 0;
  for (let wanting = benches(); wanting.length > 0; wanting = benches()) {
    const now = Date.now();
    if (now - said >= SAY_EVERY_MS) {
      say(`${run.class} waits for a benchmark (pid ${wanting.join(", ")})${said ? `, ${plan.age(now - run.asked)} so far` : ""}`);
      said = now;
    }
    await sleep(POLL_MS);
  }
}

function kill(root: number, signal: NodeJS.Signals) {
  for (const pid of plan.descendants(root, processes().parents)) {
    try {
      process.kill(pid, signal);
    } catch {}
  }
}

/// Runs the command under the lock (`flock`, shared or alone for a bench), passing its output on,
/// and stopping it once it has the lock and its whole process tree has made no progress for its
/// stall limit: no output (its own, or growth of a watched file), no CPU time and nothing on the
/// card's cores (plan.Progress). Time spent waiting for the lock is not a stall.
function execute(run: Run, stallMin: number, watch: string[]): Promise<number> {
  const flock = spawnSync("flock", ["--version"]).status === 0;
  fs.mkdirSync(path.dirname(LOCK), { recursive: true });
  const argv = flock ? ["flock", run.class === "bench" ? "-x" : "-s", LOCK, ...run.cmd] : run.cmd;
  const env = { ...process.env, TERRA_GPU_JOB: String(process.pid) };
  const child = spawn(argv[0], argv.slice(1), { stdio: ["inherit", "pipe", "pipe"], cwd: run.cwd, env });
  const stallMs = stallMin * 60000;
  const progress = new plan.Progress(Date.now(), stallMs, HZ);
  let why: string | null = null;
  const sizes = new Map<string, number>();
  const pass = (to: NodeJS.WriteStream) => (chunk: Buffer) => {
    to.write(chunk);
    progress.output(Date.now());
  };
  child.stdout.on("data", pass(process.stdout));
  child.stderr.on("data", pass(process.stderr));
  run.child = child.pid;
  save(run);
  if (run.class === "bench") fs.writeFileSync(want(), String(child.pid));
  const sample = setInterval(() => {
    const now = Date.now();
    if (run.started === undefined) {
      if (flock && holders().some((h) => h.pid === child.pid && h.waiting)) {
        progress.output(now);
        return;
      }
      run.started = now;
      if (now - run.asked > 5000) say(`${run.class} starts after waiting ${plan.age(now - run.asked)}`);
    }
    for (const file of watch) {
      const size = fs.statSync(file, { throwIfNoEntry: false })?.size ?? 0;
      if (size !== sizes.get(file)) (sizes.set(file, size), progress.output(now));
    }
    const { parents, cpu } = processes();
    const tree = plan.descendants(child.pid ?? 0, parents);
    const busy = plan.parsePmon(nvidia(["pmon", "-c", "1", "-s", "u"]));
    progress.read(now, new Map([...tree].map((pid) => [pid, cpu.get(pid) ?? 0])), [...tree].some((pid) => busy.has(pid)));
    run.progress = progress.last;
    save(run);
    if (!why && progress.stalled(now, stallMs)) {
      why = `no progress for ${stallMin} min (no output, no CPU or GPU use), the ${run.class} stall limit (--stall to change)`;
      kill(child.pid ?? 0, "SIGTERM");
      setTimeout(() => kill(child.pid ?? 0, "SIGKILL"), 10000).unref();
    }
  }, POLL_MS);
  return new Promise((done) => {
    child.on("close", (code, signal) => {
      clearInterval(sample);
      const exit = why ? 124 : code ?? 128 + (signal ? os.constants.signals[signal] : 1);
      const took = plan.age(Date.now() - (run.started ?? Date.now()));
      if (exit === 0) say(`${run.class} done in ${took}`);
      else say(`✗ ${run.class} exit ${exit} after ${took}${why ? `: ${why}` : ""}`);
      done(exit);
    });
    child.on("error", (error) => {
      clearInterval(sample);
      say(`✗ could not start ${argv[0]}: ${error.message}`);
      done(127);
    });
  });
}

interface Options {
  cls: plan.Class;
  stallMin: number;
  watch: string[];
  cmd: string[];
}

/// The run this wrapper holds, so a signal to it stops the command's processes and clears its files.
let current: Run | null = null;
const want = () => path.join(WANT, String(process.pid));
function tidy() {
  if (current) fs.rmSync(fileOf(current), { force: true });
  fs.rmSync(want(), { force: true });
}
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"] as const)
  process.on(signal, () => {
    if (current?.child) kill(current.child, signal);
    tidy();
    process.exit(128 + os.constants.signals[signal]);
  });

async function wrap(o: Options): Promise<number> {
  // A job started inside a job already has its turn: run it, as the game does under a held turn.
  if (process.env.TERRA_GPU_JOB) return spawnSync(o.cmd[0], o.cmd.slice(1), { stdio: "inherit" }).status ?? 1;
  const run: Run = { class: o.cls, pid: process.pid, cmd: o.cmd, cwd: process.cwd(), asked: Date.now() };
  current = run;
  save(run);
  if (o.cls === "bench") {
    fs.mkdirSync(WANT, { recursive: true });
    fs.writeFileSync(want(), "");
    const shared = holders().filter((h) => !h.waiting).length;
    if (shared > 0) say(`bench waits for ${shared} run${shared > 1 ? "s" : ""} holding the GPU to end; new ones hold back meanwhile`);
  } else await clear(run);
  try {
    return await execute(run, o.stallMin, o.watch);
  } finally {
    tidy();
  }
}

// ---- status, for the coordinator --------------------------------------------------------------

function status(): number {
  const now = Date.now();
  const ours = runs();
  const card = plan.parseCard(nvidia(["--query-gpu=memory.total,memory.used", "--format=csv,noheader,nounits"]));
  const gib = (mb: number) => `${(mb / 1024).toFixed(1)} GiB`;
  const running = ours.filter((r) => r.started !== undefined);
  const wanting = benches();
  const parts = [card ? `card ${gib(card.usedMB)} of ${gib(card.totalMB)} used` : "no nvidia-smi", `${running.length} running`, `${ours.length - running.length} waiting`];
  if (wanting.length > 0) parts.push(`a benchmark wants the card (pid ${wanting.join(", ")}): new runs hold back`);
  console.log(`gpu: ${parts.join(", ")}`);
  for (const r of ours.sort((a, b) => a.asked - b.asked)) {
    const state = r.started !== undefined ? `running ${plan.age(now - r.started).padStart(6)}` : `waiting ${plan.age(now - r.asked).padStart(6)}`;
    const out = r.progress ? `, progress ${plan.age(now - r.progress)} ago` : "";
    console.log(`  ${state} ${r.class.padEnd(5)}${out}  pid ${r.pid}  ${path.basename(r.cwd)}  ${r.cmd.join(" ").slice(0, 100)}`);
  }
  const children = new Set(ours.map((r) => r.child));
  for (const h of holders().filter((h) => !children.has(h.pid))) {
    const how = h.waiting ? "waits for gpu.lock alone" : h.write ? "holds gpu.lock alone" : "holds gpu.lock shared";
    console.log(`  outside ${how}  pid ${h.pid}  ${commandOf(h.pid).slice(0, 120)}`);
  }
  return 0;
}

// ---- the command line ------------------------------------------------------------------------

const USAGE = `usage: cargo xtask gpu <${Object.keys(plan.CLASSES).join("|")}> [--stall MIN] [--watch FILE]... -- <command…>
       cargo xtask gpu --status
${Object.entries(plan.CLASSES)
  .map(([name, c]) => `  ${name.padEnd(6)} ${c.does}; stopped after ${c.stallMin} min without output, CPU or GPU use`)
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
    else if (flags[i] === "--watch" && value) o.watch.push(path.resolve(value));
    // The old scheduler's memory request: accepted so older scripts still run, and ignored.
    else if (flags[i] === "--vram" && Number(value) > 0) continue;
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
  return wrap(o);
}

process.exit(await main());
