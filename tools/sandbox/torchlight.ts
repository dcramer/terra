#!/usr/bin/env node
// A held torch's light at deep night, in numbers (D71; the developers, 2026-11-09: "the light
// from the torch isnt amazing"): in play at midnight the player stands on open ground near the
// start (level, dry, nothing standing near: `world.ground`) and by a clear trunk, and at each a
// camera held where the third-person one sits (4.5 m behind, 2.6 m up) shoots the ground with
// the torch away and in hand. In hand it is shot three times in the one game (`fire.carried`):
// `main` with the carried torch's rig bare, `new` with the rig under trial (--rig, else the
// game's own), `back` bare again; an A/B line a number says main, new, new over main and back
// over main (how steady the reading is with nothing changed). Read off the mean of each
// pass's 120 standing frames, a line a number:
//
//   ground   the lit ground's linear luma a metre round the feet and 2 m ahead, clear of the
//            holder's own shadow
//   pool     the ground's linear luma by distance ahead, against its value at the feet: where
//            it falls to a half, a tenth and a hundredth, and what the night's floor is
//   night    the lit ground against the same ground with the torch away, at 3, 6, 10 and 15 m,
//            both divided by the far ground's change (the eye's exposure moves with the torch)
//   holder   the holder's back against the ground 2 m ahead
//   colour   the lit ground's hue and saturation 2 m ahead
//   shadow   the darkest ground on a ring 1.5 m round the holder against the ring's median, and
//            how many metres of the ring it is wide at half depth
//   trunk    by the tree (`world.trunks`, the first whose picture shows lit ground beside its
//            shadow: boughs to the ground hide it): its shadow on the ground
//            2 m past it against the lit ground either side, its width, each edge's 10% to
//            90%, and how much its edges change from frame to frame against the lit ground
//   flicker  the pool's luma over 120 frames: its deviation over its mean, its dominant
//            period, and its largest step from one frame to the next
//   run      the whole picture's largest step from one frame to the next, running and
//            strafing with the game's own camera
//   picture  the share of pure black and of clipped pixels
//
// Ground is taken as level with the holder's feet: stand on level ground, and read a distance
// past 10 m as about a metre either way. A reading, not a gate (D632): nothing is passed or
// failed.
//
//   cargo xtask gpu still -- node tools/sandbox/torchlight.ts [--rig '{"key":[side,up,ahead],"fill":{"at":[..],"share":0.03}}']
//       [--open EAST,SOUTH] [--trunk EAST,SOUTH] [--out target/job/torchlight]
//   node tools/sandbox/torchlight.ts --from <dir>     the numbers of a run already shot
//
// A run writes about 500 pictures a place: delete its directory once read.
//
// Node 22.18 or later; no dependencies.

import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { call, film, give, press, shot, sleep, start, state, status, stop, waitUntil, warp } from "./client.ts";
import { type Picture, readPng } from "./png.ts";

type V = [number, number, number];
/// A view: the eye, what it looks at, the holder's feet and the way they face, from the start.
type View = { eye: V; at: V; feet: V; ahead: [number, number] };
const VFOV = Math.PI / 4;
const FRAMES = 120;
// The size the game is judged at (D185), so a place's `-play.png` is a still for the desk.
const SIZE = [1920, 1080];

const sub = (a: V, b: V): V => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a: V, b: V) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V, b: V): V => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const unit = (a: V): V => {
  const l = Math.hypot(...a);
  return [a[0] / l, a[1] / l, a[2] / l];
};
const linear = (v: number) => (v / 255 <= 0.04045 ? v / 255 / 12.92 : ((v / 255 + 0.055) / 1.055) ** 2.4);

/// Where a point from the start falls on a picture, or nothing when out of it.
export function project(view: View, p: { width: number; height: number }, point: V): [number, number] | undefined {
  const f = unit(sub(view.at, view.eye));
  const r = unit(cross(f, [0, 1, 0]));
  const u = cross(r, f);
  const v = sub(point, view.eye);
  const z = dot(v, f);
  if (z < 0.2) return undefined;
  const t = Math.tan(VFOV / 2);
  const x = ((dot(v, r) / (z * t * (p.width / p.height)) + 1) / 2) * p.width;
  const y = ((1 - dot(v, u) / (z * t)) / 2) * p.height;
  return x < 3 || y < 3 || x >= p.width - 3 || y >= p.height - 3 ? undefined : [Math.round(x), Math.round(y)];
}

