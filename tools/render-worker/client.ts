// `cargo xtask remote-render` (docs/render-worker.md, D445): queues a render job on the render
// box at a pushed commit, waits for it (a look every 15 s, then up to a minute, a line when its
// state changes), downloads its files into ~/.cache/terra-review/remote/<job>/ and exits with
// the command's code. `--local` runs the same flow on this machine with a folder for a store and
// the worker in this process, no GitHub and no box: the dry run of the whole path.

import { spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { homedir, hostname, userInfo } from "node:os";
import { join } from "node:path";
import { ago, backoff, type Job, jobId, parseClient, type Rec, USAGE } from "./jobs.ts";
import { open, type Store } from "./store.ts";
import { stateDir, takeOne, worker } from "./worker.ts";

const git = (args: string[]) => spawnSync("git", args, { encoding: "utf8" });

function sleep(ms: number): void {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

const short = (job: Job) => `${job.rev.slice(0, 9)}: ${job.command.join(" ").slice(0, 100)}`;

/// The commit to run, in full, or why it cannot be: it must be on a branch of origin.
export function resolveRev(rev: string | undefined): { sha: string } | { error: string } {
  const sha = git(["rev-parse", "--verify", "-q", `${rev ?? "HEAD"}^{commit}`]).stdout.trim();
  if (!sha) return { error: `no commit ${rev}` };
  git(["fetch", "-q", "origin"]);
  if (!git(["branch", "-r", "--contains", sha]).stdout.trim()) {
    return { error: `${sha.slice(0, 9)} is on no branch of origin; push it first (git push origin HEAD:main), the box runs only pushed commits` };
  }
  return { sha };
}

export function queue(store: Store, job: Omit<Job, "id" | "queued" | "state" | "by">, now = new Date()): Rec {
  const full: Job = { ...job, id: jobId(now, randomBytes(2).toString("hex")), queued: now.toISOString(), state: "queued", by: `${userInfo().username}@${hostname()}` };
  return store.create(full);
}

/// Waits for the job to end, downloads and unpacks its files into `out`, and returns its exit.
export function wait(store: Store, id: string, out: string, pause = sleep): number {
  let state = "";
  for (let n = 0; ; n++) {
    const rec = store.list().find((r) => r.job.id === id);
    if (!rec) {
      console.log(`✗ render ${id}: no such job (deleted after a week, or cancelled)`);
      return 1;
    }
    const job = rec.job;
    if (job.state !== state && (job.state === "queued" || job.state === "running")) {
      console.log(`render ${id}: ${job.state}${job.worker ? ` on ${job.worker}` : ""} (${short(job)})`);
    }
    state = job.state;
    if (job.state === "done" || job.state === "failed") {
      rmSync(out, { recursive: true, force: true });
      store.download(rec, out);
      const tar = join(out, "files.tar");
      if (existsSync(tar)) {
        spawnSync("tar", ["-xf", tar, "-C", out]);
        rmSync(tar);
      }
      const took = job.started ? ago(job.started, new Date(job.ended ?? Date.now())) : "?";
      if (job.state === "done") {
        console.log(`✓ render ${id}: done in ${took} (${job.note}); files in ${out}`);
        return 0;
      }
      const log = join(out, "job.log");
      if (existsSync(log)) console.log(readFileSync(log, "utf8").trimEnd().split("\n").slice(-15).join("\n"));
      console.log(`✗ render ${id}: failed after ${took}: ${job.note}; log ${log}`);
      return job.exit || 1;
    }
    pause(backoff(n) * 1000);
  }
}

function status(store: Store): number {
  const seen = store.seen();
  const recs = store.list();
  const now = new Date();
  console.log(seen ? `worker: ${seen.text} (seen ${ago(seen.when, now)} ago)` : "worker: never seen; is the box set up? (tools/render-worker/README.md)");
  for (const r of recs) {
    const j = r.job;
    console.log(`  ${j.id} ${j.state.padEnd(7)} ${ago(j.queued, now).padStart(5)} ago  ${short(j)}${j.note ? `  [${j.note}]` : ""}`);
  }
  if (!recs.length) console.log("  no jobs");
  return 0;
}

function main(): number {
  const args = parseClient(process.argv.slice(2));
  if ("error" in args) {
    console.log(`✗ remote-render: ${args.error}\n${USAGE}`);
    return 2;
  }
  const top = git(["rev-parse", "--show-toplevel"]).stdout.trim();
  const local = args.local ? join(stateDir(top), "local-store") : undefined;
  let store: Store;
  try {
    store = open(top, local);
  } catch (e) {
    console.log(`✗ remote-render: ${(e as Error).message}`);
    return 1;
  }
  const outOf = (id: string, out?: string) => out ?? join(homedir(), ".cache/terra-review/remote", id);
  if (args.mode === "status") return status(store);
  if (args.mode === "wait") return wait(store, args.id, outOf(args.id, args.out));
  if (args.mode === "cancel") {
    const rec = store.list().find((r) => r.job.id === args.id);
    if (!rec || rec.job.state !== "queued") {
      console.log(`✗ render ${args.id}: ${rec ? `${rec.job.state}, so it stays` : "no such job"}`);
      return 1;
    }
    store.remove(rec);
    console.log(`✓ render ${args.id}: cancelled`);
    return 0;
  }
  const rev = resolveRev(args.rev);
  if ("error" in rev) {
    console.log(`✗ remote-render: ${rev.error}`);
    return 1;
  }
  const seen = store.seen();
  const running = store.list().some((r) => r.job.state === "running");
  if (!args.local && !running && (!seen || Date.now() - Date.parse(seen.when) > 15 * 60_000)) {
    console.log(`remote-render: the worker was ${seen ? `last seen ${ago(seen.when, new Date())} ago` : "never seen"}; the job waits until it is up`);
  }
  const rec = queue(store, { rev: rev.sha, command: args.command, collect: args.collect, cook: args.cook, minutes: args.minutes });
  console.log(`render ${rec.job.id}: queued (${short(rec.job)}) at ${store.where(rec)}`);
  if (args.local) {
    const w = worker(top);
    if ("error" in w) {
      console.log(`✗ remote-render: ${w.error}`);
      return 2;
    }
    takeOne(store, { ...w, policy: { afterCi: false } });
  }
  if (!args.wait) {
    console.log(`  wait for it: cargo xtask remote-render --wait ${rec.job.id}${args.local ? " --local" : ""}`);
    return 0;
  }
  return wait(store, rec.job.id, outOf(rec.job.id, args.out));
}

if (import.meta.main) process.exit(main());
