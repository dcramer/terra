// A swimmer's pose on the controller's filmed lake (yard.ts's lake run; docs/controller.md "What
// is presentation", D478): while it swims, the figure shows a swim clip and lies along the way it
// goes, upright treading water, level stroking at the surface, pitched along a dive: the strokes
// made lying and pitched by their way, as shipped swimmers are (Unreal's, Subnautica's; D514). Measured on the run's walk record (`swimming`, `shows`, `torso`, `going`), not
// by eye. A line a check (D388). Node 22.18 or later; no dependencies.

import type { Frame } from "../sandbox/duel/checks.ts";
import type { Check, Walk } from "./yard-checks.ts";

const FPS = 60;
/// swim.pose's bar: the torso within this of the way it should lie, deg.
export const POSE_DEG = 15;
/// After the body starts or stops swimming, its clip changes, or its way turns by more than the
/// bar within TURN_S (a dive pulled up to rise, levelled at the bed: faster than an eased lie
/// can follow within the bar), the figure eases into its new lie (`terra_walker::swim`, 95% in
/// half a second, as a shipped swimmer's pitch is rate-limited); so long is not judged, s.
export const EASE_S = 0.5;
export const TURN_DEG = POSE_DEG;
export const TURN_S = 0.25;
/// Slower than this the way it goes is no way to lie along, m/s (treading drifts).
export const GOING_MPS = 0.3;
/// A clip a swimmer may show, and the one that treads water upright.
const SWIM = /^Swim_/;
const TREAD = "Swim_Tread_Loop";
const UP = [0, 1, 0];

type Swimmer = { swimming?: boolean; shows?: string | null; torso?: number[] | null; going?: number[] };
const you = (frame: Frame) => frame.fighters.find((each) => each.who === "you") as (Frame["fighters"][number] & Swimmer) | undefined;
const length = (v: number[]) => Math.hypot(...v);
const angle = (a: number[], b: number[]) => {
  const cos = a.reduce((sum, x, i) => sum + x * b[i], 0) / (length(a) * length(b));
  return (Math.acos(Math.min(1, Math.max(-1, cos))) * 180) / Math.PI;
};

/// One swimming frame judged: the clip it shows, the way it should lie, how far off it lies.
export type Judged = { frame: number; shows: string; off: number; bad: boolean };

/// Each swimming frame past the ease after a change, with how far its torso lies off the way it
/// should (up treading, along the way it goes stroking or diving) and whether it fails.
export function judged(frames: Frame[]): Judged[] {
  const out: Judged[] = [];
  let since = 0;
  let was: string | undefined;
  const ways: { frame: number; way: number[] }[] = [];
  for (const frame of frames) {
    const me = you(frame);
    if (!me) continue;
    const now = `${me.swimming ?? false} ${me.shows ?? ""}`;
    if (now !== was) since = frame.frame;
    was = now;
    const way = me.going ?? [0, 0, 0];
    while (ways.length && ways[0].frame < frame.frame - TURN_S * FPS) ways.shift();
    if (length(way) >= GOING_MPS && ways.some((each) => angle(each.way, way) > TURN_DEG)) since = frame.frame;
    if (length(way) >= GOING_MPS) ways.push({ frame: frame.frame, way });
    if (!me.swimming || frame.frame - since < EASE_S * FPS) continue;
    const shows = me.shows ?? "";
    const treading = shows === TREAD;
    const going = me.going ?? [0, 0, 0];
    if (!treading && SWIM.test(shows) && length(going) < GOING_MPS) continue;
    const off = me.torso ? angle(me.torso, treading ? UP : going) : 180;
    out.push({ frame: frame.frame, shows, off, bad: !SWIM.test(shows) || off > POSE_DEG });
  }
  return out;
}

/// swim.pose: the share of judged swimming frames that show no swim clip or lie more than
/// POSE_DEG off their way, with the worst of each clip.
export function pose(lake: Walk): Check {
  const name = "swim pose";
  const seen = judged(lake.frames);
  if (!seen.length) return { name, ok: false, line: "✗ no swimming frame in the lake's record (`swimming`, `shows`, `torso`, `going`)", value: NaN, faults: 1 };
  const bad = seen.filter((each) => each.bad);
  const value = bad.length / seen.length;
  const clips = [...new Set(seen.map((each) => each.shows))].map((clip) => {
    const of = seen.filter((each) => each.shows === clip);
    const worst = of.reduce((a, b) => (b.off > a.off ? b : a));
    return `${clip || "no clip"} ${of.length} frames, worst ${worst.off.toFixed(0)} deg at frame ${worst.frame}`;
  });
  const ok = bad.length === 0;
  const line = `${ok ? "" : "✗ "}${(value * 100).toFixed(1)}% of ${seen.length} swimming frames off (no swim clip, or the torso over ${POSE_DEG} deg from its way); ${clips.join("; ")}`;
  return { name, ok, line, value, faults: bad.length };
}
