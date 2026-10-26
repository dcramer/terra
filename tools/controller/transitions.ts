// Starts, stops and turns in place on the controller's filmed course (yard.ts; docs/controller.md
// criterion 21d): what a production team checks of a locomotion set's transitions, measured on a
// run's walk record, not by eye. Shipped games play a start, a stop and a turn in place as clips
// (Lyra's distance-matched starts and stops and turn-in-place clips, ALS's), so the feet step as
// the clip has them; a foot the clip leaves behind is stepped by D455's rule (`stepping`), the
// fallback, which reads as a shuffle. A line a check (D388). Node 22.18 or later; no dependencies.

import type { Frame } from "../sandbox/duel/checks.ts";
import type { Check, Walk } from "./yard-checks.ts";

const FPS = 60;
/// The body stands while it goes slower than this, m/s.
export const STOPPED_MPS = 0.05;
/// A ball moving more than this in a frame is moving (a locked or resting foot moves none), m.
export const MOVED_M = 0.005;
/// stop.settle's bar: the feet at rest within so long of the body (a stop clip's last plant
/// lands as its distance curve reaches zero; a blend to the idle leaves a foot to the settle
/// step a quarter second later), s.
export const SETTLE_S = 0.3;
/// A start is so long from the body's first move off standing; a turn in place's window runs
/// on so long past its last change of facing, s.
export const START_S = 1.0;
export const TURN_TAIL_S = 0.5;
/// A change of facing in a frame past this is a turn, deg.
export const TURNING_DEG = 0.5;

type Seen = { frame: number; speed: number; yaw: number; balls: number[][]; kept: string[] };

function seen(frames: Frame[]): Seen[] {
  return frames.flatMap((frame) => {
    const w = frame.fighters.find((each) => each.who === "you") as (Frame["fighters"][number] & { speed?: number; kept?: string[] }) | undefined;
    if (!w || !w.kept) return [];
    return [{ frame: frame.frame, speed: w.speed ?? 0, yaw: (Math.atan2(w.facing[0], w.facing[2]) * 180) / Math.PI, balls: w.balls.map((b) => b?.at ?? [NaN, NaN, NaN]), kept: w.kept }];
  });
}

const turned = (a: number, b: number) => Math.abs(((b - a + 540) % 360) - 180);

/// The rule's steps (a foot going into `stepping`) in frames `from` to `to` (indices of `s`).
function ruleSteps(s: Seen[], from: number, to: number): number {
  let n = 0;
  for (let i = Math.max(1, from); i <= Math.min(to, s.length - 1); i++)
    for (let foot = 0; foot < 2; foot++) if (s[i].kept[foot] === "stepping" && s[i - 1].kept[foot] !== "stepping") n++;
  return n;
}

/// Each stop: the frame the body comes to stand after going faster than a walk, and the frame
/// it next moves or turns (or the record's end).
export function stops(s: Seen[]): { at: number; until: number }[] {
  const out: { at: number; until: number }[] = [];
  let moving = false;
  for (let i = 0; i < s.length; i++) {
    if (s[i].speed > 1.0) moving = true;
    if (moving && s[i].speed < STOPPED_MPS) {
      let j = i + 1;
      while (j < s.length && s[j].speed < STOPPED_MPS && turned(s[i].yaw, s[j].yaw) < 1) j++;
      out.push({ at: i, until: j - 1 });
      moving = false;
      i = j - 1;
    }
  }
  return out;
}

/// stop.settle: for each stop, how long after the body stands its feet still move (the last
/// frame a ball moves more than MOVED_M), s; the row is the worst stop's.
export function settle(walks: Walk[]): Check {
  const said: string[] = [];
  let worst = 0;
  for (const walk of walks) {
    const s = seen(walk.frames);
    for (const stop of stops(s)) {
      let last = stop.at;
      for (let i = stop.at + 1; i <= stop.until; i++) {
        const moved = s[i].balls.some((b, foot) => Math.hypot(...b.map((v, k) => v - s[i - 1].balls[foot][k])) > MOVED_M);
        if (moved) last = i;
      }
      const took = (s[last].frame - s[stop.at].frame) / FPS;
      const steps = ruleSteps(s, stop.at - Math.round(0.3 * FPS), stop.until);
      said.push(`${walk.name} frame ${s[stop.at].frame}: ${took.toFixed(2)} s, ${steps} rule steps`);
      worst = Math.max(worst, took);
    }
  }
  const ok = worst <= SETTLE_S && said.length > 0;
  const line = `${ok ? "" : "✗ "}feet at rest ${worst.toFixed(2)} s after the body stops at worst (bar ${SETTLE_S} s): ${said.join("; ") || "no stop in the records"}`;
  return { name: "stop settle", ok, line, value: said.length ? worst : NaN, faults: ok ? 0 : 1 };
}