/// A picture in linear light, three numbers a pixel: one frame's, or the mean of many (a
/// torch flickers: one frame against another says little, a second's mean does).
type Lin = { width: number; height: number; lin: Float32Array };
const LINEAR = Float32Array.from({ length: 256 }, (_, v) => linear(v));
function lin(p: Picture, into?: Lin): Lin {
  const out = into ?? { width: p.width, height: p.height, lin: new Float32Array(p.width * p.height * 3) };
  for (let i = 0, n = p.width * p.height; i < n; i++) for (let k = 0; k < 3; k++) out.lin[i * 3 + k] = LINEAR[p.rgba[i * 4 + k]];
  return out;
}
/// The mean of the pictures in `files`, `each` called on every one of them in turn.
function mean(files: string[], each: (p: Lin) => void = () => undefined): Lin | undefined {
  let [sum, one]: [Lin | undefined, Lin | undefined] = [undefined, undefined];
  for (const file of files) {
    one = lin(readPng(file), one);
    each(one);
    sum ??= { ...one, lin: new Float32Array(one.lin.length) };
    for (let i = 0; i < sum.lin.length; i++) sum.lin[i] += one.lin[i] / files.length;
  }
  return sum;
}

/// The mean linear colour of the 5 by 5 pixels (25) round a place on a picture.
function patch(p: Lin, [x, y]: [number, number]): V {
  const sum: V = [0, 0, 0];
  for (let j = -2; j <= 2; j++) for (let i = -2; i <= 2; i++) for (let k = 0; k < 3; k++) sum[k] += p.lin[((y + j) * p.width + x + i) * 3 + k] / 25;
  return sum;
}
const luma = (c: V) => 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
const median = (list: number[]) => [...list].sort((a, b) => a - b)[Math.floor(list.length / 2)] ?? 0;

/// The ground so far from the holder's feet on a bearing from the way they face, degrees.
function ground(view: View, away: number, bearing: number): V {
  const a = (bearing * Math.PI) / 180;
  const [x, z] = view.ahead;
  const way = [x * Math.cos(a) - z * Math.sin(a), x * Math.sin(a) + z * Math.cos(a)];
  return [view.feet[0] + way[0] * away, view.feet[1], view.feet[2] + way[1] * away];
}
/// The ground's colour so far ahead: the median by luma of bearings either side of the holder.
function ahead(view: View, p: Lin, away: number): V | undefined {
  const seen = [-45, -30, -18, 18, 30, 45].map((b) => project(view, p, ground(view, away, b))).filter((at) => at !== undefined).map((at) => patch(p, at));
  if (seen.length < 2) return undefined;
  return seen.sort((a, b) => luma(a) - luma(b))[Math.floor(seen.length / 2)];
}
/// The ground's luma so far from the holder: the mean over `bearings` of those clear of the
/// holder's own shadow (under 0.7 of their median is in it), and how many were.
function clear(view: View, p: Lin, away: number, bearings: number[]): [number, number] {
  const seen = bearings.map((b) => project(view, p, ground(view, away, b))).filter((at) => at !== undefined).map((at) => luma(patch(p, at)));
  const lit = seen.filter((v) => v >= 0.7 * median(seen));
  return [lit.reduce((a, b) => a + b, 0) / (lit.length || 1), lit.length];
}
/// Bearings a camera behind the holder sees the ground on at their feet (their body hides it
/// ahead), and ahead of them.
const ROUND = [50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150].flatMap((b) => [-b, b]);
const AHEAD = [18, 24, 30, 36, 42, 48, 54, 60].flatMap((b) => [-b, b]);
/// The pool's luma: the ground 1 to 3 m ahead.
const pool = (view: View, p: Lin) => [1, 1.5, 2, 2.5, 3].map((d) => luma(ahead(view, p, d) ?? [0, 0, 0])).reduce((a, b) => a + b, 0) / 5;
/// The far ground, past any torch's reach: what the eye's exposure alone changes.
const far = (view: View, p: Lin) => median([30, 35, 40, 50].flatMap((d) => [-30, -18, 18, 30].map((b) => project(view, p, ground(view, d, b)))).filter((at) => at !== undefined).map((at) => luma(patch(p, at))));

function hsv([r, g, b]: V): [number, number] {
  const s = (v: number) => (v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055);
  const [R, G, B] = [s(r), s(g), s(b)];
  const hi = Math.max(R, G, B);
  const span = hi - Math.min(R, G, B);
  let hue = span <= 0 ? 0 : hi === R ? 60 * (((G - B) / span) % 6) : hi === G ? 60 * ((B - R) / span + 2) : 60 * ((R - G) / span + 4);
  if (hue < 0) hue += 360;
  return [hue, hi > 0 ? span / hi : 0];
}

