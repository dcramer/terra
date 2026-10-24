// `cargo xtask remote-render` (docs/render-worker.md, D445): runs a command on a render box over
// ssh, at a commit of this checkout. It pushes the commit to the box's clone as refs/jobs/<id>,
// queues the job with the box's daemon (box.ts), waits on it there (the box watches the job; no
// polling over the network), and rsyncs what it wrote into ~/.cache/terra-review/remote/<job>/,
// exiting with the command's code. `--update` brings a box to origin/main and restarts its
// daemon, `--doctor` checks it. Settings (.env.example): the environment over this checkout's
// .env, or the main checkout's from a worktree. Node 22.18+, git, ssh, rsync; no dependencies.

import { spawn, spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { homedir, hostname, userInfo } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { ago, backoff, type ClientArgs, chooseWorker, config, type Job, jobClass, jobId, parseClient, parseEnvFile, type Seen, short, USAGE } from "./jobs.ts";

/// A render box as the client reaches it: over ssh, or `local`, this machine (the tests).
export type Remote = { host: string; dir: string };

const isLocal = (r: Remote) => r.host === "local";
let SSH = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-o", "ServerAliveInterval=30", "-o", "ServerAliveCountMax=4"];
export const q = (s: string) => (/^[A-Za-z0-9_./~:@=+,-]+$/.test(s) ? s : `'${s.replace(/'/g, `'\\''`)}'`);

/// A shell command on the box: through ssh (in the user's home), or `sh -c` here.
function onBox(r: Remote, cmd: string, input?: string, inherit = false) {
  const [exe, args] = isLocal(r) ? ["sh", ["-c", cmd]] : ["ssh", [...SSH, r.host, cmd]];
  return spawnSync(exe, args, { encoding: "utf8", input, stdio: [input === undefined ? "ignore" : "pipe", inherit ? "inherit" : "pipe", inherit ? "inherit" : "pipe"], maxBuffer: 64 * 1024 ** 2 });
}

/// `node box.ts --dir <dir> …` as the box runs it: its own Node and the worker's code.
export function boxCmd(r: Remote, args: string[]): string {
  const node = isLocal(r) ? process.execPath : `${r.dir}/node/bin/node`;
  const script = isLocal(r) ? resolve(import.meta.dirname, "box.ts") : `${r.dir}/repo/tools/render-worker/box.ts`;
  return [node, script, "--dir", r.dir, ...args].map(q).join(" ");
}

const gitUrl = (r: Remote) => (isLocal(r) ? `${r.dir}/repo` : `${r.host}:${r.dir}/repo`);
const git = (cwd: string, args: string[]) => spawnSync("git", args, { cwd, encoding: "utf8" });
const lastLine = (s: string | null | undefined) => (s ?? "").trim().split("\n").at(-1) ?? "";

/// Why a box command failed, in a line: ssh's own failure (255), or a box not set up (127).
function why(r: Remote, res: ReturnType<typeof onBox>): string {
  if (res.status === 255 && !isLocal(r)) return `ssh ${r.host}: ${lastLine(res.stderr) || "unreachable"}`;
  if (res.status === 127) return `${r.host} is not set up: cargo xtask remote-render --update --worker ${r.host}`;
  return lastLine(res.stdout) || lastLine(res.stderr) || `exit ${res.status}`;
}

type BoxStatus = { daemon?: { alive: boolean; status: string }; rev: string; jobs: Job[] };

export function boxStatus(r: Remote): BoxStatus | { error: string } {
  const res = onBox(r, boxCmd(r, ["status", "--json"]));
  if (res.status !== 0) return { error: why(r, res) };
  return JSON.parse(lastLine(res.stdout)) as BoxStatus;
}

/// Pushes the commit as the job's ref and queues the job.
export function submit(r: Remote, cwd: string, job: Omit<Job, "id" | "kind" | "queued" | "state" | "by">): { id: string; note: string } | { error: string } {
  const now = new Date();
  const full: Job = { ...job, id: jobId(now, randomBytes(2).toString("hex")), kind: "run", queued: now.toISOString(), state: "queued", by: `${userInfo().username}@${hostname()}` };
  const push = git(cwd, ["push", "-q", gitUrl(r), `${job.rev}:refs/jobs/${full.id}`]);
  if (push.status !== 0) return { error: `git push to ${r.host}: ${lastLine(push.stderr)}` };
  const res = onBox(r, boxCmd(r, ["submit"]), JSON.stringify(full));
  if (res.status !== 0) return { error: why(r, res) };
  const { alive, ahead } = JSON.parse(lastLine(res.stdout)) as { alive: boolean; ahead: number };
  return { id: full.id, note: alive ? (ahead ? `${ahead} queued ahead of it` : "next") : "the daemon is not running; the job waits for it (--update starts it)" };
}

/// Waits on the box for the job to end (the box watches it; a line when its state changes) and
/// returns it. A dropped ssh is tried again, 5 s, then up to 2 min apart.
export async function waitOn(r: Remote, id: string, print = (l: string) => console.log(l)): Promise<Job | { error: string }> {
  for (let n = 0; ; n++) {
    const [exe, args] = isLocal(r) ? ["sh", ["-c", boxCmd(r, ["wait", id])]] : ["ssh", [...SSH, r.host, boxCmd(r, ["wait", id])]];
    const child = spawn(exe, args, { stdio: ["ignore", "pipe", "pipe"] });
    let job: Job | undefined;
    let err = "";
    child.stderr.on("data", (d) => (err += d));
    for await (const line of createInterface({ input: child.stdout })) {
      if (line.startsWith("JOB ")) job = JSON.parse(line.slice(4)) as Job;
      else print(line);
    }
    const code = await new Promise<number>((done) => (child.exitCode !== null ? done(child.exitCode) : child.on("close", (c) => done(c ?? 1))));
    if (job) return job;
    if (code !== 255 || isLocal(r)) return { error: lastLine(err) || `the box's wait exited ${code}` };
    print(`render ${id}: lost ssh to ${r.host} (${lastLine(err)}); again in ${backoff(n)} s`);
    await new Promise((done) => setTimeout(done, backoff(n) * 1000));
  }
}

