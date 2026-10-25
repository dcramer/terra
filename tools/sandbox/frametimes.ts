// Two builds' frame times against each other (D185, D388): the CSVs a benchmark run writes
// with `--frame-times`, a run a file, old's and new's, judged on four bars meant for a change
// to the game's own work (the main world) on a frame the render world bounds, where a
// frame's whole p99 is mostly the view and the GPU, and so noise between two builds. Both
// builds walk the same path (the same route spots reached), so the same ground comes into view:
//   1. game:     the median of new's runs' p99 of game_ms, the sky's repaints left out (frames
//                with game_ms over 10, terra_world::skylight::paint's 7 ms), within old's
//                runs' range (or under it);
//   2. repaints: those frames, a count a run: new's median within old's range ± 1, or the
//                two builds' paths differ and the A/B compares different ground;
//   3. long:     every new frame over 25 ms is long in the render world or the GPU, not the game;
//   4. new long: every new frame over 25 ms has one in an old run within 60 frames of it
//                (the same ground's first sight, pipelines compiling), not a stall old lacks;
//   5. frame:    the median of new's runs' frame p99 no more than the slowest of old's runs.
// A change to how the player moves is judged with the camera on a fixed path, replayed alike
// in every run (`--glide` and a `--vantage …@frame` key every 10 frames, recorded from one
// run's camera), so what is in view cannot differ (D53).
// Usage: node tools/sandbox/frametimes.ts --old a.csv b.csv … --new c.csv d.csv …
// Leave a cold first run (shaders compiling, the pipeline cache empty) out of the files.
// Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";
import { basename } from "node:path";

export type Frame = { frame: number; ms: number; game: number; busy: number; gpu: number };

/// The bars' margins.
export const LONG_MS = 25;
/// A frame whose game takes longer is a sky repaint (traced: 7 ms of paint on ~4.5 of game).
export const REPAINT_MS = 10;
/// How near in frames a new long frame must be to an old one to be the same ground's.
export const LONG_NEAR_FRAMES = 60;

/// A `--frame-times` CSV, by its header's names.
export function parse(text: string): Frame[] {
  const [head, ...rows] = text.trim().split("\n");
  const cols = head.split(",");
  const at = (name: string) => {
    const i = cols.indexOf(name);
    if (i < 0) throw new Error(`no column ${name} in ${head}`);
    return i;
  };
  const [f, ms, game, busy, gpu] = ["frame", "ms", "game_ms", "render_busy_ms", "gpu_ms"].map(at);
  return rows.map((row) => {
    const v = row.split(",").map(Number);
    return { frame: v[f], ms: v[ms], game: v[game], busy: v[busy], gpu: v[gpu] };
  });
}

/// The value a share `q` of them are at or under (nearest rank).
export function quantile(values: number[], q: number): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil(q * sorted.length) - 1)];
}

export const median = (values: number[]) => quantile(values, 0.5);

export type Run = {
  name: string;
  median: number;
  p99: number;
  worst: number;
  gameMedian: number;
  /// The game's p99 over the frames that are not sky repaints.
  gameP99: number;
  /// Frames whose game is over REPAINT_MS: the sky's repaints.
  repaints: number;
  /// Frames where the game took longer than the render world's work.
  gameBound: Frame[];
  /// Frames over LONG_MS, and those whose longest part is the game's.
  long: Frame[];
  longInGame: Frame[];
};

export function run(name: string, frames: Frame[]): Run {
  const ms = frames.map((f) => f.ms);
  const game = frames.filter((f) => f.game <= REPAINT_MS).map((f) => f.game);
  const long = frames.filter((f) => f.ms > LONG_MS);
  return {
    name,
    median: median(ms),
    p99: quantile(ms, 0.99),
    worst: Math.max(...ms),
    gameMedian: median(game),
    gameP99: quantile(game, 0.99),
    repaints: frames.length - game.length,
    gameBound: frames.filter((f) => f.game > f.busy),
    long,
    longInGame: long.filter((f) => f.game >= Math.max(f.busy, f.gpu)),
  };
}

/// New's long frames no old run has one near (LONG_NEAR_FRAMES).
export function unmatchedLong(old: Run[], now: Run[]): { run: string; frame: Frame }[] {
  const near = (f: Frame) =>
    old.some((r) => r.long.some((o) => Math.abs(o.frame - f.frame) <= LONG_NEAR_FRAMES));
  return now.flatMap((r) => r.long.filter((f) => !near(f)).map((frame) => ({ run: r.name, frame })));
}

