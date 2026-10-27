// Which systems fill the slowest frames (D185, D388): reads the Chrome trace a sandbox built
// with `--features trace` writes (`TRACE_CHROME=<file>`, a span a system run), and for each
// world, the game's (a frame its `main app` span) and the render world's (its `RenderApp`
// sub-app, extraction included), says how its frame splits by schedule, which systems ran
// longest in its slowest frames and which cost the most at their worst across the run.
//
// Traced, the render world runs after the game on the main thread (terra_world's `trace`),
// so every span on any thread belongs to the world whose frame it starts in. A schedule runs
// on the main thread, one after another, so its time is the frame's wall clock; a system's
// time is its spans' sum within the frame, and as a parallel system's span also holds its
// wait for the workers, a frame's systems can sum to more than the frame. Applying a
// system's commands counts apart (`… [commands]`).
// Usage: node tools/sandbox/spans.ts <trace.json> [--skip 240] [--slowest 8] [--top 15] [--generics]
// `--skip` leaves out the first frames, loading and settling, as a benchmark's `--settle` does;
// `--generics` keeps a system's generic parameters, so each material's instance shows apart.
// Node 22.18 or later; no dependencies.

import { createReadStream } from "node:fs";
import { createInterface } from "node:readline";

export type Span = { name: string; start: number; end: number; tid: string };
export type Frame = { index: number; ms: number; systems: Map<string, number> };
export type World = "game" | "render";
export type Trace = { spans: Span[] };

/// A span's display name: a schedule's run as `[schedule] <label>`, a system's path without
/// its generic parameters (kept with `generics`), `[commands]` when it is the applying of that
/// system's commands, `main app` and `render app` for the worlds' frames, or null for any
/// other span.
export function spanName(raw: string, generics = false): string | null {
  if (raw.startsWith("main app")) return "main app";
  if (raw.startsWith("sub app: name=RenderApp")) return "render app";
  const schedule = raw.match(/^schedule: name=(.*)$/);
  if (schedule) return `[schedule] ${schedule[1]}`;
  const m = raw.match(/^(system|system_commands): name="?(.*?)"?$/);
  if (!m) return null;
  // A path's own generic parameters go; a `<T as Trait>::f` keeps its qualified head.
  const path = generics || m[2].startsWith("<") ? m[2] : m[2].replace(/<.*$/, "");
  return m[1] === "system" ? path : `${path} [commands]`;
}

/// Pairs each thread's begin and end events into the spans `spanName` keeps (`X` events
/// carry their own length), by start time.
export async function pair(
  events: Iterable<Record<string, unknown>> | AsyncIterable<Record<string, unknown>>,
  generics = false,
): Promise<Trace> {
  const open = new Map<string, { name: string; ts: number }[]>();
  const spans: Span[] = [];
  const keep = (raw: string, start: number, end: number, tid: string) => {
    const name = spanName(raw, generics);
    if (name) spans.push({ name, start, end, tid });
  };
  for await (const e of events) {
    const tid = `${e.pid}:${e.tid}`;
    const ts = Number(e.ts);
    if (e.ph === "X") keep(String(e.name), ts, ts + Number(e.dur), tid);
    else if (e.ph === "B") {
      const stack = open.get(tid) ?? [];
      stack.push({ name: String(e.name), ts });
      open.set(tid, stack);
    } else if (e.ph === "E") {
      const begun = open.get(tid)?.pop();
      if (begun) keep(begun.name, begun.ts, ts, tid);
    }
  }
  spans.sort((a, b) => a.start - b.start);
  return { spans };
}

/// One world's frames, each its length and its spans' summed time in ms by name, a span
/// counted in the frame it starts in.
export function attribute(trace: Trace, world: World): Frame[] {
  const isFrame = world === "game" ? "main app" : "render app";
  const frames = trace.spans.filter((s) => s.name === isFrame);
  const out: Frame[] = frames.map((f, index) => ({ index, ms: (f.end - f.start) / 1000, systems: new Map() }));
  let at = 0;
  for (const s of trace.spans) {
    if (s.name === "main app" || s.name === "render app") continue;
    while (at < frames.length && frames[at].end < s.start) at++;
    if (at === frames.length) break;
    if (s.start < frames[at].start) continue;
    const m = out[at].systems;
    m.set(s.name, (m.get(s.name) ?? 0) + (s.end - s.start) / 1000);
  }
  return out;
}

