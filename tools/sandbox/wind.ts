#!/usr/bin/env node
// The still-camera wind measure (D597): how a forest's crowns move in the wind, from a film
// shot from a camera that stands still, held to how Enshrouded's do. A crown that slides as
// one block moves every patch of the picture alike; real crowns and Enshrouded's stir spray
// by spray, each patch on its own (research/2026-11-02-why-our-trees-read-fake.md).
//
//   node tools/sandbox/wind.ts [--view crowns|all] [--time 15] [--date 08-25] [--out shots/wind] [-- <sandbox options>]
//   node tools/sandbox/wind.ts <clip.mp4…>      measures clips already filmed (a reference's)
//
// Each view is filmed by film.ts, 12 s from a still camera in the lush forest, then measured
// by block matching over 32-pixel tiles of the picture's upper part (the crowns) at 640x360:
// each tile's shift from one frame to the next tenth of a second, found to a tenth of a pixel
// (the least sum of differences over +-3 px, a parabola through it), tiles without texture
// left out. It prints, a line a view:
// - amplitude: the median tile's RMS shift about its own mean, pixels a tenth of a second;
// - shared: the share of that motion one shift common to the whole view explains (1: the view
//   slides as one block; 0: every tile moves on its own);
// - neighbour r: how alike side-by-side tiles' motion is (their correlation), and 4 apart;
// - second against step: how much further a tile moves over a second than over a step (a
//   sway near 10, a buzz under 2).
// It fails, exit 1, when shared or neighbour r is above Enshrouded's (SHARED, NEIGHBOUR), when
// the motion buzzes (under SLOW), or when nothing moved. Needs ffmpeg. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { ROOT } from "./client.ts";
import { place } from "./places.ts";

/// Held to Enshrouded's Springlands on a still camera: shared 0.11 to 0.24, neighbour r 0.36
/// to 0.43 (the research note's table; the clips in ~/.cache/terra-review/refs/enshrouded/yt/).
export const SHARED = 0.3;
export const NEIGHBOUR = 0.5;
/// And its motion over a second at least this many times a step's: Enshrouded's 7.2 to 7.3
/// and main's 7.9 sway; D597's first experiment, at 2.0 to 3.1, buzzed.
export const SLOW = 5;

/// The size the film is measured at, the tile and how far a tile is searched.
export const WIDTH = 640;
export const HEIGHT = 360;
const TILE = 32;
const REACH = 3;
/// A tile moving less than this, pixels RMS, counts as still.
const STILL = 0.05;

export type Motion = {
  steps: number;
  textured: number;
  moving: number;
  /// Median and 90th percentile of the tiles' RMS shift, pixels a step.
  amplitude: number;
  p90: number;
  shared: number;
  neighbour: number;
  apart4: number;
  /// The median moving tile's RMS shift over a second against over a step: a slow sway is
  /// near the second's steps (10), random jitter their root (3.2), a buzz faster than half a
  /// second under 2 (the wind round's judge: a flap that reads as an electric fan).
  slow: number;
};

/// Steps in the `slow` measure's window: a second at a tenth of a second a step.
const SLOW_STEPS = 10;

const mean = (v: number[]) => v.reduce((p, q) => p + q, 0) / Math.max(1, v.length);
const corr = (p: number[], q: number[]) => {
  const mp = mean(p);
  const mq = mean(q);
  let c = 0;
  let a = 0;
  let b = 0;
  for (let i = 0; i < p.length; i++) {
    c += (p[i] - mp) * (q[i] - mq);
    a += (p[i] - mp) ** 2;
    b += (q[i] - mq) ** 2;
  }
  return a > 0 && b > 0 ? c / Math.sqrt(a * b) : Number.NaN;
};
/// The sub-pixel offset of a minimum from the parabola through it and its two neighbours.
const vertex = (m: number, c: number, p: number) => {
  const d = m - 2 * c + p;
  return d > 0 ? (0.5 * (m - p)) / d : 0;
};

