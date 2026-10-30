#!/usr/bin/env node
// Measures a blow's phases from its clip (docs/movement.md, "Moves as data"): the held
// weapon's tip followed through the skeleton frame by frame, in the body's own space (x to its
// left, y up, z ahead, from the hips' place at the start), and when it sweeps forward of the
// body fast enough to strike: the active frames a move's data says
// (assets/data/moves/*.ron). Prints, a line a clip, the span its tip is more than --ahead
// metres in front of the hips and moving faster than --speed m/s, its fastest moment in it and
// where it is then; --trace prints every frame. `measure` is the same for a caller (clip.ts).
// A strike is a sweep: its speed is the tip's over a few frames (PACE_FRAMES either side), and
// the sweep lasts STRIKE_S at least, so a tip that jumps for a frame or two (a pop in a made take)
// is no strike but a jump, listed apart (`jumps`). The sweep is timed whole, not only its part
// ahead: a fast blow's tip (40-80 m/s, a bought take's) passes the front in 2-4 frames of a sweep
// that lasts far longer, and is a strike over those frames.
//
//   node tools/characters/phases.ts assets/models/character/fight.glb Fight_Slash Fight_Backhand
//   node tools/characters/phases.ts <file.glb> <clip>… [--socket socket.hand.R] [--tip 0.83] [--speed 4] [--ahead 0.5] [--trace]
//
// Node 22.18 or later; no dependencies.

import { Glb, posed, rot, type V3 } from "./glb.ts";

/** A frame's tip; `speed` its move from the frame before, `pace` its speed over PACE_FRAMES
 * either side, m/s. */
export type Frame = { t: number; tip: V3; speed: number; pace: number };
/** A strike's speed is the tip's over this many frames either side (at 60 a second). */
export const PACE_FRAMES = 2;
/** A strike lasts this long at least, seconds: a sweep, not a frame's jump. */
export const STRIKE_S = 0.1;
/** A frame's move is a jump when it is over JUMP_MPS and JUMP_TIMES the tip's pace. */
export const JUMP_MPS = 10;
export const JUMP_TIMES = 2;
export type Phases = {
  clip: string;
  /** The clip's length, seconds. */
  end: number;
  frames: Frame[];
  /** The striking span through its fastest moment, seconds; none if it never strikes. */
  active?: [number, number];
  fastest?: Frame;
  /** Frames whose tip jumps: much faster than the frames around it. */
  jumps: Frame[];
};
export type Options = { socket?: string; tip?: number; speed?: number; ahead?: number };

/** A clip's tip, frame by frame, and its striking span; undefined if `glb` lacks the clip. */
export function measure(glb: Glb, name: string, o: Options = {}): Phases | undefined {
  const socket = o.socket ?? "socket.hand.R";
  const [tipM, fast, reach] = [o.tip ?? 0.83, o.speed ?? 4, o.ahead ?? 0.5];
  const sock = glb.named(socket);
  const hips = glb.named("pelvis");
  if (sock < 0 || hips < 0) throw new Error(`no ${sock < 0 ? socket : "pelvis"} in the file`);
  const clip = glb.clip(name);
  if (!clip) return undefined;
  const end = clip.end;
  const start = posed(glb, clip, 0)[hips].t;
  const step = 1 / 60;
  const tips: { t: number; tip: V3 }[] = [];
  for (let t = 0; t <= end + 1e-6; t += step) {
    const hand = posed(glb, clip, t)[sock];
    const out = rot(hand.r, [0, tipM * hand.s[1], 0]);
    tips.push({ t, tip: out.map((x, j) => x + hand.t[j] - start[j]) as V3 });
  }
  return { clip: name, end, ...strike(paced(tips, step), { speed: fast, ahead: reach }) };
}

const dist = (a: V3, b: V3) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

/** Where a blow lands: of its active frames' tips (the body's space, +Z ahead), the one
 * furthest ahead, the point of its arc nearest what stands before it. A sweep lands there as
 * it crosses the front, a thrust at its full reach. Its fastest frame is no stand-in: a fast
 * take's tip (40-80 m/s) is fastest a frame or two past the front, 60 deg round the side. */
