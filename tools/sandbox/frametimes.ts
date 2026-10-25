// Two builds' frame times against each other (D185, D388): the CSVs a benchmark run writes
// with `--frame-times`, a run a file, old's and new's, judged on four bars meant for a change
// to the game's own work (the main world) on a frame the render world bounds, where a
// frame's whole p99 is mostly the view and the GPU, and so noise between two builds:
//   1. game:     the median of the runs' p99 of game_ms, new no more than old's + 0.1 ms;
//   2. bound:    no new frame where game_ms is more than render_busy_ms (the game the long pole);
//   3. long:     every new frame over 25 ms is long in the render world or the GPU, not the game;
//   4. frame:    the median of new's runs' frame p99 no more than the slowest of old's runs.
// Usage: node tools/sandbox/frametimes.ts --old a.csv b.csv … --new c.csv d.csv …
// Leave a cold first run (shaders compiling, the pipeline cache empty) out of the files.
// Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";
import { basename } from "node:path";

export type Frame = { frame: number; ms: number; game: number; busy: number; gpu: number };

/// The bars' margins.
export const GAME_MARGIN_MS = 0.1;
export const LONG_MS = 25;

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
  gameP99: number;
  /// Frames where the game took longer than the render world's work.
  gameBound: Frame[];
  /// Frames over LONG_MS whose longest part is the game's.
  longInGame: Frame[];
  long: number;
};

export function run(name: string, frames: Frame[]): Run {
  const ms = frames.map((f) => f.ms);
  const game = frames.map((f) => f.game);
  const long = frames.filter((f) => f.ms > LONG_MS);
  return {
    name,
    median: median(ms),
    p99: quantile(ms, 0.99),
    worst: Math.max(...ms),
    gameMedian: median(game),
    gameP99: quantile(game, 0.99),
    gameBound: frames.filter((f) => f.game > f.busy),
    longInGame: long.filter((f) => f.game >= Math.max(f.busy, f.gpu)),
    long: long.length,
  };
}

export type Bar = { name: string; pass: boolean; old: string; new: string; why: string };

export function bars(old: Run[], now: Run[]): Bar[] {
  const fmt = (n: number) => n.toFixed(2);
  const oldGame = median(old.map((r) => r.gameP99));
  const newGame = median(now.map((r) => r.gameP99));
  const count = (runs: Run[], key: "gameBound" | "longInGame") =>
    runs.reduce((n, r) => n + r[key].length, 0);
  const oldP99 = old.map((r) => r.p99);
  const newP99 = median(now.map((r) => r.p99));
  const oldRange = `${fmt(Math.min(...oldP99))}-${fmt(Math.max(...oldP99))}`;
  return [
    {
      name: "game p99",
      pass: newGame <= oldGame + GAME_MARGIN_MS,
      old: fmt(oldGame),
      new: fmt(newGame),
      why: `median of runs' p99 game_ms, new <= old + ${GAME_MARGIN_MS}`,
    },
    {
      name: "game-bound",
      pass: count(now, "gameBound") === 0,
      old: `${count(old, "gameBound")}`,
      new: `${count(now, "gameBound")}`,
      why: "frames with game_ms > render_busy_ms, new none",
    },
    {
      name: `>${LONG_MS} ms in game`,
      pass: count(now, "longInGame") === 0,
      old: `${count(old, "longInGame")} of ${old.reduce((n, r) => n + r.long, 0)}`,
      new: `${count(now, "longInGame")} of ${now.reduce((n, r) => n + r.long, 0)}`,
      why: "frames over the bar whose longest part is the game's, new none",
    },
    {
      name: "frame p99",
      pass: newP99 <= Math.max(...oldP99),
      old: oldRange,
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
  console.log("run              median    p99  worst  game50 game99  >25ms  game-bound");
  for (const r of [...old, ...now]) {
    console.log(
      `${r.name.padEnd(15)} ${f(r.median)} ${f(r.p99)} ${f(r.worst)} ${f(r.gameMedian)} ${f(r.gameP99)} ${String(r.long).padStart(6)} ${String(r.gameBound.length).padStart(11)}`,
    );
  }
  for (const r of now) {
    for (const fr of r.longInGame.slice(0, 5)) {
      console.log(`  ${r.name} frame ${fr.frame}: ${fr.ms.toFixed(1)} ms, game ${fr.game.toFixed(1)}, render ${fr.busy.toFixed(1)}, gpu ${fr.gpu.toFixed(1)}`);
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