/// Copies the job's out/ (its files and job.log) into `out`.
export function fetchFiles(r: Remote, id: string, out: string): string | undefined {
  mkdirSync(out, { recursive: true });
  const src = `${isLocal(r) ? "" : `${r.host}:`}${r.dir}/jobs/${id}/out/`;
  const res = spawnSync("rsync", ["-a", "-e", `ssh ${SSH.join(" ")}`, src, `${out}/`], { encoding: "utf8" });
  return res.status === 0 ? undefined : `rsync from ${r.host}: ${lastLine(res.stderr)}`;
}

function report(job: Job, host: string, out: string, fetchErr?: string): number {
  const took = job.started ? ago(job.started, new Date(job.ended ?? Date.now())) : "0m";
  if (fetchErr) console.log(`✗ render ${job.id}: ${job.state}, but its files did not come back: ${fetchErr}`);
  if (job.state === "done") {
    console.log(`✓ render ${job.id}: done in ${took} on ${host} (${job.note}); files in ${out}`);
    return fetchErr ? 1 : 0;
  }
  const log = join(out, "job.log");
  if (existsSync(log)) console.log(readFileSync(log, "utf8").trimEnd().split("\n").slice(-15).join("\n"));
  console.log(`✗ render ${job.id}: ${job.state} after ${took} on ${host}: ${job.note}; log ${log}`);
  return job.exit || 1;
}

/// Waits for a job and fetches its files: the end of every path that runs one.
export async function finish(r: Remote, id: string, out: string): Promise<number> {
  const job = await waitOn(r, id);
  if ("error" in job) {
    console.log(`✗ render ${id}: ${job.error}`);
    return 1;
  }
  return report(job, r.host, out, fetchFiles(r, id, out));
}

/// Makes the box's folder, its clone and (over ssh) its Node, with bootstrap.sh.
export function prepare(r: Remote): boolean {
  const boot = onBox(r, `sh -s -- ${q(r.dir)}${isLocal(r) ? "" : " node"}`, readFileSync(resolve(import.meta.dirname, "bootstrap.sh"), "utf8"));
  if (boot.status !== 0) return console.log(`✗ ${r.host}: setting up its folder: ${why(r, boot)}`), false;
  if (boot.stdout.trim()) console.log(`${r.host}: ${boot.stdout.trim()}`);
  return true;
}

/// Brings a box to `sha`: its folder and Node (bootstrap.sh), the commit pushed as
/// refs/worker/main, then the new code's own `box.ts update`, run from an export of it, so a
/// fix to updating itself takes effect at once.
export function update(r: Remote, cwd: string, sha: string): boolean {
  if (!prepare(r)) return false;
  const push = git(cwd, ["push", "-q", gitUrl(r), `+${sha}:refs/worker/main`]);
  if (push.status !== 0) return console.log(`✗ ${r.host}: git push: ${lastLine(push.stderr)}`), false;
  const node = isLocal(r) ? process.execPath : `${r.dir}/node/bin/node`;
  const next = `${r.dir}/next`;
  const cmd = `rm -rf ${q(next)} && mkdir -p ${q(next)} && git -C ${q(`${r.dir}/repo`)} archive ${sha} tools/render-worker | tar -x -C ${q(next)} && ${q(node)} ${q(`${next}/tools/render-worker/box.ts`)} --dir ${q(r.dir)} update ${sha}`;
  const res = onBox(r, cmd);
  for (const l of `${res.stdout}`.trim().split("\n").filter(Boolean)) console.log(`${r.host}: ${l}`);
  if (res.status !== 0) console.log(`✗ ${r.host}: ${why(r, res)}`);
  return res.status === 0;
}

/// The box that holds a job: the one named, or the first that lists it.
function holder(remotes: Remote[], id: string): Remote | undefined {
  return remotes.find((r) => {
    const s = boxStatus(r);
    return !("error" in s) && s.jobs.some((j) => j.id === id);
  });
}

