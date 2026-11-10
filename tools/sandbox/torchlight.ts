#!/usr/bin/env node
// A held torch's light at deep night, in numbers (D71; the developers, 2026-11-09: "the light
// from the torch isnt amazing"): in play at midnight the player stands in the open meadow at
// the start, by a tree, and (--den) on a den's floor, and at each a camera held where the
// third-person one sits (4.5 m behind, 2.6 m up) shoots the ground with the torch away and in
// hand. Read off the pictures, a line a number:
//
//   pool     the ground's linear luma by distance ahead, against its value at the feet: where
//            it falls to a half, a tenth and a hundredth, and what the night's floor is
//   night    the lit ground against the same ground with the torch away, at 3, 6, 10 and 15 m,
//            both divided by the far ground's change (the eye's exposure moves with the torch)
//   holder   the holder's back against the ground 2 m ahead
//   colour   the lit ground's hue and saturation 2 m ahead
//   shadow   the darkest ground on a ring 1.5 m round the holder against the ring's median, and
//            how many metres of the ring its edge takes to go from 10% to 90%
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
//   cargo xtask gpu still -- node tools/sandbox/torchlight.ts [--den] [--woods EAST,SOUTH] [--out target/job/torchlight]
//   node tools/sandbox/torchlight.ts --from <dir>     the numbers of a run already shot
//
// Node 22.18 or later; no dependencies.

import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { call, film, give, hold, press, shot, sleep, start, state, status, stop, waitUntil, warp } from "./client.ts";
import { type Picture, readPng } from "./png.ts";

type V = [number, number, number];
/// A view: the eye, what it looks at, the holder's feet and the way they face, from the start.
type View = { eye: V; at: V; feet: V; ahead: [number, number] };
const VFOV = Math.PI / 4;
const FRAMES = 120;
const SIZE = [960, 540];

const sub = (a: V, b: V): V => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a: V, b: V) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V, b: V): V => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const unit = (a: V): V => {
  const l = Math.hypot(...a);
  return [a[0] / l, a[1] / l, a[2] / l];
};
const linear = (v: number) => (v / 255 <= 0.04045 ? v / 255 / 12.92 : ((v / 255 + 0.055) / 1.055) ** 2.4);

