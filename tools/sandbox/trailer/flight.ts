// A flier's course worked out as `play::fliers` flies it (its `plan` module, the same steps in
// the same order), so a scene can aim its camera where the flier will be, and a plot can show
// a course before anything is filmed (flightplot.ts). Places are in a base's own metres, y up.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { Vec3 } from "../qa/script.ts";

/// What a kind's wings let it do (`plan::Wings`), its cruising speed beside them, and how
/// fast it eases into a hover, a share a second (`fliers::SETTLE`).
export type Wings = { cruise: number; slowest: number; mostBank: number; accel: number; brake: number };
export const SETTLE = 1.6;

/// A kind's wings, read from `assets/data/fliers.ron`.
export function wings(kind = "Dragon"): Wings {
  const ron = readFileSync(join(import.meta.dirname, "../../../assets/data/fliers.ron"), "utf8");
  const at = ron.indexOf(`name: "${kind}"`);
  if (at < 0) throw new Error(`no flier ${kind} in fliers.ron`);
  const field = (name: string) => {
    const found = new RegExp(`${name}:\\s*([-\\d.]+)`).exec(ron.slice(at));
    if (!found) throw new Error(`${kind} has no ${name}`);
    return Number(found[1]);
  };
  return {
    cruise: field("cruise_mps"),
    slowest: field("hover_mps"),
    mostBank: (field("most_bank_deg") * Math.PI) / 180,
    accel: field("accel_mps2"),
    brake: field("brake_mps2"),
  };
}

const STEP_M = 0.5;
const BEND_M = 2;
const LEAD_IN_M = 16;
const LEAD_IN_SHARE = 0.4;
const EASE_SPEED_M = 8;
const EASE_SPEED_PASSES = 2;
const CLIMB_SLOWS = 0.5;
const DIVE_SPEEDS = 0.3;
const GRAVITY = 9.81;

export type Plan = { points: Vec3[]; speed: number[]; time: number[]; bend: number[] };
export type Sample = { at: Vec3; way: Vec3; speed: number; bend: number; ended: boolean };

const add = (a: Vec3, b: Vec3): Vec3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const sub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const scale = (a: Vec3, k: number): Vec3 => [a[0] * k, a[1] * k, a[2] * k];
const dot = (a: Vec3, b: Vec3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: Vec3, b: Vec3): Vec3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const len = (a: Vec3) => Math.hypot(a[0], a[1], a[2]);
const dist = (a: Vec3, b: Vec3) => len(sub(a, b));
export const mix = (a: Vec3, b: Vec3, t: number): Vec3 => add(a, scale(sub(b, a), t));
const norm = (a: Vec3, or: Vec3): Vec3 => {
  const l = len(a);
  return l > 1e-6 ? scale(a, 1 / l) : or;
};
const level = (a: Vec3): Vec3 => [a[0], 0, a[2]];

/// A clamped cubic B-spline over the control points, finely (`plan::spline`).
function spline(control: Vec3[]): Vec3[] {
  const n = control.length;
  const at = (i: number) => control[Math.min(Math.max(i, 0), n - 1)];
  const out: Vec3[] = [control[0]];
  for (let i = -1; i < n; i++) {
    const p = [at(i - 1), at(i), at(i + 1), at(i + 2)];
    const steps = Math.max(1, Math.ceil(dist(p[3], p[0]) / (STEP_M * 0.25)));
    for (let s = 1; s <= steps; s++) {
      const t = s / steps;
      const u = 1 - t;
      const b = [(u * u * u) / 6, (3 * t * t * t - 6 * t * t + 4) / 6, (-3 * t * t * t + 3 * t * t + 3 * t + 1) / 6, (t * t * t) / 6];
      out.push(add(add(scale(p[0], b[0]), scale(p[1], b[1])), add(scale(p[2], b[2]), scale(p[3], b[3]))));
    }
  }
  return out;
}

function resample(line: Vec3[]): Vec3[] {
  const out: Vec3[] = [line[0]];
  let carried = 0;
  for (let i = 1; i < line.length; i++) {
    const length = dist(line[i - 1], line[i]);
    if (length <= 0) continue;
    let along = STEP_M - carried;
    while (along <= length) {
      out.push(mix(line[i - 1], line[i], along / length));
      along += STEP_M;
    }
    carried = length - (along - STEP_M);
  }
  const last = line[line.length - 1];
  if (dist(out[out.length - 1], last) > STEP_M * 0.25) out.push(last);
  else out[out.length - 1] = last;
  return out;
}