/// The deviation over the mean of a series, its dominant period in seconds at 60 frames a
/// second (the strongest of its Fourier terms from 0.5 Hz up), and its largest step.
export function flicker(series: number[]): { depth: number; period: number; step: number } {
  const n = series.length;
  const mean = series.reduce((a, b) => a + b, 0) / n;
  const depth = Math.sqrt(series.reduce((a, b) => a + (b - mean) ** 2, 0) / n) / mean;
  let best = [0, 0];
  for (let k = 1; k <= n / 2; k++) {
    let [re, im] = [0, 0];
    series.forEach((v, i) => ((re += (v - mean) * Math.cos((2 * Math.PI * k * i) / n)), (im += (v - mean) * Math.sin((2 * Math.PI * k * i) / n))));
    if (re * re + im * im > best[1]) best = [k, re * re + im * im];
  }
  const step = Math.max(...series.slice(1).map((v, i) => Math.abs(v - series[i]) / mean));
  return { depth, period: best[0] ? n / 60 / best[0] : 0, step };
}

const whole = (p: Picture) => {
  let [sum, black, clipped] = [0, 0, 0];
  const n = p.width * p.height;
  for (let i = 0; i < n * 4; i += 4) {
    const [r, g, b] = [p.rgba[i], p.rgba[i + 1], p.rgba[i + 2]];
    sum += 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
    if (Math.max(r, g, b) <= 2) black++;
    if (Math.max(r, g, b) >= 254) clipped++;
  }
  return { luma: sum / n, black: black / n, clipped: clipped / n };
};
const frames = (stem: string) => Array.from({ length: FRAMES }, (_, i) => `${stem}-${String(i).padStart(5, "0")}.png`);

/// A trunk's shadow on the ground behind it from the holder: the ground's luma by bearing from
/// the holder's feet, a quarter degree a step for 25° either side of the trunk's, the mean of
/// 1.5, 2 and 2.5 m past it.
function profile(view: View, tree: [number, number], p: Lin): { away: number; bearing: number; row: number[] } {
  const to = [tree[0] - view.feet[0], tree[1] - view.feet[2]];
  const [away, bearing] = [Math.hypot(to[0], to[1]), (Math.atan2(view.ahead[0] * to[1] - view.ahead[1] * to[0], view.ahead[0] * to[0] + view.ahead[1] * to[1]) * 180) / Math.PI];
  const row = Array.from({ length: 201 }, (_, i) => {
    const seen = [1.5, 2, 2.5].map((past) => project(view, p, ground(view, away + past, bearing + (i - 100) * 0.25))).filter((at) => at !== undefined);
    return seen.length ? seen.map((at) => luma(patch(p, at))).reduce((x, y) => x + y, 0) / seen.length : NaN;
  });
  return { away, bearing, row };
}
/// The lit ground either side of a trunk's shadow: the outer 8° of its profile.
const beside = (row: number[]) => median([...row.slice(0, 32), ...row.slice(-32)]);

