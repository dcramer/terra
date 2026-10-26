// The render box's side (docs/render-worker.md, D445): the job queue in a folder, and the
// commands the client runs over ssh. `node box.ts --dir <dir> <command>`:
//   submit            a job from stdin (its commit already pushed as refs/jobs/<id>)
//   wait <id>         blocks until the job ends, a line when its state changes, then `JOB {…}`
//   status [--json]   the daemon and every job
//   logs <id|daemon>  the log's last lines and where it is
//   cancel <id>       a queued job is dropped, a running one stopped
//   update <sha>      checks the worker's clone out at <sha> (pushed as refs/worker/main), tries
//                     its daemon, queues the warm builds and restarts the daemon
//   install           starts the daemon (a systemd user service, or a detached supervisor)
//   plan              what the daemon would do next, running nothing
//   doctor            the GPU, Vulkan, node, cargo, disk and daemon (doctor.ts)
// The folder: repo/ the clone pushes land in (its checkout is the worker's code), slots/<n>/
// each slot's job checkout (a worktree of repo/) with its own build in target/, homes/<n>/ each
// slot's HOME, jobs/<id>/{job.json,out/}, daemon.json/.log.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, statSync, watch, writeFileSync } from "node:fs";
import { homedir, userInfo } from "node:os";
import { join, resolve } from "node:path";
import { doctor } from "./doctor.ts";
import { ago, boxSettings, classOf, ID, type Job, jobId, mayStart, parseEnvFile, plan as planJobs, short, steps, UPDATE_REF } from "./jobs.ts";

/// `src` and `home` are the single job checkout and HOME from before slots, which slot 1 takes
/// over (daemon.ts `slotCheckout`).
export type Box = { dir: string; repo: string; src: string; home: string; jobs: string; daemonJs: string };

export function box(dir: string): Box {
  const abs = dir.startsWith("~/") ? join(homedir(), dir.slice(2)) : resolve(homedir(), dir);
  return { dir: abs, repo: join(abs, "repo"), src: join(abs, "src"), home: join(abs, "home"), jobs: join(abs, "jobs"), daemonJs: join(abs, "repo/tools/render-worker/daemon.ts") };
}

/// A slot's job checkout, and its HOME, kept apart so caches carry from job to job in a slot
/// and a job collects only what it wrote.
export const slotDir = (b: Box, n: number) => join(b.dir, "slots", String(n));
export const slotHome = (b: Box, n: number) => join(b.dir, "homes", String(n));

export function git(cwd: string, args: string[]): { status: number | null; stdout: string; stderr: string } {
  const r = spawnSync("git", args, { cwd, encoding: "utf8" });
  return { status: r.status, stdout: r.stdout ?? "", stderr: r.stderr ?? r.error?.message ?? "" };
}

// ---- Jobs on disk ----

export const jobFile = (b: Box, id: string) => join(b.jobs, id, "job.json");
export const outDir = (b: Box, id: string) => join(b.jobs, id, "out");

export function readJob(b: Box, id: string): Job | undefined {
  try {
    return JSON.parse(readFileSync(jobFile(b, id), "utf8")) as Job;
  } catch {
    return undefined;
  }
}

/// Written whole and renamed into place, so a reader never sees half a job.
export function writeJob(b: Box, job: Job): void {
  mkdirSync(join(b.jobs, job.id), { recursive: true });
  const tmp = `${jobFile(b, job.id)}.tmp`;
  writeFileSync(tmp, JSON.stringify(job, null, 2));
  renameSync(tmp, jobFile(b, job.id));
}

export function listJobs(b: Box): Job[] {
  if (!existsSync(b.jobs)) return [];
  return readdirSync(b.jobs)
    .filter((id) => ID.test(id))
    .map((id) => readJob(b, id))
    .filter((j): j is Job => !!j)
    .sort((a, b) => a.id.localeCompare(b.id));
}

/// Wakes the daemon, which watches the folder for this file.
export const wake = (b: Box) => writeFileSync(join(b.dir, "wake"), new Date().toISOString());

// ---- The daemon's record of itself ----

export type DaemonInfo = { pid: number; mode: string; rev: string; since: string; beat: string; status: string; display?: string };

export function daemonInfo(b: Box, now = Date.now()): (DaemonInfo & { alive: boolean }) | undefined {
  try {
    const d = JSON.parse(readFileSync(join(b.dir, "daemon.json"), "utf8")) as DaemonInfo;
    let running = false;
    try {
      process.kill(d.pid, 0);
      running = true;
    } catch {}
    return { ...d, alive: running && now - Date.parse(d.beat) < 3 * 60_000 };
  } catch {
    return undefined;
  }
}

