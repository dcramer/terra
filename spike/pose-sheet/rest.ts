#!/usr/bin/env node
// Do empty hands hang as a relaxed person's do? `Idle_Loop` passed every check with its palms out
// and its arms held off the body until the developer saw it. Over the clip, on every body
// template, each side's worst:
//   upper arm   degrees out from straight down, in the body's side-to-side plane
//   elbow       degrees bent from straight
//   palm        degrees turned from facing the thigh, + toward the front
//   fingers     degrees the middle finger curls, its first two joints summed
//   arm gap     cm between the upper arm's skin and the ribs, at its middle (its thickness off)
//   hand gap    cm between the palm and the thigh or hip (moves.ts's thickness off)
// The bars are a person at ease, not a clip, and not a pose sheet (one drawn from our own render
// copies its arms): an upright CT of 79 adults standing with "the palms touching the lateral
// sides of the thighs in a comfortable resting position" (Yoshida et al. 2022, J Orthop Surg Res
// 17:239, PMC9013055: the arm 4.5 degrees out at the shoulder, IQR 1 to 8, the elbow bent 15.5,
// IQR 13 to 18, the forearm mid-turn so the palm faces the thigh), and the resting hand's finger
// cascade (Lee and Jung 2014, Int J Occup Saf Ergon 20:91). The bought idles measured with this
// tool hold the arms out for every body and are no bar: research/2026-11-01-pose-sheet-blow.md,
// "The idle's arms, from a person at ease".
//
//   node spike/pose-sheet/rest.ts FILE.glb CLIP [--arms]
//
// `--arms` measures the pose the game shows: the clip, then the walker's hip-keeping turn
// (arms.ts, D535) laid over it on each template, as moves.ts --arms does.

import { Glb, len, sub, type V3 } from "../../tools/characters/glb.ts";
import { keptOff } from "../../tools/characters/arms.ts";
import { shaped, skinOf } from "../../tools/characters/skin.ts";

// [least, most, and the least it must come to at some moment] for each measure:
//   upper arm   the CT's IQR (1 to 8 at the shoulder) and a little for the sway
//   elbow       the CT's IQR (13 to 18) and the sway; a locked arm (under 10) is the fault seen
//   palm        mid-turn (the CT's 80 to 99 of pronation) less its 9 of the arm turned in, and
//               the backward turn a relaxed arm shows; never toward the front
//   fingers     curled as a resting hand is, never flat (under 40) nor a fist
//   arm gap     the upper arm against the side; how deep it goes is moves.ts's
//   hand gap    the palm touching the thigh at some moment (2 cm for the skin's give), and never
//               further off than that and the hips' own sway (3 cm in the idle's keys)
const BARS: Record<string, [number, number, number?]> = {
  upper: [-2, 10],
  elbow: [10, 25],
  palm: [-30, 10],
  fingers: [40, 110],
  "arm gap": [-Infinity, 3],
  "hand gap": [-1.5, 5, 2],
};
const UNITS: Record<string, string> = { upper: "°", elbow: "°", palm: "°", fingers: "°", "arm gap": " cm", "hand gap": " cm" };
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const mid = (a: V3, b: V3): V3 => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];
const norm = (a: V3): V3 => {
  const l = len(a) || 1;
  return [a[0] / l, a[1] / l, a[2] / l];
};
const deg = (r: number) => (r * 180) / Math.PI;
const between = (a: V3, b: V3) => deg(Math.acos(Math.max(-1, Math.min(1, dot(norm(a), norm(b))))));

const args = process.argv.slice(2);
const arms = args.includes("--arms");
const [file, name] = args.filter((a) => a !== "--arms");
const from = new Glb(file);
const clip = from.clip(name);
if (!clip) throw new Error(`rest: no clip ${name} in ${file}; give a clip the file holds (${from.clipNames().slice(0, 6).join(", ")}, ...)`);
// Played on the game's body and its templates, whatever file the clip comes from, as moves.ts does.
const g = new Glb("assets/models/character/base.glb");
const { shapes, skinned } = skinOf(g, "assets/models/character/templates.json");
const bone = (n: string) => {
  const i = g.named(n);
  if (i < 0) throw new Error(`rest: no bone ${n} in ${file}; the measures need our skeleton's arm, hand and thigh bones`);
  return i;
};

