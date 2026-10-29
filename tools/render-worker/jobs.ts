// The render worker's decisions (docs/render-worker.md, D445), pure and tested in jobs.test.ts:
// the client's settings and arguments, a job and its steps on the box, which worker takes a job,
// when the box may start one, which jobs it starts in which slots, what it collects, and the
// job's environment.

export type State = "queued" | "running" | "done" | "failed" | "cancelled";

/// What a job takes of the box: `cpu` a slot (builds, tests, lints), `gpu` a slot and a share
/// of the card (renders; at most TERRA_RENDER_GPU_SLOTS at once), `bench` the box alone.
export type Class = "cpu" | "gpu" | "bench";

export type Job = {
  id: string;
  /// `run`: the agent's command. `warm`: the builds after an update, so the next job starts warm.
  kind: "run" | "warm";
  rev: string;
  command: string[];
  /// Paths to collect: relative to the checkout, or `~/…` to the job's home. Empty: what the
  /// command wrote under `~/.cache/terra-review`.
  collect: string[];
  /// Whether `cargo xtask cook` runs first (the game draws cooked textures, D188).
  cook: boolean;
  minutes: number;
  /// Absent on jobs queued before slots: `jobClass` of the command.
  class?: Class;
  /// The slot it runs in: set when it starts; a warm job is queued for its slot.
  slot?: number;
  by: string;
  queued: string;
  state: State;
  started?: string;
  ended?: string;
  /// A running bench's part (`benchPhase`): `prep` checks out, cooks and builds in its slot
  /// beside other jobs, `ready` holds its slot built until the box is empty, `timed` has the
  /// box alone for its measured run. Absent on a bench started before phases: `timed`.
  phase?: "prep" | "ready" | "timed";
  /// When a bench was built and ready, and when its timed run began.
  prepared?: string;
  timed?: string;
  exit?: number;
  note?: string;
  /// Set when the job runs uncommitted changes (`--dirty`): `rev` is then a snapshot of the
  /// agent's working tree, a commit object made without hooks whose parent is `base`, which is
  /// on no branch here or on origin. The id ends in `-patch`, so its files never pass for a commit's.
  patch?: Patch;
};

/// What a `--dirty` job's snapshot changes against its base: `git diff --shortstat`, and the files.
export type Patch = { base: string; stat: string; files: number };

export const DEFAULT_MINUTES = 120;
export const MAX_MINUTES = 480;
/// Days a job's files and its ref are kept on the box, unless TERRA_RENDER_KEEP_DAYS says.
export const KEEP_DAYS = 7;
/// Folders under the collected tree never collected: a film's intermediate frames, and
/// downloads of earlier jobs.
export const SKIP = ["frames", "remote"];
/// The builds a `warm` job runs after an update: what `cargo xtask sandbox` and `cargo xtask
/// bench` build, so a job after it builds only what its commit changed.
/// Clippy last, so the push hook's remote check (TERRA_REMOTE_CHECKS) lints only what changed.
export const WARM: string[][] = [
  ["cargo", "build", "-q", "-p", "xtask", "-p", "terra_sandbox"],
  ["cargo", "build", "-q", "--release", "-p", "terra_sandbox"],
  ["cargo", "clippy", "-q", "--workspace", "--all-targets"],
];
/// Slots, and of them those a GPU job may take, unless TERRA_RENDER_SLOTS and
/// TERRA_RENDER_GPU_SLOTS say: a 24 GB card holds about three films at 7 GB each.
export const SLOTS = 4;
export const GPU_SLOTS = 3;
/// The ref an update's commit is pushed to in the box's clone.
export const UPDATE_REF = "refs/worker/main";
export const jobRef = (job: Pick<Job, "id" | "kind">) => (job.kind === "warm" ? UPDATE_REF : `refs/jobs/${job.id}`);
export const ID = /^[0-9]{8}-[0-9]{6}-[0-9a-f]{4}(-patch)?$/;

/// `20261023-141502-a1b2`: sorts by when it was queued, and is a valid ref name. A job that runs
/// uncommitted changes ends in `-patch`, and so do its ref, its folder and its files' folder.
export function jobId(now: Date, rand: string, patch = false): string {
  const t = now.toISOString().replace(/[-:]/g, "").replace("T", "-").slice(0, 15);
  return `${t}-${rand.slice(0, 4)}${patch ? "-patch" : ""}`;
}

// ---- The client's settings: the environment over the checkout's .env (.env.example) ----