/// Where a point from the start falls on a picture, or nothing when out of it.
export function project(view: View, p: Picture, point: V): [number, number] | undefined {
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

/// The mean linear colour of the 5 by 5 pixels round a place on a picture.
function patch(p: Picture, [x, y]: [number, number]): V {
  const sum: V = [0, 0, 0];
  for (let j = -2; j <= 2; j++) for (let i = -2; i <= 2; i++) for (let k = 0; k < 3; k++) sum[k] += linear(p.rgba[((y + j) * p.width + x + i) * 4 + k]) / 25;
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
function ahead(view: View, p: Picture, away: number): V | undefined {
  const seen = [-45, -30, -18, 18, 30, 45].map((b) => project(view, p, ground(view, away, b))).filter((at) => at !== undefined).map((at) => patch(p, at));
  if (seen.length < 2) return undefined;
  return seen.sort((a, b) => luma(a) - luma(b))[Math.floor(seen.length / 2)];
}
/// The pool's luma: the ground 1 to 3 m ahead.
const pool = (view: View, p: Picture) => [1, 1.5, 2, 2.5, 3].map((d) => luma(ahead(view, p, d) ?? [0, 0, 0])).reduce((a, b) => a + b, 0) / 5;
/// The far ground, past any torch's reach: what the eye's exposure alone changes.
const far = (view: View, p: Picture) => median([30, 35, 40, 50].flatMap((d) => [-30, -18, 18, 30].map((b) => project(view, p, ground(view, d, b)))).filter((at) => at !== undefined).map((at) => luma(patch(p, at))));

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

/// A place's numbers, said a line each.
function measure(out: string, name: string) {
  const view = JSON.parse(readFileSync(join(out, `${name}.json`), "utf8")) as View;
  const [unlit, lit] = [readPng(join(out, `${name}-unlit.png`)), readPng(join(out, `${name}-lit.png`))];
  const say = (what: string, text: string) => console.log(`torchlight: ${name.padEnd(6)} ${what.padEnd(8)} ${text}`);
  const feet = luma(ahead(view, lit, 0.75) ?? [0, 0, 0]);
  const fall: Record<string, number | undefined> = {};
  const curve: string[] = [];
  for (let d = 0.75; d <= 25; d += 0.25) {
    const here = ahead(view, lit, d);
    if (!here) continue;
    const share = luma(here) / feet;
    for (const [bar, key] of [[0.5, "half"], [0.1, "tenth"], [0.01, "hundredth"]] as const) if (fall[key] === undefined && share <= bar) fall[key] = d;
    if ([1, 2, 3, 4, 5, 6, 8, 10, 15, 20].includes(d)) curve.push(`${d}m ${share.toFixed(3)}`);
  }
  const at = (d: number | undefined) => (d === undefined ? "never" : `${d} m`);
  const floor = far(view, lit) / feet;
  say("pool", `a half at ${at(fall.half)}, a tenth at ${at(fall.tenth)}, a hundredth at ${at(fall.hundredth)}; the night's floor ${floor.toFixed(4)} of the feet's ${feet.toFixed(4)}; ${curve.join(", ")}`);
  const eye = far(view, lit) / far(view, unlit);
  const against = [3, 6, 10, 15].map((d) => {
    const [a, b] = [ahead(view, lit, d), ahead(view, unlit, d)];
    return a && b ? `${d}m ${(luma(a) / luma(b) / eye).toFixed(2)}x` : `${d}m unseen`;
  });
  say("night", `${against.join(", ")} the unlit ground (the eye's exposure ${eye.toFixed(2)}x with the torch)`);
  const back = [0.9, 1.1, 1.3].map((h) => project(view, lit, [view.feet[0], view.feet[1] + h, view.feet[2]])).filter((p) => p !== undefined).map((p) => luma(patch(lit, p)));
  const two = ahead(view, lit, 2) ?? [0, 0, 0];
  say("holder", `their back ${(median(back) / luma(two)).toFixed(3)} of the ground 2 m ahead (back ${median(back).toFixed(4)}, ground ${luma(two).toFixed(4)})`);
  const [hue, saturation] = hsv(two);
  say("colour", `the ground 2 m ahead hue ${hue.toFixed(0)}° saturation ${saturation.toFixed(2)}`);
  const ring: number[] = [];
  for (let b = -178; b <= 180; b += 2) {
    // Ahead of them the holder's own body hides the ground from a camera behind.
    const where = Math.abs(b) < 40 ? undefined : project(view, lit, ground(view, 1.5, b));
    ring.push(where ? luma(patch(lit, where)) : NaN);
  }
  const seen = ring.filter((v) => !Number.isNaN(v));
  const [low, mid] = [Math.min(...seen), median(seen)];
  const dark = ring.indexOf(low);
  let edge = 0;
  for (let i = dark; i < ring.length && !(ring[i] >= low + 0.9 * (mid - low)); i++) if (ring[i] >= low + 0.1 * (mid - low)) edge++;
  say("shadow", `the darkest ground 1.5 m round them ${(low / mid).toFixed(2)} of the ring's median, at ${dark * 2 - 178}° from ahead, its edge ${((edge * 2 * Math.PI * 1.5) / 180).toFixed(2)} m from 10% to 90% (${seen.length} of 180 bearings seen)`);
  const still = frames(join(out, `${name}-still`)).filter(existsSync).map((file) => pool(view, readPng(file)));
  if (still.length > 8) {
    const f = flicker(still);
    say("flicker", `the pool's deviation ${(f.depth * 100).toFixed(1)}% of its mean, dominant period ${f.period.toFixed(2)} s, largest step ${(f.step * 100).toFixed(1)}% a frame (${still.length} frames)`);
  }
  const ran = frames(join(out, `${name}-run`)).filter(existsSync).map((file) => whole(readPng(file)).luma);
  if (ran.length > 8) {
    const f = flicker(ran);
    say("run", `the picture's largest step ${(f.step * 100).toFixed(1)}% a frame, ${ran.slice(1).filter((v, i) => Math.abs(v - ran[i]) / v > 0.02).length} steps over 2% (${ran.length} frames)`);
  }
  const play = join(out, `${name}-play.png`);
  if (existsSync(play)) {
    const w = whole(readPng(play));
    say("picture", `${(w.black * 100).toFixed(1)}% pure black, ${(w.clipped * 100).toFixed(2)}% clipped, mean linear luma ${w.luma.toFixed(4)} (the game's own camera)`);
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

/// Shoots a place where the player now stands, facing `ahead`: unlit, lit, 120 frames standing
/// and 120 running.
async function shoot(out: string, name: string, ahead: [number, number]) {
  const feet = await player();
  const view: View = { eye: [feet[0] - ahead[0] * 4.5, feet[1] + 2.6, feet[2] - ahead[1] * 4.5], at: [feet[0] + ahead[0] * 4, feet[1], feet[2] + ahead[1] * 4], feet, ahead };
  writeFileSync(join(out, `${name}.json`), JSON.stringify(view));
  await call("sandbox.look", { vantage: [...view.eye, ...view.at] });
  await settle(90);
  await shot(join(out, `${name}-unlit.png`));
  await take("Torch");
  await settle(150);
  await shot(join(out, `${name}-lit.png`));
  await saved(await film(join(out, `${name}-still.png`), FRAMES));
  await call("sandbox.release");
  await settle(30);
  await shot(join(out, `${name}-play.png`));
  const last = await film(join(out, `${name}-run.png`), FRAMES);
  await press("w:60", "d@60:60");
  await saved(last);
  // The torch away again, for the next place's unlit picture.
  await take("Knife");
}

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

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const flag = (name: string) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined);
  const from = flag("--from");
  const out = resolve(from ?? flag("--out") ?? "target/job/torchlight");
  const places = ["meadow", "woods", "den"];
  let failed = "";
  if (!from) {
    mkdirSync(out, { recursive: true });
    try {
      const options = ["--experiment", "play", "--fresh", "--save", join(out, "save.ron"), "--offscreen", "--mute", "--time", "0", "--picture", SIZE.join("x"), ...(argv.includes("--den") ? ["--radius", "30000", "--seed", "3"] : [])];
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
      await warp([here[0], here[2]], [here[0] + 10, here[2]]);
      await shoot(out, "meadow", [1, 0]);
      const asked = flag("--woods")?.split(",").map(Number);
      const trees = asked ? [] : (await Promise.all(["Oak", "Pine", "Spruce", "Birch", "Fir", "Tree"].map((name) => call("sandbox.find", { name }) as Promise<Found>))).flat().filter((t) => t.from_start && !/leaf|card|crown/i.test(t.name));
      const tree = asked ? ([asked[0], 0, asked[1]] as V) : trees.sort((a, b) => Math.hypot(a.from_start[0], a.from_start[2]) - Math.hypot(b.from_start[0], b.from_start[2]))[0]?.from_start;
      if (tree) {
        // Three metres short of the trunk and a metre to its side, facing past it.
        await warp([tree[0] - 3, tree[2] + 1], [tree[0] + 7, tree[2] + 1]);
        await shoot(out, "woods", [1, 0]);
      } else console.log("torchlight: woods  no tree found by name (Oak, Pine, Spruce, Birch, Fir, Tree): give --woods EAST,SOUTH");
      if (argv.includes("--den")) {
        const { den } = (await call("den.state")) as { den: { mouth: V; floor: V; ahead: V } | null };
        if (!den) throw new Error("this world has no den (den.state)");
        await call("creatures.strike", { name: "Troll", damage: 5000 }).catch(() => undefined);
        await warp([den.mouth[0], den.mouth[2]], [den.ahead[0], den.ahead[2]]);
        let at = await player();
        for (let walks = 0; walks < 40 && Math.hypot(at[0] - den.floor[0], at[2] - den.floor[2]) > 4; walks++) {
          await press("w:90");
          await settle(90);
          at = await player();
        }
        const before = at;
        await press("w:30");
        await settle(40);
        at = await player();
        const way = [at[0] - before[0], at[2] - before[2]];
        const l = Math.hypot(way[0], way[1]) || 1;
        await shoot(out, "den", [way[0] / l, way[1] / l]);
      }
    } catch (problem) {
      failed = String(problem instanceof Error ? problem.message : problem);
    } finally {
      await stop().catch(() => undefined);
    }
  }
  for (const name of places) if (existsSync(join(out, `${name}-lit.png`))) measure(out, name);
  if (failed) {
    console.error(`✗ torchlight: ${failed}`);
    process.exit(1);
  }
}