/// The box's settings: its environment over the .env of the worker's clone (.env.example).
export function boxEnv(b: Box, env = process.env): Record<string, string | undefined> {
  const file = join(b.repo, ".env");
  return { ...(existsSync(file) ? parseEnvFile(readFileSync(file, "utf8")) : {}), ...env };
}

export const ciBusy = (lock: string | undefined, now = Date.now()) => !!lock && existsSync(lock) && now - statSync(lock).mtimeMs < 6 * 3600_000;

// ---- Commands ----

function submit(b: Box, text: string): number {
  const job = JSON.parse(text) as Job;
  if (!ID.test(job.id) || !Array.isArray(job.command) || readJob(b, job.id)) return fail(`bad or repeated job ${job.id}`);
  const at = git(b.repo, ["rev-parse", "-q", "--verify", `refs/jobs/${job.id}^{commit}`]).stdout.trim();
  if (at !== job.rev) return fail(`refs/jobs/${job.id} is not ${job.rev.slice(0, 9)} in ${b.repo}; push it first`);
  writeJob(b, { ...job, kind: "run", class: classOf({ command: job.command, kind: "run", class: ["cpu", "gpu", "bench"].includes(job.class ?? "") ? job.class : undefined }), slot: undefined, state: "queued", queued: new Date().toISOString() });
  wake(b);
  const d = daemonInfo(b);
  const ahead = listJobs(b).filter((j) => j.id < job.id && j.state === "queued").length;
  console.log(JSON.stringify({ alive: !!d?.alive, ahead }));
  return 0;
}

function fail(line: string): number {
  console.log(`✗ render-box: ${line}`);
  return 1;
}

/// Blocks until the job ends, watching its folder (and looking each minute, should a watch miss
/// it), a line on each change of state; then `JOB {…}` for the client.
async function waitFor(b: Box, id: string): Promise<number> {
  let said = "";
  for (;;) {
    const job = readJob(b, id);
    if (!job) return fail(`no job ${id} (deleted after TERRA_RENDER_KEEP_DAYS?)`);
    if (job.state !== said) {
      said = job.state;
      if (job.state === "queued" || job.state === "running") console.log(`render ${id}: ${job.state} (${short(job)})`);
    }
    if (job.state !== "queued" && job.state !== "running") {
      console.log(`JOB ${JSON.stringify(job)}`);
      return 0;
    }
    await new Promise<void>((done) => {
      const w = watch(join(b.jobs, id), () => finish());
      const t = setTimeout(() => finish(), 60_000);
      function finish() {
        w.close();
        clearTimeout(t);
        done();
      }
    });
  }
}

function status(b: Box, json: boolean): number {
  const d = daemonInfo(b);
  const jobs = listJobs(b);
  const rev = git(b.repo, ["rev-parse", "--short=9", "HEAD"]).stdout.trim();
  if (json) {
    console.log(JSON.stringify({ daemon: d, rev, jobs }));
    return 0;
  }
  const now = new Date();
  console.log(d ? `daemon: ${d.alive ? "running" : "not running"} (${d.mode}, at ${d.rev}, since ${ago(d.since, now)} ago, last word ${ago(d.beat, now)} ago): ${d.status}` : "daemon: never started (cargo xtask remote-render --update)");
  const s = boxSettings(boxEnv(b));
  if (!("error" in s)) {
    console.log(`slots: ${s.slots.slots}, of them ${s.slots.gpuSlots} for GPU jobs; a bench has the box alone`);
    for (let n = 1; n <= s.slots.slots; n++) {
      const j = jobs.find((j) => j.state === "running" && (j.slot ?? 1) === n);
      console.log(`  slot ${n}: ${j ? `${j.id} running ${ago(j.started ?? j.queued, now)}  ${short(j)}` : "free"}`);
    }
  }
  console.log("jobs:");
  for (const j of jobs) console.log(`  ${j.id} ${j.state.padEnd(9)} ${ago(j.queued, now).padStart(5)} ago  ${short(j)}${j.note ? `  [${j.note}]` : ""}`);
  if (!jobs.length) console.log("  no jobs");
  return 0;
}

function logs(b: Box, id: string): number {
  const file = id === "daemon" ? join(b.dir, "daemon.log") : join(outDir(b, id), "job.log");
  if (!existsSync(file)) return fail(`no log ${file}`);
  console.log(readFileSync(file, "utf8").trimEnd().split("\n").slice(-60).join("\n"));
  console.log(`(the last 60 lines of ${file})`);
  return 0;
}

