// A job's processes, found by the cookie its environment carries, TERRA_RENDER_JOB=<id> (jobs.ts
// `jobEnv`), and stopped when the job ends, is cancelled or times out, or when a daemon finds a
// job's processes still running that it does not run (it was restarted during the job). Killing
// the job's process group alone misses whatever left it: a `setsid` loop in a job's script lived
// on 2 h 46 min after its job was cancelled, holding the GPU lock it inherited. The environment
// survives setsid, nohup and a double fork, as Jenkins' ProcessTreeKiller finds a build's
// processes by its BUILD_ID cookie (jenkinsci/jenkins, core/src/main/java/hudson/util/
// ProcessTree.java, MIT). Linux /proc only: elsewhere none are found. A process that clears its
// environment (`env -i`) is not found.

import { readdirSync, readFileSync } from "node:fs";

/// A job's process: its pid, the job its cookie names and its command line.
export type Proc = { pid: number; job: string; cmd: string };

/// The job a process's environment (/proc/<pid>/environ, NUL-separated) names, if any.
export function cookie(environ: string): string | undefined {
  for (const entry of environ.split("\0")) if (entry.startsWith("TERRA_RENDER_JOB=")) return entry.slice(17) || undefined;
  return undefined;
}

const read = (file: string) => {
  try {
    return readFileSync(file, "utf8");
  } catch {
    return "";
  }
};

/// The pids from this process up to init, never stopped: a daemon run inside a job carries that
/// job's cookie.
function ancestors(): Set<number> {
  const out = new Set<number>();
  for (let pid = process.pid; pid > 1 && !out.has(pid); ) {
    out.add(pid);
    pid = Number((read(`/proc/${pid}/stat`).split(")").pop() ?? "").trim().split(/\s+/)[1]);
  }
  return out;
}

/// Every process carrying a job's cookie whose job `wanted` takes (a zombie, whose environment
/// reads empty, is not one: it is already dead).
export function scan(wanted: (job: string) => boolean): Proc[] {
  let names: string[];
  try {
    names = readdirSync("/proc");
  } catch {
    return [];
  }
  const skip = ancestors();
  const found: Proc[] = [];
  for (const name of names) {
    if (!/^\d+$/.test(name) || skip.has(Number(name))) continue;
    const job = cookie(read(`/proc/${name}/environ`));
    if (job && wanted(job)) found.push({ pid: Number(name), job, cmd: read(`/proc/${name}/cmdline`).split("\0").join(" ").trim().slice(0, 160) });
  }
  return found;
}

const signal = (pid: number, sig: NodeJS.Signals) => {
  try {
    process.kill(pid, sig);
  } catch {}
};

/// Stops every process of the jobs `wanted` takes: TERM, then KILL what is left after `graceMs`,
/// scanning again each 200 ms so what one forks meanwhile is stopped too. Returns what it found
/// and what still runs at the end (only a process stuck in the kernel survives KILL).
export async function reap(wanted: (job: string) => boolean, graceMs = 10_000): Promise<{ found: Proc[]; left: Proc[] }> {
  const found = new Map<number, Proc>();
  const end = Date.now() + graceMs + 3000;
  for (let now = Date.now(); ; now = Date.now()) {
    const live = scan(wanted);
    if (!live.length) return { found: [...found.values()], left: [] };
    if (now >= end) return { found: [...found.values()], left: live };
    const late = now >= end - 3000;
    for (const p of live) {
      if (!found.has(p.pid)) found.set(p.pid, p);
      signal(p.pid, late ? "SIGKILL" : "SIGTERM");
    }
    await new Promise((r) => setTimeout(r, 200));
  }
}

/// KILLs every process of the jobs `wanted` takes at once, for a daemon exiting.
export function killNow(wanted: (job: string) => boolean): void {
  for (const p of scan(wanted)) signal(p.pid, "SIGKILL");
}

const list = (ps: Proc[]) => ps.map((p) => `pid ${p.pid} \`${p.cmd}\``).join(", ");

/// The job log's line on what of the job was left when it ended and what became of it (D556).
export function reaped(id: string, r: { found: Proc[]; left: Proc[] }, when: string): string {
  if (r.left.length)
    return `✗ ${when}: ${r.left.length} of the job's processes still run after SIGKILL: ${list(r.left)}. They may hold the slot's gpu.lock and stall every bench after: kill them on the box by hand (kill -9 <pid>; ps -o stat= -p <pid> showing D means stuck in the kernel), then check \`cargo xtask remote-render --status\`.`;
  if (!r.found.length) return `${when}: nothing of the job is left running (no process carries TERRA_RENDER_JOB=${id})`;
  const stray = r.found.length === 1 ? "1 process" : `${r.found.length} processes`;
  return `${when}: stopped ${stray} of the job still running (${list(r.found)}); nothing of the job is left. What a job starts in the background (setsid, nohup, &) must end before its command does: stop it there (kill its pid or group), or the worker does as here.`;
}