type Shadow = { depth: number; wide: number; edge: number; noise?: [number, number]; text: string };
/// The shadow's darkest against the lit ground either side, how wide it is at half depth, how
/// far each edge takes from 10% to 90% (metres, 2 m past the trunk), and how much its edges
/// change from frame to frame against the lit ground's (`rows`, each frame's profile: a
/// jittered soft shadow's noise shows here; waving grass in both).
function trunk(view: View, tree: [number, number], lit: Lin, rows: number[][]): Shadow | string {
  const { away, bearing, row } = profile(view, tree, lit);
  if (row.some(Number.isNaN)) return `its shadow's ground is out of the picture (${away.toFixed(1)} m away, ${bearing.toFixed(0)}° from ahead)`;
  // Smoothed over five steps (the grass).
  const smooth = row.map((_, i) => median(row.slice(Math.max(0, i - 2), i + 3)));
  const open = beside(smooth);
  const low = Math.min(...smooth.slice(60, 141));
  const dark = smooth.indexOf(low, 60);
  const level = (share: number) => low + share * (open - low);
  const run = (way: number, share: number) => {
    let i = dark;
    while (i > 0 && i < 200 && smooth[i] < level(share)) i += way;
    return i;
  };
  const metres = (n: number) => (Math.abs(n) * 0.25 * Math.PI * (away + 2)) / 180;
  const edges = [-1, 1].map((way) => metres(run(way, 0.9) - run(way, 0.1)));
  const wide = metres(run(1, 0.5) - run(-1, 0.5));
  const edge = [...new Set([-1, 1].flatMap((way) => [run(way, 0.3), run(way, 0.5), run(way, 0.7)]))];
  let noise: [number, number] | undefined;
  if (rows.length > 8) {
    const sway = (i: number) => {
      const series = rows.map((each) => each[i]);
      const m = series.reduce((a, b) => a + b, 0) / series.length;
      return Math.sqrt(series.reduce((a, b) => a + (b - m) ** 2, 0) / series.length) / m;
    };
    const of = (list: number[]) => list.reduce((a, b) => a + b, 0) / list.length;
    noise = [of(edge.map(sway)), of([4, 12, 20, 28, 172, 180, 188, 196].map(sway))];
  }
  const said = noise ? `; over ${rows.length} frames its edges change ${(noise[0] * 100).toFixed(1)}% of their mean, the lit ground ${(noise[1] * 100).toFixed(1)}%` : "";
  const text = `${away.toFixed(1)} m away at ${bearing.toFixed(0)}°: its shadow 2 m past it ${(low / open).toFixed(2)} of the lit ground, ${wide.toFixed(2)} m wide at half depth, its edges ${edges.map((e) => e.toFixed(2)).join(" and ")} m from 10% to 90%${said}`;
  return { depth: low / open, wide, edge: (edges[0] + edges[1]) / 2, noise, text };
}

/// The passes a place is shot in with the torch in hand, in one game at one clock: `main`
/// with the carried torch's rig bare (its key over its flames, no fill), `new` with the rig
/// under trial, `back` bare again: back over main is how steady the reading is.
const PASSES = ["main", "new", "back"] as const;
type Pass = (typeof PASSES)[number];
/// A pass's numbers, by name, for the A/B lines.
type Numbers = Record<string, number>;

