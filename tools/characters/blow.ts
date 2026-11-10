#!/usr/bin/env node
// An unarmed creature's blow as it stands in its file, in numbers (D71, D654): what a two-fisted
// smash or a sweep must show before anyone films it, and where the game's data must put its
// moments (assets/data/creatures.ron `bite_at_s`, `bite_reach_m`, `heard`).
//
//   node tools/characters/blow.ts FILE.glb CLIP... [--sweep] [--json]
//
// `--sweep` measures a sideways blow instead (`sweep`, below): the hand that sweeps and which
// way, its tell, strike and recovery, the moment its wrist crosses straight ahead of the model's
// origin with its height and distance there (the data's `lands_at_s` and `reach_m`), and the
// arc's degrees, heights and distances.
//
// A line a clip: its length; the fists' highest moment and height (as a share of the head's
// standing height, so bodies compare); the landing, the moment after that top when the fists first
// come down to their lowest (within 2% of the body's height), with their height, how far ahead of the model's origin they are (where the game stands the body
// and measures a blow's reach from; not the hips, which an idle may hold behind it) and how far apart; the
// tell, the seconds from the clip's start to the strike's start (the last frame before the
// landing when the fists move under a fifth of their top speed); the strike, the seconds from
// there to the landing, and the fists' top speed in it; the recovery after it; how far a planted
// foot slides (a foot under 3% of the body's height over its lowest, moving along the ground);
// and how deep a wrist goes inside the trunk (nearer the spine's line than the shoulders' half
// width allows, in front of or behind it), the fault of a clip made for a thinner body.

import { Glb, len, posed, sub, type Pose, type V3 } from "./glb.ts";

const FPS = 60;

export type Blow = {
  clip: string;
  seconds: number;
  top_s: number;
  top_share: number;
  lands_s: number;
  lands_up_m: number;
  lands_ahead_m: number;
  apart_m: number;
  tell_s: number;
  strike_s: number;
  strike_mps: number;
  recovery_s: number;
  foot_slide_m: number;
  in_trunk_m: number;
};

export function blow(g: Glb, name: string): Blow {
  const clip = g.clip(name);
  if (!clip) throw new Error(`blow: no clip ${name} in the file; it holds ${g.clipNames().join(", ")}`);
  const id = (n: string) => {
    const i = g.named(n);
    if (i < 0) throw new Error(`blow: no bone ${n}; a blow is measured on our skeleton`);
    return i;
  };
  const frames = Math.max(2, Math.round(clip.end * FPS) + 1);
  const poses: Pose[][] = Array.from({ length: frames }, (_, i) => posed(g, clip, Math.min(clip.end, i / FPS)));
  const at = (f: number, n: string): V3 => poses[f][id(n)].t;
  const fists = (f: number): V3 => {
    const [r, l] = [at(f, "hand_r"), at(f, "hand_l")];
    return [(r[0] + l[0]) / 2, (r[1] + l[1]) / 2, (r[2] + l[2]) / 2];
  };
  const head = at(0, "Head")[1];
  let top = 0;
  for (let f = 1; f < frames; f++) if (fists(f)[1] > fists(top)[1]) top = f;
  let lowest = top;
  for (let f = top; f < frames; f++) if (fists(f)[1] < fists(lowest)[1]) lowest = f;
  // The landing is when they first come within 2% of the body's height of that, not a settle after.
  let land = top;
  while (land < lowest && fists(land)[1] > fists(lowest)[1] + head * 0.02) land++;
  const speed = (f: number) => (f === 0 ? 0 : len(sub(fists(f), fists(f - 1))) * FPS);
  let fastest = top;
  for (let f = top; f <= land; f++) if (speed(f) > speed(fastest)) fastest = f;
  const peak = speed(fastest);
  let begin = fastest;
  while (begin > 0 && speed(begin) > peak / 5) begin--;
  const slide = footSlide(frames, at, head);
  // Into the trunk: a wrist between the hips' and the chest's heights, nearer the spine's line
  // than half the shoulders' width less a hand, sideways and front to back alike.
  let deep = 0;
  for (let f = 0; f < frames; f++) {
    const [pel, chest] = [at(f, "pelvis"), at(f, "spine_03")];
    const half = len(sub(at(f, "upperarm_r"), at(f, "upperarm_l"))) / 2;
    for (const side of ["r", "l"]) {
      const h = at(f, `hand_${side}`);
      if (h[1] < pel[1] || h[1] > chest[1]) continue;
      const k = (h[1] - pel[1]) / Math.max(1e-6, chest[1] - pel[1]);
      const axis: V3 = [pel[0] + (chest[0] - pel[0]) * k, h[1], pel[2] + (chest[2] - pel[2]) * k];
      deep = Math.max(deep, half * 0.8 - len(sub(h, axis)));
    }
  }
  const landed = fists(land);
  return {
    clip: name,
    seconds: clip.end,
    top_s: top / FPS,
    top_share: fists(top)[1] / head,
    lands_s: land / FPS,
    lands_up_m: landed[1],
    lands_ahead_m: Math.hypot(landed[0], landed[2]),
    apart_m: len(sub(at(land, "hand_r"), at(land, "hand_l"))),
    tell_s: begin / FPS,
    strike_s: (land - begin) / FPS,
    strike_mps: peak,
    recovery_s: clip.end - land / FPS,
    foot_slide_m: slide,
    in_trunk_m: Math.max(0, deep),
  };
}

