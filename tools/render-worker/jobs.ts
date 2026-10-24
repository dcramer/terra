// The render worker's decisions (docs/render-worker.md, D445), pure and tested in jobs.test.ts:
// a job and how it is written in a draft release's notes, the client's arguments, when the
// worker may start one, which it takes, what it collects, and the job's environment.

export type State = "queued" | "running" | "done" | "failed";

export type Job = {
  id: string;
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
  worker?: string;
  started?: string;
  ended?: string;
  exit?: number;
  note?: string;
};

/// A job as a store keeps it: `ref` is the store's own handle (a release id, a folder).
export type Rec = { ref: string; job: Job; files: string[] };

export const MARK = "<!-- terra-render-job -->";
export const TAG = "render-";
/// The worker's heartbeat record, a draft release too.
export const BEAT = "render-worker";
export const DEFAULT_MINUTES = 120;
export const MAX_MINUTES = 480;
/// What a release file may hold (GitHub's limit is 2 GiB).
export const MAX_BYTES = 2 * 1024 ** 3 - 1024 ** 2;
/// Folders under the collected tree never collected: a film's intermediate frames, and
/// downloads of earlier jobs when the worker runs on an agent's own machine.
export const SKIP = ["frames", "remote"];

export function encode(job: Job): string {
  return [
    MARK,
    `Render job \`${job.id}\`: ${job.state}. Queued by \`cargo xtask remote-render\` (docs/render-worker.md); the worker edits this.`,
    "",
    "```json",
    JSON.stringify(job, null, 2),
    "```",
  ].join("\n");
}

export function decode(body: string): Job | undefined {
  if (!body.includes(MARK)) return undefined;
  const json = /```json\n([\s\S]*?)\n```/.exec(body)?.[1];
  if (!json) return undefined;
  try {
    const job = JSON.parse(json) as Job;
    return typeof job.id === "string" && typeof job.rev === "string" && Array.isArray(job.command) ? job : undefined;
  } catch {
    return undefined;
  }
}

/// `20261023-141502-a1b2`: sorts by when it was queued, and is a valid tag.
export function jobId(now: Date, rand: string): string {
  const t = now.toISOString().replace(/[-:]/g, "").replace("T", "-").slice(0, 15);
  return `${t}-${rand.slice(0, 4)}`;
}

export type ClientArgs =
  | { mode: "queue"; rev?: string; command: string[]; collect: string[]; cook: boolean; minutes: number; out?: string; wait: boolean; local: boolean }
  | { mode: "wait"; id: string; out?: string; local: boolean }
  | { mode: "cancel"; id: string; local: boolean }
  | { mode: "status"; local: boolean };

export const USAGE =
  "usage: cargo xtask remote-render [--rev <sha>] [--collect <path>]… [--minutes N] [--no-cook] [--out DIR] [--no-wait] [--local] -- <command…>\n" +
  "       cargo xtask remote-render --wait <job> [--out DIR] | --cancel <job> | --status";

export function parseClient(argv: string[]): ClientArgs | { error: string } {
  const dash = argv.indexOf("--");
  const opts = dash < 0 ? argv : argv.slice(0, dash);
  const command = dash < 0 ? [] : argv.slice(dash + 1);
  const collect: string[] = [];
  let rev: string | undefined;
  let out: string | undefined;
  let minutes = DEFAULT_MINUTES;
  let cook = true;
  let wait = true;
  let local = false;
  let id: string | undefined;
  let mode: ClientArgs["mode"] = "queue";
  for (let i = 0; i < opts.length; i++) {
    const a = opts[i];
    const value = () => {
      const v = opts[++i];
      if (v === undefined || v.startsWith("--")) throw new Error(`${a} needs a value`);
      return v;
    };
    try {
      if (a === "--rev") rev = value();
      else if (a === "--collect") collect.push(value());
      else if (a === "--out") out = value();
      else if (a === "--minutes") {
        minutes = Number(value());
        if (!Number.isInteger(minutes) || minutes < 1 || minutes > MAX_MINUTES) return { error: `--minutes is a whole number from 1 to ${MAX_MINUTES}` };
      } else if (a === "--no-cook") cook = false;
      else if (a === "--no-wait") wait = false;
      else if (a === "--local") local = true;
      else if (a === "--wait") (mode = "wait"), (id = value());
      else if (a === "--cancel") (mode = "cancel"), (id = value());
      else if (a === "--status") mode = "status";
      else return { error: `unknown option ${a}` };
    } catch (e) {
      return { error: (e as Error).message };
    }
  }
  if (mode === "status") return { mode, local };
  if (mode === "cancel") return { mode, id: id!, local };
  if (mode === "wait") return { mode, id: id!, out, local };
  if (!command.length) return { error: "no command: put it after --" };
  for (const c of collect) {
    if (c.startsWith("/") || c.split("/").includes("..")) return { error: `--collect ${c}: a path in the checkout or under ~/, without ..` };
  }
  return { mode, rev, command, collect, cook, minutes, out, wait, local };
}

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