/// How far the body turns in frames `from` to `to` (indices of `s`), deg, summed a frame at a time.
function turnedOver(s: Seen[], from: number, to: number): number {
  let deg = 0;
  for (let i = Math.max(1, from); i <= Math.min(to, s.length - 1); i++) deg += turned(s[i - 1].yaw, s[i].yaw);
  return deg;
}

/// start.steps: the rule's steps in each straight start (the first START_S after the body first
/// moves off standing, on to a jog, turning less than TURN_DEG in its first half second: a start
/// that turns is a turn's), summed over the walks: a start clip steps both feet itself.
export function starts(walks: Walk[]): Check {
  const said: string[] = [];
  let total = 0;
  for (const walk of walks) {
    const s = seen(walk.frames);
    for (let i = 1; i < s.length; i++) {
      if (!(s[i].speed > STOPPED_MPS && s[i - 1].speed <= STOPPED_MPS)) continue;
      const before = s.slice(Math.max(0, i - 15), i).every((each) => each.speed <= STOPPED_MPS);
      const goes = s.slice(i, i + Math.round(START_S * FPS)).some((each) => each.speed > 3.0);
      if (!before || !goes || turnedOver(s, i, i + 30) >= TURN_DEG) continue;
      const steps = ruleSteps(s, i, i + Math.round(START_S * FPS));
      said.push(`${walk.name} frame ${s[i].frame}: ${steps}`);
      total += steps;
    }
  }
  const ok = total === 0 && said.length > 0;
  return { name: "start steps", ok, line: `${ok ? "" : "✗ "}${total} feet stepped by the rule in straight starts (bar 0): ${said.join("; ") || "no start in the records"}`, value: said.length ? total : NaN, faults: ok ? 0 : 1 };
}

/// A turn is TURN_DEG or more of facing, its frames turning TURNING_DEG or more each, gaps under
/// TURN_TAIL_S joined, its window running TURN_TAIL_S on.
export const TURN_DEG = 45;

/// turn.steps: the rule's steps in each turn from standing or slow (the body under a jog's
/// speed at some frame of it: a turn in place, a start or stop that turns, a pivot), summed over
/// the walks: a turn-in-place or pivot clip steps the feet itself (Lyra's, ALS's).
export function turns(walks: Walk[]): Check {
  const said: string[] = [];
  let total = 0;
  const tail = Math.round(TURN_TAIL_S * FPS);
  for (const walk of walks) {
    const s = seen(walk.frames);
    let i = 1;
    while (i < s.length) {
      if (turned(s[i - 1].yaw, s[i].yaw) <= TURNING_DEG) {
        i++;
        continue;
      }
      const from = i;
      let last = i;
      while (i < s.length && i - last < tail) {
        if (turned(s[i - 1].yaw, s[i].yaw) > TURNING_DEG) last = i;
        i++;
      }
      const deg = turnedOver(s, from, last);
      const slow = s.slice(from, last + 1).some((each) => each.speed < 1.0);
      if (deg < TURN_DEG || !slow) continue;
      const steps = ruleSteps(s, from, Math.min(s.length - 1, last + tail));
      said.push(`${walk.name} frame ${s[from].frame} ${deg.toFixed(0)} deg: ${steps}`);
      total += steps;
    }
  }
  const ok = total === 0 && said.length > 0;
  return { name: "turn steps", ok, line: `${ok ? "" : "✗ "}${total} feet stepped by the rule in turns from standing or slow (bar 0): ${said.join("; ") || "no turn in the records"}`, value: said.length ? total : NaN, faults: ok ? 0 : 1 };
}

/// stop.steps: the rule's steps in each stop (from 0.3 s before the body stands until it next
/// moves or turns), summed over the walks: a stop clip braking on the planted foot steps the
/// other itself (Paragon's, ALS's).
export function stopSteps(walks: Walk[]): Check {
  const said: string[] = [];
  let total = 0;
  for (const walk of walks) {
    const s = seen(walk.frames);
    for (const stop of stops(s)) {
      const steps = ruleSteps(s, stop.at - Math.round(0.3 * FPS), stop.until);
      if (steps) said.push(`${walk.name} frame ${s[stop.at].frame}: ${steps}`);
      total += steps;
    }
  }
  const ok = total === 0;
  const counted = walks.some((walk) => stops(seen(walk.frames)).length);
  return { name: "stop steps", ok, line: `${ok ? "" : "✗ "}${total} feet stepped by the rule in stops (bar 0)${said.length ? `: ${said.join("; ")}` : ""}`, value: counted ? total : NaN, faults: ok ? 0 : 1 };
}

export const transitions = (walks: Walk[]): Check[] => [settle(walks), stopSteps(walks), starts(walks), turns(walks)];
