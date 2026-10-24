// The render box's daemon (docs/render-worker.md, D445): keeps the queue in its folder (box.ts),
// runs jobs side by side, each in a slot of its own (a job checkout and build) at the job's
// pushed commit, as many as TERRA_RENDER_SLOTS and the GPU's share allow (jobs.ts `plan`),
// within the box's hours, and puts what each job wrote in its out/ for the client to fetch. It
// listens on no port: the client reaches it over ssh through box.ts, which wakes it by writing
// `wake` in its folder. It runs as a systemd user service or under `--supervise` (box.ts
// install), and exits to be started again on new code when an update asks (`restart`), once its
// jobs end. `--selftest` loads it and exits.

import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import { appendFileSync, closeSync, copyFileSync, existsSync, linkSync, lstatSync, mkdirSync, openSync, readdirSync, renameSync, rmSync, statSync, watch, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { type Box, box, boxEnv, ciBusy, daemonInfo, git, listJobs, outDir, slotDir, slotHome, writeJob } from "./box.ts";
import { boxSettings, collected, expired, type Job, jobEnv, jobRef, limitArgs, mayStart, plan, type Policy, type Slots, type Start, short, steps } from "./jobs.ts";

const say = (line: string) => console.log(`${new Date().toISOString().slice(0, 19)} ${line}`);

function walk(path: string, out: string[]): void {
  const st = lstatSync(path, { throwIfNoEntry: false });
  if (!st || st.isSymbolicLink()) return;
  if (st.isFile()) out.push(path);
  else if (st.isDirectory()) for (const name of readdirSync(path)) walk(join(path, name), out);
}

/// The files a job hands back, as [source, name under out/]: what the command wrote under each
/// collected path since it started, `~/.cache/terra-review/` left off their names.
export function gather(job: Job, src: string, home: string, startedMs: number): [string, string][] {
  const review = join(home, ".cache/terra-review");
  const roots = job.collect.length ? job.collect.map((p) => (p.startsWith("~/") ? join(home, p.slice(2)) : join(src, p))) : [review];
  const found: [string, string][] = [];
  for (const root of roots) {
    const files: string[] = [];
    walk(root, files);
    for (const f of files) {
      if (!collected(relative(root, f), statSync(f).mtimeMs, startedMs)) continue;
      const name = f.startsWith(`${review}/`) ? relative(review, f) : f.startsWith(`${home}/`) ? join("home", relative(home, f)) : relative(src, f);
      if (!name.startsWith("..")) found.push([f, name]);
    }
  }
  return found;
}

/// The running job's child, so a cancel can stop its whole process group.
export type Running = { job: Job; child?: ChildProcess; cancelled: boolean };

function run(cmd: string[], cwd: string, env: NodeJS.ProcessEnv, log: number, r: Running): Promise<number> {
  return new Promise((done) => {
    const child = spawn(cmd[0], cmd.slice(1), { cwd, env, stdio: ["ignore", log, log], detached: true });
    r.child = child;
    child.on("error", () => done(127));
    child.on("close", (code, signal) => done(code ?? (signal ? 128 : 1)));
  });
}

export function stop(r: Running): void {
  r.cancelled = true;
  try {
    if (r.child?.pid) process.kill(-r.child.pid, "SIGTERM");
  } catch {}
}

/// A slot's job checkout: a worktree of the worker's clone, detached, so a job's checkout and
/// build never touch the worker's own code or another slot's. Slot 1 takes over the single job
/// checkout's build and HOME from before slots, so it starts warm.
export function slotCheckout(b: Box, slot: number, rev: string): string {
  const dir = slotDir(b, slot);
  const home = slotHome(b, slot);
  if (slot === 1 && existsSync(b.home) && !existsSync(home)) {
    mkdirSync(dirname(home), { recursive: true });
    renameSync(b.home, home);
  }
  mkdirSync(home, { recursive: true });
  if (existsSync(join(dir, ".git"))) return dir;
  mkdirSync(dirname(dir), { recursive: true });
  git(b.repo, ["worktree", "prune"]);
  const made = git(b.repo, ["worktree", "add", "-q", "--detach", "--no-checkout", dir, rev]);
  if (made.status !== 0) throw new Error(`could not make slot ${slot}'s checkout: ${made.stderr.trim()}`);
  if (slot === 1 && existsSync(join(b.src, "target")) && !existsSync(join(dir, "target"))) {
    renameSync(join(b.src, "target"), join(dir, "target"));
    rmSync(b.src, { recursive: true, force: true });
  }
  return dir;
}

/// Runs one job to its end in its slot: its steps in the slot's checkout, then its files into out/.
export async function runJob(b: Box, queued: Job, slot = 1, r: Running = { job: queued, cancelled: false }): Promise<Job> {
  let job: Job = { ...queued, slot, state: "running", started: new Date().toISOString() };
  r.job = job;
  writeJob(b, job);
  const out = outDir(b, job.id);
  mkdirSync(out, { recursive: true });
  const logFile = join(out, "job.log");
  writeFileSync(logFile, "");
  const note = (line: string) => appendFileSync(logFile, `render-worker: ${line}\n`);
  const log = openSync(logFile, "a");
  let exit = 0;
  let why = "";
  let startedMs = Date.now();
  let ran = false;
  const home = slotHome(b, slot);
  let src = slotDir(b, slot);
  try {
    note(`slot ${slot}: ${src}`);
    src = slotCheckout(b, slot, job.rev);
    for (const step of steps(job)) {
      if (r.cancelled) break;
      note(`${step.what}: ${step.cmd.join(" ")}`);
      if (step.run && !ran) (startedMs = Date.now()), (ran = true);
      const cmd = step.run ? [...limitArgs(job), ...step.cmd] : step.cmd;
      const env = step.run || step.cmd[0] === "cargo" ? jobEnv(process.env, home, job.id, join(src, "target")) : process.env;
      const code = await run(cmd, src, env, log, r);
      if (code !== 0) {
        exit = code;
        why = r.cancelled ? "cancelled" : step.run ? (code === 124 ? `stopped at its limit of ${job.minutes} min` : `the command exited ${code}`) : `${step.what} failed`;
        break;
      }
    }
    const files = job.kind === "run" ? gather(job, src, home, startedMs) : [];
    for (const [from, name] of files) {
      const to = join(out, name);
      mkdirSync(dirname(to), { recursive: true });
      rmSync(to, { force: true });
      try {
        linkSync(from, to);
      } catch {
        copyFileSync(from, to);
      }
    }
    const size = files.reduce((n, [f]) => n + statSync(f).size, 0);
    if (!why) why = job.kind === "warm" ? "built" : `${files.length} files, ${Math.round(size / 1024 ** 2)} MB`;
  } catch (e) {
    exit ||= 1;
    why = (e as Error).message;
    note(why);
  }
  closeSync(log);
  job = { ...job, state: r.cancelled ? "cancelled" : exit ? "failed" : "done", exit, ended: new Date().toISOString(), note: why };
  writeJob(b, job);
  return job;
}

/// Deletes jobs past their days, with their files and refs; fails jobs a dead daemon left running
/// (any running job not in `mine`, the jobs this daemon runs).
export function housekeep(b: Box, keepDays: number, now = new Date(), mine: ReadonlySet<string> = new Set()): void {
  for (const j of listJobs(b)) {
    if (j.state === "running" && !mine.has(j.id)) writeJob(b, { ...j, state: "failed", exit: 1, ended: now.toISOString(), note: "the daemon stopped during it" });
  }
  for (const j of expired(listJobs(b), now, keepDays)) {
    rmSync(join(b.jobs, j.id), { recursive: true, force: true });
    if (j.kind === "run") git(b.repo, ["update-ref", "-d", jobRef(j)]);
  }
}

/// The jobs the policy and the free slots let start now, each with its slot, and why the rest
/// wait.
export function next(b: Box, policy: Policy, ciLock: string | undefined, slots: Slots, now = new Date()): { start: Start[]; why?: string } {
  const jobs = listJobs(b);
  if (!jobs.some((j) => j.state === "queued")) return { start: [] };
  const may = mayStart(policy, now.getHours() * 60 + now.getMinutes(), policy.afterCi && ciBusy(ciLock, now.getTime()));
  return may.ok ? plan(jobs, slots) : { start: [], why: may.why };
}

async function daemon(b: Box): Promise<number> {
  const s = boxSettings(boxEnv(b));
  if ("error" in s) {
    say(`✗ ${s.error}`);
    return 1;
  }
  const rev = git(b.repo, ["rev-parse", "--short=9", "HEAD"]).stdout.trim();
  const since = new Date().toISOString();
  const mode = process.env.TERRA_RENDER_MODE ?? "by hand";
  let status = "starting";
  const beat = () => writeFileSync(join(b.dir, "daemon.json"), JSON.stringify({ pid: process.pid, mode, rev, since, beat: new Date().toISOString(), status }));
  const running = new Map<string, Running>();
  housekeep(b, s.keepDays);
  say(`render daemon at ${rev}, ${mode}, pid ${process.pid}, ${s.slots.slots} slots (${s.slots.gpuSlots} for GPU jobs)`);
  let wakeUp = () => {};
  watch(b.dir, (_e, name) => name === "wake" && wakeUp());
  let lastWhy = "";
  for (;;) {
    housekeep(b, s.keepDays, new Date(), new Set(running.keys()));
    const restart = existsSync(join(b.dir, "restart"));
    if (restart && !running.size) {
      rmSync(join(b.dir, "restart"), { force: true });
      say("restarting on new code");
      return 0;
    }
    const n = restart ? { start: [], why: "restarting on new code once its jobs end" } : next(b, s.policy, s.ciLock, s.slots);
    for (const { job, slot } of n.start) {
      const r: Running = { job, cancelled: false };
      running.set(job.id, r);
      say(`slot ${slot}: running ${job.id} (${short(job)})`);
      runJob(b, job, slot, r).then((j) => {
        say(`${j.state === "done" ? "✓" : "✗"} slot ${slot}: ${j.id} ${j.state}: ${j.note}`);
        running.delete(j.id);
        wakeUp();
      });
    }
    for (const r of running.values()) {
      if (!r.cancelled && existsSync(join(b.jobs, r.job.id, "cancel"))) {
        say(`cancelling ${r.job.id}`);
        stop(r);
      }
    }
    const busy = running.size ? `${running.size} of ${s.slots.slots} slots running` : "idle";
    status = n.why ? `${busy}; a job waits: ${n.why}` : busy;
    if ((n.why ?? "") !== lastWhy && n.why) say(`a job waits: ${n.why}`);
    lastWhy = n.why ?? "";
    beat();
    await new Promise<void>((r) => ((wakeUp = r), setTimeout(r, 60_000).unref()));
  }
}

/// Runs the daemon and starts it again 5 s after it exits, for a box without systemd lingering.
async function supervise(b: Box): Promise<number> {
  const log = process.env.TERRA_RENDER_LOG ?? join(b.dir, "daemon.log");
  for (;;) {
    const fd = openSync(log, "a");
    const code = await new Promise<number>((done) => {
      const c = spawn(process.execPath, [resolve(import.meta.dirname, "daemon.ts"), "--dir", b.dir], { stdio: ["ignore", fd, fd] });
      c.on("close", (code) => done(code ?? 1));
    });
    closeSync(fd);
    appendFileSync(log, `supervisor: the daemon exited ${code}; starting it again in 5 s\n`);
    await new Promise((r) => setTimeout(r, 5000));
  }
}

async function main(argv: string[]): Promise<number> {
  const at = argv.indexOf("--dir");
  const b = box(at >= 0 ? argv[at + 1] : resolve(import.meta.dirname, "../../.."));
  if (argv.includes("--selftest")) {
    const s = boxSettings(boxEnv(b));
    if ("error" in s) return console.log(s.error), 1;
    if (spawnSync("git", ["--version"]).status !== 0) return console.log("no git"), 1;
    return 0;
  }
  if (argv.includes("--supervise")) return supervise(b);
  const other = daemonInfo(b);
  if (other?.alive && other.pid !== process.pid) {
    say(`another daemon runs (pid ${other.pid}); exiting`);
    return 0;
  }
  return daemon(b);
}

if (import.meta.main) process.exit(await main(process.argv.slice(2)));