/// Plans a flight from `from`, heading `way` at `speedNow`, through the points (`plan::plan`).
export function plan(from: Vec3, way: Vec3, speedNow: number, through: Vec3[], cruise: number, w: Wings): Plan {
  const up: Vec3 = [0, 1, 0];
  const control = [from, ...through].filter((p, i, all) => i === 0 || dist(p, all[i - 1]) >= 1e-3);
  if (control.length < 2) return { points: control, speed: [Math.min(speedNow, w.slowest)], time: [0], bend: [0] };
  const heading = norm(way, norm(sub(control[1], control[0]), [0, 0, 1]));
  const lead = Math.min(dist(control[1], from) * LEAD_IN_SHARE, LEAD_IN_M);
  control.splice(1, 0, add(from, scale(heading, lead)));
  {
    const n = control.length;
    const [last, before] = [control[n - 1], control[n - 2]];
    const levelWay = norm(level(sub(last, before)), [0, 0, 0]);
    const leadOut = Math.min(dist(last, before) * LEAD_IN_SHARE, LEAD_IN_M);
    control.splice(n - 1, 0, sub(last, scale(levelWay, leadOut)));
  }
  const points = resample(spline(control));
  const n = points.length;
  const tangent = (i: number) => norm(sub(points[Math.min(i + 1, n - 1)], points[Math.max(i - 1, 0)]), heading);
  const k = Math.max(1, Math.round(BEND_M / STEP_M));
  const bend: number[] = [];
  const speed: number[] = [];
  for (let i = 0; i < n; i++) {
    const [a, b] = [Math.max(i - k, 0), Math.min(i + k, n - 1)];
    const span = Math.max(b - a, 1) * STEP_M;
    const [ta, tb] = [tangent(a), tangent(b)];
    const whole = len(sub(tb, ta)) / span;
    bend.push(dot(cross(ta, tb), up) / span);
    const slope = dot(tangent(i), up);
    const bySlope = slope > 0 ? 1 - CLIMB_SLOWS * slope : 1 - DIVE_SPEEDS * slope;
    const byBend = Math.sqrt((GRAVITY * Math.tan(w.mostBank)) / Math.max(whole, 1e-5));
    speed.push(Math.max(Math.min(cruise * bySlope, byBend), w.slowest));
  }
  const reach = Math.round(EASE_SPEED_M / STEP_M);
  const ease = (values: number[]) => {
    const sums = [0];
    for (const v of values) sums.push(sums[sums.length - 1] + v);
    return values.map((_, i) => {
      const k = Math.min(reach, i, n - 1 - i);
      return (sums[i + k + 1] - sums[i - k]) / (2 * k + 1);
    });
  };
  const least = speed.map((_, i) => Math.min(...speed.slice(Math.max(0, i - reach), Math.min(n, i + reach + 1))));
  const limit = ease(least);
  speed.splice(0, n, ...limit);
  const bound = () => {
    speed[0] = Math.min(speed[0], Math.max(speedNow, 0));
    for (let i = 1; i < n; i++) speed[i] = Math.min(speed[i], Math.sqrt(speed[i - 1] ** 2 + 2 * w.accel * dist(points[i], points[i - 1])));
    speed[n - 1] = 0;
    for (let i = n - 2; i >= 0; i--) speed[i] = Math.min(speed[i], Math.sqrt(speed[i + 1] ** 2 + 2 * w.brake * dist(points[i + 1], points[i])));
  };
  bound();
  for (let pass = 0; pass < EASE_SPEED_PASSES; pass++) speed.splice(0, n, ...ease(speed));
  for (let i = 0; i < n; i++) speed[i] = Math.min(speed[i], limit[i]);
  bound();
  const time = [0];
  for (let i = 1; i < n; i++) time.push(time[i - 1] + dist(points[i], points[i - 1]) / Math.max((speed[i] + speed[i - 1]) * 0.5, 0.05));
  return { points, speed, time, bend };
}

export const seconds = (p: Plan) => p.time[p.time.length - 1];