/// A place's pass's numbers, said a line each (`back`'s only kept).
function measure(out: string, name: string, pass: Pass): Numbers | undefined {
  const stem = join(out, `${name}-${pass}`);
  if (!existsSync(`${stem}-lit.png`)) return undefined;
  const view = JSON.parse(readFileSync(join(out, `${name}.json`), "utf8")) as View;
  const say = (what: string, text: string) => pass !== "back" && console.log(`torchlight: ${name.padEnd(5)} ${pass.padEnd(4)} ${what.padEnd(7)} ${text}`);
  const unlit = lin(readPng(join(out, `${name}-unlit.png`)));
  const tree = existsSync(join(out, `${name}-tree.json`)) ? (JSON.parse(readFileSync(join(out, `${name}-tree.json`), "utf8")) as [number, number]) : undefined;
  // The standing frames' mean is what is read; each frame's pool and trunk profile, how it changes.
  const [still, rows]: [number[], number[][]] = [[], []];
  const lit =
    mean(frames(`${stem}-still`).filter(existsSync), (p) => {
      still.push(pool(view, p));
      if (tree) rows.push(profile(view, tree, p).row);
    }) ?? lin(readPng(`${stem}-lit.png`));
  const n: Numbers = {};
  const [feet, clearAtFeet] = clear(view, lit, 1, ROUND);
  const [two, clearAhead] = clear(view, lit, 2, AHEAD);
  [n["feet (1 m round)"], n["ground 2 m ahead"]] = [feet, two];
  const fall: Record<string, number | undefined> = {};
  const curve: string[] = [];
  const first = luma(ahead(view, lit, 0.75) ?? [0, 0, 0]);
  for (let d = 0.75; d <= 25; d += 0.25) {
    const here = ahead(view, lit, d);
    if (!here) continue;
    const share = luma(here) / first;
    for (const [bar, key] of [[0.5, "half"], [0.1, "tenth"], [0.01, "hundredth"]] as const) if (fall[key] === undefined && share <= bar) fall[key] = d;
    if ([1, 2, 3, 4, 5, 6, 8, 10, 15, 20].includes(d)) curve.push(`${d}m ${share.toFixed(3)}`);
  }
  const at = (d: number | undefined) => (d === undefined ? "never" : `${d} m`);
  say("ground", `${feet.toFixed(4)} a metre round the feet (${clearAtFeet} of ${ROUND.length} bearings clear of their shadow), ${two.toFixed(4)} 2 m ahead (${clearAhead} of ${AHEAD.length})`);
  say("pool", `a half at ${at(fall.half)}, a tenth at ${at(fall.tenth)}, a hundredth at ${at(fall.hundredth)} of the ground 0.75 m ahead (${first.toFixed(4)}); the night's floor ${(far(view, lit) / first).toFixed(4)}; ${curve.join(", ")}`);
  const eye = far(view, lit) / far(view, unlit);
  const against = [3, 6, 10, 15].map((d) => {
    const [a, b] = [ahead(view, lit, d), ahead(view, unlit, d)];
    return a && b ? `${d}m ${(luma(a) / luma(b) / eye).toFixed(2)}x` : `${d}m unseen`;
  });
  say("night", `${against.join(", ")} the unlit ground (the eye's exposure ${eye.toFixed(2)}x with the torch)`);
  const back = median([0.9, 1.1, 1.3].map((h) => project(view, lit, [view.feet[0], view.feet[1] + h, view.feet[2]])).filter((p) => p !== undefined).map((p) => luma(patch(lit, p))));
  n["holder's back / ground 2 m"] = back / two;
  say("holder", `their back ${(back / two).toFixed(3)} of the ground 2 m ahead (back ${back.toFixed(4)})`);
  const [hue, saturation] = hsv(ahead(view, lit, 2) ?? [0, 0, 0]);
  say("colour", `the ground 2 m ahead hue ${hue.toFixed(0)}° saturation ${saturation.toFixed(2)}`);
  // Their own shadow, on a ring 1.5 m round them; ahead their body hides the ground.
  const ring = Array.from({ length: 180 }, (_, i) => {
    const b = i * 2 - 178;
    const where = Math.abs(b) < 40 ? undefined : project(view, lit, ground(view, 1.5, b));
    return where ? luma(patch(lit, where)) : NaN;
  });
  const seen = ring.filter((v) => !Number.isNaN(v));
  const [low, mid] = [Math.min(...seen), median(seen)];
  const dark = ring.indexOf(low);
  const span = (share: number) => {
    let [a, b] = [dark, dark];
    while (a > 0 && ring[a - 1] < low + share * (mid - low)) a--;
    while (b < 179 && ring[b + 1] < low + share * (mid - low)) b++;
    return ((b - a + 1) * 2 * Math.PI * 1.5) / 180;
  };
  [n["own shadow's depth"], n["own shadow's width m"]] = [low / mid, span(0.5)];
  say("shadow", `the darkest ground 1.5 m round them ${(low / mid).toFixed(2)} of the ring's median, at ${dark * 2 - 178}° from ahead, ${span(0.5).toFixed(2)} m wide at half depth, ${span(0.9).toFixed(2)} m at a tenth (${seen.length} of 180 bearings seen)`);
  if (tree) {
    const t = trunk(view, tree, lit, rows);
    say("trunk", typeof t === "string" ? t : t.text);
    if (typeof t !== "string") {
      [n["trunk's shadow depth"], n["trunk's shadow width m"], n["trunk's shadow edge m"]] = [t.depth, t.wide, t.edge];
      if (t.noise) n["trunk's edges' change"] = t.noise[0];
    }
  }
  if (still.length > 8) {
    const f = flicker(still);
    [n["pool's deviation"], n["pool's largest step"]] = [f.depth, f.step];
    say("flicker", `the pool's deviation ${(f.depth * 100).toFixed(1)}% of its mean, dominant period ${f.period.toFixed(2)} s, largest step ${(f.step * 100).toFixed(1)}% a frame (${still.length} frames)`);
  }
  const ran = frames(`${stem}-run`).filter(existsSync).map((file) => whole(readPng(file)).luma);
  if (ran.length > 8) {
    const f = flicker(ran);
    n["running: picture's largest step"] = f.step;
    say("run", `the picture's largest step ${(f.step * 100).toFixed(1)}% a frame, ${ran.slice(1).filter((v, i) => Math.abs(v - ran[i]) / v > 0.02).length} steps over 2% (${ran.length} frames)`);
  }
  if (existsSync(`${stem}-play.png`)) {
    const w = whole(readPng(`${stem}-play.png`));
    say("picture", `${(w.black * 100).toFixed(1)}% pure black, ${(w.clipped * 100).toFixed(2)}% clipped, mean linear luma ${w.luma.toFixed(4)} (the game's own camera)`);
  }
  return n;
}