export function contactOf<T extends { tip: V3 }>(frames: T[]): T | undefined {
  return frames.reduce<T | undefined>((best, f) => (!best || f.tip[2] > best.tip[2] ? f : best), undefined);
}

/** Each tip's speed from the frame before, and its pace over PACE_FRAMES frames either side. */
export function paced(tips: { t: number; tip: V3 }[], step: number): Frame[] {
  return tips.map(({ t, tip }, i) => {
    const [a, b] = [Math.max(0, i - PACE_FRAMES), Math.min(tips.length - 1, i + PACE_FRAMES)];
    const pace = b > a ? dist(tips[b].tip, tips[a].tip) / ((b - a) * step) : 0;
    return { t, tip, speed: i > 0 ? dist(tip, tips[i - 1].tip) / step : 0, pace };
  });
}

/** The striking span of `frames`: within the sweeps at pace lasting STRIKE_S at least, the runs
 * of frames whose tip is ahead, the one through the fastest moment; and the frames whose tip
 * jumps. */
export function strike(frames: Frame[], o: { speed: number; ahead: number }): Omit<Phases, "clip" | "end"> {
  const jumps = frames.filter((f) => f.speed > JUMP_MPS && f.speed > JUMP_TIMES * f.pace);
  const spans = (from: number, to: number, keep: (f: Frame) => boolean) => {
    const out: [number, number][] = [];
    for (let i = from; i <= to; i++) {
      if (!keep(frames[i])) continue;
      let j = i;
      while (j + 1 <= to && keep(frames[j + 1])) j++;
      out.push([i, j]);
      i = j;
    }
    return out;
  };
  const sweeps = spans(0, frames.length - 1, (f) => f.pace > o.speed).filter(([i, j]) => frames[j].t - frames[i].t >= STRIKE_S - 1e-6);
  const runs = sweeps.flatMap(([i, j]) => spans(i, j, (f) => f.tip[2] > o.ahead));
  if (runs.length === 0) return { frames, jumps };
  const top = ([a, b]: [number, number]) => frames.slice(a, b + 1).reduce((x, y) => (y.pace > x.pace ? y : x));
  const best = runs.reduce((a, b) => (top(b).pace > top(a).pace ? b : a));
  return { frames, active: [frames[best[0]].t, frames[best[1]].t], fastest: top(best), jumps };
}

function main() {
  const args = process.argv.slice(2);
  const opt = (name: string, fallback: string) => {
    const i = args.indexOf(name);
    return i >= 0 ? args.splice(i, 2)[1] : fallback;
  };
  const trace = args.includes("--trace") ? (args.splice(args.indexOf("--trace"), 1), true) : false;
  const o = { socket: opt("--socket", "socket.hand.R"), tip: Number(opt("--tip", "0.83")), speed: Number(opt("--speed", "4")), ahead: Number(opt("--ahead", "0.5")) };
  const [file, ...clips] = args;
  if (!file || clips.length === 0) {
    console.error("usage: phases.ts <file.glb> <clip>… [--socket name] [--tip m] [--speed m/s] [--trace]");
    process.exit(1);
  }
  const glb = new Glb(file);
  const f = (v: number) => v.toFixed(2);
  for (const name of clips) {
    const p = measure(glb, name, o);
    if (!p) {
      console.log(`${name}: no such clip in ${file}`);
      continue;
    }
    if (trace) for (const fr of p.frames) console.log(`${f(fr.t)} tip ${fr.tip.map(f).join(" ")} speed ${f(fr.speed)} m/s, pace ${f(fr.pace)} m/s`);
    if (!p.active || !p.fastest) {
      console.log(`${name} (${f(p.end)} s): the tip never moves faster than ${o.speed} m/s more than ${o.ahead} m ahead`);
      continue;
    }
    const [left, up, ahead] = p.fastest.tip;
    console.log(
      `${name} (${f(p.end)} s): striking ${f(p.active[0])}-${f(p.active[1])} s; fastest ${f(p.fastest.pace)} m/s at ${f(p.fastest.t)} s, ${f(ahead)} m ahead, ${f(up)} m up, ${f(left)} m left`,
    );
  }
}

if (import.meta.main) main();