/// Where a plan has its flier `t` seconds in (`Plan::at`).
export function sample(p: Plan, t: number): Sample {
  const n = p.points.length;
  if (n < 2) return { at: p.points[0], way: [0, 0, 1], speed: 0, bend: 0, ended: true };
  let lo = 0;
  let hi = n;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (p.time[mid] <= t) lo = mid + 1;
    else hi = mid;
  }
  const i = Math.min(Math.max(lo, 1), n - 1);
  const [t0, t1] = [p.time[i - 1], p.time[i]];
  const f = Math.min(Math.max((t - t0) / Math.max(t1 - t0, 1e-5), 0), 1);
  return {
    at: mix(p.points[i - 1], p.points[i], f),
    way: norm(sub(p.points[i], p.points[i - 1]), [0, 0, 1]),
    speed: p.speed[i - 1] + (p.speed[i] - p.speed[i - 1]) * f,
    bend: p.bend[i - 1] + (p.bend[i] - p.bend[i - 1]) * f,
    ended: t >= seconds(p),
  };
}

/// When, from its start, a plan passes nearest a point.
export function passes(p: Plan, point: Vec3): number {
  let best = 0;
  p.points.forEach((q, i) => {
    if (dist(q, point) < dist(p.points[best], point)) best = i;
  });
  return p.time[best];
}

type Leg = { frame: number; plan?: Plan; face: Vec3; hover?: Vec3; from: Vec3 };

/// A flier's whole course as the remote is told it, each leg from its frame (a sixtieth of a
/// second), so a scene knows where it is on any frame: what `flight.path` and `flight.hover`
/// do with it in the game.
export class Course {
  legs: Leg[] = [];
  start: Vec3;
  face: Vec3;
  w: Wings;
  constructor(start: Vec3, face: Vec3, w: Wings = wings()) {
    this.start = start;
    this.face = face;
    this.w = w;
  }

  /// Where it is on a frame, the way it heads and its speed, as the game has them.
  state(frame: number): { at: Vec3; way: Vec3; speed: number; leg?: Leg } {
    let here = this.start;
    let way = norm(level(sub(this.face, this.start)), [0, 0, 1]);
    let speed = 0;
    let current: Leg | undefined;
    for (const [i, leg] of this.legs.entries()) {
      if (leg.frame > frame) break;
      current = leg;
      const next = this.legs[i + 1]?.frame ?? Infinity;
      const t = (Math.min(frame, next) - leg.frame) / 60;
      if (leg.plan) {
        const now = sample(leg.plan, t);
        here = now.at;
        if (now.ended) {
          way = norm(level(sub(leg.face, here)), now.way);
          speed = 0;
        } else {
          [way, speed] = [now.way, now.speed];
        }
      } else if (leg.hover) {
        const from = leg.from;
        here = mix(from, leg.hover, 1 - Math.exp(-SETTLE * t));
        way = norm(level(sub(leg.face, here)), way);
        speed = Math.min(this.w.slowest, SETTLE * dist(here, leg.hover));
      }
    }
    return { at: here, way, speed, leg: current };
  }

  at(frame: number): Vec3 {
    return this.state(frame).at;
  }

  /// Flies it through points from a frame, to hover at the last facing a point (on along its
  /// way if not given), at its cruise or the speed given; the plan.
  fly(frame: number, through: Vec3[], facing?: Vec3, speed?: number): Plan {
    const now = this.state(frame);
    const last = through[through.length - 1];
    const before = through.length >= 2 ? through[through.length - 2] : now.at;
    const face = facing ?? add(last, sub(last, before));
    const p = plan(now.at, now.way, now.speed, through, speed ?? this.w.cruise, this.w);
    this.legs.push({ frame, plan: p, face, from: now.at });
    return p;
  }

  /// Holds it hovering at a point facing another from a frame: easing there if it is slow,
  /// else flying there and braking into it, as `flight.hover` does.
  hover(frame: number, point: Vec3, face: Vec3) {
    const now = this.state(frame);
    const flying = now.leg?.plan !== undefined && now.speed > this.w.slowest + 0.5;
    if (flying) {
      this.legs.push({ frame, plan: plan(now.at, now.way, now.speed, [point], this.w.cruise, this.w), face, from: now.at });
    } else {
      this.legs.push({ frame, hover: point, face, from: now.at });
    }
  }
}
