// The render worker (docs/render-worker.md, D445): takes render jobs from the store (draft
// GitHub releases), runs each in its own checkout at the job's commit, and uploads what it
// wrote. `--watch [seconds]` is what its container runs; `--once` takes one job if the policy
// allows; `--plan` prints the policy's verdict and the next job's steps and runs nothing.
// Settings from the environment: TERRA_RENDER_AFTER_CI (1: not while nightly runs),
// TERRA_RENDER_WINDOW (22:00-08:00, empty any hour), TERRA_RENDER_CI_LOCK (the CI agent's lock),
// TERRA_RENDER_STORE (dir:<path> for a folder instead of GitHub). Node 22.18+, git, gh, tar.

import { spawnSync } from "node:child_process";
import { closeSync, copyFileSync, existsSync, linkSync, lstatSync, mkdirSync, openSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { hostname } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { abandoned, ago, collected, expired, type Job, jobEnv, MAX_BYTES, mayStart, parseWindow, pick, type Policy, type Rec, steps } from "./jobs.ts";
import { open, type Store } from "./store.ts";

export type Worker = {
  /// The checkout the worker's own code runs from; its origin is where jobs' commits come from.
  root: string;
  /// The worker's folder (TERRA_RENDER_DIR, or `.git/terra-render`): `src` the job checkout (its build kept), `jobs/<id>` logs and files.
  state: string;
  /// The job's HOME; the box's own, so caches (cooked textures, shaders, clips) carry over.
  home: string;
  policy: Policy;
  ciLock?: string;
  name: string;
  days: number;
  say: (line: string) => void;
};

const git = (cwd: string, args: string[]) => spawnSync("git", args, { cwd, encoding: "utf8" });

export function stateDir(root: string): string {
  const common = git(root, ["rev-parse", "--path-format=absolute", "--git-common-dir"]).stdout.trim();
  return process.env.TERRA_RENDER_DIR || join(common, "terra-render");
}

/// Busy while the CI agent's lock is held, unless it is older than nightly ever takes.
export function ciBusy(lock: string | undefined, now = Date.now()): boolean {
  if (!lock || !existsSync(lock)) return false;
  return now - statSync(lock).mtimeMs < 6 * 3600_000;
}

export function settings(env: Record<string, string | undefined>): { policy: Policy; ciLock?: string } | { error: string } {
  const window = parseWindow(env.TERRA_RENDER_WINDOW ?? "");
  if (window && "error" in window) return window;
  return { policy: { window, afterCi: (env.TERRA_RENDER_AFTER_CI ?? "1") !== "0" }, ciLock: env.TERRA_RENDER_CI_LOCK || undefined };
}

function walk(path: string, out: string[]): void {
  const st = lstatSync(path, { throwIfNoEntry: false });
  if (!st || st.isSymbolicLink()) return;
  if (st.isFile()) out.push(path);
  else if (st.isDirectory()) for (const name of readdirSync(path)) walk(join(path, name), out);
}

/// The files a job hands back, as [source, name in files.tar]: what the command wrote under
/// each collected path since it started, `~/.cache/terra-review/` left off their names.
export function gather(job: Job, src: string, home: string, startedMs: number): [string, string][] {
  const review = join(home, ".cache/terra-review");
  const roots = job.collect.length ? job.collect.map((p) => (p.startsWith("~/") ? join(home, p.slice(2)) : join(src, p))) : [review];
  const found: [string, string][] = [];
  for (const root of roots) {
    const files: string[] = [];
    walk(root, files);
    for (const f of files) {
      if (!collected(relative(root, f), statSync(f).mtimeMs, startedMs)) continue;
      const name = f.startsWith(review + "/") ? relative(review, f) : f.startsWith(home + "/") ? join("home", relative(home, f)) : relative(src, f);
      if (!name.startsWith("..")) found.push([f, name]);
    }
  }
  return found;
}

function pack(files: [string, string][], dir: string): string {
  const stage = join(dir, "files");
  rmSync(stage, { recursive: true, force: true });
  for (const [from, name] of files) {
    const to = join(stage, name);
    mkdirSync(dirname(to), { recursive: true });
    try {
      linkSync(from, to);
    } catch {
      copyFileSync(from, to);
    }
  }
  const tar = join(dir, "files.tar");
  const r = spawnSync("tar", ["-cf", tar, "-C", stage, "."]);
  rmSync(stage, { recursive: true, force: true });
  if (r.status !== 0) throw new Error("tar failed");
  return tar;
}

/// Takes one job if one is queued and the policy allows: runs it and uploads its files.
export function takeOne(store: Store, w: Worker, now = new Date()): { did: "nothing" | "waiting" | "ran"; why?: string; rec?: Rec } {
  const recs = store.list();
  for (const r of expired(recs, now, w.days)) store.remove(r);
  for (const r of recs.filter((r) => abandoned(r, now))) {
    store.save(r, { ...r.job, state: "failed", ended: now.toISOString(), note: "the worker stopped during it" });
  }
  const rec = pick(store.list());
  if (!rec) return { did: "nothing" };
  const may = mayStart(w.policy, now.getHours() * 60 + now.getMinutes(), w.policy.afterCi && ciBusy(w.ciLock, now.getTime()));
  if (!may.ok) return { did: "waiting", why: may.why, rec };

  const job: Job = { ...rec.job, state: "running", worker: w.name, started: new Date().toISOString() };
  store.save(rec, job);
  const dir = join(w.state, "jobs", job.id);
  mkdirSync(dir, { recursive: true });
  const logFile = join(dir, "job.log");
  const src = join(w.state, "src");
  writeFileSync(logFile, "");
  const log = openSync(logFile, "a");
  const note = (line: string) => writeFileSync(logFile, `render-worker: ${line}\n`, { flag: "a" });
  let exit = 0;
  let why = "";
  try {
    if (!existsSync(join(src, ".git"))) {
      // A clone of its own rather than a worktree, so no worktree tool lists or clears it
      // (D259); its objects copied from the worker's checkout, its origin the same.
      const url = git(w.root, ["remote", "get-url", "origin"]).stdout.trim();
      const made = git(w.root, ["clone", "-q", "--no-checkout", "--reference-if-able", w.root, "--dissociate", url, src]);
      if (made.status !== 0) throw new Error(`could not make the job checkout: ${made.stderr.trim()}`);
    }
    let startedMs = Date.now();
    for (const step of steps(job)) {
      note(`${step.what}: ${step.cmd.join(" ")}`);
      const last = step.what === "run";
      if (last) startedMs = Date.now();
      const cmd = last ? ["timeout", "-k", "30", String(job.minutes * 60), ...step.cmd] : step.cmd;
      const env = last || step.cmd[0] === "cargo" ? jobEnv(process.env, w.home, job.id) : process.env;
      const r = spawnSync(cmd[0], cmd.slice(1), { cwd: src, env, stdio: ["ignore", step.what.startsWith("check the commit") ? "pipe" : log, log], encoding: "utf8" });
      if (step.what.startsWith("check the commit") && !(r.stdout ?? "").trim()) {
        throw new Error(`${job.rev.slice(0, 10)} is on no branch of origin; push it first`);
      }
      if (r.status !== 0) {
        exit = r.status ?? 1;
        why = last ? (exit === 124 ? `stopped at its limit of ${job.minutes} min` : `the command exited ${exit}`) : `${step.what} failed`;
        if (!last) throw new Error(why);
        break;
      }
    }
    closeSync(log);
    const files = gather(job, src, w.home, startedMs);
    const upload = [logFile];
    if (files.length) {
      const tar = pack(files, dir);
      if (statSync(tar).size > MAX_BYTES) {
        exit ||= 1;
        why = `its files are over 2 GiB (${Math.round(statSync(tar).size / 1024 ** 2)} MB); narrow them with --collect`;
      } else upload.push(tar);
    }
    store.upload(rec, upload);
    const size = files.reduce((n, [f]) => n + statSync(f).size, 0);
    store.save(rec, { ...job, state: exit ? "failed" : "done", exit, ended: new Date().toISOString(), note: why || `${files.length} files, ${Math.round(size / 1024 ** 2)} MB` });
  } catch (e) {
    try {
      closeSync(log);
    } catch {}
    note((e as Error).message);
    try {
      store.upload(rec, [logFile]);
    } catch {}
    store.save(rec, { ...job, state: "failed", exit: exit || 1, ended: new Date().toISOString(), note: (e as Error).message });
  }
  return { did: "ran", rec };
}

function sleep(ms: number): void {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

/// Fast-forwards the worker's own checkout; true when that changed this folder, so it exits
/// to be started again on the new code (the container's restart policy), as the CI agent does.
function updated(root: string): boolean {
  git(root, ["fetch", "-q", "origin", "main"]);
  const head = git(root, ["rev-parse", "HEAD"]).stdout.trim();
  const tip = git(root, ["rev-parse", "origin/main"]).stdout.trim();
  const onMain = git(root, ["symbolic-ref", "-q", "--short", "HEAD"]).stdout.trim() === "main";
  const clean = git(root, ["status", "--porcelain", "--untracked-files=no"]).stdout.trim() === "";
  if (!onMain || !clean || head === tip || git(root, ["merge", "-q", "--ff-only", "origin/main"]).status !== 0) return false;
  return git(root, ["diff", "--quiet", head, tip, "--", "tools/render-worker"]).status !== 0;
}

export function worker(root: string, env = process.env): Worker | { error: string } {
  const s = settings(env);
  if ("error" in s) return s;
  return { root, state: stateDir(root), home: env.HOME ?? "/root", ...s, name: env.TERRA_RENDER_NAME || hostname(), days: 7, say: (l) => console.log(l) };
}

function describe(w: Worker): string {
  const window = w.policy.window ? `${fmt(w.policy.window.from)}-${fmt(w.policy.window.to)}` : "any hour";
  return `${w.policy.afterCi ? "after CI" : "beside CI"}, ${window}`;
}
const fmt = (m: number) => `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`;

function report(w: Worker, r: ReturnType<typeof takeOne>): void {
  if (r.did !== "ran" || !r.rec) return;
  const j = r.rec.job;
  w.say(`${j.state === "done" ? "✓" : "✗"} render ${j.id}: ${j.state} (${ago(j.started ?? j.queued, new Date())}, ${j.note ?? ""})`);
}

function main(): number {
  const args = process.argv.slice(2);
  const root = resolve(import.meta.dirname, "../..");
  const w = worker(root);
  if ("error" in w) {
    console.log(`✗ render-worker: ${w.error}`);
    return 2;
  }
  const store = open(root);
  if (args[0] === "--plan") {
    const rec = pick(store.list());
    const now = new Date();
    const may = mayStart(w.policy, now.getHours() * 60 + now.getMinutes(), w.policy.afterCi && ciBusy(w.ciLock));
    console.log(`render-worker: ${describe(w)}; ${may.ok ? "would start a job now" : `would wait: ${may.why}`}`);
    if (!rec) console.log("  no job queued");
    else for (const s of steps(rec.job)) console.log(`  ${rec.job.id} ${s.what}: ${s.cmd.join(" ")}`);
    return 0;
  }
  if (args[0] === "--once") {
    const r = takeOne(store, w);
    report(w, r);
    if (r.did !== "ran") console.log(`render-worker: ${r.did === "waiting" ? `a job waits: ${r.why}` : "no job queued"}`);
    return 0;
  }
  if (args[0] !== "--watch") {
    console.log("usage: node tools/render-worker/worker.ts --watch [seconds] | --once | --plan");
    return 2;
  }
  const every = (Number(args[1]) || 30) * 1000;
  console.log(`render-worker: ${w.name} watching for jobs every ${every / 1000} s (${describe(w)})`);
  let beat = 0;
  let said = "";
  for (;;) {
    try {
      if (updated(root)) {
        console.log("render-worker: tools/render-worker changed; exiting to be started again");
        return 0;
      }
      const r = takeOne(store, w);
      report(w, r);
      const status = r.did === "waiting" ? `a job waits: ${r.why}` : "idle";
      if (status !== said && r.did === "waiting") console.log(`render-worker: ${status}`);
      if (status !== said || r.did === "ran" || Date.now() - beat > 5 * 60_000) {
        store.beat(`${w.name} (${describe(w)}): ${status}`);
        beat = Date.now();
      }
      said = status;
    } catch (e) {
      console.log(`✗ render-worker: ${(e as Error).message}`);
    }
    sleep(every);
  }
}

if (import.meta.main) process.exit(main());