/// Whether a job may start now: CI first (nightly not running), and within the window if set.
export function mayStart(policy: Policy, minute: number, ciBusy: boolean): { ok: true } | { ok: false; why: string } {
  if (policy.afterCi && ciBusy) return { ok: false, why: "nightly is running" };
  if (policy.window && !inWindow(policy.window, minute)) return { ok: false, why: "outside the window" };
  return { ok: true };
}

/// The oldest queued job, unless one is running already (one at a time).
export function pick(recs: Rec[]): Rec | undefined {
  if (recs.some((r) => r.job.state === "running")) return undefined;
  return recs.filter((r) => r.job.state === "queued").sort((a, b) => a.job.id.localeCompare(b.job.id))[0];
}

/// Jobs to delete: ended, or queued, longer than `days` ago.
export function expired(recs: Rec[], now: Date, days: number): Rec[] {
  const limit = now.getTime() - days * 86400_000;
  return recs.filter((r) => r.job.state !== "running" && Date.parse(r.job.ended ?? r.job.queued) < limit);
}

/// A job left running by a worker that died: older than its limit and an hour's grace.
export function abandoned(r: Rec, now: Date): boolean {
  return r.job.state === "running" && !!r.job.started && now.getTime() - Date.parse(r.job.started) > (r.job.minutes + 60) * 60_000;
}

/// Seconds between the client's looks: 15, 20, 30, 45, then a minute.
export function backoff(n: number): number {
  return [15, 20, 30, 45][n] ?? 60;
}

/// The job's environment: the box's, with no token, and HOME the given one. Cargo and rustup
/// keep their homes, which default to HOME's.
export function jobEnv(env: Record<string, string | undefined>, home: string, id: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(env)) {
    if (v === undefined || /TOKEN|SECRET|PASSWORD|COOKIE|API_KEY|^GH_|^GITHUB_/.test(k)) continue;
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

/// The worker's steps for a job, in its checkout: what each is, as a command, so a dry run can
/// print them and the worker runs them in order.
export function steps(job: Job): { what: string; cmd: string[] }[] {
  const s = [
    { what: "fetch origin's branches", cmd: ["git", "fetch", "-q", "--prune", "origin", "+refs/heads/*:refs/remotes/origin/*"] },
    { what: "check the commit is on a branch of origin", cmd: ["git", "branch", "-r", "--contains", job.rev] },
    { what: "check it out", cmd: ["git", "checkout", "-q", "--detach", "--force", job.rev] },
    { what: "clean all but the build", cmd: ["git", "clean", "-q", "-ffdx", "-e", "target/"] },
  ];
  if (job.cook) s.push({ what: "cook textures", cmd: ["cargo", "xtask", "cook"] });
  s.push({ what: "run", cmd: ["nice", "-n", "10", ...job.command] });
  return s;
}

export const ago = (from: string, now: Date) => {
  const m = Math.round((now.getTime() - Date.parse(from)) / 60000);
  return m < 60 ? `${m}m` : `${Math.floor(m / 60)}h${String(m % 60).padStart(2, "0")}`;
};
