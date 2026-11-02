// The render box's disk guard (docs/render-worker.md#the-disk, D445): free space checked before
// each job and in every pass of the daemon's housekeeping, the box's caches pruned in a fixed
// order when it runs low, and a full disk (ENOSPC) caught, so the daemon stops taking jobs and
// keeps saying why instead of crash-looping. As CI agents guard their disks: Buildkite's
// Elastic CI Stack checks free space before each job (`bk-check-disk-space.sh`), prunes its
// caches when it is low and refuses the job when it stays low; cargo-sweep prunes a target/'s
// incremental state first, the part a rebuild remakes cheapest. Also what `--status` and a
// client's wait say of a daemon that is down or out of disk (`health`). Pure but for `space`.

import { statfsSync } from "node:fs";
import { join } from "node:path";
import type { DaemonInfo } from "./box.ts";
import { ago, type Job, whole } from "./jobs.ts";

export const GB = 1024 ** 3;
export const gb = (bytes: number) => (bytes >= 1024 * GB ? `${(bytes / 1024 / GB).toFixed(1)} TB` : `${Math.round(bytes / GB)} GB`);

/// TERRA_RENDER_DISK_LOW_PERCENT and _GB: the daemon prunes while free space is under either.
/// TERRA_RENDER_DISK_FULL_GB: under it, after pruning, no job starts. TERRA_RENDER_MEDIA_DAYS:
/// the age past which an ended job's collected files may be pruned (its job.json and log stay).
export type DiskSettings = { lowPercent: number; lowGb: number; fullGb: number; mediaDays: number };
export const DISK = { lowPercent: 15, lowGb: 150, fullGb: 30, mediaDays: 3 };

export function diskSettings(env: Record<string, string | undefined>): DiskSettings | { error: string } {
  const lowPercent = whole(env.TERRA_RENDER_DISK_LOW_PERCENT, DISK.lowPercent, 90);
  if (!lowPercent) return { error: "TERRA_RENDER_DISK_LOW_PERCENT: a whole percentage from 1 to 90" };
  const lowGb = whole(env.TERRA_RENDER_DISK_LOW_GB, DISK.lowGb, 100_000);
  if (!lowGb) return { error: "TERRA_RENDER_DISK_LOW_GB: whole GB, at least 1" };
  const fullGb = whole(env.TERRA_RENDER_DISK_FULL_GB, DISK.fullGb, 100_000);
  if (!fullGb || fullGb >= lowGb) return { error: "TERRA_RENDER_DISK_FULL_GB: whole GB, at least 1 and under TERRA_RENDER_DISK_LOW_GB" };
  const mediaDays = whole(env.TERRA_RENDER_MEDIA_DAYS, DISK.mediaDays, 365);
  if (!mediaDays) return { error: "TERRA_RENDER_MEDIA_DAYS: whole days from 1 to 365" };
  return { lowPercent, lowGb, fullGb, mediaDays };
}

/// Free space (what this user may write, `bavail`) and the disk's size, in bytes.
export type Space = { free: number; total: number };
export function space(path: string): Space {
  const fs = statfsSync(path);
  return { free: fs.bavail * fs.bsize, total: fs.blocks * fs.bsize };
}

/// The low mark (prune under it), the mark pruning stops at (a fifth above it, so one prune
/// buys a run of jobs rather than one), and the full mark (no job starts under it).
export const lowMark = (s: DiskSettings, total: number) => Math.max((total * s.lowPercent) / 100, s.lowGb * GB);
export const pruneTo = (s: DiskSettings, total: number) => lowMark(s, total) * 1.2;
export const fullMark = (s: DiskSettings) => s.fullGb * GB;
export function level(s: DiskSettings, sp: Space): "ok" | "low" | "full" {
  return sp.free < fullMark(s) ? "full" : sp.free < lowMark(s, sp.total) ? "low" : "ok";
}

/// Whether an error is the disk being out of space or quota.
export const diskFull = (e: unknown) => {
  const code = (e as NodeJS.ErrnoException | undefined)?.code;
  return code === "ENOSPC" || code === "EDQUOT";
};

/// What the daemon last saw of its disk, in daemon.json: `full` says why it takes no job.
export type DiskState = Space & { at: string; full?: string };

/// The line `--status` gives the disk.
export const diskLine = (s: DiskSettings, sp: Space) =>
  `disk: ${gb(sp.free)} free of ${gb(sp.total)} (${Math.round((100 * sp.free) / Math.max(1, sp.total))}%); under ${gb(lowMark(s, sp.total))} the daemon prunes, under ${gb(fullMark(s))} it takes no job`;

// ---- What may go, in order ----

/// A job's files kept when its collected files are pruned.
export const KEEP = ["job.log", "patch.diff"];

/// One thing to prune: a folder removed whole (`dir`), an ended job's collected files but its
/// log (`media`), or the files of a slot's HOME review cache older than `before` (`review`).
export type Prune = { stage: 1 | 2 | 3; kind: "dir" | "media" | "review"; path: string; what: string; before?: number };

/// A slot as the pruner sees it: whether a job runs in it, its build folder, the profiles there
/// with an `incremental/` folder, and its HOME's `.cache/terra-review` when it holds files old
/// enough to go.
export type SlotUse = { slot: number; busy: boolean; target: string; profiles: string[]; review?: string };
/// An ended job whose out/ holds more than KEEP.
export type JobUse = Pick<Job, "id" | "state" | "queued" | "ended"> & { out: string };

