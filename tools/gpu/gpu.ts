// The GPU wrapper (D319): `cargo xtask gpu <class> -- <command…>` runs the command at once under
// the game's turn, `~/.cache/terra/gpu.lock`, held shared; a `bench` holds it alone. Shots,
// films, sheets and bakes share the card side by side with no queue; only a benchmark waits, for
// the shared runs to end, and while it waits it leaves a want-file that holds back new shared
// runs, so it is never starved. A run that makes no progress for its class's limit is stopped.
// `cargo xtask gpu --status` lists what runs and waits.
//
// The game runs one copy a machine and refuses a second (terra_world::single_instance), so a
// command that starts it waits its turn first, first come first served: behind the wrapper's
// other game runs (a queue: a lock a helper of this wrapper holds, gpu.game/ beside the lock),
// then for any copy started outside it (its record beating in the instance folder). Other
// commands (`--no-game`, a Blender run, a `--multirun` copy) start at once, as before.
//
// State lives in ~/.cache/terra/gpu/runs (TERRA_GPU_DIR): a file a run, its wrapper's pid and its
// command's, removed when it ends or by whoever reads next once its processes are gone. The
// want-files sit beside the lock, in gpu.want/, one a benchmark, named by its wrapper's pid.
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
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
/// How often a run waiting for the game's one copy says so and who holds it: under the 5 minutes
/// of silence an agent's background run is ended at (TERRA_GPU_SAY_S for the tests).
const SAY_GAME_MS = Number(process.env.TERRA_GPU_SAY_S) * 1000 || 2 * 60000;
/// How many times a game run the game refused all the same (a copy started outside the wrapper
/// took the machine as this one built) waits and starts again.
const TRIES = 3;
/// How long a holder no run owns holds the lock before it is named with what to do (plan.ts;
/// TERRA_GPU_OUTSIDE_S for the tests).
const OUTSIDE_MS = Number(process.env.TERRA_GPU_OUTSIDE_S) * 1000 || plan.OUTSIDE_MS;

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
  /// A game run's place in the queue for the one copy a machine (plan.Queued), and whom it waits for.
  game?: string;
  queued?: number;
  turn?: number;
  waitsFor?: string;
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

/// Whether this machine has /proc (Linux, WSL); macOS has not, and `ps` says the same there.
const PROC = fs.existsSync("/proc/self/stat");

