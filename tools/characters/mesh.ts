#!/usr/bin/env node
// Does a blow made elsewhere mesh with our fight? A take, as it stands in its file, measured
// against the guard the fight plays, before anything is eased, driven or locked (the ladder's
// raw and carried stages, research/2026-10-31-why-the-bought-blows-slide.md):
//   start  the frame before the wind-up nearest the guard: each foot's distance from where the
//          guard has it, and the hips' height against the guard's, after the best turn and
//          shift along the ground. Easing into another stance bends the legs.
//   feet   a foot that stands slips under SLIP_M; a foot that steps lifts its ball LIFT_M and
//          never crosses the floor at sprint speed (slip and steps as the ladder read them).
//   body   the sword hand's fastest 100 ms in the body's frame, the hips' and shoulders' turn.
//   shape  right hand against left, how far apart they stay, where the blow lands against the
//          facing, and when, so a move's bars can be reached at a play rate (D585).
// The first clip named is the row the others are timed against (`rate`).
//   node tools/characters/mesh.ts <file.glb[:Clip[@rate]]>... [--guard Low_Guard] [--body fight.glb] [--json]
//
// Node 22.18 or later; no dependencies.

import { parseArgs } from "node:util";
import { Glb, type Pose, posed, sub, len, type V3 } from "./glb.ts";

export const RATE = 120;
export const SLIP_M = 0.03;
export const LIFT_M = 0.03;
export const START_M = 0.15;
// A foot stands while its sole's least-moving point is under this, and it is low.
const STOOD_MPS = 0.25;
const LOW_M = 0.06;
const STOOD_S = 4 / 60;
const MARGIN_S = 0.05;
// A gap between stands is a step when the ball went this far.
const STEP_M = 0.05;
// Crossing the floor faster than the game's sprint (movers.ron) reads as a skate.
const SPRINT_MPS = 6;
// The strike is the run around the peak over 40% of it, the wind-up back to 15% of it.
const STRIKE = 0.4;
const STILL = 0.15;
const TURN_DEG = 40;

export type Frame = { t: number; ball: [V3, V3]; heel: [V3, V3]; pelvis: V3; hip: [V3, V3]; shoulder: [V3, V3]; hand: [V3, V3] };

/** A clip's frames, each side [left, right], played at `rate`, in the model's space or on `under`. */
export function frames(g: Glb, name: string | undefined, rate = 1, under?: (t: number) => { at: V3; a: number }): Frame[] {
  const clip = g.clip(name ?? (g.gltf.animations?.[0]?.name as string));
  if (!clip) throw new Error(`${g.file} has no clip ${name}: name one of ${g.gltf.animations?.map((a) => a.name).join(" ")}`);
  const pair = (n: string): [number, number] => [g.named(`${n}_l`), g.named(`${n}_r`)];
  const [ball, heel, hip, shoulder, hand] = ["ball", "foot", "thigh", "upperarm", "hand"].map(pair);
  const pelvis = g.named("pelvis");
  const out: Frame[] = [];
  for (let i = 0; i <= Math.round((clip.end / rate) * RATE); i++) {
    const t = i / RATE;
    const p: Pose[] = posed(g, clip, Math.min(t * rate, clip.end));
    const u = under?.(t * rate);
    const put = (n: number): V3 => (u ? shifted(turned(p[n].t, u.a), u.at) : p[n].t);
    const two = (b: [number, number]): [V3, V3] => [put(b[0]), put(b[1])];
    out.push({ t, ball: two(ball), heel: two(heel), pelvis: put(pelvis), hip: two(hip), shoulder: two(shoulder), hand: two(hand) });
  }
  return out;
}