/// How the tiles of grey frames (WIDTH x HEIGHT, one byte a pixel) move, over the picture's
/// upper `top` share, comparing frames `step` apart.
export function measure(frames: Uint8Array[], top = 0.6, step = 3): Motion {
  const W = WIDTH;
  const tx = Math.floor((W - 2 * REACH) / TILE);
  const ty = Math.floor((HEIGHT * top - 2 * REACH) / TILE);
  const tiles: { x: number; y: number }[] = [];
  for (let j = 0; j < ty; j++) for (let i = 0; i < tx; i++) tiles.push({ x: REACH + i * TILE, y: REACH + j * TILE });
  // A tile is textured when the first frame's values spread over it.
  const textured = tiles.map(({ x, y }) => {
    let s = 0;
    let s2 = 0;
    for (let v = 0; v < TILE; v++) {
      for (let u = 0; u < TILE; u++) {
        const p = frames[0][(y + v) * W + x + u];
        s += p;
        s2 += p * p;
      }
    }
    const m = s / (TILE * TILE);
    return Math.sqrt(Math.max(0, s2 / (TILE * TILE) - m * m)) > 6;
  });
  const sad = (a: Uint8Array, b: Uint8Array, x: number, y: number, dx: number, dy: number) => {
    let s = 0;
    for (let v = 0; v < TILE; v += 2) for (let u = 0; u < TILE; u += 2) s += Math.abs(a[(y + v) * W + x + u] - b[(y + v + dy) * W + x + u + dx]);
    return s;
  };
  const dxs: number[][] = tiles.map(() => []);
  const dys: number[][] = tiles.map(() => []);
  for (let f = 0; f + step < frames.length; f += step) {
    const a = frames[f];
    const b = frames[f + step];
    tiles.forEach(({ x, y }, k) => {
      if (!textured[k]) return;
      let best = Number.POSITIVE_INFINITY;
      let bx = 0;
      let by = 0;
      const grid: number[][] = [];
      for (let dy = -REACH; dy <= REACH; dy++) {
        const row: number[] = [];
        for (let dx = -REACH; dx <= REACH; dx++) {
          const s = sad(a, b, x, y, dx, dy);
          row.push(s);
          if (s < best) {
            best = s;
            bx = dx;
            by = dy;
          }
        }
        grid.push(row);
      }
      const gx = bx + REACH;
      const gy = by + REACH;
      const fx = gx > 0 && gx < 2 * REACH ? vertex(grid[gy][gx - 1], grid[gy][gx], grid[gy][gx + 1]) : 0;
      const fy = gy > 0 && gy < 2 * REACH ? vertex(grid[gy - 1][gx], grid[gy][gx], grid[gy + 1][gx]) : 0;
      dxs[k].push(bx + fx);
      dys[k].push(by + fy);
    });
  }
  const live = tiles.map((_, k) => k).filter((k) => textured[k]);
  const steps = live.length ? dxs[live[0]].length : 0;
  const rms = live.map((k) => {
    const mx = mean(dxs[k]);
    const my = mean(dys[k]);
    return Math.sqrt(mean(dxs[k].map((v, t) => (v - mx) ** 2 + (dys[k][t] - my) ** 2)));
  });
  const moving = live.filter((_, i) => rms[i] > STILL);
  // Shared: the variance the view's mean shift explains, against the tiles' whole variance.
  const centre = new Map(moving.map((k) => [k, [mean(dxs[k]), mean(dys[k])]]));
  let common = 0;
  let total = 0;
  for (let t = 0; t < steps; t++) {
    let mx = 0;
    let my = 0;
    for (const k of moving) {
      const [cx, cy] = centre.get(k) as number[];
      const ex = dxs[k][t] - cx;
      const ey = dys[k][t] - cy;
      mx += ex;
      my += ey;
      total += ex * ex + ey * ey;
    }
    mx /= Math.max(1, moving.length);
    my /= Math.max(1, moving.length);
    common += moving.length * (mx * mx + my * my);
  }
  const set = new Set(moving);
  const at = (gap: number) => {
    const r: number[] = [];
    for (const k of moving) {
      if ((k % tx) + gap >= tx || !set.has(k + gap)) continue;
      const c = corr(dxs[k], dxs[k + gap]);
      if (!Number.isNaN(c)) r.push(c);
    }
    return r.length ? mean(r) : Number.NaN;
  };
  const sorted = [...rms].sort((p, q) => p - q);
  // Each moving tile's shift summed over a second's window, about its mean, against a step's.
  const slows = moving
    .map((k) => {
      const mx = mean(dxs[k]);
      const my = mean(dys[k]);
      let quick = 0;
      for (let t = 0; t < steps; t++) quick += (dxs[k][t] - mx) ** 2 + (dys[k][t] - my) ** 2;
      quick /= Math.max(1, steps);
      let long = 0;
      let windows = 0;
      for (let t = 0; t + SLOW_STEPS <= steps; t++) {
        let sx = 0;
        let sy = 0;
        for (let u = t; u < t + SLOW_STEPS; u++) {
          sx += dxs[k][u] - mx;
          sy += dys[k][u] - my;
        }
        long += sx * sx + sy * sy;
        windows++;
      }
      return quick > 0 && windows > 0 ? Math.sqrt(long / windows / quick) : Number.NaN;
    })
    .filter((v) => !Number.isNaN(v))
    .sort((p, q) => p - q);
  return {
    slow: slows.length ? slows[Math.floor(slows.length / 2)] : Number.NaN,
    steps,
    textured: live.length,
    moving: moving.length,
    amplitude: sorted.length ? sorted[Math.floor(sorted.length / 2)] : 0,
    p90: sorted.length ? sorted[Math.floor(sorted.length * 0.9)] : 0,
    shared: total > 0 ? common / total : Number.NaN,
    neighbour: at(1),
    apart4: at(4),
  };
}

