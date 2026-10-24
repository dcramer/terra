// The render worker's decisions (docs/render-worker.md, D445), pure and tested in jobs.test.ts:
// the client's settings and arguments, a job and its steps on the box, which worker takes a job,
// when the box may start one, which it takes, what it collects, and the job's environment.

export type State = "queued" | "running" | "done" | "failed" | "cancelled";

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
  by: string;
  queued: string;
  state: State;
  started?: string;
  ended?: string;
  exit?: number;
  note?: string;
};

export const DEFAULT_MINUTES = 120;
export const MAX_MINUTES = 480;
/// Days a job's files and its ref are kept on the box, unless TERRA_RENDER_KEEP_DAYS says.
export const KEEP_DAYS = 7;
/// Folders under the collected tree never collected: a film's intermediate frames, and
/// downloads of earlier jobs.
export const SKIP = ["frames", "remote"];
/// The builds a `warm` job runs after an update: what `cargo xtask sandbox` and `cargo xtask
/// bench` build, so a job after it builds only what its commit changed.
export const WARM: string[][] = [
  ["cargo", "build", "-q", "-p", "xtask", "-p", "terra_sandbox"],
  ["cargo", "build", "-q", "--release", "-p", "terra_sandbox"],
];
/// The ref an update's commit is pushed to in the box's clone.
export const UPDATE_REF = "refs/worker/main";
export const jobRef = (job: Pick<Job, "id" | "kind">) => (job.kind === "warm" ? UPDATE_REF : `refs/jobs/${job.id}`);
export const ID = /^[0-9]{8}-[0-9]{6}-[0-9a-f]{4}$/;

/// `20261023-141502-a1b2`: sorts by when it was queued, and is a valid ref name.
export function jobId(now: Date, rand: string): string {
  const t = now.toISOString().replace(/[-:]/g, "").replace("T", "-").slice(0, 15);
  return `${t}-${rand.slice(0, 4)}`;
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
  | { mode: "queue"; rev?: string; command: string[]; collect: string[]; cook: boolean; minutes: number; out?: string; wait: boolean; worker?: string }
  | { mode: "wait"; id: string; out?: string; worker?: string }
  | { mode: "cancel" | "logs"; id: string; worker?: string }
  | { mode: "status" | "update" | "doctor"; worker?: string };

export const USAGE =
  "usage: cargo xtask remote-render [--rev <sha>] [--collect <path>]… [--minutes N] [--no-cook] [--out DIR] [--no-wait] [--worker HOST] -- <command…>\n" +
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
  for (const c of collect) {
    if (c.startsWith("/") || c.split("/").includes("..")) return { error: `--collect ${c}: a path in the checkout or under ~/, without ..` };
  }
  return { mode, rev, command, collect, cook, minutes, out, wait, worker };
}

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

/// The box's settings (its environment over the .env of the worker's clone there):
/// TERRA_RENDER_WINDOW (`22:00-08:00`, empty any hour), TERRA_RENDER_CI_LOCK (a CI agent's lock
/// file; no job starts while it is held), TERRA_RENDER_AFTER_CI (0 ignores that lock) and
/// TERRA_RENDER_KEEP_DAYS.
export function boxSettings(env: Record<string, string | undefined>): { policy: Policy; ciLock?: string; keepDays: number } | { error: string } {
  const window = parseWindow(env.TERRA_RENDER_WINDOW ?? "");
  if (window && "error" in window) return window;
  const keepDays = whole(env.TERRA_RENDER_KEEP_DAYS, KEEP_DAYS, 365);
  if (!keepDays) return { error: "TERRA_RENDER_KEEP_DAYS: whole days from 1 to 365" };
  return { policy: { window, afterCi: (env.TERRA_RENDER_AFTER_CI ?? "1") !== "0" }, ciLock: env.TERRA_RENDER_CI_LOCK || undefined, keepDays };
}

/// Whether a job may start now: CI first, and within the window if set. A warm job waits too:
/// it takes the box as a render does.
export function mayStart(policy: Policy, minute: number, ciBusy: boolean): { ok: true } | { ok: false; why: string } {
  if (policy.afterCi && ciBusy) return { ok: false, why: "the CI agent is running" };
  if (policy.window && !inWindow(policy.window, minute)) return { ok: false, why: "outside the window" };
  return { ok: true };
}

/// The next job: none while one runs (one at a time, the card its own); a warm job first, since
/// it makes every later one faster; then the oldest queued.
export function pick(jobs: Job[]): Job | undefined {
  if (jobs.some((j) => j.state === "running")) return undefined;
  const queued = jobs.filter((j) => j.state === "queued").sort((a, b) => a.id.localeCompare(b.id));
  return queued.find((j) => j.kind === "warm") ?? queued[0];
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

/// The job's environment: the daemon's, with no token, and HOME the given one. Cargo and
/// rustup keep their homes, which default to HOME's.
export function jobEnv(env: Record<string, string | undefined>, home: string, id: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(env)) {
    if (v === undefined || /TOKEN|SECRET|PASSWORD|COOKIE|API_KEY|^GH_|^GITHUB_|^SSH_AUTH_SOCK$/.test(k)) continue;
    out[k] = v;
  }
  const realHome = env.HOME ?? home;
  out.CARGO_HOME = env.CARGO_HOME ?? `${realHome}/.cargo`;
  out.RUSTUP_HOME = env.RUSTUP_HOME ?? `${realHome}/.rustup`;
  out.HOME = home;
  out.TERRA_RENDER_JOB = id;
  return out;
}

/// Whether a file found under a collected folder is collected: written since the job started
/// (a second's slack for coarse clocks), and in no skipped folder.
export function collected(relative: string, mtimeMs: number, startedMs: number): boolean {
  if (relative.split("/").some((part) => SKIP.includes(part))) return false;
  return mtimeMs >= startedMs - 1000;
}

export type Step = { what: string; cmd: string[]; run?: boolean };

/// The box's steps for a job, in its job checkout `src`, whose commit was pushed to `repo` as
/// the job's ref: what each is, as a command, so `--plan` can print them and the daemon runs
/// them in order. The `run` steps are niced and held to the job's limit.
export function steps(job: Job, repo: string): Step[] {
  const ref = jobRef(job);
  const s: Step[] = [
    { what: "fetch the job's commit", cmd: ["git", "fetch", "-q", "--no-tags", repo, `+${ref}:${ref}`] },
    { what: "check it out", cmd: ["git", "checkout", "-q", "--detach", "--force", job.rev] },
    { what: "clean all but the build", cmd: ["git", "clean", "-q", "-ffdx", "-e", "target/"] },
  ];
  if (job.cook) s.push({ what: "cook textures", cmd: ["cargo", "xtask", "cook"] });
  const runs = job.kind === "warm" ? WARM : [job.command];
  for (const cmd of runs) s.push({ what: job.kind === "warm" ? "warm build" : "run", cmd: ["nice", "-n", "10", ...cmd], run: true });
  return s;
}

/// The whole limit of a job's run steps, in seconds, under `timeout`.
export const limitArgs = (job: Job) => ["timeout", "-k", "30", String(job.minutes * 60)];

export const ago = (from: string, now: Date) => {
  const m = Math.round((now.getTime() - Date.parse(from)) / 60000);
  return m < 60 ? `${m}m` : `${Math.floor(m / 60)}h${String(m % 60).padStart(2, "0")}`;
};

/// One line for a job in lists and logs.
export const short = (job: Job) => `${job.rev.slice(0, 9)}: ${job.kind === "warm" ? "warm builds after an update" : job.command.join(" ").slice(0, 100)}`;

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
