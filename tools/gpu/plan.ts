// The GPU wrapper's decisions (D319), pure so they are tested without a GPU: each class's stall
// limit, reading what nvidia-smi and /proc say, and whether a running job still makes progress.
// gpu.ts takes the lock, runs the job and keeps the state.

/// A kind of GPU job. Only `bench` is treated apart (the card alone); the rest differ by how long
/// they may go without progress.
export type Class = "still" | "sheet" | "film" | "bake" | "bench";

export interface ClassInfo {
  /// A run that makes no progress for this long is stuck and is stopped, minutes.
  stallMin: number;
  does: string;
}

export const CLASSES: Record<Class, ClassInfo> = {
  still: { stallMin: 5, does: "a few stills or vantages" },
  sheet: { stallMin: 8, does: "a sheet at a small place (the yard, the fire sheet, a workbench)" },
  film: { stallMin: 10, does: "a film or a long probe at a big place" },
  bake: { stallMin: 10, does: "a bake-off or sweep over several scenes" },
  bench: { stallMin: 10, does: "a benchmark: waits for the card alone, and holds back new runs while it waits (D319)" },
};

export const isClass = (name: string): name is Class => Object.hasOwn(CLASSES, name);

/// The card's total and used memory from `nvidia-smi --query-gpu=memory.total,memory.used
/// --format=csv,noheader,nounits` (the first GPU's line), MB.
export function parseCard(csv: string): { totalMB: number; usedMB: number } | null {
  const [total, used] = (csv.trim().split("\n")[0] ?? "").split(",").map((field) => Number(field.trim()));
  return Number.isFinite(total) && Number.isFinite(used) && total > 0 ? { totalMB: total, usedMB: used } : null;
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

/// A process's CPU time so far, clock ticks, from its /proc/<pid>/stat line: its own (utime,
/// stime) and its reaped children's (cutime, cstime), so a child's work stays counted once it ends.
export function parseCpu(stat: string): number {
  const fields = (stat.split(")").pop() ?? "").trim().split(/\s+/);
  const ticks = fields.slice(11, 15).map(Number).reduce((sum, n) => sum + n, 0);
  return Number.isFinite(ticks) ? ticks : 0;
}

/// Every process's parent and CPU time so far, hundredths of a second, where there is no /proc
/// (macOS), from `ps -A -S -o pid=,ppid=,time=`: `-S` adds its reaped children's time, as
/// parseCpu does, and the time reads `[dd-][hh:]mm:ss.ss`.
export function parsePs(text: string): { parents: Map<number, number>; cpu: Map<number, number> } {
  const parents = new Map<number, number>();
  const cpu = new Map<number, number>();
  for (const line of text.split("\n")) {
    const [pid, parent, time] = line.trim().split(/\s+/);
    const [days, clock] = time?.includes("-") ? time.split("-") : ["0", time ?? ""];
    const seconds = clock.split(":").reduce((sum, part) => sum * 60 + Number(part), 0) + Number(days) * 86400;
    if (!Number.isInteger(Number(pid)) || !Number.isInteger(Number(parent)) || !clock || !Number.isFinite(seconds)) continue;
    parents.set(Number(pid), Number(parent));
    cpu.set(Number(pid), Math.round(seconds * 100));
  }
  return { parents, cpu };
}

/// The processes busy on the card's cores now, from `nvidia-smi pmon -c 1 -s u` (a line a
/// process: gpu, pid, type, sm %, ...; `-` when idle). Empty where pmon is missing, as under WSL.
export function parsePmon(text: string): Set<number> {
  const busy = new Set<number>();
  for (const line of text.split("\n")) {
    if (line.trimStart().startsWith("#")) continue;
    const [, pid, , sm] = line.trim().split(/\s+/);
    if (Number.isInteger(Number(pid)) && Number(sm) > 0) busy.add(Number(pid));
  }
  return busy;
}

/// CPU ticks a job's processes used between two readings: each one's growth, a new one's all.
export function cpuGrowth(before: Map<number, number>, after: Map<number, number>): number {
  let used = 0;
  for (const [pid, ticks] of after) used += Math.max(0, ticks - (before.get(pid) ?? 0));
  return used;
}

/// The least use of a core, averaged over a window, that counts as a job working: a hung
/// process uses none, a waiting one a few ticks a minute, a rendering one most of a core.
export const BUSY_SHARE = 0.05;
export const BUSY_WINDOW_MS = 60000;

/// Whether a job is making progress (D319): new output (its own, or a watched log), its process
/// tree's CPU time growing by BUSY_SHARE of a core over a window, or one of its processes busy on
/// the card. A job is stalled only once none of these has happened for its stall limit.
export class Progress {
  last: number;
  private mark: { at: number; cpu: Map<number, number> };
  private window: number;
  private hz: number;

  constructor(now: number, stallMs: number, hz = 100) {
    this.last = now;
    this.mark = { at: now, cpu: new Map() };
    this.window = Math.min(BUSY_WINDOW_MS, stallMs / 4);
    this.hz = hz;
  }

  output(now: number) {
    this.last = now;
  }

  /// A reading of the tree: each process's CPU ticks, and whether one is busy on the card.
  read(now: number, cpu: Map<number, number>, gpuBusy: boolean) {
    if (gpuBusy) this.last = now;
    const span = now - this.mark.at;
    if (span < this.window) return;
    if (cpuGrowth(this.mark.cpu, cpu) / this.hz >= (BUSY_SHARE * span) / 1000) this.last = now;
    this.mark = { at: now, cpu };
  }

  stalled(now: number, stallMs: number): boolean {
    return now - this.last > stallMs;
  }
}

/// A duration for a person: 40s, 3m10s, 2h05m.
export function age(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m${String(s % 60).padStart(2, "0")}s`;
  return `${Math.floor(s / 3600)}h${String(Math.floor(s / 60) % 60).padStart(2, "0")}m`;
}
