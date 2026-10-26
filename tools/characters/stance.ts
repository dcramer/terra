// How a clip that stays where it stands (an idle) stands, on a body: how far its knees bend,
// how high its hips stand and how far its head hangs forward, each against the body's rest, the
// measures an idle's bar is set on (motion.ts; the Uthana idle's soft knees and forward head
// were what failed it, 2026-10-26). Node 22.18 or later; no dependencies.

import { type Clip, type Glb, posed, sub, dot, len, type V3 } from "./glb.ts";

/** An idle's bar, set on the references (README.md, "Made clips"): Quaternius's CC0 idle, the
 * idle the game played, Uthana's on its own Manny. */
export const BAR = { knee: 18, kneeStep: 2, pelvis: 3, head: 4, arms: 2, seamCm: 2, seamDeg: 3, wander: 3 };

export type Stance = {
  /** The knees' bend, the thigh's line to the shin's (deg): the median of both through the clip, the most, and at rest. */
  knee: number; kneeMost: number; kneeRest: number;
  /** The most a knee's bend changes from one frame to the next, at 30 frames a second (deg). */
  kneeStep: number;
  /** The hips' height against rest (cm), lowest and highest. */
  pelvis: [number, number];
  /** How far the head stands ahead of the hips, against rest (cm), the mean through the clip:
   * ahead along where the hips face, so a body turned on the ground measures the same. */
  head: number;
};

/** A bone by name, any case: ours is `Head`, Unreal's Manny's `head`. */
const bone = (body: Glb, name: string) => body.gltf.nodes.findIndex((n) => n.name?.toLowerCase() === name.toLowerCase());

const angle = (a: V3, b: V3) => (Math.acos(Math.max(-1, Math.min(1, dot(a, b) / (len(a) * len(b))))) * 180) / Math.PI;

export const median = (xs: number[]) => {
  const s = [...xs].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};

/** The knee's bend on side `s` ("l", "r") of a posed body (deg). */
function bend(body: Glb, p: ReturnType<typeof posed>, s: string) {
  const [h, k, f] = ["thigh", "calf", "foot"].map((b) => p[bone(body, `${b}_${s}`)].t);
  return angle(sub(k, h), sub(f, k));
}

/** How far the head is ahead of the hips in a posed body, along where the hips face (m): the
 * left thigh to the right crossed with up is ahead, Y being up (glTF). */
function ahead(body: Glb, p: ReturnType<typeof posed>) {
  const across = sub(p[bone(body, "thigh_l")].t, p[bone(body, "thigh_r")].t);
  const fwd: V3 = [-across[2], 0, across[0]];
  const l = Math.hypot(fwd[0], fwd[2]);
  const head = sub(p[bone(body, "head")].t, p[bone(body, "pelvis")].t);
  return (head[0] * fwd[0] + head[2] * fwd[2]) / l;
}

/** How `clip` stands on `body`, sampled at 30 frames a second. */
export function stance(body: Glb, clip: Clip): Stance {
  const rest = posed(body, undefined, 0);
  const restHips = rest[bone(body, "pelvis")].t[1];
  const restAhead = ahead(body, rest);
  const knees: number[] = [];
  let step = 0, lo = Infinity, hi = -Infinity, sum = 0, n = 0;
  let was: number[] | undefined;
  for (let t = 0; t <= clip.end + 1e-6; t += 1 / 30) {
    const p = posed(body, clip, t);
    const now = ["l", "r"].map((s) => bend(body, p, s));
    knees.push(...now);
    if (was) step = Math.max(step, ...now.map((k, i) => Math.abs(k - was![i])));
    was = now;
    const dh = p[bone(body, "pelvis")].t[1] - restHips;
    lo = Math.min(lo, dh);
    hi = Math.max(hi, dh);
    sum += ahead(body, p) - restAhead;
    n++;
  }
  return {
    knee: median(knees), kneeMost: Math.max(...knees), kneeRest: (bend(body, rest, "l") + bend(body, rest, "r")) / 2,
    kneeStep: step, pelvis: [lo * 100, hi * 100], head: (sum / n) * 100,
  };
}

/** One line of a stance, with ✗ beside each measure over its bar. */
export function stanceLine(s: Stance): string {
  const x = (over: boolean) => (over ? " ✗" : "");
  return `knees ${s.knee.toFixed(0)} deg median${x(s.knee > BAR.knee)} (most ${s.kneeMost.toFixed(0)}, rest ${s.kneeRest.toFixed(0)}), ` +
    `step ${s.kneeStep.toFixed(1)} deg a frame${x(s.kneeStep > BAR.kneeStep)}; ` +
    `hips ${s.pelvis[0].toFixed(1)} to ${s.pelvis[1].toFixed(1)} cm from rest${x(Math.max(-s.pelvis[0], s.pelvis[1]) > BAR.pelvis)}; ` +
    `head ${s.head.toFixed(1)} cm forward of rest${x(s.head > BAR.head)}`;
}
