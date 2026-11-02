// The render box's daemon (docs/render-worker.md, D445): keeps the queue in its folder (box.ts),
// runs jobs side by side, each in a slot of its own (a job checkout and build) at the job's
// pushed commit, as many as TERRA_RENDER_SLOTS and the GPU's share allow (jobs.ts `plan`),
// within the box's hours, and puts what each job wrote in its out/ for the client to fetch. It
// listens on no port: the client reaches it over ssh through box.ts, which wakes it by writing
// `wake` in its folder. It runs as a systemd user service or under `--supervise` (box.ts
// install), and exits to be started again on new code when an update asks (`restart`), once its
// jobs end. A job that draws gets a display (display.ts): an Xvfb the daemon keeps when it has
// none. Before each job it checks the disk, prunes the box's caches when it runs low and starts
// no job while it is full, never crashing on ENOSPC (guard.ts, `Keeper`). `--selftest` loads it
// and exits.

import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import { appendFileSync, closeSync, copyFileSync, existsSync, linkSync, lstatSync, mkdirSync, openSync, readdirSync, readFileSync, readSync, renameSync, rmSync, statSync, watch, writeFileSync, writeSync } from "node:fs";
import { rm } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import { type Box, box, boxEnv, ciBusy, daemonInfo, git, listJobs, outDir, slotDir, slotHome, writeJob } from "./box.ts";
import { Display } from "./display.ts";
import { type DiskSettings, type DiskState, diskFull, diskSettings, fullMark, gb, type JobUse, KEEP, level, lowMark, type Prune, prune, pruneOrder, pruneTo, type SlotUse, type Space, space } from "./guard.ts";
import { killNow, type Proc, reap, reaped, scan } from "./reap.ts";
import { boxSettings, checkoutsInBuild, classOf, cleanCommands, collected, displayPlan, expired, ID, incrementalClears, type Job, jobEnv, jobRef, limitArgs, mayStart, needsDisplay, performanceCores, plan, type Policy, rustcIce, type Slots, type Start, short, staleBuild, steps, workspacePackages } from "./jobs.ts";

const say = (line: string) => console.log(`${new Date().toISOString().slice(0, 19)} ${line}`);

function walk(path: string, out: string[]): void {
  const st = lstatSync(path, { throwIfNoEntry: false });
  if (!st || st.isSymbolicLink()) return;
  if (st.isFile()) out.push(path);
  else if (st.isDirectory()) for (const name of readdirSync(path)) walk(join(path, name), out);
}