export type Bar = { name: string; pass: boolean; old: string; new: string; why: string };

export function bars(old: Run[], now: Run[]): Bar[] {
  const fmt = (n: number) => n.toFixed(2);
  const range = (values: number[], f = fmt) => `${f(Math.min(...values))}-${f(Math.max(...values))}`;
  const oldGame = old.map((r) => r.gameP99);
  const newGame = median(now.map((r) => r.gameP99));
  const oldPaints = old.map((r) => r.repaints);
  const newPaints = median(now.map((r) => r.repaints));
  const count = (runs: Run[], key: "long" | "longInGame") =>
    runs.reduce((n, r) => n + r[key].length, 0);
  const unmatched = unmatchedLong(old, now);
  const oldP99 = old.map((r) => r.p99);
  const newP99 = median(now.map((r) => r.p99));
  return [
    {
      name: "game p99",
      pass: newGame <= Math.max(...oldGame),
      old: range(oldGame),
      new: fmt(newGame),
      why: `median of runs' p99 game_ms, repaints (game > ${REPAINT_MS}) apart, within old's range`,
    },
    {
      name: "repaints",
      pass: newPaints >= Math.min(...oldPaints) - 1 && newPaints <= Math.max(...oldPaints) + 1,
      old: range(oldPaints, String),
      new: String(newPaints),
      why: "frames with game > 10 a run, new's median within old's range ± 1 (same path)",
    },
    {
      name: `>${LONG_MS} ms in game`,
      pass: count(now, "longInGame") === 0,
      old: `${count(old, "longInGame")} of ${count(old, "long")}`,
      new: `${count(now, "longInGame")} of ${count(now, "long")}`,
      why: "frames over the bar whose longest part is the game's, new none",
    },
    {
      name: `new >${LONG_MS} ms`,
      pass: unmatched.length === 0,
      old: "-",
      new: `${unmatched.length} of ${count(now, "long")}`,
      why: `new long frames with none in old within ${LONG_NEAR_FRAMES} frames, none`,
    },
    {
      name: "frame p99",
      pass: newP99 <= Math.max(...oldP99),
      old: range(oldP99),
      new: fmt(newP99),
      why: "median of new's runs' p99 within old's run-to-run range (or under it)",
    },
  ];
}

function main(args: string[]) {
  const sides: Record<string, string[]> = { "--old": [], "--new": [] };
  let side: string | undefined;
  for (const a of args) {
    if (a in sides) side = a;
    else if (side) sides[side].push(a);
  }
  if (!sides["--old"].length || !sides["--new"].length) {
    console.error("usage: node tools/sandbox/frametimes.ts --old a.csv … --new b.csv …");
    process.exit(2);
  }
  const load = (files: string[]) =>
    files.map((f) => run(basename(f, ".csv"), parse(readFileSync(f, "utf8"))));
  const old = load(sides["--old"]);
  const now = load(sides["--new"]);
  const f = (n: number) => n.toFixed(2).padStart(6);
  console.log(`run              median    p99  worst  game50 game99  paints  >25ms  game-bound  (game99 without game > ${REPAINT_MS})`);
  for (const r of [...old, ...now]) {
    console.log(
      `${r.name.padEnd(15)} ${f(r.median)} ${f(r.p99)} ${f(r.worst)} ${f(r.gameMedian)} ${f(r.gameP99)} ${String(r.repaints).padStart(7)} ${String(r.long.length).padStart(6)} ${String(r.gameBound.length).padStart(11)}`,
    );
  }
  const unmatched = new Set(unmatchedLong(old, now).map((u) => u.frame));
  for (const r of [...old, ...now]) {
    for (const fr of r.long.slice(0, 8)) {
      const mark = unmatched.has(fr) ? " (no old run near)" : "";
      console.log(`  ${r.name} frame ${fr.frame}: ${fr.ms.toFixed(1)} ms, game ${fr.game.toFixed(1)}, render ${fr.busy.toFixed(1)}, gpu ${fr.gpu.toFixed(1)}${mark}`);
    }
  }
  const all = bars(old, now);
  for (const b of all) {
    console.log(`${b.pass ? "✓" : "✗"} ${b.name.padEnd(14)} old ${b.old.padEnd(12)} new ${b.new.padEnd(10)} ${b.why}`);
  }
  const passed = all.every((b) => b.pass);
  console.log(passed ? "PASS" : "FAIL");
  process.exit(passed ? 0 : 1);
}

if (import.meta.main) main(process.argv.slice(2));