export function quantile(sorted: number[], q: number): number {
  if (sorted.length === 0) return 0;
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(q * sorted.length) - 1))];
}

/// Spans by their 99th-percentile time a frame over the frames given (a frame one did not
/// run in counts as 0), with their median and worst.
export function costliest(frames: Frame[], top: number, keep = (_: string) => true) {
  const names = new Set<string>();
  for (const f of frames) for (const n of f.systems.keys()) if (keep(n)) names.add(n);
  const rows = [...names].map((name) => {
    const t = frames.map((f) => f.systems.get(name) ?? 0).sort((a, b) => a - b);
    return { name, p50: quantile(t, 0.5), p99: quantile(t, 0.99), max: t[t.length - 1] };
  });
  return rows.sort((a, b) => b.p99 - a.p99 || b.max - a.max).slice(0, top);
}

/// The trace's events a line each. A trace runs to gigabytes, so a line is parsed only when
/// it names a span `spanName` keeps; begin and end events both carry
/// the name, so what is left still pairs.
export async function* events(path: string): AsyncGenerator<Record<string, unknown>> {
  const lines = createInterface({ input: createReadStream(path), crlfDelay: Infinity });
  for await (const raw of lines) {
    if (
      !raw.includes('"name":"main app') &&
      !raw.includes('"name":"system') &&
      !raw.includes('"name":"schedule') &&
      !raw.includes('"name":"sub app: name=RenderApp')
    )
      continue;
    const line = raw.trim().replace(/^\[/, "").replace(/[,\]]+$/, "");
    if (!line.startsWith("{")) continue;
    try {
      yield JSON.parse(line);
    } catch {
      // A run ended before its trace was flushed leaves its last line cut short.
    }
  }
}

function arg(args: string[], name: string, fallback: number): number {
  const at = args.indexOf(name);
  return at >= 0 ? Number(args[at + 1]) : fallback;
}

const ms = (v: number) => v.toFixed(2).padStart(7);
const isSchedule = (n: string) => n.startsWith("[schedule]");

function report(world: World, all: Frame[], args: string[]) {
  const lengths = all.map((f) => f.ms).sort((a, b) => a - b);
  const what = world === "game" ? "the game's update" : "the render world's, extraction included,";
  console.log(
    `\n${world}: ${all.length} frames, ${what} median ${quantile(lengths, 0.5).toFixed(2)} ms, ` +
      `99th ${quantile(lengths, 0.99).toFixed(2)}, worst ${lengths[lengths.length - 1].toFixed(2)}`,
  );
  const schedules = costliest(all, 12, isSchedule).sort((a, b) => b.p50 - a.p50);
  console.log(`  schedules, median (99th) ms: ${schedules.map((r) => `${r.name.slice(11)} ${r.p50.toFixed(2)} (${r.p99.toFixed(2)})`).join(", ")}`);
  console.log("  slowest frames, their longest systems (ms):");
  for (const f of [...all].sort((a, b) => b.ms - a.ms).slice(0, arg(args, "--slowest", 8))) {
    const longest = [...f.systems].filter(([n]) => !isSchedule(n)).sort((a, b) => b[1] - a[1]).slice(0, 4);
    console.log(`    frame ${String(f.index).padStart(5)} ${ms(f.ms)}  ${longest.map(([n, t]) => `${n} ${t.toFixed(2)}`).join(", ")}`);
  }
  console.log("  systems by 99th percentile a frame (ms):".padEnd(94) + " median     99th    worst");
  for (const r of costliest(all, arg(args, "--top", 15), (n) => !isSchedule(n))) {
    console.log(`    ${r.name.slice(-88).padEnd(88)} ${ms(r.p50)}  ${ms(r.p99)}  ${ms(r.max)}`);
  }
}

async function main(args: string[]) {
  const path = args.find((a, i) => !a.startsWith("--") && !args[i - 1]?.startsWith("--"));
  if (!path) {
    console.error("usage: node tools/sandbox/spans.ts <trace.json> [--skip 240] [--slowest 8] [--top 15] [--generics]");
    process.exit(2);
  }
  const trace = await pair(events(path), args.includes("--generics"));
  for (const world of ["game", "render"] as const) {
    const all = attribute(trace, world).slice(arg(args, "--skip", 240));
    if (all.length === 0) {
      console.error(`✗ no ${world} frames after --skip in ${path}: was the sandbox built with --features trace?`);
      process.exit(1);
    }
    report(world, all, args);
  }
}

if (import.meta.main) await main(process.argv.slice(2));