type Side = "r" | "l";
/// Each side's measures at one moment on one template, in the body's own frame: its side-to-side
/// axis from the hip joints, level, so a clip turned in the file reads the same.
function measure(shape: (typeof shapes)[number], t: number): Record<Side, Record<string, number>> {
  let p = shaped(g, shape, clip!, t);
  if (arms) p = shaped(g, shape, clip!, t, keptOff(g, p));
  const inside = skinned(shape, p);
  const at = (n: string) => p[bone(n)].t;
  const across = sub(at("thigh_r"), at("thigh_l"));
  const right = norm([across[0], 0, across[2]]);
  const forward = cross([0, 1, 0], right);
  const out = { r: right, l: right.map((v) => -v) as V3 };
  const res = {} as Record<Side, Record<string, number>>;
  for (const s of ["r", "l"] as const) {
    const b = (n: string) => at(`${n}_${s}`);
    const upper = sub(b("lowerarm"), b("upperarm"));
    const fore = sub(b("hand"), b("lowerarm"));
    const fingers = norm(sub(b("middle_01"), b("hand")));
    const thumbward = norm(sub(b("index_01"), b("pinky_01")));
    // The palm's way, mirrored for the left hand.
    const palm = s === "r" ? cross(thumbward, fingers) : cross(fingers, thumbward);
    // Skin to skin: the distance outside the body less the limb's half-thickness (skin.ts's pads).
    const gap = (q: V3, pad: number) => (-inside(q).depth - pad) * 100;
    res[s] = {
      upper: deg(Math.atan2(dot(upper, out[s]), -upper[1])),
      elbow: between(upper, fore),
      palm: deg(Math.atan2(dot(palm, forward), -dot(palm, out[s]))),
      fingers: between(sub(b("middle_01"), b("hand")), sub(b("middle_02"), b("middle_01"))) + between(sub(b("middle_02"), b("middle_01")), sub(b("middle_03"), b("middle_02"))),
      "arm gap": gap(mid(b("upperarm"), b("lowerarm")), 0.045),
      "hand gap": gap(mid(b("hand"), b("middle_01")), 0.025),
    };
  }
  return res;
}

let failed = false;
const f = (m: string, v: number) => `${v.toFixed(0)}${UNITS[m]}`;
const range = (m: string, vs: number[]) => {
  const [a, b] = [Math.min(...vs), Math.max(...vs)];
  return { a, b, text: Math.abs(b - a) < 0.5 ? f(m, a) : `${a.toFixed(0)}..${f(m, b)}` };
};
const all = shapes.map((shape) => ({ shape: shape.name, moments: Array.from({ length: 13 }, (_, k) => measure(shape, (k / 12) * clip.end)) }));
for (const side of ["r", "l"] as const) {
  const parts: string[] = [];
  const bad: string[] = [];
  for (const [m, [lo, hi, reaches = Infinity]] of Object.entries(BARS)) {
    // The gaps by template, since the widest hips and the narrowest set them; the angles over all.
    const byShape = m.endsWith("gap") ? all.map((x) => ({ who: x.shape, ...range(m, x.moments.map((y) => y[side][m])) })) : [{ who: "", ...range(m, all.flatMap((x) => x.moments.map((y) => y[side][m]))) }];
    const off = byShape.filter((r) => r.a < lo || r.b > hi || r.a > reaches);
    const text = byShape.length > 1 ? byShape.map((r) => `${r.who} ${r.text}`).join(" ") : byShape[0].text;
    parts.push(`${m} ${text}${off.length ? " ✗" : ""}`);
    if (off.length) bad.push(m);
  }
  failed ||= bad.length > 0;
  console.log(`${bad.length ? "✗" : "✓"} ${name} ${side}${arms ? " (as the game shows it)" : ""}: ${parts.join("; ")}`);
}
const barText = ([m, [lo, hi, reaches]]: [string, [number, number, number?]]) => `${m} ${lo === -Infinity ? `at most ${f(m, hi)}` : `${lo}..${f(m, hi)}`}${reaches === undefined ? "" : `, down to ${f(m, reaches)} at some moment`}`;
console.log(`  bars: ${Object.entries(BARS).map(barText).join("; ")}`);
if (failed) console.log("  pose the hanging arm in the keys (`rest_arm`, `fingers`); don't widen the bars: they are a person at ease, the header says whose");
process.exit(failed ? 1 : 0);
