// The GPU scheduler's decisions (D319), pure so they are tested without a GPU: which waiting
// job runs next, how much memory each class is expected to need, how long a job will wait, and
// reading what nvidia-smi and /proc/locks say. gpu.ts does the waiting, running and recording.
//
// The model is a batch scheduler's (Slurm's backfill, Kubernetes' resource requests): each job
// asks for an amount of the one resource that runs out, the GPU's memory; a job starts when its
// request fits what is free; the queue is ordered by class, short first, and by age, so nothing
// starves; a job that cannot fit holds back those behind it once it has waited long enough.

/// A kind of GPU job. What tells them apart is how long they hold the GPU and how much of it.
export type Class = "still" | "sheet" | "film" | "bake" | "bench";

export interface ClassInfo {
  /// Order in the queue, lowest first; one step is worth AGE_STEP_MIN minutes of waiting.
  rank: number;
  /// Memory on the card before any is measured, MiB as nvidia-smi counts it: measured 2026-10-19 with cooked textures
  /// (D188): a settlement run peaks near 7.7 GB, a yard or fire sheet under 5, a still under 3.
  vramMB: number;
  /// Run time before any is measured, minutes.
  minutes: number;
  /// A run that writes nothing for this long is stuck and is stopped, minutes.
  stallMin: number;
  /// Once at the head of the queue this long, it holds back those behind it, minutes.
  drainMin: number;
  does: string;
}

export const CLASSES: Record<Class, ClassInfo> = {
  still: { rank: 0, vramMB: 3000, minutes: 1, stallMin: 5, drainMin: 10, does: "a few stills or vantages" },
  sheet: { rank: 1, vramMB: 5000, minutes: 5, stallMin: 8, drainMin: 10, does: "a sheet at a small place (the yard, the fire sheet, a workbench)" },
  bench: { rank: 1, vramMB: 8000, minutes: 10, stallMin: 10, drainMin: 5, does: "a benchmark: the GPU alone (D319)" },
  film: { rank: 3, vramMB: 7900, minutes: 15, stallMin: 10, drainMin: 15, does: "a film or a long probe at a big place" },
  bake: { rank: 3, vramMB: 7900, minutes: 20, stallMin: 10, drainMin: 15, does: "a bake-off or sweep over several scenes" },
};

export const isClass = (name: string): name is Class => Object.hasOwn(CLASSES, name);

/// Minutes of waiting worth one class step: a film waiting 15 minutes is ahead of a new still.
export const AGE_STEP_MIN = 5;
/// Memory kept free on the card for the desktop and the drivers' growth, MB.
export const MARGIN_MB = 400;
/// Most jobs run at once, however small: CPU and the disk are shared too.
export const MAX_JOBS = 4;

/// A job as the scheduler sees it, waiting or running. Times are milliseconds since the epoch.
export interface Job {
  id: string;
  class: Class;
  /// Memory it asks for, MB.
  vramMB: number;
  enqueued: number;
  started?: number;
  /// Memory its processes hold now, MB (nvidia-smi), when running.
  usedMB?: number;
  /// Not ours: a run outside the scheduler, standing in for what it holds or waits for.
  foreign?: string;
}

/// The card: its memory and what processes other than our running jobs hold of it, MB.
export interface Card {
  totalMB: number;
  otherMB: number;
}

const minutes = (ms: number) => ms / 60000;

/// A waiting job's place in the queue now: its class's rank less a step per AGE_STEP_MIN waited.
export function priority(job: Job, now: number): number {
  return CLASSES[job.class].rank - minutes(now - job.enqueued) / AGE_STEP_MIN;
}

/// The queue in the order jobs are offered the GPU: by priority, then first come.
export function order(queued: Job[], now: number): Job[] {
  return [...queued].sort((a, b) => priority(a, now) - priority(b, now) || a.enqueued - b.enqueued || (a.id < b.id ? -1 : 1));
}

/// What a running job is taken to hold: what it asked for, or more if it has grown past it.
const claim = (job: Job) => Math.max(job.vramMB, job.usedMB ?? 0);

/// Memory free for a new job, MB: the card less what others hold, the margin and every running
/// job's claim (a job that has just started holds little yet, but will grow to its claim).
export function available(card: Card, running: Job[]): number {
  return card.totalMB - card.otherMB - MARGIN_MB - running.reduce((sum, job) => sum + claim(job), 0);
}

/// Which waiting jobs start now, in order. Walks the queue: a job starts if it fits; one that
/// does not is passed over (smaller jobs behind it fill the gap) until it has waited its class's
/// drainMin, then nothing behind it starts until it has. A benchmark starts only on an empty
/// card, and nothing starts beside one.
export function admit(queued: Job[], running: Job[], card: Card, now: number, maxJobs = MAX_JOBS): Job[] {
  const started: Job[] = [];
  const all = [...running];
  if (all.some((job) => job.class === "bench")) return started;
  let free = available(card, all);
  for (const job of order(queued, now)) {
    const info = CLASSES[job.class];
    const fits = job.class === "bench" ? all.length === 0 : all.length < maxJobs && job.vramMB <= free;
    if (fits) {
      started.push(job);
      all.push(job);
      free -= job.vramMB;
      if (job.class === "bench") break;
      continue;
    }
    if (minutes(now - job.enqueued) >= info.drainMin || all.length >= maxJobs) break;
  }
  return started;
}