/// The .env of this checkout, else of the main checkout, parsed; empty when neither has one.
export function envFile(cwd: string): Record<string, string> {
  const top = git(cwd, ["rev-parse", "--show-toplevel"]).stdout.trim();
  const common = git(cwd, ["rev-parse", "--path-format=absolute", "--git-common-dir"]).stdout.trim();
  const file = [top, common && resolve(common, "..")].filter(Boolean).map((d) => join(d, ".env")).find((f) => existsSync(f));
  return file ? parseEnvFile(readFileSync(file, "utf8")) : {};
}

/// The settings: the environment over the .env of this checkout, else of the main checkout.
export function loadConfig(cwd: string, env = process.env) {
  return config(env, envFile(cwd));
}

async function main(argv: string[]): Promise<number> {
  const cwd = process.cwd();
  const cfg = loadConfig(cwd);
  if ("error" in cfg) return console.log(`✗ remote-render: ${cfg.error}`), 2;
  const args = parseClient(argv, cfg.minutes);
  if ("error" in args) return console.log(`✗ remote-render: ${args.error}\n${USAGE}`), 2;
  SSH = SSH.map((o) => (o.startsWith("ConnectTimeout=") ? `ConnectTimeout=${cfg.connectTimeout}` : o));
  const remotes = (args.worker ? [args.worker] : cfg.workers).map((host) => ({ host, dir: cfg.dir }));
  const outOf = (id: string, out?: string) => out ?? join(homedir(), ".cache/terra-review/remote", id);
  return run(args, remotes, cwd, outOf);
}

async function run(args: ClientArgs, remotes: Remote[], cwd: string, outOf: (id: string, out?: string) => string): Promise<number> {
  if (args.mode === "status" || args.mode === "doctor") {
    let bad = 0;
    for (const r of remotes) {
      if (remotes.length > 1) console.log(`== ${r.host}`);
      const res = onBox(r, boxCmd(r, [args.mode]), undefined, true);
      if (res.status === 255 || res.status === 127) console.log(`✗ ${why(r, res)}`);
      if (res.status !== 0) bad++;
    }
    return bad ? 1 : 0;
  }
  if (args.mode === "update") {
    if (git(cwd, ["fetch", "-q", "origin", "main"]).status !== 0) return console.log("✗ remote-render: git fetch origin main failed"), 1;
    const sha = git(cwd, ["rev-parse", "origin/main"]).stdout.trim();
    console.log(`remote-render: updating ${remotes.map((r) => r.host).join(", ")} to origin/main ${sha.slice(0, 9)} (the first push sends the whole history)`);
    return remotes.map((r) => update(r, cwd, sha)).every(Boolean) ? 0 : 1;
  }
  if (args.mode === "wait" || args.mode === "cancel" || args.mode === "logs") {
    const r = args.mode === "logs" && args.id === "daemon" ? remotes[0] : remotes.length === 1 ? remotes[0] : holder(remotes, args.id);
    if (!r) return console.log(`✗ render ${args.id}: no worker lists it`), 1;
    if (args.mode === "wait") return finish(r, args.id, outOf(args.id, args.out));
    const res = onBox(r, boxCmd(r, [args.mode, args.id]), undefined, true);
    if (res.status === 255 || res.status === 127) console.log(`✗ ${why(r, res)}`);
    return res.status === 0 ? 0 : 1;
  }
  const sha = git(cwd, ["rev-parse", "--verify", "-q", `${args.rev ?? "HEAD"}^{commit}`]).stdout.trim();
  if (!sha) return console.log(`✗ remote-render: no commit ${args.rev}`), 1;
  if (!args.rev && git(cwd, ["status", "--porcelain", "--untracked-files=no"]).stdout.trim()) {
    console.log(`remote-render: uncommitted changes stay here; the job runs ${sha.slice(0, 9)}`);
  }
  const seen: Seen[] = remotes.map((r) => {
    const s = boxStatus(r);
    if ("error" in s) return { host: r.host, reachable: false, alive: false, waiting: 0 };
    return { host: r.host, reachable: true, alive: !!s.daemon?.alive, waiting: s.jobs.filter((j) => j.state === "queued" || j.state === "running").length };
  });
  const chosen = chooseWorker(seen);
  if ("error" in chosen) return console.log(`✗ remote-render: ${chosen.error}`), 1;
  const r = remotes.find((x) => x.host === chosen.host)!;
  if (chosen.note) console.log(`remote-render: ${chosen.note}`);
  const job = { rev: sha, command: args.command, collect: args.collect, cook: args.cook, minutes: args.minutes, class: jobClass(args.command, args.class) };
  const sub = submit(r, cwd, job);
  if ("error" in sub) return console.log(`✗ remote-render: ${sub.error}`), 1;
  console.log(`render ${sub.id}: queued on ${r.host}, ${sub.note} (${short({ ...job, id: sub.id, kind: "run" } as Job)})`);
  if (!args.wait) return console.log(`  wait for it: cargo xtask remote-render --wait ${sub.id}`), 0;
  return finish(r, sub.id, outOf(sub.id, args.out));
}

// Exit once stdout has drained (see box.ts): a pipe keeps only the first 64 KiB otherwise.
if (import.meta.main) {
  const code = await main(process.argv.slice(2));
  process.stdout.write("", () => process.exit(code));
}
