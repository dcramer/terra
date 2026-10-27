#!/usr/bin/env node
// Measures a walk clip's weight on a body, the way an animator checks a heavy walk
// (research/2026-10-26-giant-walker-weight.md): each footfall's moment and where the foot
// lands; how high the swinging foot clears the ground at mid-swing; how far the hips sink
// after each contact and when they are lowest; how long the crown lags the hips; and the pace
// the planted feet pass under the body. In the body's own space: metres to its left, up and
// ahead of its root, a clip played in place. One line a clip; --falls prints the footfalls as
// a scene keys them (tools/sandbox/trailer/scenes/cetri.ts's WALK).
//
//   node tools/characters/stride.ts assets/models/characters/cetri.glb Troll_Walk Giant_Walk [--falls]
//
// Node 22.18 or later; no dependencies.

import { Glb, posed, type V3 } from "./glb.ts";

/** A foot this near its lowest in the clip is on the ground, metres. */
export const ON_GROUND_M = 0.04;
/** Where the hips' lowest after a contact is looked for, seconds after it. */
export const SINK_S = 0.6;

export type Sample = { t: number; feet: Record<"l" | "r", { ankle: V3; ball: V3 }>; hips: V3; crown: V3 };
export type Fall = { side: "l" | "r"; at: number; left: number; ahead: number; clearance: number; drop: number; lowest: number; lag: number };
export type Measure = { end: number; pace: number; falls: Fall[]; range: number };

const ORDER = ["l", "r"] as const;

/** The measures of a loop sampled evenly over its length `end`. */
export function measure(samples: Sample[], end: number): Measure {
  const n = samples.length;
  const dt = end / n;
  const at = (i: number) => samples[((i % n) + n) % n];
  const sole = (i: number, s: "l" | "r") => Math.min(at(i).feet[s].ankle[1] - low[s].ankle, at(i).feet[s].ball[1] - low[s].ball);
  const low = Object.fromEntries(
    ORDER.map((s) => [s, { ankle: Math.min(...samples.map((x) => x.feet[s].ankle[1])), ball: Math.min(...samples.map((x) => x.feet[s].ball[1])) }]),
  ) as Record<"l" | "r", { ankle: number; ball: number }>;
  const range = Math.max(...samples.map((x) => x.hips[1])) - Math.min(...samples.map((x) => x.hips[1]));
  const falls: Fall[] = [];
  const speeds: number[] = [];
  for (const s of ORDER) {
    const down = (i: number) => at(i).feet[s].ankle[1] - low[s].ankle < ON_GROUND_M;
    for (let i = 0; i < n; i++) {
      if (down(i)) speeds.push(-(at(i + 1).feet[s].ankle[2] - at(i).feet[s].ankle[2]) / dt);
      if (!down(i) || down(i - 1)) continue;
      // A contact: the ankle down again after a swing; the swing from its lift-off.
      let off = i - 1;
      while (!down(off - 1) && off > i - n) off--;
      const mid = Math.round((off + i) / 2);
      const hips = (k: number) => at(k).hips[1];
      const crown = (k: number) => at(k).crown[1];
      const span = Math.round(SINK_S / dt);
      let lowest = i;
      for (let k = i; k <= i + span; k++) if (hips(k) < hips(lowest)) lowest = k;
      let bowed = lowest;
      for (let k = lowest; k <= lowest + span; k++) if (crown(k) - hips(k) < crown(bowed) - hips(bowed)) bowed = k;
      const foot = at(i).feet[s].ankle;
      falls.push({
        side: s,
        at: i * dt,
        left: foot[0],
        ahead: foot[2],
        clearance: sole(mid, s),
        drop: hips(i) - hips(lowest),
        lowest: (lowest - i) * dt,
        lag: (bowed - lowest) * dt,
      });
    }
  }
  falls.sort((a, b) => a.at - b.at);
  speeds.sort((a, b) => a - b);
  return { end, pace: speeds.length ? speeds[speeds.length >> 1] : 0, falls, range };
}

function sampled(body: Glb, name: string, fps = 60): { samples: Sample[]; end: number } | undefined {
  const clip = body.clip(name);
  if (!clip) return undefined;
  const node = (b: string) => {
    const i = body.named(b);
    if (i < 0) throw new Error(`${body.file} has no bone ${b}`);
    return i;
  };
  const [hips, crown] = [node("pelvis"), node("Head")];
  const feet = ORDER.map((s) => [node(`foot_${s}`), node(`ball_${s}`)]);
  const samples: Sample[] = [];
  const n = Math.max(2, Math.round(clip.end * fps));
  for (let k = 0; k < n; k++) {
    const t = (k * clip.end) / n;
    const p = posed(body, clip, t);
    samples.push({
      t,
      hips: p[hips].t,
      crown: p[crown].t,
      feet: { l: { ankle: p[feet[0][0]].t, ball: p[feet[0][1]].t }, r: { ankle: p[feet[1][0]].t, ball: p[feet[1][1]].t } },
    });
  }
  return { samples, end: clip.end };
}

function main() {
  const args = process.argv.slice(2);
  const [file, ...clips] = args.filter((a) => !a.startsWith("--"));
  if (!file || !clips.length) {
    console.error("usage: node tools/characters/stride.ts <body.glb> <clip>... [--falls] [--trace]");
    process.exit(2);
  }
  const body = new Glb(file);
  const f2 = (x: number) => x.toFixed(2);
  for (const name of clips) {
    const got = sampled(body, name);
    if (!got) {
      console.log(`✗ ${name}: no such clip in ${file}`);
      continue;
    }
    const m = measure(got.samples, got.end);
    const each = (k: keyof Fall) => m.falls.map((f) => f2(f[k] as number)).join("/");
    console.log(
      `STRIDE ${name}: ${f2(m.end)} s, ${m.falls.length} footfalls at ${each("at")} s, pace ${f2(m.pace)} m/s; ` +
        `clearance at mid-swing ${each("clearance")} m; hips sink ${each("drop")} m after contact, lowest ${each("lowest")} s on (range ${f2(m.range)} m); ` +
        `crown lowest ${each("lag")} s after the hips`,
    );
    if (args.includes("--trace"))
      for (let k = 0; k < got.samples.length; k += 6) {
        const x = got.samples[k];
        const v = (p: V3) => p.map((c) => c.toFixed(2)).join(",");
        console.log(`TRACE ${name} ${x.t.toFixed(2)} s: hips ${v(x.hips)} crown ${v(x.crown)} ankle l ${v(x.feet.l.ankle)} r ${v(x.feet.r.ankle)} ball l ${v(x.feet.l.ball)} r ${v(x.feet.r.ball)}`);
      }
    if (args.includes("--falls"))
      console.log(`FALLS ${name}: ` + JSON.stringify(m.falls.map((f) => ({ at: +f2(f.at), left: +f2(f.left), ahead: +f2(f.ahead) }))));
  }
}

if (import.meta.main) main();