/// A place's A/B: each number with the rig bare and under trial, the trial's over the bare's,
/// and the bare's again over itself (what the reading moves with nothing changed).
function compare(out: string, name: string) {
  const [main, trial, back] = PASSES.map((pass) => measure(out, name, pass));
  if (!main || !trial) return;
  const rig = join(out, `${name}-rig.json`);
  console.log(`torchlight: ${name.padEnd(5)} A/B  main is the rig bare, new is ${existsSync(rig) ? readFileSync(rig, "utf8") : "unknown"}`);
  for (const key of Object.keys(main)) {
    const steady = back?.[key] === undefined ? "" : `  back/main ${(back[key] / main[key]).toFixed(2)}`;
    console.log(`torchlight: ${name.padEnd(5)} A/B  ${key.padEnd(32)} main ${main[key].toFixed(4)}  new ${(trial[key] ?? NaN).toFixed(4)}  new/main ${((trial[key] ?? NaN) / main[key]).toFixed(2)}${steady}`);
  }
}

type Found = { name: string; from_start: V }[];
const player = async () => ((await call("sandbox.find", { name: "Player" })) as Found).find((each) => each.name === "Player")!.from_start;
const settle = async (n: number) => {
  const from = (await status()).frame;
  await waitUntil(`${n} frames`, status, (now) => now.frame >= from + n, 900);
};
const saved = async (file: string) => {
  let last = -1;
  for (let i = 0; i < 9000; i++) {
    const size = existsSync(file) ? statSync(file).size : 0;
    if (size > 0 && size === last) return;
    last = size;
    await sleep(200);
  }
  throw new Error(`the sandbox did not save ${file}`);
};

/// Takes a tool on the bar in hand, waiting in frames (the client's `hold` waits in seconds,
/// too few on a slow renderer).
async function take(tool: string) {
  const slot = ((await state()).bar as (string | null)[] | undefined)?.indexOf(tool) ?? -1;
  if (slot < 0) throw new Error(`no ${tool} is on the bar (play.state's bar: ${JSON.stringify((await state()).bar)})`);
  await press(String(slot + 1));
  await settle(30);
  const holding = (await state()).holding;
  if (holding !== tool) throw new Error(`slot ${slot + 1}'s key pressed, the player holds ${holding}, not the ${tool}`);
}

/// A carried fire's rig, as `fire.carried` tells and takes it.
type Rig = { key?: V | null; fill?: { at: V; share: number } | null };
/// A spot east and south of the start.
type Spot = [number, number];

/// Stands the player on a spot facing east, the camera held behind them, and says the view.
async function stand(at: Spot): Promise<View> {
  await warp(at, [at[0] + 10, at[1]]);
  await settle(20);
  const feet = await player();
  const view: View = { eye: [feet[0] - 4.5, feet[1] + 2.6, feet[2]], at: [feet[0] + 4, feet[1], feet[2]], feet, ahead: [1, 0] };
  await call("sandbox.look", { vantage: [...view.eye, ...view.at] });
  return view;
}

/// Shoots a place from `at`, facing east: unlit, then with the torch in hand each pass's lit
/// picture and standing frames, then main's and the trial's picture and run in the game's own
/// camera. `trial` is the rig under trial: the game's own for a held torch when not given.
async function shoot(out: string, name: string, at: Spot, trial: Rig | undefined) {
  writeFileSync(join(out, `${name}.json`), JSON.stringify(await stand(at)));
  await settle(90);
  await shot(join(out, `${name}-unlit.png`));
  await take("Torch");
  const own = ((await call("fire.carried")) as Rig[])[0];
  if (!own) throw new Error("the torch is in hand and fire.carried tells no carried fire");
  const rig = trial ?? { key: own.key, fill: own.fill };
  writeFileSync(join(out, `${name}-rig.json`), JSON.stringify(rig));
  const set = (pass: Pass) => call("fire.carried", { set: pass === "new" ? rig : {} });
  for (const pass of PASSES) {
    await set(pass);
    await settle(45);
    await shot(join(out, `${name}-${pass}-lit.png`));
    await saved(await film(join(out, `${name}-${pass}-still.png`), FRAMES));
  }
  for (const pass of ["main", "new"] as const) {
    await stand(at);
    await call("sandbox.release");
    await set(pass);
    await settle(45);
    await shot(join(out, `${name}-${pass}-play.png`));
    const last = await film(join(out, `${name}-${pass}-run.png`), FRAMES);
    await press("w:60", "d@60:60");
    await saved(last);
  }
  // The torch away again, for the next place's unlit picture.
  await take("Knife");
}