/// `KEY=value` lines; `#` comments, an `export ` prefix and quotes around the value allowed.
export function parseEnvFile(text: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const raw of text.split("\n")) {
    const line = raw.trim().replace(/^export\s+/, "");
    const m = /^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$/.exec(line);
    if (!m || line.startsWith("#")) continue;
    let v = m[2].trim();
    if (/^(["']).*\1$/.test(v)) v = v.slice(1, -1);
    else v = v.replace(/\s+#.*$/, "");
    out[m[1]] = v;
  }
  return out;
}

export type Config = {
  /// SSH hosts (aliases from ~/.ssh/config, or user@host), in order of preference; `local` is
  /// this machine without ssh (the tests).
  workers: string[];
  /// The worker's folder on each box, relative to its home unless absolute.
  dir: string;
  /// A job's limit when --minutes is not given.
  minutes: number;
  /// Seconds ssh waits to reach a box.
  connectTimeout: number;
};

const whole = (v: string | undefined, fallback: number, max: number) => {
  const n = v === undefined || v === "" ? fallback : Number(v);
  return Number.isInteger(n) && n >= 1 && n <= max ? n : undefined;
};

const HOST = /^[A-Za-z0-9_][A-Za-z0-9._@:-]*$/;
const DIR = /^[A-Za-z0-9_~/][A-Za-z0-9._/~-]*$/;

/// The settings, the environment's over the .env file's (every one in .env.example):
/// TERRA_WORKERS (a comma list of ssh hosts), TERRA_WORKER_DIR (default `terra-render`),
/// TERRA_RENDER_MINUTES and TERRA_RENDER_CONNECT_TIMEOUT.
export function config(env: Record<string, string | undefined>, file: Record<string, string> = {}): Config | { error: string } {
  const pickv = (k: string) => (env[k] !== undefined && env[k] !== "" ? env[k] : file[k]);
  const workers = (pickv("TERRA_WORKERS") ?? "")
    .split(",")
    .map((h) => h.trim())
    .filter(Boolean);
  if (!workers.length) return { error: "no render worker: set TERRA_WORKERS to an ssh host, in .env (copy .env.example) or the environment (tools/render-worker/README.md)" };
  for (const h of workers) if (!HOST.test(h)) return { error: `TERRA_WORKERS: ${h} is not an ssh host` };
  const dir = pickv("TERRA_WORKER_DIR") || "terra-render";
  if (!DIR.test(dir) || dir.split("/").includes("..")) return { error: `TERRA_WORKER_DIR=${dir}: a plain path` };
  const minutes = whole(pickv("TERRA_RENDER_MINUTES"), DEFAULT_MINUTES, MAX_MINUTES);
  if (!minutes) return { error: `TERRA_RENDER_MINUTES: a whole number from 1 to ${MAX_MINUTES}` };
  const connectTimeout = whole(pickv("TERRA_RENDER_CONNECT_TIMEOUT"), 10, 600);
  if (!connectTimeout) return { error: "TERRA_RENDER_CONNECT_TIMEOUT: whole seconds from 1 to 600" };
  return { workers, dir, minutes, connectTimeout };
}

// ---- The client's arguments ----

export type ClientArgs =
  | { mode: "queue"; rev?: string; dirty: boolean; command: string[]; collect: string[]; cook: boolean; minutes: number; out?: string; wait: boolean; worker?: string; class?: "cpu" | "gpu" }
  | { mode: "wait"; id: string; out?: string; worker?: string }
  | { mode: "cancel" | "logs"; id: string; worker?: string }
  | { mode: "status" | "update" | "doctor"; worker?: string };

export const USAGE =
  "usage: cargo xtask remote-render [--rev <sha> | --dirty] [--collect <path>]… [--minutes N] [--no-cook] [--gpu|--cpu] [--out DIR] [--no-wait] [--worker HOST] -- <command…>\n" +
  "       cargo xtask remote-render --wait <job> | --logs <job|daemon> | --cancel <job> | --status | --update | --doctor  [--worker HOST]";

export function parseClient(argv: string[], defaultMinutes = DEFAULT_MINUTES): ClientArgs | { error: string } {
  const dash = argv.indexOf("--");
  const opts = dash < 0 ? argv : argv.slice(0, dash);
  const command = dash < 0 ? [] : argv.slice(dash + 1);
  const collect: string[] = [];
  let rev: string | undefined;
  let out: string | undefined;
  let worker: string | undefined;
  let minutes = defaultMinutes;
  let cook = true;
  let wait = true;
  let dirty = false;
  let cls: "cpu" | "gpu" | undefined;
  let id = "";
  let mode: ClientArgs["mode"] = "queue";
  const setMode = (m: ClientArgs["mode"]) => {
    if (mode !== "queue") throw new Error(`--${mode} and --${m} together`);
    mode = m;
  };
  try {
    for (let i = 0; i < opts.length; i++) {
      const a = opts[i];
      const value = () => {
        const v = opts[++i];
        if (v === undefined || v.startsWith("--")) throw new Error(`${a} needs a value`);
        return v;
      };
      if (a === "--rev") rev = value();
      else if (a === "--collect") collect.push(value());
      else if (a === "--out") out = value();
      else if (a === "--worker") worker = value();
      else if (a === "--minutes") {
        minutes = Number(value());
        if (!Number.isInteger(minutes) || minutes < 1 || minutes > MAX_MINUTES) throw new Error(`--minutes is a whole number from 1 to ${MAX_MINUTES}`);
      } else if (a === "--no-cook") cook = false;
      else if (a === "--no-wait") wait = false;
      else if (a === "--dirty") dirty = true;
      else if (a === "--gpu" || a === "--cpu") {
        if (cls && cls !== a.slice(2)) throw new Error("--gpu and --cpu together");
        cls = a.slice(2) as "gpu";
      }
      else if (a === "--wait" || a === "--cancel" || a === "--logs") {
        setMode(a.slice(2) as "wait");
        id = value();
        if (!ID.test(id) && !(a === "--logs" && id === "daemon")) throw new Error(`${a} ${id}: not a job id (${a === "--logs" ? "or daemon, " : ""}e.g. 20261024-101500-a1b2)`);
      } else if (a === "--status" || a === "--update" || a === "--doctor") setMode(a.slice(2) as "status");
      else throw new Error(`unknown option ${a}`);
    }
  } catch (e) {
    return { error: (e as Error).message };
  }
  if (worker !== undefined && !HOST.test(worker)) return { error: `--worker ${worker}: not an ssh host` };
  if (mode === "status" || mode === "update" || mode === "doctor") return { mode, worker };
  if (mode === "cancel" || mode === "logs") return { mode, id, worker };
  if (mode === "wait") return { mode, id, out, worker };
  if (!command.length) return { error: "no command: put it after --" };
  if (dirty && rev) return { error: "--dirty runs the working tree's changes on HEAD; --rev names a commit instead" };
  for (const c of collect) {
    if (c.startsWith("/") || c.split("/").includes("..")) return { error: `--collect ${c}: a path in the checkout or under ~/, without ..` };
  }
  return { mode, rev, dirty, command, collect, cook, minutes, out, wait, worker, ...(cls ? { class: cls } : {}) };
}

/// Words after `cargo xtask` that only build, lint or test, and `cargo` commands that do.
const CPU_XTASK = ["check", "sizes", "seed", "decision"];
const CPU_CARGO = ["build", "check", "clippy", "doc", "fmt", "test", "tree"];

/// A job's class from its command, `--gpu` or `--cpu` overriding: a bench (`cargo xtask bench`,
/// `cargo xtask gpu bench`) always has the box alone; cargo's builds, tests and lints, and
/// `cargo xtask check`, are CPU; anything else (a trailer, the sandbox, a script) may draw.
export function jobClass(command: string[], flag?: "cpu" | "gpu"): Class {
  const cargo = command.indexOf("cargo");
  const words = command.slice(cargo + 1).filter((w) => !w.startsWith("-") && !w.startsWith("+"));
  const xtask = cargo >= 0 && words[0] === "xtask";
  if (xtask && (words[1] === "bench" || (words[1] === "gpu" && words[2] === "bench"))) return "bench";
  if (flag) return flag;
  if (cargo < 0) return "gpu";
  if (xtask) return CPU_XTASK.includes(words[1]) ? "cpu" : "gpu";
  return CPU_CARGO.includes(words[0]) ? "cpu" : "gpu";
}

/// A job's class: as queued, or for one queued before classes, from its command.
export const classOf = (job: Pick<Job, "class" | "command" | "kind">): Class => job.class ?? (job.kind === "warm" ? "cpu" : jobClass(job.command));

/// A running bench's part: `timed` unless it says (one started before phases had the box alone).
export const benchPhase = (job: Pick<Job, "phase">) => job.phase ?? "timed";

/// Whether a running job holds the box alone: a bench in its timed run.
const alone = (job: Job) => classOf(job) === "bench" && benchPhase(job) === "timed";

// ---- Which worker takes a job ----

/// What the client learns from a worker before queueing.
export type Seen = { host: string; reachable: boolean; alive: boolean; waiting: number };

/// The worker for a new job: a running daemon with the fewest jobs ahead, earlier in
/// TERRA_WORKERS on a tie; else the first one reachable, where the job waits for its daemon.
export function chooseWorker(seen: Seen[]): { host: string; note?: string } | { error: string } {
  const up = seen.filter((s) => s.reachable && s.alive).sort((a, b) => a.waiting - b.waiting);
  if (up.length) return { host: up[0].host };
  const reachable = seen.find((s) => s.reachable);
  if (reachable) return { host: reachable.host, note: `the daemon on ${reachable.host} is not running; the job waits for it (cargo xtask remote-render --update starts it)` };
  return { error: `no render worker answers over ssh (${seen.map((s) => s.host).join(", ")})` };
}

// ---- The box: when a job may start, which, and how it runs ----

/// A window of the day in minutes from midnight, `22:00-08:00` wrapping past midnight.
export type Window = { from: number; to: number };

export function parseWindow(text: string): Window | undefined | { error: string } {
  if (!text.trim()) return undefined;
  const m = /^(\d{1,2}):(\d{2})-(\d{1,2}):(\d{2})$/.exec(text.trim());
  const at = (h: string, min: string) => Number(h) * 60 + Number(min);
  if (!m || Number(m[1]) > 23 || Number(m[3]) > 23 || Number(m[2]) > 59 || Number(m[4]) > 59) {
    return { error: `TERRA_RENDER_WINDOW=${text}: give it as 22:00-08:00` };
  }
  return { from: at(m[1], m[2]), to: at(m[3], m[4]) };
}

export function inWindow(w: Window, minute: number): boolean {
  if (w.from === w.to) return true;
  return w.from < w.to ? minute >= w.from && minute < w.to : minute >= w.from || minute < w.to;
}

export type Policy = { window?: Window; afterCi: boolean };
export type Slots = { slots: number; gpuSlots: number };

/// The box's settings (its environment over the .env of the worker's clone there):
/// TERRA_RENDER_WINDOW (`22:00-08:00`, empty any hour), TERRA_RENDER_CI_LOCK (a CI agent's lock
/// file; no job starts while it is held), TERRA_RENDER_AFTER_CI (0 ignores that lock) and
/// TERRA_RENDER_KEEP_DAYS, TERRA_RENDER_SLOTS (jobs at once, default 4) and
/// TERRA_RENDER_GPU_SLOTS (of them, GPU jobs at once, default 3, at most the slots).
export function boxSettings(env: Record<string, string | undefined>): { policy: Policy; ciLock?: string; keepDays: number; slots: Slots } | { error: string } {
  const window = parseWindow(env.TERRA_RENDER_WINDOW ?? "");
  if (window && "error" in window) return window;
  const keepDays = whole(env.TERRA_RENDER_KEEP_DAYS, KEEP_DAYS, 365);
  if (!keepDays) return { error: "TERRA_RENDER_KEEP_DAYS: whole days from 1 to 365" };
  const slots = whole(env.TERRA_RENDER_SLOTS, SLOTS, 32);
  if (!slots) return { error: "TERRA_RENDER_SLOTS: a whole number from 1 to 32" };
  const gpuSlots = whole(env.TERRA_RENDER_GPU_SLOTS, GPU_SLOTS, 32);
  if (!gpuSlots) return { error: "TERRA_RENDER_GPU_SLOTS: a whole number from 1 to 32" };
  return {
    policy: { window, afterCi: (env.TERRA_RENDER_AFTER_CI ?? "1") !== "0" },
    ciLock: env.TERRA_RENDER_CI_LOCK || undefined,
    keepDays,
    slots: { slots, gpuSlots: Math.min(gpuSlots, slots) },
  };
}

/// Whether a job may start now: CI first, and within the window if set. A warm job waits too:
/// it takes the box as a render does.
export function mayStart(policy: Policy, minute: number, ciBusy: boolean): { ok: true } | { ok: false; why: string } {
  if (policy.afterCi && ciBusy) return { ok: false, why: "the CI agent is running" };
  if (policy.window && !inWindow(policy.window, minute)) return { ok: false, why: "outside the window" };
  return { ok: true };
}

export type Start = { job: Job; slot: number };

// ---- How long a job takes, from the jobs like it the box ran (for a bench's drain) ----

/// A command with its revisions and numbers blanked, so `sh film.sh art c3` and `… c4`, or a
/// check at another `--base`, are one kind; a warm build is its own, and so is a bench's build
/// ahead of its turn, whatever the bench (the same cook and build of the sandbox).
export const PREP = "bench prep";
export function commandKey(job: Pick<Job, "kind" | "command" | "phase">): string {
  if (job.kind === "warm") return "warm";
  if (job.phase === "prep") return PREP;
  return job.command.join(" ").replace(/\b[0-9a-f]{7,40}\b/g, "<rev>").replace(/\d+/g, "#");
}

/// How many ended jobs of a kind an estimate reads, the latest first, and how many CPU jobs of
/// any kind stand in for a CPU job of a kind never run.
const ESTIMATE_FROM = 5;
const CPU_FROM = 50;

/// A job's likely run time in ms from the last few of its kind (`commandKey`) the box ran to
/// their end: `long`, the longest of them (done or failed, at most the job's own limit), says
/// whether a job will be over in time; `short`, the shortest done, how long a running one has
/// at least. A CPU job of a new kind is as long as the longest of the box's last CPU jobs (a
/// test or lint, a minute or so; 5 at most in October's two busiest days); a GPU job of a new
/// kind, or a running one of any new kind, is undefined, so nothing is guessed.
export type Estimate = (job: Job, side: "long" | "short") => number | undefined;

export function estimator(jobs: Job[]): Estimate {
  type Run = { ms: number; done: boolean; ended: number };
  const runs = new Map<string, Run[]>();
  const cpu: Run[] = [];
  const add = (key: string, run: Run) => runs.set(key, [...(runs.get(key) ?? []), run]);
  for (const j of jobs) {
    if (!j.started || !j.ended || (j.state !== "done" && j.state !== "failed")) continue;
    const ended = Date.parse(j.ended);
    // A bench built ahead is two runs: its build (to when it was ready, or to its end if it
    // failed there) and its timed run, the part that has the box.
    if (j.phase) {
      const built = j.prepared ? Date.parse(j.prepared) : ended;
      add(PREP, { ms: built - Date.parse(j.started), done: !!j.prepared, ended: built });
      if (j.timed) add(commandKey({ ...j, phase: undefined }), { ms: ended - Date.parse(j.timed), done: j.state === "done", ended });
      continue;
    }
    const run = { ms: ended - Date.parse(j.started), done: j.state === "done", ended };
    add(commandKey(j), run);
    if (classOf(j) === "cpu" && j.kind === "run") cpu.push(run);
  }
  for (const list of runs.values()) list.sort((a, b) => b.ended - a.ended).splice(ESTIMATE_FROM);
  const cpuLong = cpu.length ? Math.max(...cpu.sort((a, b) => b.ended - a.ended).slice(0, CPU_FROM).map((r) => r.ms)) : undefined;
  return (job, side) => {
    const list = runs.get(commandKey(job)) ?? [];
    if (side === "long") {
      const long = list.length ? Math.max(...list.map((r) => r.ms)) : classOf(job) === "cpu" && job.kind === "run" ? cpuLong : undefined;
      return long === undefined ? undefined : Math.min(long, job.minutes * 60_000);
    }
    const done = list.filter((r) => r.done);
    return done.length ? Math.min(...done.map((r) => r.ms)) : undefined;
  };
}

/// When the box will have drained of its running jobs, at the soonest the estimates allow: the
/// latest of their earliest likely ends, `now` for any past its shortest run or of a kind never
/// run (it may end at any moment).
export function drainEnd(running: Job[], now: number, est: Estimate): number {
  let end = now;
  for (const j of running) {
    const short = est(j, "short");
    if (short !== undefined && j.started) end = Math.max(end, Date.parse(j.started) + short);
  }
  return end;
}

/// The jobs to start now, each in a free slot, and why the rest wait. Warm jobs first, since
/// they make every later one faster, each in the slot it warms; then the oldest queued. A GPU
/// job waits while TERRA_RENDER_GPU_SLOTS run, and a CPU job behind it may pass.
///
/// A bench has the box alone, card and cores, for its timed run, since a build beside it moves
/// its frame times (D185, D319): that run starts only on a box empty but for it and nothing
/// starts while it runs. What comes before it, the checkout, the cook and the build of what it
/// times (`buildAhead`), needs no quiet box and changes nothing measured (the same binary,
/// built in the same slot), so the first queued bench does it in a slot of its own as a CPU
/// job at its turn while the box drains (`prep`), then holds that slot built (`ready`) and is
/// let go (`go`) once the rest of the box is empty: its time alone is only the timed run.
///
/// Once a bench is queued the box drains for it. Jobs queued ahead of it keep their turn. A job
/// behind it starts only if it will be over before the box would empty anyway: the longest of
/// the last of its kind (`estimator`) ends before the running jobs' soonest end and the bench's
/// build (`drainEnd`), so it fills a free slot and never holds the bench back. On a box empty
/// but for the bench, it goes ahead of the jobs queued before it since the last one (back to
/// back) when that costs less waiting than draining again for it: those jobs times the bench's
/// run against the bench and the jobs behind it times the drain they would make. Replayed on
/// the box's queue of 27-28 October the drain rules cut the jobs' waiting by 14% and the
/// benches' by 2%, no bench ever beside another job (D319).
export function plan(jobs: Job[], s: Slots, now = Date.now()): { start: Start[]; go?: Job; why?: string } {
  const running = jobs.filter((j) => j.state === "running");
  const busy = new Set(running.map((j) => j.slot ?? 1));
  const queued = jobs.filter((j) => j.state === "queued").sort((a, b) => Number(b.kind === "warm") - Number(a.kind === "warm") || a.id.localeCompare(b.id));
  const start: Start[] = [];
  if (running.some(alone)) return { start, why: queued.length ? "a bench has the box" : undefined };
  // The bench building or built ahead (one at a time), else the first queued.
  const held = running.find((j) => classOf(j) === "bench");
  const others = running.filter((j) => j !== held);
  let gpu = others.filter((j) => classOf(j) !== "cpu").length;
  const benches = queued.filter((j) => classOf(j) === "bench").sort((a, b) => a.id.localeCompare(b.id));
  const bench = held ?? benches[0];
  const pinned = (j: Job) => (j.slot && j.slot <= s.slots ? j.slot : undefined);
  const reserved = new Set(queued.map(pinned).filter((n) => n !== undefined));
  const est = estimator(jobs);
  const busyWhy = queued.length ? "a bench has the box" : undefined;
  let first = false;
  if (bench && !others.length) {
    const ahead = queued.filter((j) => classOf(j) !== "bench" && j.id.localeCompare(bench.id) < 0);
    const behind = queued.filter((j) => j.id.localeCompare(bench.id) > 0).length;
    const ms = (j: Job) => est({ ...j, phase: undefined }, "long") ?? j.minutes * 60_000;
    const drain = Math.max(0, ...ahead.map(ms));
    first = ahead.length * ms(bench) <= (1 + behind) * drain;
    if (first && held?.phase === "ready") return { start, go: held, why: busyWhy };
  }
  // When the box will be empty for the bench: the others drained and the bench built.
  const built = (j: Job, from: number) => Math.max(now, from + (est({ ...j, phase: "prep" }, "short") ?? 0));
  let until = !bench ? Infinity : Math.max(drainEnd(others, now, est), held?.phase === "prep" && held.started ? built(held, Date.parse(held.started)) : now);
  let why: string | undefined = bench ? "a bench waits for the box to empty" : undefined;
  for (const job of queued) {
    const c = classOf(job);
    if (c === "bench") {
      if (job !== bench) continue;
      const slot = Array.from({ length: s.slots }, (_, i) => i + 1).find((n) => !busy.has(n) && !reserved.has(n));
      if (!slot) {
        why = `the bench waits for a slot to build in; all ${s.slots} slots are taken`;
        continue;
      }
      busy.add(slot);
      start.push({ job, slot });
      until = Math.max(until, built(job, now));
      continue;
    }
    // The bench goes ahead of the jobs queued before it on the box empty for it.
    if (bench && first && job.id.localeCompare(bench.id) < 0) continue;
    if (bench && job.id.localeCompare(bench.id) > 0) {
      const long = est(job, "long");
      if (long === undefined || now + long > until) continue;
    }
    if (c === "gpu" && gpu >= s.gpuSlots) {
      why ??= `the ${s.gpuSlots} GPU slots are taken`;
      continue;
    }
    const want = pinned(job);
    const slot = want ?? Array.from({ length: s.slots }, (_, i) => i + 1).find((n) => !busy.has(n) && !reserved.has(n));
    if (!slot || busy.has(slot)) {
      why ??= `all ${s.slots} slots are taken`;
      continue;
    }
    busy.add(slot);
    if (c === "gpu") gpu++;
    start.push({ job, slot });
  }
  // A box empty but for the built bench, where none of the jobs ahead could start after all: it goes.
  if (held?.phase === "ready" && !others.length && !start.length) return { start, go: held, why: busyWhy };
  return { start, why };
}

const span = (ms: number) => ago(new Date(0).toISOString(), new Date(Math.max(0, ms)));

/// What `--status` says of a bench's hold on the queue, nothing when no bench is queued or
/// running: how long the box has been draining for benches or running them without a break,
/// the bench running or the box's drain, the benches next, and the jobs waiting behind.
export function benchHold(jobs: Job[], now: Date): string[] {
  const benches = jobs.filter((j) => classOf(j) === "bench");
  const live = benches.filter((j) => j.state === "queued" || j.state === "running");
  if (!live.length) return [];
  // The hold began when the first of them was queued, or earlier if a bench was running then.
  let since = Math.min(...live.map((j) => Date.parse(j.queued)));
  for (let again = true; again; ) {
    again = false;
    for (const b of benches) {
      if (b.started && b.ended && Date.parse(b.started) < since && Date.parse(b.ended) >= since - 60_000 && Date.parse(b.queued) < since) {
        since = Date.parse(b.queued);
        again = true;
      }
    }
  }
  const est = estimator(jobs);
  const t = now.getTime();
  const running = jobs.filter((j) => j.state === "running");
  const bench = running.find(alone);
  const held = running.find((j) => classOf(j) === "bench" && !alone(j));
  const others = running.filter((j) => j !== held);
  const queued = jobs.filter((j) => j.state === "queued").sort((a, b) => a.id.localeCompare(b.id));
  const next = queued.filter((j) => classOf(j) === "bench");
  const kind = (j: Job) => {
    const long = est(j, "long");
    return long === undefined ? "" : `, its kind took up to ${span(long)}`;
  };
  const out = [`bench hold: ${span(t - since)}, since ${new Date(since).toISOString().slice(11, 16)} UTC`];
  if (bench) out.push(`  running ${bench.id} for ${ago(bench.timed ?? bench.started ?? bench.queued, now)}${kind(bench)}: ${short(bench)}`);
  else {
    if (held?.phase === "prep") out.push(`  building ${held.id} ahead in slot ${held.slot} for ${ago(held.started ?? held.queued, now)}: ${short(held)}`);
    if (held?.phase === "ready") out.push(`  ${held.id} built in slot ${held.slot} ${ago(held.prepared ?? held.queued, now)} ago, its timed run next: ${short(held)}`);
    const left = drainEnd(others, t, est) - t;
    out.push(`  draining: ${others.length} job${others.length === 1 ? "" : "s"} still running, the box empty ${left >= 60_000 ? `in ${span(left)} at the soonest` : "any moment"}; only jobs that end before then start`);
  }
  if (next.length) out.push(`  benches next: ${next.map((j) => `${j.id} (waiting ${ago(j.queued, now)}${kind(j)})`).join(", ")}`);
  const behind = queued.filter((j) => classOf(j) !== "bench");
  if (behind.length) {
    const n = (c: Class) => behind.filter((j) => classOf(j) === c).length;
    out.push(`  waiting behind it: ${behind.length} job${behind.length === 1 ? "" : "s"} (${n("gpu")} GPU, ${n("cpu")} CPU), the oldest ${behind[0].id} for ${ago(behind[0].queued, now)}`);
  }
  return out;
}

/// Jobs to delete with their files and ref: ended, or queued, longer than `days` ago.
export function expired(jobs: Job[], now: Date, days: number): Job[] {
  const limit = now.getTime() - days * 86400_000;
  return jobs.filter((j) => j.state !== "running" && Date.parse(j.ended ?? j.queued) < limit);
}

/// Seconds before the client tries ssh again after losing it while waiting.
export function backoff(n: number): number {
  return [5, 15, 30, 60][n] ?? 120;
}

/// The job's environment: the daemon's, with no token, HOME the slot's, and the build in the
/// slot's own `target/` (where the tools look for the sandbox), so slots never wait on each
/// other's cargo lock. Cargo and rustup keep their homes, which default to HOME's. `display`,
/// when given, is the X display the job draws through (`displayPlan`); it replaces any Wayland
/// one, so winit opens X.
export function jobEnv(env: Record<string, string | undefined>, home: string, id: string, target: string, display?: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(env)) {
    if (v === undefined || /TOKEN|SECRET|PASSWORD|COOKIE|API_KEY|^GH_|^GITHUB_|^SSH_AUTH_SOCK$/.test(k)) continue;
    out[k] = v;
  }
  const realHome = env.HOME ?? home;
  out.CARGO_HOME = env.CARGO_HOME ?? `${realHome}/.cargo`;
  out.RUSTUP_HOME = env.RUSTUP_HOME ?? `${realHome}/.rustup`;
  out.HOME = home;
  out.CARGO_TARGET_DIR = target;
  out.TERRA_RENDER_JOB = id;
  // Blender through Vulkan: over the box's Xvfb its OpenGL is software (doctor.ts `blender`).
  out.TERRA_BLENDER_GPU_BACKEND = env.TERRA_BLENDER_GPU_BACKEND ?? "vulkan";
  if (display) {
    out.DISPLAY = display;
    delete out.WAYLAND_DISPLAY;
  }
  return out;
}

// ---- The display a job draws through ----

/// Whether a job needs a display: winit's event loop wants one even offscreen, so every GPU job
/// and bench, and any sandbox run (a `--cpu` one too). Warm builds never open one.
export function needsDisplay(job: Pick<Job, "class" | "command" | "kind">): boolean {
  return job.kind === "run" && (classOf(job) !== "cpu" || job.command.some((w) => w === "sandbox" || w.includes("terra_sandbox")));
}

/// How the box gives jobs a display. TERRA_RENDER_DISPLAY (`:1`) fixes one. Unset or empty: the
/// daemon's own WAYLAND_DISPLAY or DISPLAY when it has one (a desktop session), else an Xvfb the
/// daemon starts on a free number and keeps for its lifetime (a systemd user service has none).
export type DisplayPlan = { how: "inherit"; said: string } | { how: "fixed"; display: string } | { how: "xvfb" };

export function displayPlan(env: Record<string, string | undefined>): DisplayPlan | { error: string } {
  const fixed = (env.TERRA_RENDER_DISPLAY ?? "").trim();
  if (fixed) {
    const m = /^:?(\d{1,4})(\.\d{1,2})?$/.exec(fixed);
    return m ? { how: "fixed", display: `:${m[1]}${m[2] ?? ""}` } : { error: `TERRA_RENDER_DISPLAY=${fixed}: an X display such as :1, or empty for the daemon's own or an Xvfb` };
  }
  if (env.WAYLAND_DISPLAY) return { how: "inherit", said: `WAYLAND_DISPLAY=${env.WAYLAND_DISPLAY}` };
  if (env.DISPLAY) return { how: "inherit", said: `DISPLAY=${env.DISPLAY}` };
  return { how: "xvfb" };
}

/// The display numbers in use, from the names in /tmp/.X11-unix (`X0`) and /tmp (`.X0-lock`).
export function takenDisplays(names: string[]): Set<number> {
  const taken = new Set<number>();
  for (const n of names) {
    const m = /^(?:X(\d+)|\.X(\d+)-lock)$/.exec(n);
    if (m) taken.add(Number(m[1] ?? m[2]));
  }
  return taken;
}

/// The first free display number from 99, clear of a desktop's low ones.
export function freeDisplay(taken: ReadonlySet<number>, from = 99): number {
  let n = from;
  while (taken.has(n)) n++;
  return n;
}

/// The Xvfb the daemon starts: a 1920x1080 screen, no TCP, never reset when its last client goes.
export const xvfbArgs = (n: number) => [`:${n}`, "-screen", "0", "1920x1080x24", "-nolisten", "tcp", "-noreset"];

/// Seconds before Xvfb is started again after it died: 2, doubling while it keeps dying within a
/// minute of starting, at most 60.
export const xvfbBackoff = (quickDeaths: number) => Math.min(60, 2 * 2 ** Math.max(0, quickDeaths - 1));

/// Whether a file found under a collected folder is collected: written since the job started
/// (a second's slack for coarse clocks), and in no skipped folder.
export function collected(relative: string, mtimeMs: number, startedMs: number): boolean {
  if (relative.split("/").some((part) => SKIP.includes(part))) return false;
  return mtimeMs >= startedMs - 1000;
}

/// `run`: the job's command (or a warm build). `prep`: a bench's step before its timed run,
/// done in its slot beside other jobs. `build`: a build ahead of a bench, held to the job's
/// limit and in the job's environment as a run is, so the run finds it fresh.
export type Step = { what: string; cmd: string[]; run?: boolean; prep?: boolean; build?: boolean };

/// What a bench's command will build before it times anything, built ahead in its slot with
/// the same arguments, profile and environment, so the bench's own build finds it fresh and
/// the binary timed is the one it would have built: xtask for any `cargo xtask`, and for
/// `cargo xtask bench` (alone or after `cargo xtask gpu bench --`) the sandbox in the bench
/// profile, the dev one with `--debug` (xtask/src/bench.rs `sandbox_program`). Another
/// command's own builds stay in its timed run.
export function buildAhead(command: string[]): string[][] {
  const builds: string[][] = [];
  for (let i = 0; i + 1 < command.length; i++) {
    if (command[i] !== "cargo" || command[i + 1] !== "xtask") continue;
    if (!builds.length) builds.push(["cargo", "build", "-q", "-p", "xtask"]);
    const rest = command.slice(i + 2);
    const words = rest.filter((w) => !w.startsWith("-"));
    if (words[0] === "bench") {
      const profile = rest.includes("--debug") ? "dev" : "bench";
      return [...builds, ["cargo", "build", "-q", "--profile", profile, "-p", "terra_sandbox"]];
    }
  }
  return builds;
}

/// The box's steps for a job in its slot's checkout, a worktree of the clone its commit was
/// pushed to, so the commit is there already: what each is, as a command, so `plan` can print
/// them and the daemon runs them in order. The `run` steps are held to the job's limit, and
/// niced, but a bench's: it has the box alone, so it runs at the normal priority (niced, the
/// box's own services outrank it), on the performance cores when `pCores` names them
/// (`performanceCores`), so its main and render threads never land on an efficiency core.
/// A bench's steps before its run are `prep`, with its builds ahead (`buildAhead`), niced:
/// they run while the box drains, and only the run has the box (`plan`).
export function steps(job: Job, pCores?: string): Step[] {
  const s: Step[] = [
    { what: "check out the job's commit", cmd: ["git", "checkout", "-q", "--detach", "--force", job.rev] },
    { what: "clean all but the build", cmd: ["git", "clean", "-q", "-ffdx", "-e", "target/"] },
  ];
  if (job.patch) s.push({ what: `PATCH, not a commit: uncommitted changes on ${job.patch.base.slice(0, 9)}`, cmd: ["git", "--no-pager", "diff", "--stat", job.patch.base, job.rev] });
  if (job.cook) s.push({ what: "cook textures", cmd: ["cargo", "xtask", "cook"] });
  const runs = job.kind === "warm" ? WARM : [job.command];
  const bench = job.kind === "run" && classOf(job) === "bench";
  if (bench) {
    for (const cmd of buildAhead(job.command)) s.push({ what: "build ahead of the box", cmd: ["nice", "-n", "10", ...cmd], build: true });
    for (const step of s) step.prep = true;
  }
  const prefix = bench ? (pCores ? ["taskset", "-c", pCores] : []) : ["nice", "-n", "10"];
  for (const cmd of runs) s.push({ what: job.kind === "warm" ? "warm build" : "run", cmd: [...prefix, ...cmd], run: true });
  return s;
}

/// The performance cores of a hybrid processor, as a `taskset -c` list (`0-15`), from
/// `lscpu -p=CPU,MAXMHZ`: the logical processors whose top clock is nearer the fastest's than
/// the slowest's (an i9-14900K's P-cores reach 5.7-6.0 GHz, its E-cores 4.4), none when every
/// processor tops out alike or lscpu does not say.
export function performanceCores(lscpu: string): string | undefined {
  const rows = lscpu
    .split("\n")
    .filter((line) => line.trim() && !line.startsWith("#"))
    .map((line) => line.split(",").map(Number))
    .filter(([cpu, mhz]) => Number.isInteger(cpu) && mhz > 0);
  if (!rows.length) return undefined;
  const top = Math.max(...rows.map(([, mhz]) => mhz));
  const low = Math.min(...rows.map(([, mhz]) => mhz));
  if (top - low < 0.1 * top) return undefined;
  const cpus = rows.filter(([, mhz]) => mhz > (top + low) / 2).map(([cpu]) => cpu).sort((a, b) => a - b);
  const ranges: string[] = [];
  for (let i = 0; i < cpus.length; ) {
    let j = i;
    while (j + 1 < cpus.length && cpus[j + 1] === cpus[j] + 1) j++;
    ranges.push(i === j ? `${cpus[i]}` : `${cpus[i]}-${cpus[j]}`);
    i = j + 1;
  }
  return ranges.join(",");
}

// ---- A stale build, cleaned and tried again ----

/// A failed run whose build broke on a slot's stale artifacts, not on its code: what said so,
/// the workspace packages to clean and the profiles they were built in.
export type StaleBuild = { why: string; packages: string[]; profiles: ("release" | "dev")[] };

// What a stale artifact looks like: the linker missing a symbol an old rlib lacks (GNU ld, lld,
// mold), rustc's own link failure, or rustc finding a crate's metadata older or broken
// (E0460, E0786). Build farms (Bazel's, Chromium's) treat these as the cache's fault: clean
// what is named and build once more.
const LINK_ERROR = /undefined reference to|undefined symbol|error: linking with|error\[E0460\]|error\[E0786\]/;
const SYMBOL_LINE = /undefined reference to|undefined symbol/;
const CRATE_LINE = /error\[E0460\]|error\[E0786\]/;

// lld's report under a missing symbol: the archive that wants it, and where a symbol of that
// name (or the one it suggests) is defined.
const WANTED_IN = /^\s*>>>.*\bin archive \S*\/lib([A-Za-z0-9_]+)-[0-9a-f]+\.rlib/;
const DEFINED_IN = /^\s*>>> defined in: \S*\/lib([A-Za-z0-9_]+)-[0-9a-f]+\.rlib/;

/// Whether a run step's output is a stale build's, and what to clean: the workspace packages
/// (`workspace`, their names as in Cargo.toml) named in its missing symbols or crate errors,
/// and the package that failed to compile, in the profile its paths show (both when none do).
/// A registry crate is cleaned only when its own rlib is corrupt: lld finds the rlib wanting a
/// symbol that lld says the same rlib defines (a code unit of it out of step with another,
/// as a build cut short leaves it); `registry` names Cargo.lock's packages, so the clean
/// names the package as Cargo does (erased-serde for the rlib liberased_serde).
/// Undefined when the output has no such error or names no package to clean, so a missing
/// system library or a real error is not run twice for nothing.
export function staleBuild(output: string, workspace: Iterable<string>, registry: Iterable<string> = []): StaleBuild | undefined {
  const at = output.search(LINK_ERROR);
  if (at < 0) return undefined;
  const failed = output.slice(at);
  const lines = failed.split("\n");
  const named = new Set<string>();
  const crate = (name: string) => name.replace(/-/g, "_");
  const names = [...workspace].map((p) => ({ p, re: new RegExp(`(?<![A-Za-z_])${crate(p)}(?![A-Za-z_])`) }));
  for (const line of lines.filter((l) => SYMBOL_LINE.test(l))) for (const { p, re } of names) if (re.test(line)) named.add(p);
  const byCrate = (name: string) => names.find(({ p }) => crate(p) === crate(name))?.p;
  for (const line of lines.filter((l) => CRATE_LINE.test(l))) for (const m of line.matchAll(/`([A-Za-z0-9_-]+)`/g)) if (byCrate(m[1])) named.add(byCrate(m[1]) as string);
  for (const m of failed.matchAll(/could not compile `([A-Za-z0-9_-]+)`/g)) if (byCrate(m[1])) named.add(byCrate(m[1]) as string);
  const locked = [...registry];
  let wanted: string | undefined;
  for (const line of lines) {
    if (SYMBOL_LINE.test(line)) wanted = undefined;
    const w = line.match(WANTED_IN)?.[1];
    if (w) wanted = w;
    const d = line.match(DEFINED_IN)?.[1];
    if (d && d === wanted && !byCrate(d)) named.add(locked.find((p) => crate(p) === d) ?? d);
  }
  if (!named.size) return undefined;
  const release = /\/release\/(deps|build|incremental)\//.test(failed);
  const dev = /\/debug\/(deps|build|incremental)\//.test(failed);
  const profiles: StaleBuild["profiles"] = release && !dev ? ["release"] : dev && !release ? ["dev"] : ["release", "dev"];
  const why = (lines.find((l) => SYMBOL_LINE.test(l) || CRATE_LINE.test(l)) ?? lines[0]).trim().replace(/\s+/g, " ").slice(0, 160);
  return { why, packages: [...named].sort(), profiles };
}

/// The commands that clean a stale build's packages, one per profile.
export const cleanCommands = (s: StaleBuild): string[][] =>
  s.profiles.map((profile) => ["cargo", "clean", ...(profile === "release" ? ["--release"] : []), ...s.packages.flatMap((p) => ["-p", p])]);

// ---- A corrupt incremental cache, cleared and built without it ----

/// A failed run whose rustc panicked in its incremental cache (an internal compiler error
/// with rustc_incremental's frames, as `SerializedDepGraph::decode` reading a session it cannot
/// parse): what said so, the crates that failed to compile (their names as in rustc's
/// `incremental/<crate>-<hash>` folders), and the incremental folders their rustc used, all
/// under the slot's target.
export type IncrementalIce = { why: string; crates: string[]; dirs: string[] };

const ICE = /the compiler unexpectedly panicked|error: internal compiler error/;
const INCREMENTAL_FRAME = /rustc_incremental\b|dep_graph::serialized|SerializedDepGraph/;

/// Whether a run step's output is rustc crashing on its incremental cache, not on the code:
/// an ICE whose backtrace passes through rustc_incremental. The folders are the
/// `-C incremental=` paths of the failed rustc commands that lie under `target`; when none
/// shows, release's and dev's. Undefined for any other failure, an ICE elsewhere included.
export function incrementalIce(output: string, target: string): IncrementalIce | undefined {
  if (!ICE.test(output)) return undefined;
  const frame = output.split("\n").find((l) => INCREMENTAL_FRAME.test(l));
  if (!frame) return undefined;
  const crates = new Set([...output.matchAll(/could not compile `([A-Za-z0-9_-]+)`/g)].map((m) => m[1].replace(/-/g, "_")));
  const dirs = new Set<string>();
  for (const m of output.matchAll(/-C incremental=(\/[^\s`'"]+)/g)) {
    const d = m[1].replace(/\/+$/, "");
    if (d.startsWith(`${target}/`) && !d.split("/").includes("..")) dirs.add(d);
  }
  if (!dirs.size) for (const p of ["release", "debug"]) dirs.add(`${target}/${p}/incremental`);
  const why = `rustc panicked in ${frame.replace(/^\s*\d+:\s*(0x[0-9a-f]+\s*-\s*)?/, "").replace(/\[[0-9a-f]+\]/g, "").trim().slice(0, 120)}`;
  return { why, crates: [...crates].sort(), dirs: [...dirs].sort() };
}

/// What to remove for an incremental ICE: each named crate's sessions (`<crate>-<hash>`) in
/// each folder, through `list` (a folder in, its entries out); every folder whole when no crate
/// is named.
export function incrementalClears(ice: IncrementalIce, list: (dir: string) => string[]): string[] {
  if (!ice.crates.length) return [...ice.dirs];
  return ice.dirs.flatMap((d) =>
    list(d)
      .filter((name) => ice.crates.some((c) => name.startsWith(`${c}-`) && !name.slice(c.length + 1).includes("-")))
      .map((name) => `${d}/${name}`),
  );
}

/// The workspace's package names from the root Cargo.toml's members (`dir/*` or `dir`) and
/// each member's `[package] name`, read through `read` (a path in, its text or undefined out)
/// and `list` (a folder in, its entries out).
export function workspacePackages(rootToml: string, read: (path: string) => string | undefined, list: (dir: string) => string[]): string[] {
  const members = rootToml.match(/^\s*members\s*=\s*\[([^\]]*)\]/m)?.[1].match(/"([^"]+)"/g)?.map((s) => s.slice(1, -1)) ?? [];
  const dirs = members.flatMap((m) => (m.endsWith("/*") ? list(m.slice(0, -2)).map((d) => `${m.slice(0, -2)}/${d}`) : [m]));
  const out: string[] = [];
  for (const dir of dirs) {
    const toml = read(`${dir}/Cargo.toml`);
    const name = toml?.match(/^\[package\][^[]*?^\s*name\s*=\s*"([^"]+)"/ms)?.[1];
    if (name) out.push(name);
  }
  return out.sort();
}

/// The whole limit of a job's run steps, in seconds, under `timeout`.
export const limitArgs = (job: Job) => ["timeout", "-k", "30", String(job.minutes * 60)];

export const ago = (from: string, now: Date) => {
  const m = Math.round((now.getTime() - Date.parse(from)) / 60000);
  return m < 60 ? `${m}m` : `${Math.floor(m / 60)}h${String(m % 60).padStart(2, "0")}`;
};

/// What a job ran: its commit, or `<base>+patch` for uncommitted changes, never the snapshot's own sha.
export const revLabel = (job: Pick<Job, "rev" | "patch">) => (job.patch ? `${job.patch.base.slice(0, 9)}+patch` : job.rev.slice(0, 9));

/// One line for a job in lists and logs.
export const short = (job: Job) => `${revLabel(job)} ${classOf(job)}: ${job.kind === "warm" ? `warm builds after an update${job.slot ? ` (slot ${job.slot})` : ""}` : job.command.join(" ").slice(0, 100)}`;

// ---- The box's GPU, for --doctor ----

/// What to tell the developer when the card is not reachable: the commands that bring the
/// driver's kernel modules to the running kernel, which only they can run (sudo).
export function gpuFix(kernel: string, branch: string | undefined, moduleFound: boolean): string[] {
  if (!branch) return ["no NVIDIA driver package: `sudo ubuntu-drivers install`, then `sudo reboot`"];
  if (!moduleFound) {
    return [
      `the ${branch} driver has no kernel module for the running kernel ${kernel}; on the box:`,
      `  sudo apt-get update && sudo apt-get install -y linux-modules-nvidia-${branch}-generic linux-generic`,
      "  sudo reboot",
      "  (the metapackage brings the module for the newest kernel, which the reboot starts; `nvidia-smi` then prints the card)",
    ];
  }
  return [`the ${branch} module exists for ${kernel} but is not loaded: \`sudo modprobe nvidia\`, or \`sudo reboot\``];
}
