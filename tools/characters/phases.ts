#!/usr/bin/env node
// Measures a blow's phases from its clip (docs/movement.md, "Moves as data"): the held
// weapon's tip followed through the skeleton frame by frame, in the body's own space (x to its
// left, y up, z ahead, from the hips' place at the start), and when it sweeps forward of the
// body fast enough to strike: the active frames a move's data says
// (assets/data/moves/*.ron). Prints, a line a clip, the span its tip is more than --ahead
// metres in front of the hips and moving faster than --speed m/s, its fastest moment in it and
// where it is then; --trace prints every frame.
//
//   node tools/characters/phases.ts assets/models/character/fight.glb Fight_Slash Fight_Backhand
//   node tools/characters/phases.ts <file.glb> <clip>… [--socket socket.hand.R] [--tip 0.83] [--speed 4] [--ahead 0.5] [--trace]
//
// Node 22.18 or later; no dependencies.

import { Glb, posed, rot, type V3 } from "./glb.ts";

const args = process.argv.slice(2);
const opt = (name: string, fallback: string) => {
  const i = args.indexOf(name);
  return i >= 0 ? args.splice(i, 2)[1] : fallback;
};
const trace = args.includes("--trace") ? (args.splice(args.indexOf("--trace"), 1), true) : false;
const socket = opt("--socket", "socket.hand.R");
const tipM = Number(opt("--tip", "0.83"));
const fast = Number(opt("--speed", "4"));
const reach = Number(opt("--ahead", "0.5"));
const [file, ...clips] = args;
if (!file || clips.length === 0) {
  console.error("usage: phases.ts <file.glb> <clip>… [--socket name] [--tip m] [--speed m/s] [--trace]");
  process.exit(1);
}

const glb = new Glb(file);
const sock = glb.named(socket);
const hips = glb.named("pelvis");
if (sock < 0 || hips < 0) throw new Error(`no ${sock < 0 ? socket : "pelvis"} in ${file}`);

for (const name of clips) {
  const clip = glb.clip(name);
  if (!clip) {
    console.log(`${name}: no such clip in ${file}`);
    continue;
  }
  const end = clip.end;
  const start = posed(glb, clip, 0)[hips].t;
  const step = 1 / 60;
  const frames: { t: number; tip: V3; speed: number }[] = [];
  for (let t = 0; t <= end + 1e-6; t += step) {
    const hand = posed(glb, clip, t)[sock];
    const out = rot(hand.r, [0, tipM * hand.s[1], 0]);
    const tip = out.map((x, j) => x + hand.t[j] - start[j]) as V3;
    const prev = frames.at(-1);
    const moved = prev ? Math.hypot(...tip.map((x, j) => x - prev.tip[j])) : 0;
    frames.push({ t, tip, speed: moved / step });
  }
  const clipName = name;
  const f = (v: number) => v.toFixed(2);
  if (trace) for (const fr of frames) console.log(`${f(fr.t)} tip ${fr.tip.map(f).join(" ")} speed ${f(fr.speed)} m/s`);
  const striking = frames.filter((fr) => fr.tip[2] > reach && fr.speed > fast);
  if (striking.length === 0) {
    console.log(`${clipName} (${f(end)} s): the tip never moves faster than ${fast} m/s more than ${reach} m ahead`);
    continue;
  }
  // The one sweep through the fastest moment, not every quick frame after it.
  const fastest = striking.reduce((a, b) => (b.speed > a.speed ? b : a));
  const strikes = (i: number) => frames[i].tip[2] > reach && frames[i].speed > fast;
  let from = frames.indexOf(fastest);
  let to = from;
  while (from > 0 && strikes(from - 1)) from--;
  while (to < frames.length - 1 && strikes(to + 1)) to++;
  const [left, up, ahead] = fastest.tip;
  console.log(
    `${clipName} (${f(end)} s): striking ${f(frames[from].t)}-${f(frames[to].t)} s; fastest ${f(fastest.speed)} m/s at ${f(fastest.t)} s, ${f(ahead)} m ahead, ${f(up)} m up, ${f(left)} m left`,
  );
}