type Ground = { up: number; water: number };
type Trunk = { at: Spot; apart: number; radius: number; height: number };
const near = async (method: string, at: Spot, within: number) => (await call(method, { at, within })) as unknown[];

/// The offsets a spot's ground is asked at: itself, a disc 8 m round it, and a line east.
const DISC: Spot[] = [[0, 0], ...[4, 8].flatMap((r) => Array.from({ length: 8 }, (_, i): Spot => [r * Math.cos((i * Math.PI) / 4), r * Math.sin((i * Math.PI) / 4)]))];
const LINE: Spot[] = Array.from({ length: 17 }, (_, i): Spot => [10 + i * 2.5, 0]);
/// Whether a spot is open ground to stand and look east from, or why not: level within
/// 0.25 m over the disc and 1 m out to 25 m ahead, dry out to 50 m.
export function level(grounds: Ground[]): string | undefined {
  const here = grounds[0].up;
  const off = (list: Ground[]) => Math.max(...list.map((g) => Math.abs(g.up - here)));
  const disc = grounds.slice(0, DISC.length);
  const line = grounds.slice(DISC.length);
  if (off(disc) > 0.25) return `its ground is ${off(disc).toFixed(2)} m off level within 8 m`;
  const ahead = line.filter((_, i) => LINE[i][0] <= 25);
  if (off(ahead) > 1) return `its ground is ${off(ahead).toFixed(1)} m off level within 25 m ahead`;
  const wet = Math.max(...grounds.map((g) => g.water));
  if (wet > -0.3) return `water stands ${wet.toFixed(1)} m over the ground within 50 m ahead`;
  return undefined;
}
/// The grounds of spots, each asked at `offsets`, in one call.
async function grounds(spots: Spot[], offsets: Spot[]): Promise<Ground[][]> {
  const all = (await call("world.ground", { at: spots.flatMap((s) => offsets.map((o): Spot => [s[0] + o[0], s[1] + o[1]])) })) as Ground[];
  return spots.map((_, i) => all.slice(i * offsets.length, (i + 1) * offsets.length));
}

/// An open place: of a grid of spots 10 m apart out to 100 m each way from `from`, the
/// levellest that is dry and has nothing standing within 5 m and no tree within 6 (the
/// start's wood leaves no wider clearing on level ground: every number a rig is judged by is
/// read within 3 m); `asked` alone when given, still checked and said.
async function openPlace(from: Spot, asked: Spot | undefined): Promise<Spot> {
  const grid = Array.from({ length: 441 }, (_, i): Spot => [from[0] + ((i % 21) - 10) * 10, from[1] + (Math.floor(i / 21) - 10) * 10]);
  const spots = asked ? [asked] : grid;
  const found = await grounds(spots, [...DISC, ...LINE]);
  const where = (spot: Spot) => `${spot[0].toFixed(1)} m east and ${spot[1].toFixed(1)} m south of the start`;
  // Where none is level, the levellest dry clear one, its fault said: a place's numbers are
  // read with it in mind. The disc's ground counts four times the line's.
  const weight = (j: number) => (j < DISC.length ? 4 : LINE[j - DISC.length][0] <= 25 ? 1 : 0);
  const rough = (g: Ground[]) => Math.max(...g.map((each, j) => (each.water > -0.3 ? 100 : Math.abs(each.up - g[0].up) * weight(j))));
  const order = spots.map((_, i) => i).sort((a, b) => rough(found[a]) - rough(found[b]));
  const faults: string[] = [];
  for (const i of order) {
    const spot = spots[i];
    const uneven = level(found[i]);
    const standing = (await near("world.obstacles", spot, 5)).length ? "something stands within 5 m" : (await near("world.trunks", spot, 6)).length ? "a tree stands within 6 m" : undefined;
    if (asked || (!standing && !uneven?.startsWith("water"))) {
      const though = [uneven, standing].filter((each) => each !== undefined).join(" and ");
      console.log(`torchlight: open  ${where(spot)}${though ? `, though ${though} (the levellest dry clear spot of ${spots.length})` : ": level, dry, nothing standing within 5 m"}`);
      return spot;
    }
    faults.push(`${where(spot)}: ${uneven ?? standing}`);
  }
  throw new Error(`no dry, clear spot within 100 m of the start (${spots.length} tried; the levellest: ${faults.slice(0, 3).join("; ")}): give --open EAST,SOUTH`);
}