/// Every process's parent and CPU ticks so far, from /proc/<pid>/stat (the parent is its fourth
/// field, after the name in brackets), or from `ps` without /proc.
function processes(): { parents: Map<number, number>; cpu: Map<number, number> } {
  if (!PROC) return plan.parsePs(spawnSync("ps", ["-A", "-S", "-o", "pid=,ppid=,time="], { encoding: "utf8" }).stdout ?? "");
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

/// Clock ticks a second, the unit of /proc's CPU times; parsePs counts hundredths.
const HZ = (PROC && Number(spawnSync("getconf", ["CLK_TCK"], { encoding: "utf8" }).stdout)) || 100;

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

/// The lock file's inode and real path, to know its descriptors in /proc; null without either.
function lockFile(): { ino: number; real: string } | null {
  try {
    return PROC ? { ino: fs.statSync(LOCK).ino, real: fs.realpathSync(LOCK) } : null;
  } catch {
    return null;
  }
}

/// Whether a process holds the lock through a descriptor of its file (/proc/<pid>/fd and fdinfo).
/// A waiter's descriptor carries no lock line. /proc/locks cannot say: it skips a lock whose taker
/// has died (it shows that pid as 0), and with it every waiter queued behind that lock.
function holdsLock(pid: number | string, file: { ino: number; real: string }): boolean {
  let fds: string[];
  try {
    fds = fs.readdirSync(`/proc/${pid}/fd`);
  } catch {
    return false;
  }
  return fds.some((fd) => {
    try {
      return fs.readlinkSync(`/proc/${pid}/fd/${fd}`) === file.real && plan.parseFdinfo(readText(`/proc/${pid}/fdinfo/${fd}`), file.ino) === "holds";
    } catch {
      return false;
    }
  });
}

/// Whether a run's `flock` still waits for the lock: alive and not yet holding it.
function waits(pid: number): boolean {
  const file = lockFile();
  if (!file) return holders().some((h) => h.pid === pid && h.waiting);
  return alive(pid) && !holdsLock(pid, file);
}

/// Every process holding the lock through a descriptor of its file, its own or one it inherited
/// (/proc/<pid>/fd and fdinfo), with its group, age, folder and the run and render job its
/// environment names. Empty without /proc.
function holding(): plan.Holding[] {
  const file = lockFile();
  if (!file) return [];
  const boot = Date.now() - Number(readText("/proc/uptime").split(" ")[0]) * 1000;
  const found: plan.Holding[] = [];
  for (const name of fs.readdirSync("/proc")) {
    if (!/^\d+$/.test(name)) continue;
    if (!holdsLock(name, file)) continue;
    const pid = Number(name);
    const { pgrp, start } = plan.parseStat(readText(`/proc/${name}/stat`));
    const env = readText(`/proc/${name}/environ`).split("\0");
    const value = (key: string) => env.find((e) => e.startsWith(`${key}=`))?.slice(key.length + 1) || undefined;
    let cwd = "";
    try {
      cwd = fs.readlinkSync(`/proc/${name}/cwd`);
    } catch {}
    const ageMs = Date.now() - (boot + (start / HZ) * 1000);
    found.push({ pid, pgrp, cmd: commandOf(pid), cwd, ageMs, gpuJob: Number(value("TERRA_GPU_JOB")) || undefined, renderJob: value("TERRA_RENDER_JOB") });
  }
  return found;
}

/// The lock's holders no live run owns, one a process group.
const strays = () => plan.byGroup(plan.outside(holding(), runs(), alive));

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

// ---- one game copy a machine -----------------------------------------------------------------

const FLOCK = spawnSync("flock", ["--version"]).status === 0;

/// The folder the game keeps its one-copy slots and their records in, as it finds it itself
/// (terra_world::single_instance::folders): TERRA_INSTANCE_DIR, under WSL the Windows
/// %LOCALAPPDATA%\terra the game noted in the cache folder, else the cache folder.
function instanceDir(): string {
  if (process.env.TERRA_INSTANCE_DIR) return process.env.TERRA_INSTANCE_DIR;
  if (/microsoft/i.test(readText("/proc/version"))) {
    const local = readText(path.join(CACHE, "windows-local-app-data")).trim();
    if (local && fs.existsSync(local)) return path.join(local, "terra");
  }
  return CACHE;
}

const describe = (r: Run) => `the ${r.class} of wrapper pid ${r.pid} in ${path.basename(r.cwd)} (\`${r.cmd.join(" ").slice(0, 80)}\`, ${r.turn !== undefined ? `running ${plan.age(Date.now() - r.turn)}` : "waiting before this one"})`;

/// Takes the queue's lock in a helper of this wrapper's, `flock` running `cat` on a pipe from
/// here: the kernel frees it when this wrapper ends however it ends, and the command, no child of
/// the helper, never inherits it (as a run's background process inherits gpu.lock).
function holdQueue(file: string): { held: Promise<void>; release: () => void } {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const child = spawn("flock", ["-x", file, "sh", "-c", "echo held; exec cat"], { stdio: ["pipe", "pipe", "ignore"] });
  child.stdin.on("error", () => {});
  const held = new Promise<void>((done) => {
    child.stdout.once("data", () => done());
    child.once("error", () => done());
    child.once("close", () => done());
  });
  return { held, release: () => child.stdin.end() };
}

/// Waits for the game's one copy a machine (D319), then holds the queue until the returned
/// function is called: behind the wrapper's game runs that asked first, then for any copy the
/// instance folder's records show alive, saying whom it waits for at first and every 2 minutes.
async function turn(run: Run): Promise<() => void> {
  const folder = instanceDir();
  Object.assign(run, { game: folder, queued: Date.now(), turn: undefined, started: undefined, child: undefined });
  save(run);
  let said = 0;
  const waitFor = (who: string) => {
    const now = Date.now();
    if (run.waitsFor !== who) (run.waitsFor = who), save(run);
    if (now - said < SAY_GAME_MS) return;
    say(`${run.class} waits for the game's one copy a machine (D319), held by ${who}${said ? `, ${plan.age(now - run.queued!)} so far` : ""}`);
    said = now;
  };
  for (let before = plan.ahead(runs(), run); before.length > 0; before = plan.ahead(runs(), run)) {
    waitFor(describe(before[0]));
    await sleep(POLL_MS / 2);
  }
  const queue = holdQueue(path.join(path.dirname(LOCK), "gpu.game", `${createHash("sha1").update(folder).digest("hex").slice(0, 12)}.lock`));
  await queue.held;
  // A copy started outside the wrapper (someone playing, a run by hand): either slot's record beating.
  const slots = [0, 1].map((n) => ({ file: path.join(folder, `instance-${n}.txt`), beat: new plan.Beat() }));
  for (;;) {
    const now = Date.now();
    const read = slots.map((s) => {
      const text = fs.existsSync(s.file) ? readText(s.file) : null;
      return { state: s.beat.read(now, text), text };
    });
    if (read.every((r) => r.state === "free")) break;
    const held = read.find((r) => r.state === "held");
    const copy = held && plan.parseRecord(held.text ?? "");
    if (held) waitFor(copy ? `the game copy pid ${copy.pid} (${copy.os}) of ${copy.worktree}, started outside this queue (\`${copy.command.slice(0, 80)}\`)` : `a game copy whose record in ${folder} could not be read`);
    await sleep(POLL_MS / 2);
  }
  Object.assign(run, { turn: Date.now(), waitsFor: undefined });
  save(run);
  return queue.release;
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
/// `again`: a start the game refused will be tried again, so it is said as that, not as a failure.
function execute(run: Run, stallMin: number, watch: string[], again: boolean): Promise<{ exit: number; refused: boolean }> {
  const flock = FLOCK;
  fs.mkdirSync(path.dirname(LOCK), { recursive: true });
  const argv = flock ? ["flock", run.class === "bench" ? "-x" : "-s", LOCK, ...run.cmd] : run.cmd;
  const env = { ...process.env, TERRA_GPU_JOB: String(process.pid) };
  const child = spawn(argv[0], argv.slice(1), { stdio: ["inherit", "pipe", "pipe"], cwd: run.cwd, env });
  const stallMs = stallMin * 60000;
  const progress = new plan.Progress(Date.now(), stallMs, HZ);
  let why: string | null = null;
  const sizes = new Map<string, number>();
  // The last of what it said, to tell a start the game refused (another copy runs, D319).
  let tail = "";
  const pass = (to: NodeJS.WriteStream) => (chunk: Buffer) => {
    to.write(chunk);
    tail = (tail + chunk.toString()).slice(-8192);
    progress.output(Date.now());
  };
  child.stdout.on("data", pass(process.stdout));
  child.stderr.on("data", pass(process.stderr));
  run.child = child.pid;
  save(run);
  if (run.class === "bench") fs.writeFileSync(want(), String(child.pid));
  let named = 0;
  const sample = setInterval(() => {
    const now = Date.now();
    if (run.started === undefined) {
      if (flock && waits(child.pid ?? 0)) {
        progress.output(now);
        // Waiting minutes on a holder no run owns: name it and what to do, every 5 minutes.
        if (now - run.asked >= OUTSIDE_MS && now - named >= SAY_EVERY_MS) {
          named = now;
          for (const h of strays().filter((h) => h.ageMs >= OUTSIDE_MS)) say(`✗ ${run.class} waits ${plan.age(now - run.asked)}: ${plan.advise(h, LOCK)}`);
        }
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
      const refused = !why && exit !== 0 && plan.refused(tail) !== null;
      if (exit === 0) say(`${run.class} done in ${took}`);
      else if (refused && again) say(`${run.class}: the game refused to start, as a copy started outside this queue took the machine first (the lines above name it); waiting for it, then starting again`);
      else {
        const reason = why ?? plan.refused(tail);
        say(`✗ ${run.class} exit ${exit} after ${took}${reason ? `: ${reason}` : ""}`);
      }
      done({ exit, refused });
    });
    child.on("error", (error) => {
      clearInterval(sample);
      say(`✗ could not start ${argv[0]}: ${error.message}`);
      done({ exit: 127, refused: false });
    });
  });
}

interface Options {
  cls: plan.Class;
  stallMin: number;
  watch: string[];
  cmd: string[];
  /// Whether the command starts the game, and so waits for its one copy a machine.
  game: boolean;
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
  }
  try {
    for (let tries = 1; ; tries++) {
      if (o.cls !== "bench") await clear(run);
      // Without flock (macOS) there is no queue, as there is no lock: the game refuses as it did.
      const release = o.game && FLOCK ? await turn(run) : () => {};
      const again = o.game && FLOCK && tries < TRIES;
      const { exit, refused } = await execute(run, o.stallMin, o.watch, again).finally(release);
      if (!(refused && again)) return exit;
    }
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
    const out = r.started === undefined && r.waitsFor ? `, for the game's one copy, held by ${r.waitsFor}` : r.progress ? `, progress ${plan.age(now - r.progress)} ago` : "";
    console.log(`  ${state} ${r.class.padEnd(5)}  pid ${r.pid}  ${path.basename(r.cwd)}  ${r.cmd.join(" ").slice(0, 100)}${out}`);
  }
  const children = new Set(ours.map((r) => r.child));
  // With /proc, holders are read from who has the lock's file open, since /proc/locks names
  // only the lock's taker, which may be dead while what it started holds on.
  for (const h of holders().filter((h) => !children.has(h.pid) && (h.waiting || !PROC))) {
    const how = h.waiting ? "waits for gpu.lock alone" : h.write ? "holds gpu.lock alone" : "holds gpu.lock shared";
    console.log(`  outside ${how}  pid ${h.pid}  ${commandOf(h.pid).slice(0, 120)}`);
  }
  for (const h of strays()) {
    if (h.ageMs >= OUTSIDE_MS) console.log(`  ✗ outside: ${plan.advise(h, LOCK)}`);
    else console.log(`  outside holds gpu.lock ${plan.age(h.ageMs).padStart(6)}  pid ${h.pid}  ${h.cwd}  ${h.cmd.slice(0, 100)}`);
  }
  return 0;
}

// ---- the command line ------------------------------------------------------------------------

const USAGE = `usage: cargo xtask gpu <${Object.keys(plan.CLASSES).join("|")}> [--stall MIN] [--watch FILE]... [--no-game] -- <command…>
       cargo xtask gpu --status
${Object.entries(plan.CLASSES)
  .map(([name, c]) => `  ${name.padEnd(6)} ${c.does}; stopped after ${c.stallMin} min without output, CPU or GPU use`)
  .join("\n")}
  A command that starts the game waits for the game's one copy a machine (D319), first come first
  served; every command is taken for one but a Blender run and a --multirun copy. --no-game: this
  command starts no game, so it does not wait.`;

function parse(args: string[]): Options | string {
  const dash = args.indexOf("--");
  const [cls, ...given] = dash < 0 ? args : args.slice(0, dash);
  const flags = given.filter((flag) => flag !== "--no-game");
  const cmd = dash < 0 ? [] : args.slice(dash + 1);
  if (!cls || !plan.isClass(cls) || cmd.length === 0) return USAGE;
  const o: Options = { cls, stallMin: plan.CLASSES[cls].stallMin, watch: [], cmd, game: flags.length === given.length && plan.isGame(cmd) };
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