/// The files a job hands back, as [source, name under out/]: what the command wrote under each
/// collected path since it started, `~/.cache/terra-review/` left off their names; `missing`,
/// when given, takes each `--collect` path that is not there when the job ends.
export function gather(job: Job, src: string, home: string, startedMs: number, missing?: (path: string) => void): [string, string][] {
  const review = join(home, ".cache/terra-review");
  const roots = job.collect.length ? job.collect.map((p) => (p.startsWith("~/") ? join(home, p.slice(2)) : join(src, p))) : [review];
  const found: [string, string][] = [];
  for (const [i, root] of roots.entries()) {
    if (job.collect.length && !existsSync(root)) missing?.(job.collect[i]);
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

/// The running job's child, so a cancel can stop its whole process group, and the stopping of
/// what of the job left the group (reap.ts), which the job's end waits for. A bench built ahead
/// calls `ready` and waits until the daemon calls its `go` (`plan` says when).
export type Running = { job: Job; child?: ChildProcess; cancelled: boolean; ready?: () => void; go?: () => void; reaping?: Promise<{ found: Proc[]; left: Proc[] }> };

function run(cmd: string[], cwd: string, env: NodeJS.ProcessEnv, log: number, r: Running): Promise<number> {
  return new Promise((done) => {
    const child = spawn(cmd[0], cmd.slice(1), { cwd, env, stdio: ["ignore", log, log], detached: true });
    r.child = child;
    child.on("error", () => done(127));
    child.on("close", (code, signal) => done(code ?? (signal ? 128 : 1)));
  });
}

/// Cancels a job: TERM to its step's process group, and every process carrying its cookie,
/// in the group or out of it, TERMed then KILLed (reap.ts).
export function stop(r: Running, graceMs?: number): void {
  r.cancelled = true;
  try {
    if (r.child?.pid) process.kill(-r.child.pid, "SIGTERM");
  } catch {}
  const id = r.job.id;
  r.reaping ??= reap((j) => j === id, graceMs);
  r.go?.();
}

/// Stops the processes of this box's jobs that no running job of this daemon owns (a daemon
/// restarted during them, or anything a job's end missed), saying so in each job's log.
export async function reapOrphans(b: Box, mine: ReadonlySet<string>, graceMs?: number): Promise<Proc[]> {
  const ours = (j: string) => !mine.has(j) && ID.test(j) && existsSync(join(b.jobs, j));
  if (!scan(ours).length) return [];
  const r = await reap(ours, graceMs);
  for (const id of new Set(r.found.map((p) => p.job))) {
    const log = join(outDir(b, id), "job.log");
    const one = { found: r.found.filter((p) => p.job === id), left: r.left.filter((p) => p.job === id) };
    if (existsSync(dirname(log))) append(log, `render-worker: ${reaped(id, one, "found running after its job ended (the daemon restarted during it, or it escaped its end)")}\n`);
    say(`${one.left.length ? "✗" : "✓"} ${id}: ${reaped(id, one, "orphans")}`);
  }
  return r.found;
}

/// What a step wrote to the log since `from`, at most its last `most` bytes.
function since(file: string, from: number, most = 4 * 1024 ** 2): string {
  const size = statSync(file).size;
  const start = Math.max(from, size - most);
  const buf = Buffer.alloc(Math.max(0, size - start));
  const fd = openSync(file, "r");
  readSync(fd, buf, 0, buf.length, start);
  closeSync(fd);
  return buf.toString("utf8");
}

/// The checkout's workspace packages, none when it is no Cargo workspace.
function packagesOf(src: string): string[] {
  const read = (p: string) => (existsSync(join(src, p)) ? readFileSync(join(src, p), "utf8") : undefined);
  const list = (d: string) => (existsSync(join(src, d)) ? readdirSync(join(src, d)) : []);
  return workspacePackages(read("Cargo.toml") ?? "", read, list);
}

/// The packages the checkout's Cargo.lock names, none without one.
function lockedOf(src: string): string[] {
  const lock = join(src, "Cargo.lock");
  return existsSync(lock) ? [...readFileSync(lock, "utf8").matchAll(/^name = "([^"]+)"$/gm)].map((m) => m[1]) : [];
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

/// Removes the checkouts a job left inside the slot's build (`checkoutsInBuild`), so no crate
/// cargo built from one stands in for the slot's own; says so in the job's log.
function clearBuildCheckouts(b: Box, src: string, note: (line: string) => void): void {
  const found = checkoutsInBuild(git(b.repo, ["worktree", "list", "--porcelain"]).stdout, join(src, "target"));
  for (const dir of found) {
    note(
      `a checkout left in the slot's build, ${relative(src, dir)}: removed, as cargo would go on judging the crates built from it by its files, not this checkout's, and link its code (a failed fire gate's base, or a script's worktree under target/; put one outside target/ with a target of its own)`,
    );
    git(b.repo, ["worktree", "remove", "--force", dir]);
    rmSync(dir, { recursive: true, force: true });
  }
  if (found.length) git(b.repo, ["worktree", "prune"]);
}

/// Ends whatever of a job still runs, in its process group or out of it, once its steps are
/// done, and says in its log that nothing of it is left (or what is); what to add to its note.
async function endProcesses(id: string, r: Running, why: string, note: (line: string) => void): Promise<string> {
  const ended = r.cancelled ? "cancelled" : why.startsWith("stopped at its limit") ? "timed out" : "the job ended";
  const early = r.reaping ? await r.reaping : { found: [], left: [] };
  const last = await reap((j) => j === id);
  const found = [...early.found, ...last.found.filter((p) => !early.found.some((q) => q.pid === p.pid))];
  note(reaped(id, { found, left: last.left }, ended));
  if (last.left.length) return `${last.left.length} of its processes still run after SIGKILL (job.log names them)`;
  return last.found.length ? `stopped ${last.found.length} of its processes left running` : "";
}

/// A line in a log that is lost, not thrown, when the disk is full (guard.ts).
function append(file: string, text: string): void {
  try {
    appendFileSync(file, text);
  } catch (e) {
    if (!diskFull(e)) throw e;
  }
}

/// Runs one job to its end in its slot: its steps in the slot's checkout, then its files into out/.
/// `save` writes its record (the daemon's keeps it to write again when the disk is full).
export async function runJob(b: Box, queued: Job, slot = 1, r: Running = { job: queued, cancelled: false }, display?: Display, pCores?: string, save = (j: Job) => writeJob(b, j)): Promise<Job> {
  const bench = queued.kind === "run" && classOf(queued) === "bench";
  let job: Job = { ...queued, slot, state: "running", started: new Date().toISOString(), ...(bench ? { phase: "prep" as const } : {}) };
  r.job = job;
  save(job);
  const out = outDir(b, job.id);
  mkdirSync(out, { recursive: true });
  const logFile = join(out, "job.log");
  writeFileSync(logFile, "");
  const note = (line: string) => append(logFile, `render-worker: ${line}\n`);
  const log = openSync(logFile, "a");
  let exit = 0;
  let why = "";
  let startedMs = Date.now();
  let ran = false;
  let cleaned = "";
  let iced = "";
  let strays = "";
  let nospace = "";
  const absent: string[] = [];
  const home = slotHome(b, slot);
  let src = slotDir(b, slot);
  try {
    note(`slot ${slot}: ${src}`);
    src = slotCheckout(b, slot, job.rev);
    clearBuildCheckouts(b, src, note);
    if (job.patch) {
      // A --dirty job: say so first, and keep the patch beside the log, so its files are never
      // taken for a commit's.
      note(`PATCH: this job runs uncommitted changes on ${job.patch.base} (${job.patch.stat}), not a commit; the patch is patch.diff beside this log`);
      const fd = openSync(join(out, "patch.diff"), "w");
      spawnSync("git", ["--no-pager", "diff", "--binary", job.patch.base, job.rev], { cwd: src, stdio: ["ignore", fd, "ignore"] });
      closeSync(fd);
    }
    const dpy = display && needsDisplay(job) ? await display.get() : undefined;
    if (display && needsDisplay(job)) note(`display: ${dpy ?? (display.why ? `none, ${display.why}` : display.describe())}`);
    for (const step of steps(job, pCores)) {
      if (r.cancelled) break;
      if (bench && !step.prep && job.phase === "prep") {
        // Built: hold the slot until the box is empty but for this bench, then time it alone.
        job = { ...job, phase: "ready", prepared: new Date().toISOString() };
        r.job = job;
        save(job);
        note(`built ahead in ${Math.round((Date.parse(job.prepared as string) - Date.parse(job.started as string)) / 1000)} s; waiting for the box to empty`);
        await new Promise<void>((resolve) => {
          r.go = () => {
            r.go = undefined;
            if (r.cancelled) return resolve();
            job = { ...job, phase: "timed", timed: new Date().toISOString() };
            r.job = job;
            save(job);
            note(`the box is empty after ${Math.round((Date.parse(job.timed as string) - Date.parse(job.prepared as string)) / 1000)} s: the timed run has it alone`);
            resolve();
          };
          r.ready?.();
        });
        if (r.cancelled) break;
      }
      note(`${step.what}: ${step.cmd.join(" ")}`);
      if (step.run && !ran) (startedMs = Date.now()), (ran = true);
      const cmd = step.run || step.build ? [...limitArgs(job), ...step.cmd] : step.cmd;
      const env = step.run || step.build || step.cmd[0] === "cargo" ? jobEnv(process.env, home, job.id, join(src, "target"), dpy) : process.env;
      const from = statSync(logFile).size;
      let code = await run(cmd, src, env, log, r);
      const failed = code !== 0 && code !== 124 && (step.run || step.build) && !r.cancelled;
      if (failed && !nospace && NO_SPACE.test(since(logFile, from))) nospace = step.what;
      // rustc crashing (any ICE) is often its incremental state's fault, a corrupt session or
      // a query answered from it: the crates' sessions are removed and the step runs once more
      // without incremental, so a cache that breaks again cannot fail it twice.
      const ice = failed && !iced ? rustcIce(since(logFile, from), join(src, "target")) : undefined;
      if (ice) {
        const gone = incrementalClears(ice, (d) => (existsSync(d) ? readdirSync(d) : []));
        const named = ice.crates.length ? ice.crates.join(", ") : "every crate";
        iced = ice.cache
          ? `rustc crashed on slot ${slot}'s incremental cache of ${named}: removed it and ran it again with CARGO_INCREMENTAL=0`
          : `rustc crashed (an ICE) on slot ${slot} building ${named}: removed its incremental sessions and ran it again with CARGO_INCREMENTAL=0`;
        const sessions = `${named}'s incremental sessions (${gone.length ? gone.map((p) => relative(src, p)).join(", ") : "none left"})`;
        note(
          ice.cache
            ? `rustc crashed on its incremental cache (${ice.why}), not on the code: removing ${sessions} and running it again with CARGO_INCREMENTAL=0. ` +
                `Nothing to do: the next incremental build makes the cache anew. If the run below crashes too, the toolchain is at fault, not the cache (reinstall it on the box with rustup).`
            : `rustc crashed, an internal compiler error (${ice.why}), which an incremental build can hit where one without incremental does not: removing ${sessions} and running it again with CARGO_INCREMENTAL=0. ` +
                `If the run below crashes too, it is a rustc bug the code hits, not the slot: work round it in the code or on another toolchain, with the query stack above.`,
        );
        for (const p of gone) rmSync(p, { recursive: true, force: true });
        if (!r.cancelled) {
          note(`${step.what} again, CARGO_INCREMENTAL=0: ${step.cmd.join(" ")}`);
          const again = statSync(logFile).size;
          code = await run(cmd, src, { ...env, CARGO_INCREMENTAL: "0" }, log, r);
          if (code !== 0 && !r.cancelled && rustcIce(since(logFile, again), join(src, "target"))) {
            iced += "; it crashed again without incremental, a rustc bug the code hits, not the slot";
            note("rustc crashed again with CARGO_INCREMENTAL=0: a rustc bug the code hits, not the slot's cache; work round it in the code or on another toolchain");
          }
        }
      }
      // A build that broke on the slot's stale artifacts (a link error naming a workspace
      // crate, or a registry crate's corrupt rlib) is cleaned of them and run once more, as
      // build farms retry a cache's fault.
      const stale = failed && !ice && code !== 0 && !r.cancelled && !cleaned ? staleBuild(since(logFile, from), packagesOf(src), lockedOf(src)) : undefined;
      if (stale) {
        cleaned = stale.packages.join(", ");
        note(`stale build (${stale.why}): cleaning ${cleaned} and running it again`);
        const buildEnv = jobEnv(process.env, home, job.id, join(src, "target"), dpy);
        for (const clean of cleanCommands(stale)) {
          note(`clean: ${clean.join(" ")}`);
          await run(clean, src, buildEnv, log, r);
        }
        if (!r.cancelled) {
          note(`${step.what} again: ${step.cmd.join(" ")}`);
          code = await run(cmd, src, env, log, r);
        }
      }
      if (code !== 0) {
        exit = code;
        why = r.cancelled ? "cancelled" : step.run ? (code === 124 ? `stopped at its limit of ${job.minutes} min` : `the command exited ${code}`) : `${step.what} failed`;
        break;
      }
    }
    strays = await endProcesses(job.id, r, why, note);
    const files = job.kind === "run" ? gather(job, src, home, startedMs, (p) => absent.push(p)) : [];
    for (const p of absent) note(`--collect ${p}: not there when the job ended (${p.startsWith("~/") ? `under the slot's HOME, ${home}` : `in the checkout, ${src}`}), so nothing came from it; check the path the command writes, one path a --collect`);
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
    why = diskFull(e) ? `the box's disk filled during it (${(e as Error).message})` : (e as Error).message;
    note(why);
    strays ||= await endProcesses(job.id, r, why, note);
  }
  closeSync(log);
  if (nospace) why += `; the box's disk filled during its ${nospace} ("No space left on device" in its log): the daemon prunes, and takes no job while the disk stays full; run it again`;
  if (cleaned) why += `; cleaned a stale build of ${cleaned} and ran it again`;
  if (iced) why += `; ${iced}`;
  if (strays) why += `; ${strays}`;
  if (absent.length) why += `; not there to collect: ${absent.join(", ")}`;
  job = { ...job, state: r.cancelled ? "cancelled" : exit ? "failed" : "done", exit, ended: new Date().toISOString(), note: why };
  save(job);
  return job;
}

/// What a tool prints when a write fails for want of space.
const NO_SPACE = /No space left on device|\bENOSPC\b/;

/// Deletes jobs past their days, with their files and refs; fails jobs a dead daemon left running
/// (any running job not in `mine`, the jobs this daemon runs or holds a record of to write).
export function housekeep(b: Box, keepDays: number, now = new Date(), mine: ReadonlySet<string> = new Set(), save = (j: Job) => writeJob(b, j)): void {
  for (const j of listJobs(b)) {
    if (j.state === "running" && !mine.has(j.id)) save({ ...j, state: "failed", exit: 1, ended: now.toISOString(), note: "the daemon stopped during it" });
  }
  for (const j of expired(listJobs(b), now, keepDays)) {
    rmSync(join(b.jobs, j.id), { recursive: true, force: true });
    if (j.kind === "run") git(b.repo, ["update-ref", "-d", jobRef(j)]);
  }
}

// ---- The disk (guard.ts) ----

/// The slots as the pruner sees them; a HOME review cache only when it holds files from before
/// `before` (ms).
function slotUses(b: Box, slots: number, busy: ReadonlySet<number>, before: number): SlotUse[] {
  const out: SlotUse[] = [];
  for (let n = 1; n <= slots; n++) {
    const target = join(slotDir(b, n), "target");
    // A target/ holds files too (logs a script left): only folders with an incremental/ in them.
    const profiles = existsSync(target) ? readdirSync(target, { withFileTypes: true }).filter((e) => e.isDirectory() && lstatSync(join(target, e.name, "incremental"), { throwIfNoEntry: false })?.isDirectory()).map((e) => e.name) : [];
    const review = join(slotHome(b, n), ".cache/terra-review");
    const files: string[] = [];
    if (!busy.has(n)) walk(review, files);
    const old = files.some((f) => (statSync(f, { throwIfNoEntry: false })?.mtimeMs ?? before) < before);
    out.push({ slot: n, busy: busy.has(n), target, profiles, ...(old ? { review } : {}) });
  }
  return out;
}

/// Ended jobs whose out/ holds more than their log.
function jobUses(b: Box): JobUse[] {
  return listJobs(b)
    .filter((j) => j.state !== "queued" && j.state !== "running")
    .map((j) => ({ id: j.id, state: j.state, queued: j.queued, ended: j.ended, out: outDir(b, j.id) }))
    .filter((j) => existsSync(j.out) && readdirSync(j.out).some((n) => !KEEP.includes(n)));
}

/// Removes one thing `pruneOrder` named, off the event loop (fs/promises), so the daemon keeps
/// its heartbeat through a prune of hundreds of GB.
async function removePrune(p: Prune, now = new Date()): Promise<void> {
  if (p.kind === "dir") return rm(p.path, { recursive: true, force: true });
  if (p.kind === "media") {
    for (const name of readdirSync(p.path).filter((n) => !KEEP.includes(n))) await rm(join(p.path, name), { recursive: true, force: true });
    return append(join(p.path, "job.log"), `render-worker: its collected files were deleted ${now.toISOString()} to free the box's disk (older than TERRA_RENDER_MEDIA_DAYS); this log is kept\n`);
  }
  const files: string[] = [];
  walk(p.path, files);
  for (const f of files) if ((statSync(f, { throwIfNoEntry: false })?.mtimeMs ?? Number.POSITIVE_INFINITY) < (p.before ?? 0)) await rm(f, { force: true });
}

/// Writes daemon.json whole and renames it into place; on a full disk, over the blocks the
/// file already holds (it is padded to 4 KiB for that), so `--status` still reads the daemon's
/// last word when not one new block can be had.
export function writeStatus(file: string, text: string): void {
  const bytes = Buffer.byteLength(text);
  const padded = Buffer.from(text.padEnd(text.length + Math.ceil((bytes + 1) / 4096) * 4096 - bytes, " "));
  try {
    writeFileSync(`${file}.tmp`, padded);
    renameSync(`${file}.tmp`, file);
  } catch (e) {
    if (!diskFull(e)) throw e;
    rmSync(`${file}.tmp`, { force: true });
    const size = statSync(file, { throwIfNoEntry: false })?.size ?? 0;
    if (size < padded.length) throw e;
    const fd = openSync(file, "r+");
    try {
      writeSync(fd, Buffer.concat([padded, Buffer.alloc(size - padded.length, " ")]), 0, size, 0);
    } finally {
      closeSync(fd);
    }
  }
}

/// The daemon's hold on its disk: before each pass's job starts, it writes again the job
/// records a full disk kept it from writing (`save` keeps them rather than throw), does the
/// housekeeping, and prunes when free space is under the low mark (guard.ts `pruneOrder`). A
/// full disk (ENOSPC on any write, or free space under the full mark after pruning) never
/// throws out of `pass`: it returns why, and the daemon starts no job while it says so.
export class Keeper {
  readonly unsaved = new Map<string, Job>();
  /// The first write this pass that failed for want of space.
  noSpace = "";
  disk?: DiskState;
  readonly b: Box;
  readonly ds: DiskSettings;
  readonly log: (line: string) => void;
  readonly measure: () => Space;
  constructor(b: Box, ds: DiskSettings, log: (line: string) => void = say, measure: () => Space = () => space(b.dir)) {
    this.b = b;
    this.ds = ds;
    this.log = log;
    this.measure = measure;
  }

  save = (j: Job) => {
    try {
      writeJob(this.b, j);
      this.unsaved.delete(j.id);
    } catch (e) {
      if (!diskFull(e)) throw e;
      this.unsaved.set(j.id, j);
      this.noSpace ||= (e as Error).message;
    }
  };

  async pass(keepDays: number, running: ReadonlyMap<string, Running>, slots: number, now = new Date()): Promise<string> {
    this.noSpace = "";
    try {
      for (const j of [...this.unsaved.values()]) this.save(j);
      const mine = new Set([...running.keys(), ...this.unsaved.keys()]);
      housekeep(this.b, keepDays, now, mine, this.save);
      await reapOrphans(this.b, mine);
      try {
        await this.prune(slots, new Set([...running.values()].map((r) => r.job.slot ?? 1)), now);
      } catch (e) {
        // Pruning that breaks on something else (a file it cannot remove) is said, never fatal.
        if (diskFull(e)) throw e;
        this.log(`✗ disk: pruning stopped: ${(e as Error).message}`);
      }
    } catch (e) {
      if (!diskFull(e)) throw e;
      this.noSpace ||= (e as Error).message;
    }
    const sp = this.measure();
    const full = this.noSpace ? `a write failed: ${this.noSpace}` : sp.free < fullMark(this.ds) ? `${gb(sp.free)} free, under the ${gb(fullMark(this.ds))} mark` : "";
    if (full && !this.disk?.full) this.log(`✗ disk full (${full}): no job starts until space is freed; cargo xtask remote-render --status says what holds it`);
    if (!full && this.disk?.full) this.log(`✓ disk: ${gb(sp.free)} free again; taking jobs`);
    this.disk = { ...sp, at: now.toISOString(), ...(full ? { full } : {}) };
    return full;
  }

  /// Prunes in `pruneOrder`'s order while free space is under the low mark (or a write failed),
  /// up to `pruneTo`, saying what it freed.
  async prune(slots: number, busy: ReadonlySet<number>, now: Date): Promise<void> {
    const sp = this.measure();
    if (level(this.ds, sp) === "ok" && !this.noSpace) return;
    const items = pruneOrder(slotUses(this.b, slots, busy, now.getTime() - this.ds.mediaDays * 86400_000), jobUses(this.b), now, this.ds.mediaDays);
    if (!items.length) return;
    this.log(`disk: ${gb(sp.free)} free of ${gb(sp.total)}, under the low mark of ${gb(lowMark(this.ds, sp.total))}: pruning to ${gb(pruneTo(this.ds, sp.total))}`);
    await prune(items, pruneTo(this.ds, sp.total), () => this.measure().free, (p) => removePrune(p, now), this.log);
    this.log(`disk: ${gb(this.measure().free)} free after pruning`);
  }
}

/// The jobs the policy and the free slots let start now, each with its slot, the bench built
/// ahead to let go to its timed run, and why the rest wait.
export function next(b: Box, policy: Policy, ciLock: string | undefined, slots: Slots, now = new Date()): { start: Start[]; go?: Job; why?: string } {
  const jobs = listJobs(b);
  if (!jobs.some((j) => j.state === "queued" || (j.state === "running" && j.phase === "ready"))) return { start: [] };
  const may = mayStart(policy, now.getHours() * 60 + now.getMinutes(), policy.afterCi && ciBusy(ciLock, now.getTime()));
  return may.ok ? plan(jobs, slots) : { start: [], why: may.why };
}

async function daemon(b: Box): Promise<number> {
  const s = boxSettings(boxEnv(b));
  if ("error" in s) {
    say(`✗ ${s.error}`);
    return 1;
  }
  const dp = displayPlan(boxEnv(b));
  if ("error" in dp) {
    say(`✗ ${dp.error}`);
    return 1;
  }
  const ds = diskSettings(boxEnv(b));
  if ("error" in ds) {
    say(`✗ ${ds.error}`);
    return 1;
  }
  const display = new Display(dp, say);
  const running = new Map<string, Running>();
  const keeper = new Keeper(b, ds);
  // A daemon stopped mid-job takes its jobs' processes with it (systemd would kill its cgroup,
  // the supervisor would not); one that dies uncleanly leaves them to the next (`reapOrphans`).
  process.on("exit", () => (display.stop(), killNow((j) => running.has(j))));
  for (const sig of ["SIGTERM", "SIGINT"] as const) process.on(sig, () => process.exit(128 + (sig === "SIGTERM" ? 15 : 2)));
  const rev = git(b.repo, ["rev-parse", "--short=9", "HEAD"]).stdout.trim();
  const since = new Date().toISOString();
  const mode = process.env.TERRA_RENDER_MODE ?? "by hand";
  let status = "starting";
  // Its last word, on every pass and every 30 s between (a prune of hundreds of GB takes
  // minutes), so `--status` and a client's wait know it alive; never thrown for a full disk.
  const beat = () => {
    try {
      writeStatus(join(b.dir, "daemon.json"), JSON.stringify({ pid: process.pid, mode, rev, since, beat: new Date().toISOString(), status, display: display.describe(), disk: keeper.disk }));
    } catch (e) {
      if (!diskFull(e)) throw e;
    }
  };
  beat();
  setInterval(beat, 30_000).unref();
  // A bench runs on the performance cores of a hybrid processor (jobs.ts `steps`).
  const lscpu = spawnSync("lscpu", ["-p=CPU,MAXMHZ"], { encoding: "utf8" });
  const pCores = lscpu.status === 0 && spawnSync("taskset", ["-V"]).status === 0 ? performanceCores(lscpu.stdout) : undefined;
  say(`render daemon at ${rev}, ${mode}, pid ${process.pid}, ${s.slots.slots} slots (${s.slots.gpuSlots} for GPU jobs), benches on ${pCores ? `processors ${pCores}` : "any processor"}`);
  let wakeUp = () => {};
  watch(b.dir, (_e, name) => name === "wake" && wakeUp());
  let lastWhy = "";
  for (;;) {
    // Housekeeping and the disk's check before any job starts (guard.ts); a full disk stops
    // new jobs, not the daemon.
    const full = await keeper.pass(s.keepDays, running, s.slots.slots);
    const restart = existsSync(join(b.dir, "restart"));
    if (restart && !running.size && !keeper.unsaved.size) {
      rmSync(join(b.dir, "restart"), { force: true });
      say("restarting on new code");
      return 0;
    }
    // Restarting, nothing new starts, but a bench built ahead still has its timed run.
    const planned = next(b, s.policy, s.ciLock, s.slots);
    const n = full
      ? { start: [], why: `the box's disk is full (${full}); no job starts until space is freed (cargo xtask remote-render --status says what holds it)` }
      : restart
        ? { start: [], go: planned.go, why: "restarting on new code once its jobs end" }
        : planned;
    if (n.go) {
      say(`slot ${n.go.slot}: ${n.go.id} has the box for its timed run`);
      running.get(n.go.id)?.go?.();
    }
    for (const { job, slot } of n.start) {
      const r: Running = { job, cancelled: false, ready: () => wakeUp() };
      running.set(job.id, r);
      say(`slot ${slot}: running ${job.id} (${short(job)})`);
      runJob(b, job, slot, r, display, pCores, keeper.save)
        .then(
          (j) => say(`${j.state === "done" ? "✓" : "✗"} slot ${slot}: ${j.id} ${j.state}: ${j.note}`),
          (e) => {
            // A job's run that throws (its folder unwritable on a full disk) fails that job, not the daemon.
            stop(r);
            const why = diskFull(e) ? `the box's disk filled during it (${(e as Error).message})` : `the daemon failed running it: ${(e as Error).message}`;
            keeper.save({ ...r.job, state: "failed", exit: 1, ended: new Date().toISOString(), note: why });
            say(`✗ slot ${slot}: ${job.id} failed: ${why}`);
          },
        )
        .finally(() => {
          running.delete(job.id);
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
    append(log, `supervisor: the daemon exited ${code}; starting it again in 5 s\n`);
    await new Promise((r) => setTimeout(r, 5000));
  }
}

async function main(argv: string[]): Promise<number> {
  const at = argv.indexOf("--dir");
  const b = box(at >= 0 ? argv[at + 1] : resolve(import.meta.dirname, "../../.."));
  if (argv.includes("--selftest")) {
    const s = boxSettings(boxEnv(b));
    if ("error" in s) return console.log(s.error), 1;
    const dp = displayPlan(boxEnv(b));
    if ("error" in dp) return console.log(dp.error), 1;
    const ds = diskSettings(boxEnv(b));
    if ("error" in ds) return console.log(ds.error), 1;
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