const turned = (p: V3, a: number): V3 => [p[0] * Math.cos(a) + p[2] * Math.sin(a), p[1], -p[0] * Math.sin(a) + p[2] * Math.cos(a)];
const shifted = (p: V3, d: V3): V3 => [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
const flat = (a: V3, b: V3) => Math.hypot(a[0] - b[0], a[2] - b[2]);
const deg = (a: number) => (a * 180) / Math.PI;
/** Which way a left-to-right line faces, radians, 0 along +Z, positive to the left (+X). */
const facing = ([l, r]: [V3, V3]) => Math.atan2(-(l[2] - r[2]), l[0] - r[0]);
const near = (a: number, to: number) => a + Math.round((to - a) / (2 * Math.PI)) * 2 * Math.PI;

export type Fit = { turn: number; at: V3; feet: [number, number]; hips: number };

/** The turn and ground shift that lay frame `f`'s feet and hips best on `guard`'s, and what is
 * left: each foot's worst point off the guard's, and the hips' height against it, metres. */
export function fit(f: Frame, guard: Frame): Fit {
  const points = (x: Frame) => [...x.ball, ...x.heel, x.pelvis];
  const [a, b] = [points(f), points(guard)];
  const mid = (ps: V3[]): V3 => [0, 2].map((k) => ps.reduce((s, p) => s + p[k], 0) / ps.length) as unknown as V3;
  const [ma, mb] = [mid(a), mid(b)];
  const off = (p: V3, t: number): V3 => {
    const r = turned([p[0] - ma[0], p[1], p[2] - ma[1]], t);
    return [r[0] + mb[0], r[1], r[2] + mb[1]];
  };
  const lead = facing(guard.hip) - facing(f.hip);
  let best = lead;
  let least = Infinity;
  for (let d = -TURN_DEG; d <= TURN_DEG; d += 1) {
    const t = lead + (d * Math.PI) / 180;
    const e = a.reduce((s, p, i) => s + flat(off(p, t), b[i]) ** 2, 0);
    if (e < least) [least, best] = [e, t];
  }
  const foot = (s: 0 | 1) => Math.max(len(sub(off(f.ball[s], best), guard.ball[s])), len(sub(off(f.heel[s], best), guard.heel[s])));
  const zero = off([0, 0, 0], best);
  return { turn: best, at: zero, feet: [foot(0), foot(1)], hips: f.pelvis[1] - guard.pelvis[1] };
}

/** The speed of the sole's least-moving point, m/s: a pivot on heel or toe is no slip. */
function slipRate(a: Frame, b: Frame, s: 0 | 1): number {
  const dt = Math.max(b.t - a.t, 1e-6);
  const v = (p: V3, q: V3) => [(q[0] - p[0]) / dt, (q[2] - p[2]) / dt];
  const [vb, vh] = [v(a.ball[s], b.ball[s]), v(a.heel[s], b.heel[s])];
  let least = Infinity;
  for (let u = 0; u <= 1.0001; u += 0.05) least = Math.min(least, Math.hypot(vh[0] + u * (vb[0] - vh[0]), vh[1] + u * (vb[1] - vh[1])));
  return least;
}

export type Step = { from: number; to: number; m: number; peak: number; lift: number; sweep: number };
export type Feet = { slip: [number, number]; steps: Step[]; dragged: number; swept: number };

/** Each foot's stands and steps over `fr`: the worst slip of a stand, m, and every step, the
 * ball's lift over its floor, its peak speed and its fastest while still on the floor. */
export function feet(fr: Frame[]): Feet {
  const slip: [number, number] = [0, 0];
  const steps: Step[] = [];
  for (const s of [0, 1] as const) {
    const floor = Math.min(...fr.map((f) => f.ball[s][1]));
    const heelFloor = Math.min(...fr.map((f) => f.heel[s][1]));
    const low = (f: Frame) => f.ball[s][1] < floor + LOW_M || f.heel[s][1] < heelFloor + LOW_M;
    const spans: [number, number][] = [];
    let from = -1;
    for (let i = 1; i <= fr.length; i++) {
      const still = i < fr.length && low(fr[i]) && slipRate(fr[i - 1], fr[i], s) < STOOD_MPS;
      if (still && from < 0) from = i - 1;
      if (!still && from >= 0) {
        if (fr[i - 1].t - fr[from].t >= STOOD_S) spans.push([from, i - 1]);
        from = -1;
      }
    }
    for (const [a, z] of spans) {
      let sum = 0;
      for (let i = a + 1; i <= z; i++) if (fr[i].t >= fr[a].t + MARGIN_S && fr[i].t <= fr[z].t - MARGIN_S) sum += slipRate(fr[i - 1], fr[i], s) / RATE;
      slip[s] = Math.max(slip[s], sum);
    }
    const gaps: [number, number][] = [];
    if (spans.length && spans[0][0] > 0) gaps.push([0, spans[0][0]]);
    spans.forEach((span, i) => gaps.push([span[1], spans[i + 1]?.[0] ?? fr.length - 1]));
    if (!spans.length) gaps.push([0, fr.length - 1]);
    for (const [a, z] of gaps) {
      if (z <= a || flat(fr[a].ball[s], fr[z].ball[s]) < STEP_M) continue;
      let [peak, lift, sweep] = [0, 0, 0];
      for (let i = a + 1; i <= z; i++) {
        const v = flat(fr[i - 1].ball[s], fr[i].ball[s]) * RATE;
        const up = fr[i].ball[s][1] - floor;
        peak = Math.max(peak, v);
        lift = Math.max(lift, up);
        if (up < LIFT_M) sweep = Math.max(sweep, v);
      }
      steps.push({ from: fr[a].t, to: fr[z].t, m: flat(fr[a].ball[s], fr[z].ball[s]), peak, lift, sweep });
    }
  }
  return { slip, steps, dragged: steps.filter((s) => s.lift < LIFT_M).length, swept: steps.filter((s) => s.sweep >= SPRINT_MPS).length };
}

/** A hand's speed in the body's frame (the pelvis's ground travel taken off), lightly smoothed. */
function speed(fr: Frame[], s: 0 | 1): number[] {
  const path = fr.map((f) => [f.hand[s][0] - f.pelvis[0], f.hand[s][1], f.hand[s][2] - f.pelvis[2]] as V3);
  const v = path.map((p, i) => (i ? len(sub(p, path[i - 1])) * RATE : 0));
  return v.map((_, i) => {
    const w = v.slice(Math.max(1, i - 2), i + 3);
    return w.reduce((a, x) => a + x, 0) / (w.length || 1);
  });
}
/** The fastest 100 ms of a speed curve, m/s, and the middle of it. */
function fastest(v: number[]): { mps: number; at: number } {
  const w = Math.round(0.1 * RATE);
  let best = { mps: 0, at: 0 };
  let sum = 0;
  for (let i = 0; i < v.length; i++) {
    sum += v[i] - (i >= w ? v[i - w] : 0);
    if (i + 1 >= w && sum / w > best.mps) best = { mps: sum / w, at: i - Math.floor(w / 2) };
  }
  return best;
}

export type Strike = { hand: number; left: number; windUp: number; from: number; peak: number; to: number };

/** The sword hand's fastest 100 ms, and the frames of its wind-up, strike and peak. */
export function strike(fr: Frame[]): Strike {
  const v = speed(fr, 1);
  const best = fastest(v);
  const top = Math.max(...v);
  const peak = v.indexOf(top);
  const back = (share: number) => {
    let i = peak;
    while (i > 0 && v[i - 1] >= top * share) i--;
    return i;
  };
  let to = peak;
  while (to < v.length - 1 && v[to + 1] >= top * STRIKE) to++;
  return { hand: best.mps, left: fastest(speed(fr, 0)).mps, windUp: back(STILL), from: back(STRIKE), peak, to };
}

const span = (xs: number[]) => Math.max(...xs) - Math.min(...xs);
/** A joint line's turn over `fr`, degrees, followed through a full circle. */
function turning(fr: Frame[], pick: (f: Frame) => [V3, V3]): number {
  const as: number[] = [];
  for (const f of fr) as.push(as.length ? near(facing(pick(f)), as[as.length - 1]) : facing(pick(f)));
  return deg(span(as));
}

/** Which frames a cut can start or end on: both feet down and still. A foot passing through the
 * guard's place in the middle of a step is no guard. */
export function heldFrames(fr: Frame[]): boolean[] {
  const floor = [0, 1].map((s) => Math.min(...fr.map((f) => f.ball[s][1])));
  const heel = [0, 1].map((s) => Math.min(...fr.map((f) => f.heel[s][1])));
  return fr.map((f, i) => {
    const p = fr[Math.max(i - 1, 0)];
    const q = fr[Math.min(i + 1, fr.length - 1)];
    return ([0, 1] as const).every((s) => (f.ball[s][1] < floor[s] + LOW_M || f.heel[s][1] < heel[s] + LOW_M) && Math.max(flat(p.ball[s], q.ball[s]), flat(p.heel[s], q.heel[s])) * (RATE / 2) < STOOD_MPS);
  });
}

export type Mesh = {
  cut: number; start: Fit; end: number; back: Fit; feet: Feet;
  hand: number; left: number; hips: number; shoulders: number;
  apart: number; ahead: number; bearing: number; up: number; across: number; travel: [number, number]; turnEnd: number;
  hit: number; strike: number; length: number;
};

/** A take against the guard: `cut`, the frame before its wind-up nearest the guard; `end`, the
 * frame after its strike nearest the guard again; everything else measured between them. */
export function mesh(fr: Frame[], guard: Frame): Mesh {
  const s = strike(fr);
  const worst = (x: Fit) => Math.max(...x.feet);
  const nearest = (a: number, z: number) => {
    const held = heldFrames(fr);
    let at = -1;
    for (let i = a; i <= z; i++) if (held[i] && (at < 0 || worst(fit(fr[i], guard)) < worst(fit(fr[at], guard)))) at = i;
    return at < 0 ? a : at;
  };
  const cut = nearest(0, s.windUp);
  const end = nearest(s.to, fr.length - 1);
  const start = fit(fr[cut], guard);
  const inGuard = (p: V3): V3 => shifted(turned([p[0] - fr[cut].pelvis[0], p[1], p[2] - fr[cut].pelvis[2]], start.turn), [guard.pelvis[0], 0, guard.pelvis[2]]);
  const blow = fr.slice(s.windUp, s.to + 1);
  const hit = inGuard(fr[s.peak].hand[1]);
  const [a, z] = [inGuard(fr[s.from].hand[1]), inGuard(fr[s.to].hand[1])];
  const last = inGuard(fr[end].pelvis);
  const zero = inGuard(fr[cut].pelvis);
  return {
    cut: fr[cut].t, start, end: fr[end].t, back: fit(fr[end], guard), feet: feet(fr.slice(cut, end + 1)),
    hand: s.hand, left: s.left, hips: turning(blow, (f) => f.hip), shoulders: turning(blow, (f) => f.shoulder),
    apart: Math.min(...blow.map((f) => len(sub(f.hand[0], f.hand[1])))),
    ahead: hit[2] - guard.pelvis[2], bearing: deg(Math.atan2(hit[0] - guard.pelvis[0], hit[2] - guard.pelvis[2])), up: hit[1],
    across: z[0] - a[0], travel: [last[2] - zero[2], last[0] - zero[0]],
    turnEnd: deg(near(facing(fr[end].hip) - facing(fr[cut].hip), 0)),
    hit: fr[s.peak].t - fr[cut].t, strike: fr[s.to].t - fr[s.from].t, length: fr[end].t - fr[cut].t,
  };
}

const n = (x: number, d = 2) => x.toFixed(d);
const cm = (x: number) => (x * 100).toFixed(1);

/** A row, what fails marked `!`; `rate` is the play rate that lays its hit on `vs`'s. */
export function row(name: string, m: Mesh, vs?: Mesh): string {
  const flag = (bad: boolean) => (bad ? "!" : " ");
  const steps = m.feet.steps.length;
  return [
    `${name}`,
    `  start${flag(Math.max(...m.start.feet) > START_M)} t ${n(m.cut)}: feet ${cm(m.start.feet[0])} / ${cm(m.start.feet[1])} cm, hips ${cm(m.start.hips)} cm, turned ${n(deg(m.start.turn), 0)} deg; back at t ${n(m.cut + m.length)}: feet ${cm(m.back.feet[0])} / ${cm(m.back.feet[1])} cm`,
    `  feet ${flag(Math.max(...m.feet.slip) >= SLIP_M || m.feet.dragged > 0 || m.feet.swept > 0)} slip ${cm(m.feet.slip[0])} / ${cm(m.feet.slip[1])} cm; ${steps} steps, ${m.feet.dragged} dragged, ${m.feet.swept} swept` +
      (steps ? ` (${m.feet.steps.map((s) => `${cm(s.m)} cm, up ${cm(s.lift)}, ${n(s.peak, 1)} m/s`).join("; ")})` : ""),
    `  body  hand ${n(m.hand, 1)} m/s, left ${n(m.left, 1)}; hips ${n(m.hips, 0)} deg, shoulders ${n(m.shoulders, 0)} deg`,
    `  shape hit ${n(m.ahead)} m ahead, ${n(m.bearing, 0)} deg (+ left), up ${n(m.up)} m; across ${n(m.across)} m (+ right to left); hands apart ${n(m.apart)} m; body ${n(m.travel[0])} m ahead, ${n(m.travel[1])} m left, ${n(m.turnEnd, 0)} deg`,
    `  time  hit ${n(m.hit)} s, strike ${n(m.strike)} s, whole ${n(m.length)} s` + (vs ? `; rate ${n(m.hit / vs.hit)} to its hit, ${n(m.length / vs.length)} to its length` : ""),
  ].join("\n");
}

export function parse(spec: string): { file: string; clip?: string; rate: number } {
  const [, file, clip, rate] = spec.match(/^(.+?\.glb)(?::([^@]+))?(?:@([\d.]+))?$/) ?? [];
  if (!file) throw new Error(`${spec}: name a clip as file.glb, file.glb:Clip or file.glb:Clip@rate`);
  return { file, clip, rate: rate ? Number(rate) : 1 };
}

function main(): void {
  const { values: v, positionals } = parseArgs({
    allowPositionals: true,
    options: { guard: { type: "string", default: "Low_Guard" }, body: { type: "string", default: "assets/models/character/fight.glb" }, json: { type: "boolean", default: false } },
  });
  if (!positionals.length) throw new Error("name a take: node tools/characters/mesh.ts take.glb, or the blow to time against first, assets/models/character/fight.glb:Open_Slash@1.21 take.glb");
  const body = new Glb(v.body!);
  const guard = frames(body, v.guard)[0];
  const rows = positionals.map((spec) => {
    const { file, clip, rate } = parse(spec);
    return { spec, m: mesh(frames(new Glb(file), clip, rate), guard) };
  });
  if (v.json) return console.log(JSON.stringify(rows));
  for (const r of rows) console.log(row(r.spec, r.m, rows[0].m === r.m ? undefined : rows[0].m));
}

if (import.meta.main) main();