function cancel(b: Box, id: string): number {
  const job = readJob(b, id);
  if (!job) return fail(`no job ${id}`);
  if (job.state === "queued") {
    writeJob(b, { ...job, state: "cancelled", ended: new Date().toISOString(), note: "cancelled before it started" });
    console.log(`✓ render ${id}: cancelled`);
  } else if (job.state === "running") {
    writeFileSync(join(b.jobs, id, "cancel"), "");
    wake(b);
    console.log(`✓ render ${id}: the daemon stops it`);
  } else return fail(`render ${id} is ${job.state} already`);
  return 0;
}

function plan(b: Box): number {
  const s = boxSettings(boxEnv(b));
  if ("error" in s) return fail(s.error);
  const now = new Date();
  const may = mayStart(s.policy, now.getHours() * 60 + now.getMinutes(), s.policy.afterCi && ciBusy(s.ciLock));
  const p = planJobs(listJobs(b), s.slots);
  console.log(`render-box: ${!may.ok ? `would wait: ${may.why}` : p.start.length ? `would start ${p.start.length} now` : "would start nothing now"}${p.why ? `; ${p.why}` : ""}`);
  for (const { job, slot } of p.start) for (const st of steps(job)) console.log(`  ${job.id} in slot ${slot}, ${st.what}: ${st.cmd.join(" ")}`);
  return 0;
}

// ---- Updating and starting the daemon ----

const UNIT = "terra-render.service";
const unitPath = () => join(homedir(), ".config/systemd/user", UNIT);
/// The daemon's and so its jobs' PATH; ~/.local/bin holds tools installed without sudo (ImageMagick's `magick`).
const pathEnv = () => [join(process.execPath, ".."), join(homedir(), ".cargo/bin"), join(homedir(), ".local/bin"), "/usr/local/bin", "/usr/bin", "/bin"].join(":");

/// Whether the daemon can be a systemd user service that outlives the ssh session: the user's
/// manager answers and lingering is on (turned on here when polkit lets the user do it).
function systemdUsable(): boolean {
  if (spawnSync("systemctl", ["--user", "show-environment"], { stdio: "ignore" }).status !== 0) return false;
  const linger = () => spawnSync("loginctl", ["show-user", userInfo().username, "-p", "Linger", "--value"], { encoding: "utf8" }).stdout.trim() === "yes";
  if (linger()) return true;
  spawnSync("loginctl", ["enable-linger"], { stdio: "ignore" });
  return linger();
}

function unit(b: Box): string {
  return [
    "# Written by tools/render-worker/box.ts install (docs/render-worker.md); rewritten on every update.",
    "[Unit]",
    "Description=Terra render worker daemon",
    "",
    "[Service]",
    `ExecStart=${process.execPath} ${b.daemonJs} --dir ${b.dir}`,
    `WorkingDirectory=${b.repo}`,
    `Environment=PATH=${pathEnv()}`,
    "Environment=TERRA_RENDER_MODE=systemd",
    "Restart=always",
    "RestartSec=5",
    `StandardOutput=append:${join(b.dir, "daemon.log")}`,
    `StandardError=append:${join(b.dir, "daemon.log")}`,
    "",
    "[Install]",
    "WantedBy=default.target",
    "",
  ].join("\n");
}

/// Starts the daemon, or restarts it on its current code: under systemd when it can, else a
/// detached supervisor (its own session, so it survives the ssh session) that restarts it.
export function install(b: Box): string {
  mkdirSync(b.dir, { recursive: true });
  if (systemdUsable()) {
    mkdirSync(join(unitPath(), ".."), { recursive: true });
    writeFileSync(unitPath(), unit(b));
    const sc = (...a: string[]) => spawnSync("systemctl", ["--user", ...a], { encoding: "utf8" });
    sc("daemon-reload");
    sc("enable", UNIT);
    const r = sc("restart", UNIT);
    if (r.status !== 0) throw new Error(`systemctl --user restart ${UNIT}: ${r.stderr.trim()}`);
    return `a systemd user service (${UNIT}, lingering)`;
  }
  const pidFile = join(b.dir, "supervisor.pid");
  const sup = existsSync(pidFile) ? Number(readFileSync(pidFile, "utf8")) : 0;
  let supAlive = false;
  try {
    if (sup) process.kill(sup, 0), (supAlive = true);
  } catch {}
  const d = daemonInfo(b);
  if (supAlive && d) {
    try {
      process.kill(d.pid, "SIGTERM");
    } catch {}
    return "a detached supervisor, which starts it again";
  }
  const log = join(b.dir, "daemon.log");
  const child = spawn(process.execPath, [b.daemonJs, "--dir", b.dir, "--supervise"], {
    cwd: b.repo,
    detached: true,
    stdio: ["ignore", "ignore", "ignore"],
    env: { ...process.env, PATH: pathEnv(), TERRA_RENDER_MODE: "detached", TERRA_RENDER_LOG: log },
  });
  writeFileSync(pidFile, String(child.pid));
  child.unref();
  return "a detached supervisor (no systemd user service with lingering here)";
}