/// When each waiting job is expected to start, ms since the epoch, by playing the queue forward:
/// each running job ends after its class's usual run time (or a minute from now if past it),
/// and the queue is offered the card at each end. Infinity for one that never fits.
export function expectedStarts(
  queued: Job[],
  running: Job[],
  card: Card,
  now: number,
  runMinutes: (job: Job) => number,
): Map<string, number> {
  const starts = new Map<string, number>();
  const live = running.map((job) => ({ job, end: Math.max((job.started ?? now) + runMinutes(job) * 60000, now + 60000) }));
  let waiting = [...queued];
  let t = now;
  for (let step = 0; step < 200 && waiting.length > 0; step++) {
    for (const job of admit(waiting, live.map((l) => l.job), card, t)) {
      starts.set(job.id, t);
      live.push({ job: { ...job, started: t, usedMB: 0 }, end: t + runMinutes(job) * 60000 });
    }
    waiting = waiting.filter((job) => !starts.has(job.id));
    if (waiting.length === 0 || live.length === 0) break;
    t = Math.min(...live.map((l) => l.end));
    for (let i = live.length - 1; i >= 0; i--) if (live[i].end <= t) live.splice(i, 1);
  }
  for (const job of waiting) starts.set(job.id, Infinity);
  return starts;
}

/// One finished run, as recorded in the history the estimates are learned from.
export interface Run {
  class: Class;
  peakMB: number;
  runS: number;
  code: number;
}

/// The memory a class is expected to need, MB: the 90th percentile of its last 20 measured
/// peaks with a tenth on top, once three are known; the table's figure until then.
export function estimate(cls: Class, history: Run[]): number {
  const peaks = history.filter((run) => run.class === cls && run.peakMB > 0).slice(-20).map((run) => run.peakMB);
  if (peaks.length < 3) return CLASSES[cls].vramMB;
  peaks.sort((a, b) => a - b);
  const p90 = peaks[Math.min(peaks.length - 1, Math.floor(peaks.length * 0.9))];
  return Math.ceil((p90 * 1.1) / 100) * 100;
}

/// A class's usual run time, minutes: the median of its last 20 runs that passed, or the table's figure.
export function usualMinutes(cls: Class, history: Run[]): number {
  const times = history.filter((run) => run.class === cls && run.runS > 0 && run.code === 0).slice(-20).map((run) => run.runS);
  if (times.length === 0) return CLASSES[cls].minutes;
  times.sort((a, b) => a - b);
  return times[Math.floor(times.length / 2)] / 60;
}

/// Why a run failed in a way worth one more try: the device lost or out of its memory.
export function retryable(output: string): string | null {
  if (/device ?lost|DEVICE_LOST|Parent device is lost/i.test(output)) return "the GPU device was lost";
  if (/out of (device )?memory|OUT_OF_DEVICE_MEMORY|OutOfMemory|failed to allocate/i.test(output)) return "the GPU ran out of memory";
  return null;
}

/// The card's total and used memory from `nvidia-smi --query-gpu=memory.total,memory.used
/// --format=csv,noheader,nounits` (the first GPU's line), MB.
export function parseCard(csv: string): { totalMB: number; usedMB: number } | null {
  const [total, used] = (csv.trim().split("\n")[0] ?? "").split(",").map((field) => Number(field.trim()));
  return Number.isFinite(total) && Number.isFinite(used) && total > 0 ? { totalMB: total, usedMB: used } : null;
}

/// Each process's memory on the card from `nvidia-smi --query-compute-apps=pid,used_memory
/// --format=csv,noheader,nounits`, MB by pid.
export function parseApps(csv: string): Map<number, number> {
  const apps = new Map<number, number>();
  for (const line of csv.trim().split("\n")) {
    const [pid, used] = line.split(",").map((field) => Number(field.trim()));
    if (Number.isInteger(pid) && Number.isFinite(used)) apps.set(pid, (apps.get(pid) ?? 0) + used);
  }
  return apps;
}

/// Who holds or waits for a lock on a file by its inode, from /proc/locks: lines such as
/// `33: FLOCK  ADVISORY  WRITE 704977 00:1c:7468718 0 EOF`, a waiter's marked `->`.
export interface Holder {
  pid: number;
  write: boolean;
  waiting: boolean;
}

export function parseLocks(locks: string, inode: number): Holder[] {
  const holders: Holder[] = [];
  for (const line of locks.split("\n")) {
    const fields = line.trim().split(/\s+/);
    const waiting = fields[1] === "->";
    const [, , , mode, pid, device] = waiting ? fields.slice(1) : fields;
    if (!device || Number(device.split(":").pop()) !== inode) continue;
    holders.push({ pid: Number(pid), write: mode === "WRITE", waiting });
  }
  return holders;
}

/// Every process under a root, itself included, from each process's parent.
export function descendants(root: number, parents: Map<number, number>): Set<number> {
  const children = new Map<number, number[]>();
  for (const [pid, parent] of parents) children.set(parent, [...(children.get(parent) ?? []), pid]);
  const found = new Set<number>([root]);
  const stack = [root];
  while (stack.length > 0) for (const child of children.get(stack.pop()!) ?? []) if (!found.has(child)) found.add(child), stack.push(child);
  return found;
}

/// A duration for a person: 40s, 3m10s, 2h05m.
export function age(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m${String(s % 60).padStart(2, "0")}s`;
  return `${Math.floor(s / 3600)}h${String(Math.floor(s / 60) % 60).padStart(2, "0")}m`;
}