/// A trunk to cast a shadow: tall, alone, on dry level ground, and (the picture alone can say:
/// `world.trunks` knows no boughs) the ground beside its shadow lit at least half as well as
/// the open place's as far off. Up to five are tried; `asked` is taken as it is.
async function trunkPlace(out: string, from: Spot, asked: Spot | undefined): Promise<Spot | undefined> {
  if (asked) return asked;
  const all = (await near("world.trunks", from, 150)) as Trunk[];
  const alone = all.filter((t) => t.height > 6 && t.radius > 0.15 && all.every((o) => o === t || Math.hypot(o.at[0] - t.at[0], o.at[1] - t.at[1]) > 5));
  const steps: Spot[] = [[-3, 1], [0, 0], [3, 0]];
  const found = alone.length ? await grounds(alone.map((t) => t.at), steps) : [];
  const dry = alone.filter((_, i) => Math.max(...found[i].map((g) => Math.abs(g.up - found[i][0].up))) < 0.4 && found[i].every((g) => g.water < -0.3));
  const open = join(out, "open-main-lit.png");
  const bar = existsSync(open) ? luma(ahead(JSON.parse(readFileSync(join(out, "open.json"), "utf8")) as View, lin(readPng(open)), 5.25) ?? [0, 0, 0]) : 0;
  let best: [Spot, number] | undefined;
  for (const t of dry.slice(0, 5)) {
    // Three metres short of the trunk and a metre to its side, facing past it.
    const view = await stand([t.at[0] - 3, t.at[1] + 1]);
    await take("Torch");
    await settle(45);
    const file = join(out, "woods-try.png");
    await shot(file);
    const lit = beside(profile(view, t.at, lin(readPng(file))).row);
    await take("Knife");
    const where = `a trunk ${t.radius.toFixed(2)} m in radius, ${t.height.toFixed(0)} m tall, ${t.at[0].toFixed(1)} m east and ${t.at[1].toFixed(1)} m south of the start`;
    if (lit >= 0.5 * bar) {
      console.log(`torchlight: woods ${where}, 3 m ahead of the holder and 1 m to their left: the ground beside its shadow ${(lit / bar).toFixed(2)} of the open place's as far off`);
      return t.at;
    }
    console.log(`torchlight: woods passed over ${where}: the ground beside its shadow ${(lit / bar).toFixed(2)} of the open place's (boughs, or it is out of the picture)`);
    if (lit > (best?.[1] ?? 0)) best = [t.at, lit];
  }
  console.log(`torchlight: woods no clear trunk within 150 m of the start (${all.length} trunks, ${alone.length} tall and alone, ${dry.length} on dry level ground)${best ? `: the best lit of those tried is shot, ${best[0].map((v) => v.toFixed(1)).join(",")}` : ""}; or give --trunk EAST,SOUTH`);
  return best?.[0];
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const flag = (name: string) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined);
  const spot = (name: string) => flag(name)?.split(",").map(Number) as Spot | undefined;
  const from = flag("--from");
  const out = resolve(from ?? flag("--out") ?? "target/job/torchlight");
  let failed = "";
  if (!from) {
    mkdirSync(out, { recursive: true });
    try {
      const trial = flag("--rig") ? (JSON.parse(flag("--rig")!) as Rig) : undefined;
      const options = ["--experiment", "play", "--fresh", "--save", join(out, "save.ron"), "--offscreen", "--clean", "--mute", "--time", "0", "--picture", SIZE.join("x")];
      // The game runs one copy a machine (D319): while another's runs, wait for it, half an
      // hour at most.
      for (let tries = 0; ; tries++) {
        try {
          await start(options, { release: true });
          break;
        } catch (problem) {
          if (tries >= 60 || !String(problem).includes("another copy of the game runs")) throw problem;
          if (tries % 4 === 0) console.log(`torchlight: waiting for the machine's one game copy (${tries / 2} min)`);
          await sleep(30_000);
        }
      }
      await give("torch").catch(() => undefined);
      const here = await player();
      const open = await openPlace([here[0], here[2]], spot("--open"));
      await shoot(out, "open", open, trial);
      const tree = await trunkPlace(out, open, spot("--trunk"));
      if (tree) {
        writeFileSync(join(out, "woods-tree.json"), JSON.stringify(tree));
        await shoot(out, "woods", [tree[0] - 3, tree[1] + 1], trial);
      }
    } catch (problem) {
      failed = String(problem instanceof Error ? problem.message : problem);
    } finally {
      await stop().catch(() => undefined);
    }
  }
  for (const name of ["open", "woods"]) compare(out, name);
  if (failed) {
    console.error(`✗ torchlight: ${failed}`);
    process.exit(1);
  }
}