/// The ground a planted foot covers over the clip: a foot is planted while its ball is within 3%
/// of the body's height of its lowest.
function footSlide(frames: number, at: (f: number, n: string) => V3, head: number): number {
  let slide = 0;
  for (const side of ["r", "l"]) {
    let low = Infinity;
    for (let f = 0; f < frames; f++) low = Math.min(low, at(f, `ball_${side}`)[1]);
    for (let f = 1; f < frames; f++) {
      const [a, b] = [at(f - 1, `ball_${side}`), at(f, `ball_${side}`)];
      if (a[1] - low < head * 0.03 && b[1] - low < head * 0.03) slide += Math.hypot(b[0] - a[0], b[2] - a[2]);
    }
  }
  return slide;
}

export type Sweep = {
  clip: string;
  seconds: number;
  /// The hand that sweeps (the faster), and the way it goes across the front.
  hand: "r" | "l";
  toward: "left" | "right";
  /// The strike: from the last frame before the hand's fastest when it moves under a fifth of
  /// its top speed, to the first after it.
  tell_s: number;
  strike_s: number;
  strike_mps: number;
  recovery_s: number;
  /// The moment the wrist passes straight ahead of the model's origin (where the game stands the
  /// body and measures a blow's reach from), its height there and how far ahead; NaN if it never does.
  cross_s: number;
  cross_up_m: number;
  cross_ahead_m: number;
  /// The arc of the strike about the origin, seen from above: the degrees it covers, the bearings
  /// it begins and ends at (0 straight ahead, + the body's left), the wrist's lowest and highest
  /// in it, and its nearest and farthest from the origin along the ground.
  arc_deg: number;
  arc_from_deg: number;
  arc_to_deg: number;
  arc_low_m: number;
  arc_high_m: number;
  arc_near_m: number;
  arc_far_m: number;
  foot_slide_m: number;
};