/// Checks the worker's clone out at `sha`, tries its daemon (`--selftest`) and goes back to the
/// commit before when that fails; queues the warm builds; restarts the daemon now, or after the
/// running job.
function update(b: Box, sha: string): number {
  const at = git(b.repo, ["rev-parse", "-q", "--verify", `${UPDATE_REF}^{commit}`]).stdout.trim();
  if (!at.startsWith(sha)) return fail(`${UPDATE_REF} is not ${sha.slice(0, 9)}; the client pushes it first`);
  const before = git(b.repo, ["rev-parse", "-q", "--verify", "HEAD"]).stdout.trim();
  const co = git(b.repo, ["checkout", "-q", "--detach", "--force", at]);
  if (co.status !== 0) return fail(`checkout ${at.slice(0, 9)}: ${co.stderr.trim()}`);
  const self = spawnSync(process.execPath, [b.daemonJs, "--dir", b.dir, "--selftest"], { encoding: "utf8" });
  if (self.status !== 0) {
    if (before) git(b.repo, ["checkout", "-q", "--detach", "--force", before]);
    return fail(`the daemon at ${at.slice(0, 9)} fails its self-test, so the box stays at ${before.slice(0, 9) || "nothing"}:\n${(self.stderr || self.stdout).trim().split("\n").slice(-8).join("\n")}`);
  }
  const jobs = listJobs(b);
  for (const j of jobs.filter((j) => j.kind === "warm" && j.state === "queued")) writeJob(b, { ...j, state: "cancelled", ended: new Date().toISOString(), note: "a newer update" });
  const s = boxSettings(boxEnv(b));
  const slots = "error" in s ? 1 : s.slots.slots;
  const now = new Date();
  const ids: string[] = [];
  for (let slot = 1; slot <= slots; slot++) {
    const id = jobId(now, `${Math.random().toString(16).slice(2, 5).padEnd(3, "0")}${slot.toString(16).slice(-1)}`);
    writeJob(b, { id, kind: "warm", rev: at, command: [], collect: [], cook: true, minutes: 180, class: "cpu", slot, by: "update", queued: now.toISOString(), state: "queued" });
    ids.push(id);
  }
  console.log(`render-box: at ${at.slice(0, 9)}; warm builds queued in all ${slots} slots, side by side (${ids.join(", ")})`);
  if (jobs.some((j) => j.state === "running") && daemonInfo(b)?.alive) {
    writeFileSync(join(b.dir, "restart"), "");
    wake(b);
    console.log("✓ render-box: the daemon starts no new job and restarts on the new code once the running ones end");
  } else {
    rmSync(join(b.dir, "restart"), { force: true });
    console.log(`✓ render-box: the daemon runs on the new code as ${install(b)}`);
  }
  return 0;
}

async function main(argv: string[]): Promise<number> {
  const at = argv.indexOf("--dir");
  const b = box(at >= 0 ? argv[at + 1] : resolve(import.meta.dirname, "../../.."));
  const [cmd, arg] = at >= 0 ? [...argv.slice(0, at), ...argv.slice(at + 2)] : argv;
  if (cmd === "submit") return submit(b, readFileSync(0, "utf8"));
  if (cmd === "wait" && ID.test(arg ?? "")) return waitFor(b, arg);
  if (cmd === "status") return status(b, arg === "--json");
  if (cmd === "logs" && (arg === "daemon" || ID.test(arg ?? ""))) return logs(b, arg);
  if (cmd === "cancel" && ID.test(arg ?? "")) return cancel(b, arg);
  if (cmd === "update" && /^[0-9a-f]{7,40}$/.test(arg ?? "")) return update(b, arg);
  if (cmd === "install") return console.log(`✓ render-box: the daemon runs as ${install(b)}`), 0;
  if (cmd === "plan") return plan(b);
  if (cmd === "doctor") return doctor(b);
  console.log("usage: node box.ts [--dir DIR] submit | wait <id> | status [--json] | logs <id|daemon> | cancel <id> | update <sha> | install | plan | doctor");
  return 2;
}

// Exit once stdout has drained: `process.exit` straight after a large `console.log` to a pipe
// drops all past the pipe's 64 KiB, which cut `status --json` short once the job list outgrew it.
if (import.meta.main) {
  const code = await main(process.argv.slice(2));
  process.stdout.write("", () => process.exit(code));
}