/// What fails in a measured view, each with what it means; none when it holds.
export function judge(m: Motion): string[] {
  if (m.moving < 4) return [`nothing moved: ${m.moving} of ${m.textured} textured tiles shifted over ${STILL} px, so there is no wind to judge`];
  const failed: string[] = [];
  if (!(m.shared <= SHARED)) failed.push(`shared ${m.shared.toFixed(2)} over ${SHARED}: the crowns slide as one block`);
  if (!(m.neighbour <= NEIGHBOUR)) failed.push(`neighbour r ${m.neighbour.toFixed(2)} over ${NEIGHBOUR}: neighbouring patches move alike`);
  if (!(m.slow >= SLOW)) failed.push(`second against step ${m.slow.toFixed(1)} under ${SLOW}: the crowns buzz rather than sway; slow the fastest level (the cards' flap)`);
  return failed;
}

/// A clip's frames as grey at the measured size.
export function decode(clip: string): Uint8Array[] {
  const raw = spawnSync("ffmpeg", ["-loglevel", "error", "-i", clip, "-vf", `scale=${WIDTH}:${HEIGHT},format=gray`, "-f", "rawvideo", "-"], { maxBuffer: 1 << 30 });
  if (raw.status !== 0) throw new Error(`wind: ffmpeg could not read ${clip}: ${raw.stderr}`);
  const frames: Uint8Array[] = [];
  const size = WIDTH * HEIGHT;
  for (let o = 0; o + size <= raw.stdout.length; o += size) frames.push(raw.stdout.subarray(o, o + size));
  return frames;
}

const line = (name: string, m: Motion) =>
  `${name}: amplitude ${m.amplitude.toFixed(2)} px (p90 ${m.p90.toFixed(2)}), shared ${m.shared.toFixed(2)} (at most ${SHARED}), ` +
  `neighbour r ${m.neighbour.toFixed(2)} (at most ${NEIGHBOUR}), 4 apart ${m.apart4.toFixed(2)}, second against step ${m.slow.toFixed(1)} (at least ${SLOW}); ${m.moving} of ${m.textured} tiles moving`;

/// The views a still camera films: up under a crown by the spruce grove, where the research
/// note measured 0.85 shared and r 0.91 on main (2026-11-02, its film `wind-oak.mp4`). The
/// view toward the sun under the pines (-56,1.7,-43 to -80,9,-25) shows only silhouettes
/// against the sky and moved nothing measurable, so it is not held.
const VIEWS: Record<string, string> = {
  crowns: "-21.9,1.7,-96,-21.9,12,-114",
};

if (import.meta.main) {
  const args = process.argv.slice(2);
  const dash = args.indexOf("--");
  const own = dash < 0 ? args : args.slice(0, dash);
  const extra = dash < 0 ? [] : args.slice(dash + 1);
  if (own.includes("--help") || own.includes("-h")) {
    console.log("wind.ts [--view crowns|all] [--time 15] [--date 08-25] [--out shots/wind] [-- <sandbox options>]\nwind.ts <clip.mp4…>   measure clips already filmed");
    process.exit(0);
  }
  const arg = (name: string, fallback: string) => {
    const at = own.indexOf(name);
    return at >= 0 ? own[at + 1] : fallback;
  };
  const clips = own.filter((a) => a.endsWith(".mp4"));
  let failed = 0;
  if (clips.length) {
    for (const clip of clips) {
      const m = measure(decode(clip));
      const why = judge(m);
      console.log(`${why.length ? "✗" : "✓"} ${line(clip, m)}`);
      for (const w of why) console.log(`  ${w}`);
      if (why.length) failed++;
    }
    process.exit(failed ? 1 : 0);
  }
  const which = arg("--view", "crowns");
  const views = which === "all" ? Object.keys(VIEWS) : which.split(",");
  for (const v of views) {
    if (!VIEWS[v]) {
      console.error(`wind: no view "${v}"; the views are ${Object.keys(VIEWS).join(", ")} (or all)`);
      process.exit(2);
    }
  }
  const out = resolve(ROOT, arg("--out", "shots/wind"));
  mkdirSync(out, { recursive: true });
  const date = arg("--date", "");
  const lush = place("lush").args;
  for (const v of views) {
    const film = resolve(out, `${v}.mp4`);
    const sandbox = [...lush, "--time", arg("--time", "15"), ...(date ? ["--date", date] : []), "--vantage", VIEWS[v], ...extra];
    const run = spawnSync(process.execPath, [resolve(import.meta.dirname, "film.ts"), "--frames", "300-1020", "--out", film, "--", ...sandbox], { cwd: ROOT, encoding: "utf8" });
    if (run.status !== 0) {
      console.error(`${run.stdout}${run.stderr}`.trim().split("\n").slice(-12).join("\n"));
      console.error(`wind: the film of ${v} failed; its log is ${film}.log`);
      process.exit(1);
    }
    const m = measure(decode(film));
    const why = judge(m);
    console.log(`${why.length ? "✗" : "✓"} wind ${line(v, m)}, ${film}`);
    for (const w of why) console.log(`  ${w}`);
    if (why.length) failed++;
  }
  if (failed) {
    console.log("wind: crowns move as one block; D597 (wind in levels) is the change that answers it, held to Enshrouded's numbers in research/2026-11-02-why-our-trees-read-fake.md");
    process.exit(1);
  }
}