/// A sideways blow: one hand carried flat across the front. What the data's `lands_at_s` and
/// `reach_m` must agree with: the crossing's moment, and the arc's distance and height.
export function sweep(g: Glb, name: string): Sweep {
  const clip = g.clip(name);
  if (!clip) throw new Error(`blow: no clip ${name} in the file; it holds ${g.clipNames().join(", ")}`);
  const id = (n: string) => {
    const i = g.named(n);
    if (i < 0) throw new Error(`blow: no bone ${n}; a blow is measured on our skeleton`);
    return i;
  };
  const frames = Math.max(2, Math.round(clip.end * FPS) + 1);
  const poses: Pose[][] = Array.from({ length: frames }, (_, i) => posed(g, clip, Math.min(clip.end, i / FPS)));
  const at = (f: number, n: string): V3 => poses[f][id(n)].t;
  const speed = (f: number, n: string) => (f === 0 ? 0 : len(sub(at(f, n), at(f - 1, n))) * FPS);
  const fastestOf = (n: string) => {
    let best = 1;
    for (let f = 1; f < frames; f++) if (speed(f, n) > speed(best, n)) best = f;
    return best;
  };
  const hand = speed(fastestOf("hand_r"), "hand_r") >= speed(fastestOf("hand_l"), "hand_l") ? "r" : "l";
  const bone = `hand_${hand}`;
  const fastest = fastestOf(bone);
  const peak = speed(fastest, bone);
  let [begin, finish] = [fastest, fastest];
  while (begin > 0 && speed(begin, bone) > peak / 5) begin--;
  while (finish < frames - 1 && speed(finish + 1, bone) > peak / 5) finish++;
  // The model faces +Z and its left is +X: a bearing is 0 straight ahead, + to its left.
  const bearing = (f: number) => (Math.atan2(at(f, bone)[0], at(f, bone)[2]) * 180) / Math.PI;
  let cross = -1;
  let [arc, low, high, near, far] = [0, Infinity, -Infinity, Infinity, 0];
  for (let f = begin; f <= finish; f++) {
    const p = at(f, bone);
    [low, high] = [Math.min(low, p[1]), Math.max(high, p[1])];
    [near, far] = [Math.min(near, Math.hypot(p[0], p[2])), Math.max(far, Math.hypot(p[0], p[2]))];
    if (f === begin) continue;
    let step = bearing(f) - bearing(f - 1);
    step -= 360 * Math.round(step / 360);
    arc += step;
    if (cross < 0 && p[2] > 0 && bearing(f) * bearing(f - 1) <= 0) cross = f;
  }
  const crossed = cross < 0 ? undefined : at(cross, bone);
  return {
    clip: name,
    seconds: clip.end,
    hand,
    toward: arc >= 0 ? "left" : "right",
    tell_s: begin / FPS,
    strike_s: (finish - begin) / FPS,
    strike_mps: peak,
    recovery_s: clip.end - finish / FPS,
    cross_s: crossed ? cross / FPS : NaN,
    cross_up_m: crossed ? crossed[1] : NaN,
    cross_ahead_m: crossed ? Math.hypot(crossed[0], crossed[2]) : NaN,
    arc_deg: Math.abs(arc),
    arc_from_deg: bearing(begin),
    arc_to_deg: bearing(finish),
    arc_low_m: low,
    arc_high_m: high,
    arc_near_m: near,
    arc_far_m: far,
    foot_slide_m: footSlide(frames, at, at(0, "Head")[1]),
  };
}

if (import.meta.main) {
  const args = process.argv.slice(2).filter((a) => !a.startsWith("--"));
  if (args.length < 2) throw new Error("blow: give FILE.glb CLIP... [--sweep] (assets/models/characters/troll.glb Troll_Smash)");
  const g = new Glb(args[0]);
  const sideways = process.argv.includes("--sweep");
  const rows = args.slice(1).map((c) => (sideways ? sweep(g, c) : blow(g, c)));
  if (process.argv.includes("--json")) console.log(JSON.stringify(rows, null, 1));
  else if (sideways)
    for (const s of rows as Sweep[])
      console.log(
        `${s.clip}: ${s.seconds.toFixed(2)} s; hand_${s.hand} sweeps to its ${s.toward}; tell ${s.tell_s.toFixed(2)} s, strike ${s.strike_s.toFixed(2)} s at ${s.strike_mps.toFixed(1)} m/s, recovery ${s.recovery_s.toFixed(2)} s; ` +
          (Number.isNaN(s.cross_s) ? "never crosses straight ahead; " : `crosses ahead ${s.cross_s.toFixed(2)} s, ${s.cross_up_m.toFixed(2)} m up, ${s.cross_ahead_m.toFixed(2)} m out; `) +
          `arc ${s.arc_deg.toFixed(0)} degrees (${s.arc_from_deg.toFixed(0)} to ${s.arc_to_deg.toFixed(0)}), ${s.arc_low_m.toFixed(2)} to ${s.arc_high_m.toFixed(2)} m up, ${s.arc_near_m.toFixed(2)} to ${s.arc_far_m.toFixed(2)} m out; ` +
          `feet slide ${s.foot_slide_m.toFixed(2)} m`,
      );
  else
    for (const b of rows as Blow[])
      console.log(
        `${b.clip}: ${b.seconds.toFixed(2)} s; fists top ${b.top_s.toFixed(2)} s at ${(b.top_share * 100).toFixed(0)}% of the head; ` +
          `land ${b.lands_s.toFixed(2)} s, ${b.lands_up_m.toFixed(2)} m up, ${b.lands_ahead_m.toFixed(2)} m ahead, ${b.apart_m.toFixed(2)} m apart; ` +
          `tell ${b.tell_s.toFixed(2)} s, strike ${b.strike_s.toFixed(2)} s at ${b.strike_mps.toFixed(1)} m/s, recovery ${b.recovery_s.toFixed(2)} s; ` +
          `feet slide ${b.foot_slide_m.toFixed(2)} m; wrist in the trunk ${b.in_trunk_m.toFixed(2)} m`,
      );
}
