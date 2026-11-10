#!/usr/bin/env node
// An unarmed creature's blow as it stands in its file, in numbers (D71, D654): what a two-fisted
// smash or a sweep must show before anyone films it, and where the game's data must put its
// moments (assets/data/creatures.ron `bite_at_s`, `bite_reach_m`, `heard`).
//
//   node tools/characters/blow.ts FILE.glb CLIP... [--json]
//
// A line a clip: its length; the fists' highest moment and height (as a share of the head's
// standing height, so bodies compare); the landing, the moment after that top when the fists first
// come down to their lowest (within 2% of the body's height), with their height, how far ahead of the hips' start they are and how far apart; the
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
  const hips0 = at(0, "pelvis");
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
  // The feet: a foot is planted while its ball is near its lowest; its slide is the ground it
  // covers so, over the whole clip.
  let slide = 0;
  for (const side of ["r", "l"]) {
    const low = Math.min(...poses.map((_, f) => at(f, `ball_${side}`)[1]));
    for (let f = 1; f < frames; f++) {
      const [a, b] = [at(f - 1, `ball_${side}`), at(f, `ball_${side}`)];
      if (a[1] - low < head * 0.03 && b[1] - low < head * 0.03) slide += Math.hypot(b[0] - a[0], b[2] - a[2]);
    }
  }
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
    lands_ahead_m: Math.hypot(landed[0] - hips0[0], landed[2] - hips0[2]),
    apart_m: len(sub(at(land, "hand_r"), at(land, "hand_l"))),
    tell_s: begin / FPS,
    strike_s: (land - begin) / FPS,
    strike_mps: peak,
    recovery_s: clip.end - land / FPS,
    foot_slide_m: slide,
    in_trunk_m: Math.max(0, deep),
  };
}

if (import.meta.main) {
  const args = process.argv.slice(2).filter((a) => !a.startsWith("--"));
  if (args.length < 2) throw new Error("blow: give FILE.glb CLIP... (assets/models/characters/troll.glb Troll_Smash)");
  const g = new Glb(args[0]);
  const rows = args.slice(1).map((c) => blow(g, c));
  if (process.argv.includes("--json")) console.log(JSON.stringify(rows, null, 1));
  else
    for (const b of rows)
      console.log(
        `${b.clip}: ${b.seconds.toFixed(2)} s; fists top ${b.top_s.toFixed(2)} s at ${(b.top_share * 100).toFixed(0)}% of the head; ` +
          `land ${b.lands_s.toFixed(2)} s, ${b.lands_up_m.toFixed(2)} m up, ${b.lands_ahead_m.toFixed(2)} m ahead, ${b.apart_m.toFixed(2)} m apart; ` +
          `tell ${b.tell_s.toFixed(2)} s, strike ${b.strike_s.toFixed(2)} s at ${b.strike_mps.toFixed(1)} m/s, recovery ${b.recovery_s.toFixed(2)} s; ` +
          `feet slide ${b.foot_slide_m.toFixed(2)} m; wrist in the trunk ${b.in_trunk_m.toFixed(2)} m`,
      );
}