export const STAGES = { 1: "debug incremental of idle slots", 2: "collected files of jobs that ended", 3: "release incremental of idle slots" } as const;

/// What may be pruned, in the order it goes: (1) the debug profile's incremental state in each
/// idle slot (the most of a slot's build, 35-48 GB on 2026-11-02, and what a rebuild remakes
/// cheapest, its crates' own code only); (2) the collected files of jobs that ended over
/// `mediaDays` ago, and the files under each idle slot's HOME review cache older than that
/// (the files out/ holds are often hard links of them, so out/ alone frees nothing); (3) every
/// other profile's incremental state in each idle slot (release's, which renders and benches
/// rebuild from). A slot that runs a job is never touched, since rustc would crash on its
/// incremental state going mid-build; nor is anything of a job queued or running.
export function pruneOrder(slots: SlotUse[], jobs: JobUse[], now: Date, mediaDays: number): Prune[] {
  const idle = slots.filter((s) => !s.busy).sort((a, b) => a.slot - b.slot);
  const before = now.getTime() - mediaDays * 86400_000;
  const out: Prune[] = [];
  for (const s of idle) if (s.profiles.includes("debug")) out.push({ stage: 1, kind: "dir", path: join(s.target, "debug", "incremental"), what: `slot ${s.slot}'s debug/incremental` });
  const old = jobs.filter((j) => j.state !== "queued" && j.state !== "running" && Date.parse(j.ended ?? j.queued) < before).sort((a, b) => a.id.localeCompare(b.id));
  for (const j of old) out.push({ stage: 2, kind: "media", path: j.out, what: `job ${j.id}'s files` });
  for (const s of idle) if (s.review) out.push({ stage: 2, kind: "review", path: s.review, what: `slot ${s.slot}'s HOME review cache`, before });
  for (const s of idle) for (const p of s.profiles.filter((p) => p !== "debug").sort()) out.push({ stage: 3, kind: "dir", path: join(s.target, p, "incremental"), what: `slot ${s.slot}'s ${p}/incremental` });
  return out;
}

/// Prunes in order until free space reaches `target`, measuring each removal by the disk's
/// own free space (hard links and sparse files make sizes lie); a line a stage of what it freed.
/// Returns the bytes freed.
export async function prune(items: Prune[], target: number, free: () => number, remove: (p: Prune) => Promise<void>, say: (line: string) => void): Promise<number> {
  const start = free();
  const done = new Map<number, { freed: number; what: string[] }>();
  for (const p of items) {
    if (free() >= target) break;
    const before = free();
    await remove(p);
    const st = done.get(p.stage) ?? { freed: 0, what: [] };
    st.freed += Math.max(0, free() - before);
    st.what.push(p.what);
    done.set(p.stage, st);
  }
  for (const [stage, st] of done) {
    const what = stage === 2 ? `${st.what.filter((w) => w.startsWith("job ")).length} jobs' files, ${st.what.filter((w) => !w.startsWith("job ")).length} HOME caches` : st.what.join(", ");
    say(`disk: pruned the ${STAGES[stage as 1 | 2 | 3]} (${what}): freed ${gb(st.freed)}`);
  }
  return free() - start;
}

// ---- What --status and a wait say of a daemon in trouble ----

/// How long a daemon's last word may be old before a client's wait gives up on it: longer than
/// a restart (5 s) and its first pass.
export const DOWN_MS = 2 * 60_000;

/// A problem with the box, what fixes it, and whom it stops: `all` jobs (a daemon down: a job
/// shown running there is not), or only `queued` ones (a full disk: a running job may finish).
export type Problem = { what: string; fix: string; stops: "all" | "queued"; downMs?: number };

export function health(d: (DaemonInfo & { alive: boolean; disk?: DiskState }) | undefined, s: DiskSettings, sp: Space, dir: string, now: Date): Problem[] {
  const out: Problem[] = [];
  if (!d) out.push({ what: "the daemon has never run on this box", fix: "start it: cargo xtask remote-render --update", stops: "all", downMs: Number.POSITIVE_INFINITY });
  else if (!d.alive) {
    out.push({
      what: `the daemon has not answered for ${ago(d.beat, now)} (its last word: ${d.status}), so no job runs and a job shown running is not`,
      fix: "why: cargo xtask remote-render --logs daemon; start it again: cargo xtask remote-render --update (on the box: systemctl --user status terra-render)",
      stops: "all",
      downMs: now.getTime() - Date.parse(d.beat),
    });
  }
  const full = sp.free < fullMark(s) ? `${gb(sp.free)} free, under the ${gb(fullMark(s))} mark` : d?.alive ? d.disk?.full : undefined;
  if (full) {
    out.push({
      what: `the box's disk is full (${full}): the daemon has pruned what it may and starts no job until space is freed`,
      fix: `see what holds it on the box: du -xsh ${dir}/slots/*/target ${dir}/jobs ${dir}/homes; a free slot's whole target/ may go (its next job rebuilds it), or lower TERRA_RENDER_MEDIA_DAYS or TERRA_RENDER_KEEP_DAYS in ${dir}/repo/.env`,
      stops: "queued",
    });
  }
  return out;
}
